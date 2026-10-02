//! M6.6 cross-service end-to-end over real HTTP.
//!
//! `reconstruction-contracts.toml` OTEL-001 cannot be promoted to
//! `verified` while the end-to-end OTLP/HTTP scenario exists only as the
//! M6.6 spike's `examples/smoke_concurrent.rs`. That file is a
//! `fn main() -> ExitCode`: `cargo test` never runs it, so it is a design
//! reference rather than evidence. This file is the same scenario as a
//! real test, so a green run is evidence and a red run names the property
//! that broke.
//!
//! ## The scenario (kept from the spike)
//!
//! `service_a -> service_b -> consumer` over loopback HTTP. Two concurrent
//! requests, each carrying its own W3C `traceparent`. Every service runs
//! the product pipeline (`run_service_pipeline`: parse -> correlate ->
//! export -> redact -> render) and POSTs its JSON Lines to the consumer.
//! The consumer is the verifier: everything asserted below is read back
//! out of the bytes that actually crossed a socket, never carried over
//! from the producing thread.
//!
//! The property the spike was reaching for, and the reason this file
//! exists, is that two concurrent requests with distinct `traceparent`s
//! do not mix. `cross_service::run_uat_m6_01` asserts that at the
//! correlation layer, in-process. Here it has to survive a transport:
//! two distinct trace ids arrive, each record belongs to exactly one of
//! them, and each invocation id belongs to exactly one trace.
//!
//! ## The harness (rewritten)
//!
//! - **Ephemeral ports.** `TcpListener::bind("127.0.0.1:0")` and then
//!   `local_addr()`, never the spike's 4819/4820/4821. A fixed port cannot
//!   run twice in a row nor alongside itself, which is exactly when the
//!   evidence is most wanted.
//! - **Every wait is bounded.** Listeners are bound in the test thread
//!   before their accept loop starts, and a bound `TcpListener` is already
//!   listening, so there is no "wait for the servers to boot" sleep
//!   anywhere. Inbound overlap is a deadline-carrying condvar gate, the
//!   consumer hands evidence over a channel read with `recv_timeout`, and
//!   every socket carries a read and a write timeout. A record that never
//!   arrives fails the test in seconds rather than hanging until the CI
//!   runner kills the process.
//! - **Nothing outlives the test.** `Server::drop` sets the stop flag and
//!   joins the accept loop, which joins its per-connection workers; the
//!   listener socket closes when the loop returns.
//! - **The HTTP layer is deliberately dumb.** It moves bytes. Every
//!   property under test lives in `chronos_domain::otlp`, which is why
//!   this file is in `chronos-sandbox` and not in `chronos-domain`: that
//!   library is pure, takes no transport, and ADR-0033 section 2.2
//!   rejects making it choose one.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde_json::Value;

use chronos_domain::otlp::correlation::{ChronosEvent, EventField};
use chronos_domain::otlp::exporter::{ExportLimits, OptInFilter, TIMESTAMP_DOMAIN};
use chronos_domain::otlp::gates::run_service_pipeline;
use chronos_domain::otlp::parse::parse_traceparent;
use chronos_domain::otlp::redaction::{CardinalityLimits, RedactionPolicy};

// ---------------------------------------------------------------------------
// Bounds and fixtures
// ---------------------------------------------------------------------------

/// Bound for a single socket operation. Only reached when the peer is
/// stuck: the normal path is sub-millisecond on loopback, so this is a
/// ceiling, not a wait.
const SOCKET_TIMEOUT: Duration = Duration::from_secs(10);

/// Bound for one expected record at the consumer. This is the bound that
/// turns "a service never forwarded" into a failing test instead of a
/// hanging one.
const EVIDENCE_TIMEOUT: Duration = Duration::from_secs(5);

/// Bound for the inbound rendezvous inside `service_a`.
const GATE_TIMEOUT: Duration = Duration::from_secs(5);

/// Poll interval of the non-blocking accept loops. The only sleep in this
/// file, and it exists solely so a stop flag can end the loop.
const ACCEPT_POLL: Duration = Duration::from_millis(2);

/// Request A: trace id `aaaa1111...`.
const TRACE_A: &str = "00-aaaa1111aaaa1111aaaa1111aaaa1111-1111111111111111-01";

/// Request B: trace id `bbbb2222...`.
const TRACE_B: &str = "00-bbbb2222bbbb2222bbbb2222bbbb2222-2222222222222222-01";

/// Requests fired at `service_a`, concurrently.
const REQUESTS: usize = 2;

/// Services in the chain, each minting its own invocation id per request.
const SERVICES: usize = 2;

/// Probe names the opt-in filter allows; anything else is dropped by the
/// exporter before redaction ever sees it.
const PROBE_IN: &str = "http_in";
const PROBE_CALL: &str = "http_call";

/// `service_a` seeds: inbound request, then the outbound call.
const A_INBOUND_US: u64 = 1_000;
const A_OUTBOUND_US: u64 = 2_000;

/// `service_b` seed: the inbound request it was forwarded.
const B_INBOUND_US: u64 = 3_000;

/// The exporter publishes session-monotonic microseconds as nanoseconds
/// (`start_time_unix_nano` stays 0), so the expected nanosecond values
/// follow from the seeds rather than being written out by hand.
const NS_PER_US: u64 = 1_000;
const A_INBOUND_NS: u64 = A_INBOUND_US * NS_PER_US;
const A_OUTBOUND_NS: u64 = A_OUTBOUND_US * NS_PER_US;
const B_INBOUND_NS: u64 = B_INBOUND_US * NS_PER_US;

/// Causal sequence every trace must carry: A inbound, A outbound, B
/// inbound. One request, one causal order, on both wires.
const EXPECTED_CHRONOLOGY_NS: [u64; 3] = [A_INBOUND_NS, A_OUTBOUND_NS, B_INBOUND_NS];

/// Spans `service_a` emits per request (both probes are opted in).
const A_SPANS: usize = 2;

/// Spans `service_b` emits per request.
const B_SPANS: usize = 1;

/// Records the consumer must receive for the whole run. Strictly equal:
/// one record per emitted span, so a duplicate is a bug and a shortfall is
/// a bug.
const EXPECTED_RECORDS: usize = REQUESTS * (A_SPANS + B_SPANS);

/// Attribute keys carrying a secret in each service. `RedactionPolicy`
/// matches these case-insensitively (`api_key` is a listed pattern,
/// `api_token` contains `token`).
const SECRET_FIELD_A: &str = "api_key";
const SECRET_FIELD_B: &str = "api_token";

/// The secret values. They must appear nowhere in the received evidence.
const SECRET_A: &str = "service-a-secret-value";
const SECRET_B: &str = "service-b-secret-value";

/// Service identity attribute. Present so the verifier can tell which
/// service produced a record after it has left every producing thread.
const SERVICE_FIELD: &str = "service";
const SERVICE_A: &str = "service_a";
const SERVICE_B: &str = "service_b";

/// Timestamp disclosure attribute keys, as the exporter spells them.
const ATTR_TIMESTAMP_DOMAIN: &str = "chronos.timestamp_domain";
const ATTR_TIMESTAMP_MONOTONIC_NS: &str = "chronos.timestamp_monotonic_ns";

// ---------------------------------------------------------------------------
// Pipeline inputs (the product owns the pipeline; these are only data)
// ---------------------------------------------------------------------------

fn opt_in_filter() -> OptInFilter {
    OptInFilter::new(vec![PROBE_IN.to_string(), PROBE_CALL.to_string()])
}

fn export_limits() -> ExportLimits {
    ExportLimits::default()
}

/// Per-service redaction, applied inside `run_service_pipeline`. Each
/// service owns its policy because redaction runs before forwarding: a
/// secret in `service_b` is redacted by `service_b`, not on arrival.
fn redaction() -> RedactionPolicy {
    RedactionPolicy::default_secrets()
}

fn cardinality() -> CardinalityLimits {
    CardinalityLimits::default()
}

fn events_for_service_a() -> Vec<ChronosEvent> {
    vec![
        ChronosEvent {
            ts_micros: A_INBOUND_US,
            probe: PROBE_IN.to_string(),
            fields: vec![
                (
                    SERVICE_FIELD.to_string(),
                    EventField::Str(SERVICE_A.to_string()),
                ),
                ("peer".to_string(), EventField::Str("client".to_string())),
                (
                    SECRET_FIELD_A.to_string(),
                    EventField::Str(SECRET_A.to_string()),
                ),
            ],
        },
        ChronosEvent {
            ts_micros: A_OUTBOUND_US,
            probe: PROBE_CALL.to_string(),
            fields: vec![
                (
                    SERVICE_FIELD.to_string(),
                    EventField::Str(SERVICE_A.to_string()),
                ),
                ("target".to_string(), EventField::Str(SERVICE_B.to_string())),
            ],
        },
    ]
}

fn events_for_service_b() -> Vec<ChronosEvent> {
    vec![ChronosEvent {
        ts_micros: B_INBOUND_US,
        probe: PROBE_IN.to_string(),
        fields: vec![
            (
                SERVICE_FIELD.to_string(),
                EventField::Str(SERVICE_B.to_string()),
            ),
            (
                SECRET_FIELD_B.to_string(),
                EventField::Str(SECRET_B.to_string()),
            ),
        ],
    }]
}

/// The trace id the product parser reads out of a `traceparent`. Taking it
/// from `parse_traceparent` rather than slicing the string is the point:
/// the test compares against the same interpretation the pipeline made.
fn trace_id_of(traceparent: &str) -> String {
    let context = parse_traceparent(traceparent)
        .unwrap_or_else(|e| panic!("fixture traceparent {traceparent} does not parse: {e}"));
    let mut hex = String::with_capacity(32);
    for byte in context.trace_id().bytes() {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

// ---------------------------------------------------------------------------
// Inbound rendezvous
// ---------------------------------------------------------------------------

/// Bounded rendezvous that forces the two inbound requests to be in flight
/// inside `service_a` at the same time.
///
/// A `Barrier` would be the obvious choice and is wrong here. Both clients
/// connect unconditionally, so a handler that waits for its peer can only
/// be released once the accept loop has accepted the second connection,
/// which proves the two requests were live together rather than merely
/// issued one after the other. What a barrier cannot do is give up: this
/// gate is joined from `Server::drop`, so an unbounded wait would hang the
/// test process instead of failing it. Every wait below therefore carries
/// `GATE_TIMEOUT`, and a request that never arrives becomes a `500` the
/// assertions below can report.
///
/// `peak` records the high-water mark of concurrent handlers. Forcing the
/// overlap is not the same as checking it: a service that dropped this
/// gate entirely would still process both requests and still emit correct
/// evidence, and only `peak` would notice that the two never coexisted.
struct ArrivalGate {
    arrived: Mutex<usize>,
    peak: AtomicUsize,
    release: Condvar,
}

impl ArrivalGate {
    fn new() -> Self {
        Self {
            arrived: Mutex::new(0),
            peak: AtomicUsize::new(0),
            release: Condvar::new(),
        }
    }

    /// Highest number of handlers ever inside the gate at the same time.
    fn peak(&self) -> usize {
        self.peak.load(Ordering::SeqCst)
    }

    /// Announce this handler and block until `expected` handlers are in
    /// flight together.
    fn enter(&self, expected: usize) -> Result<(), String> {
        let deadline = Instant::now() + GATE_TIMEOUT;
        let mut arrived = self.arrived.lock().unwrap_or_else(|e| e.into_inner());
        *arrived += 1;
        self.peak.fetch_max(*arrived, Ordering::SeqCst);
        self.release.notify_all();
        while *arrived < expected {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(format!(
                    "only {arrived} of {expected} inbound requests were in flight within {GATE_TIMEOUT:?}"
                ));
            }
            let (guard, _) = self
                .release
                .wait_timeout(arrived, remaining)
                .unwrap_or_else(|e| e.into_inner());
            arrived = guard;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Server harness
// ---------------------------------------------------------------------------

/// One loopback HTTP server bound to an ephemeral port.
///
/// The listener is bound in the caller's thread, before the accept loop
/// exists: a bound `TcpListener` is already listening, so the port is
/// reachable the instant `spawn` returns and no test ever has to sleep
/// waiting for a server to boot.
struct Server {
    port: u16,
    stop: Arc<AtomicBool>,
    accept: Option<JoinHandle<()>>,
}

impl Server {
    fn spawn<H>(label: &'static str, handler: H) -> Self
    where
        H: Fn(TcpStream) + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0")
            .unwrap_or_else(|e| panic!("bind {label} to an ephemeral port: {e}"));
        listener
            .set_nonblocking(true)
            .unwrap_or_else(|e| panic!("put the {label} listener in non-blocking mode: {e}"));
        let port = listener
            .local_addr()
            .unwrap_or_else(|e| panic!("read the {label} ephemeral port: {e}"))
            .port();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = Arc::clone(&stop);
        let handler = Arc::new(handler);
        let accept = thread::Builder::new()
            .name(label.to_string())
            .spawn(move || accept_loop(label, listener, handler, stop_thread))
            .unwrap_or_else(|e| panic!("spawn the {label} accept loop: {e}"));
        Self {
            port,
            stop,
            accept: Some(accept),
        }
    }

    fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(accept) = self.accept.take() {
            // A panic here would fire while the test is already unwinding
            // and abort the process, hiding the real failure. A stuck
            // worker is bounded by SOCKET_TIMEOUT anyway, so this join
            // always ends, and any assertion below reports the real reason.
            let _ = accept.join();
        }
    }
}

fn accept_loop<H>(
    label: &'static str,
    listener: TcpListener,
    handler: Arc<H>,
    stop: Arc<AtomicBool>,
) where
    H: Fn(TcpStream) + Send + Sync + 'static,
{
    let mut workers: Vec<JoinHandle<()>> = Vec::new();
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                let handler = Arc::clone(&handler);
                workers.push(
                    thread::Builder::new()
                        .name(label.to_string())
                        .spawn(move || handler(stream))
                        .unwrap_or_else(|e| panic!("spawn a {label} connection worker: {e}")),
                );
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => thread::sleep(ACCEPT_POLL),
            Err(e) => panic!("accept on {label}: {e}"),
        }
    }
    // The listener closed by dropping out of the loop; join the workers so
    // no thread of this server outlives the test.
    for worker in workers {
        let _ = worker.join();
    }
}

// ---------------------------------------------------------------------------
// Minimal HTTP
// ---------------------------------------------------------------------------

struct Request {
    target: String,
    body: String,
}

fn read_request(stream: &TcpStream) -> Result<Request, String> {
    let mut reader = BufReader::new(
        stream
            .try_clone()
            .map_err(|e| format!("clone the inbound stream: {e}"))?,
    );
    let mut head = String::new();
    loop {
        let mut line = String::new();
        let read = reader
            .read_line(&mut line)
            .map_err(|e| format!("read the request head within {SOCKET_TIMEOUT:?}: {e}"))?;
        if read == 0 {
            return Err("the connection closed before the request head ended".to_string());
        }
        head.push_str(&line);
        if line == "\r\n" || line == "\n" {
            break;
        }
    }
    let target = head
        .lines()
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .nth(1)
        .unwrap_or_default()
        .to_string();
    let length = content_length(&head);
    let mut body = vec![0u8; length];
    if length > 0 {
        reader
            .read_exact(&mut body)
            .map_err(|e| format!("read the {length}-byte request body: {e}"))?;
    }
    let body = String::from_utf8(body).map_err(|e| format!("request body is not UTF-8: {e}"))?;
    Ok(Request { target, body })
}

fn content_length(head: &str) -> usize {
    head.lines()
        .filter_map(|line| line.split_once(':'))
        .filter(|(name, _)| name.trim().eq_ignore_ascii_case("content-length"))
        .find_map(|(_, value)| value.trim().parse().ok())
        .unwrap_or(0)
}

fn respond(stream: &mut TcpStream, status: &str, body: &str) -> Result<(), String> {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(response.as_bytes())
        .map_err(|e| format!("write the response: {e}"))?;
    stream
        .flush()
        .map_err(|e| format!("flush the response: {e}"))
}

/// One status line, for failure messages.
fn status_line(response: &str) -> String {
    response
        .lines()
        .next()
        .unwrap_or("<empty response>")
        .to_string()
}

fn connect(port: u16) -> Result<TcpStream, String> {
    let stream = TcpStream::connect(("127.0.0.1", port))
        .map_err(|e| format!("connect to 127.0.0.1:{port}: {e}"))?;
    stream
        .set_read_timeout(Some(SOCKET_TIMEOUT))
        .map_err(|e| format!("arm the read timeout on port {port}: {e}"))?;
    stream
        .set_write_timeout(Some(SOCKET_TIMEOUT))
        .map_err(|e| format!("arm the write timeout on port {port}: {e}"))?;
    Ok(stream)
}

fn http_get(port: u16, path: &str) -> Result<String, String> {
    let mut stream = connect(port)?;
    let request = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("write GET {path} to port {port}: {e}"))?;
    stream
        .flush()
        .map_err(|e| format!("flush GET {path} to port {port}: {e}"))?;
    let mut response = String::new();
    stream.read_to_string(&mut response).map_err(|e| {
        format!("read the answer to GET {path} on port {port} within {SOCKET_TIMEOUT:?}: {e}")
    })?;
    Ok(response)
}

/// POST one JSON Lines record and wait for the consumer's acknowledgement.
///
/// Waiting for the `202` is what orders the evidence handoff: the consumer
/// enqueues the record before it answers, so a service that has posted its
/// spans and answered its own caller has, by definition, already handed
/// those spans to the verifier. Without this wait the test could look at
/// the channel before the last record arrived.
fn post_record(port: u16, line: &str) -> Result<(), String> {
    let mut stream = connect(port)?;
    let head = format!(
        "POST /collect HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/jsonl\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        line.len()
    );
    stream
        .write_all(head.as_bytes())
        .map_err(|e| format!("write the POST head to the consumer: {e}"))?;
    stream
        .write_all(line.as_bytes())
        .map_err(|e| format!("write the record to the consumer: {e}"))?;
    stream
        .flush()
        .map_err(|e| format!("flush the record to the consumer: {e}"))?;
    let mut ack = String::new();
    stream
        .read_to_string(&mut ack)
        .map_err(|e| format!("read the consumer acknowledgement within {SOCKET_TIMEOUT:?}: {e}"))?;
    if !ack.starts_with("HTTP/1.1 202 Accepted") {
        return Err(format!(
            "the consumer answered '{}' to a record",
            status_line(&ack)
        ));
    }
    Ok(())
}

/// `traceparent` out of a request target, the W3C header travelling as a
/// query parameter over this deliberately dumb transport.
fn traceparent_in(target: &str) -> Option<String> {
    let query = target.split_once('?')?.1;
    query
        .split('&')
        .find_map(|pair| pair.strip_prefix("traceparent="))
        .map(str::to_string)
}

/// Answer the caller. A failed request is answered too, with the reason,
/// so a caller blocked on `read_to_string` is released immediately instead
/// of waiting out its socket timeout.
fn finish(stream: &mut TcpStream, label: &'static str, outcome: Result<String, String>) {
    match outcome {
        Ok(body) => {
            let _ = respond(stream, "200 OK", &body);
        }
        Err(reason) => {
            eprintln!("[{label}] {reason}");
            let _ = respond(stream, "500 Internal Server Error", &reason);
        }
    }
}

// ---------------------------------------------------------------------------
// The three services
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Downstream {
    service_b: u16,
    consumer: u16,
}

/// Inbound leg. Runs the pipeline for one request, forwards the inbound
/// context to `service_b`, hands its own spans to the consumer, and answers
/// its caller with the correlation identity it minted.
fn service_a(stream: TcpStream, downstream: Downstream, gate: Arc<ArrivalGate>) {
    let mut stream = stream;
    let outcome = (|| -> Result<String, String> {
        let request = read_request(&stream)?;
        let traceparent = traceparent_in(&request.target)
            .ok_or_else(|| format!("service_a got no traceparent in '{}'", request.target))?;
        // Overlap gate. Placing it after the request read means a handler
        // is only counted once it is genuinely serving a request.
        gate.enter(REQUESTS)
            .map_err(|e| format!("service_a: {e}"))?;
        let outcome = run_service_pipeline(
            Some(&traceparent),
            &events_for_service_a(),
            &opt_in_filter(),
            &export_limits(),
            &redaction(),
            &cardinality(),
        )
        .map_err(|e| e.to_string())?;
        // Forward first: a propagation failure has to reach the caller
        // rather than hide behind a clean response.
        let forwarded = http_get(
            downstream.service_b,
            &format!("/ingest?traceparent={traceparent}"),
        )?;
        if !forwarded.starts_with("HTTP/1.1 200 OK") {
            return Err(format!(
                "service_b answered '{}' to the forwarded request",
                status_line(&forwarded)
            ));
        }
        for line in &outcome.lines {
            post_record(downstream.consumer, line)
                .map_err(|e| format!("service_a could not hand over a span: {e}"))?;
        }
        Ok(format!("recorded\n{}", outcome.recorded.to_recorded()))
    })();
    finish(&mut stream, SERVICE_A, outcome);
}

/// Second leg. Same pipeline, its own invocation id, its own redaction.
fn service_b(stream: TcpStream, consumer_port: u16) {
    let mut stream = stream;
    let outcome = (|| -> Result<String, String> {
        let request = read_request(&stream)?;
        let traceparent = traceparent_in(&request.target)
            .ok_or_else(|| format!("service_b got no traceparent in '{}'", request.target))?;
        let outcome = run_service_pipeline(
            Some(&traceparent),
            &events_for_service_b(),
            &opt_in_filter(),
            &export_limits(),
            &redaction(),
            &cardinality(),
        )
        .map_err(|e| e.to_string())?;
        for line in &outcome.lines {
            post_record(consumer_port, line)
                .map_err(|e| format!("service_b could not hand over a span: {e}"))?;
        }
        Ok(format!("recorded\n{}", outcome.recorded.to_recorded()))
    })();
    finish(&mut stream, SERVICE_B, outcome);
}

/// The verifier's ingress. Enqueues every record of the body, then
/// acknowledges, so a producer's acknowledgement means the evidence is
/// already readable.
fn consumer(stream: TcpStream, evidence: Sender<String>) {
    let mut stream = stream;
    let outcome = (|| -> Result<String, String> {
        let request = read_request(&stream)?;
        let mut handed_over = 0usize;
        for line in request.body.lines().filter(|l| !l.trim().is_empty()) {
            evidence
                .send(line.to_string())
                .map_err(|e| format!("hand a record to the verifier: {e}"))?;
            handed_over += 1;
        }
        Ok(format!("accepted {handed_over}\n"))
    })();
    match outcome {
        Ok(body) => {
            let _ = respond(&mut stream, "202 Accepted", &body);
        }
        Err(reason) => {
            eprintln!("[consumer] {reason}");
            let _ = respond(&mut stream, "500 Internal Server Error", &reason);
        }
    }
}

// ---------------------------------------------------------------------------
// The evidence, read back out of the wire
// ---------------------------------------------------------------------------

/// One record as the consumer's side sees it. Every field is parsed from
/// the JSON that crossed a socket; nothing is carried over from a
/// producing thread.
struct ReceivedSpan {
    line: String,
    trace_id: String,
    span_id: String,
    name: String,
    start_time_unix_nano: u64,
    invocation_id: String,
    probe: String,
    service: String,
    timestamp_domain: String,
    monotonic_ns: u64,
    attributes: Vec<(String, String)>,
}

fn parse_record(line: &str) -> ReceivedSpan {
    // One span per line, decided by parsing rather than by counting
    // braces: a doubled or truncated record can balance and still be wrong.
    let value: Value = serde_json::from_str(line).unwrap_or_else(|e| {
        panic!("the consumer received a record that is not one JSON object: {line} ({e})")
    });
    let attributes: Vec<(String, String)> = attribute_array(&value, line)
        .iter()
        .map(|attribute| {
            (
                string_field(attribute, "key", line).to_string(),
                string_field(attribute, "value", line).to_string(),
            )
        })
        .collect();
    ReceivedSpan {
        line: line.to_string(),
        trace_id: string_field(&value, "trace_id", line).to_string(),
        span_id: string_field(&value, "span_id", line).to_string(),
        name: string_field(&value, "name", line).to_string(),
        start_time_unix_nano: number_field(&value, "start_time_unix_nano", line),
        invocation_id: string_field(&value, "chronos_invocation_id", line).to_string(),
        probe: attribute_of(&attributes, "probe", line).to_string(),
        service: attribute_of(&attributes, SERVICE_FIELD, line).to_string(),
        timestamp_domain: attribute_of(&attributes, ATTR_TIMESTAMP_DOMAIN, line).to_string(),
        monotonic_ns: attribute_of(&attributes, ATTR_TIMESTAMP_MONOTONIC_NS, line)
            .parse()
            .unwrap_or_else(|e| {
                panic!("{ATTR_TIMESTAMP_MONOTONIC_NS} is not a number in {line}: {e}")
            }),
        attributes,
    }
}

fn string_field<'a>(value: &'a Value, key: &str, line: &str) -> &'a str {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("record field {key} is missing or not a string: {line}"))
}

fn number_field(value: &Value, key: &str, line: &str) -> u64 {
    value
        .get(key)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("record field {key} is missing or not a number: {line}"))
}

fn attribute_array<'a>(value: &'a Value, line: &str) -> &'a Vec<Value> {
    value
        .get("attributes")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("record has no attributes array: {line}"))
}

fn attribute_of<'a>(attributes: &'a [(String, String)], key: &str, line: &str) -> &'a str {
    attributes
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.as_str())
        .unwrap_or_else(|| panic!("record carries no attribute {key}: {line}"))
}

fn group_by_trace(spans: &[ReceivedSpan]) -> BTreeMap<String, Vec<&ReceivedSpan>> {
    let mut groups: BTreeMap<String, Vec<&ReceivedSpan>> = BTreeMap::new();
    for span in spans {
        groups.entry(span.trace_id.clone()).or_default().push(span);
    }
    groups
}

// ---------------------------------------------------------------------------
// The properties, one named function each
// ---------------------------------------------------------------------------

/// ADR-0018 section 2.3 wire shape: one span per line, every field the
/// contract names, span ids well formed, `start_time_unix_nano` never
/// invented.
fn assert_one_span_per_record(spans: &[ReceivedSpan]) {
    assert_eq!(
        spans.len(),
        EXPECTED_RECORDS,
        "expected {EXPECTED_RECORDS} records, one per emitted span, got {}",
        spans.len()
    );
    for span in spans {
        assert_eq!(
            span.span_id.len(),
            16,
            "span_id is not 16 hex chars: {}",
            span.line
        );
        assert!(
            span.span_id
                .chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
            "span_id is not lowercase hex: {}",
            span.line
        );
        assert!(
            span.probe == PROBE_IN || span.probe == PROBE_CALL,
            "probe {} is not on the opt-in list: {}",
            span.probe,
            span.line
        );
        assert_eq!(
            span.name,
            format!("chronos.event.{}", span.probe),
            "span name does not name its probe: {}",
            span.line
        );
        assert_eq!(
            span.start_time_unix_nano, 0,
            "start_time_unix_nano must stay 0: chronos observes no wall clock, so a non-zero value here is an invented timestamp (ADR-0018 section 2.3): {}",
            span.line
        );
        assert_eq!(
            span.timestamp_domain, TIMESTAMP_DOMAIN,
            "the clock domain must be disclosed on every span: {}",
            span.line
        );
        assert!(
            span.monotonic_ns > 0,
            "{ATTR_TIMESTAMP_MONOTONIC_NS} must carry the real monotonic value: {}",
            span.line
        );
    }
}

/// Each trace id present in the evidence is exactly the one the matching
/// inbound `traceparent` carried, and each trace carries exactly the spans
/// of one request.
fn assert_trace_ids_match_requests(groups: &BTreeMap<String, Vec<&ReceivedSpan>>) {
    let trace_a = trace_id_of(TRACE_A);
    let trace_b = trace_id_of(TRACE_B);
    assert_ne!(trace_a, trace_b, "fixture trace ids collide");
    assert_eq!(
        groups.len(),
        REQUESTS,
        "the consumer saw {} distinct trace ids ({:?}), expected {REQUESTS}",
        groups.len(),
        groups.keys().collect::<Vec<_>>()
    );
    for trace_id in [&trace_a, &trace_b] {
        let count = groups.get(trace_id.as_str()).map_or(0, Vec::len);
        assert_eq!(
            count,
            A_SPANS + B_SPANS,
            "trace {trace_id} carries {count} records, expected {} ({A_SPANS} from {SERVICE_A}, {B_SPANS} from {SERVICE_B})",
            A_SPANS + B_SPANS
        );
    }
}

/// The M6.6 property: two concurrent requests with distinct contexts do
/// not mix. Every invocation id belongs to exactly one trace, and each
/// service contributed its own invocation to each trace.
fn assert_no_context_crossing(
    spans: &[ReceivedSpan],
    groups: &BTreeMap<String, Vec<&ReceivedSpan>>,
) {
    let mut traces_per_invocation: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for span in spans {
        traces_per_invocation
            .entry(span.invocation_id.as_str())
            .or_default()
            .insert(span.trace_id.as_str());
    }
    for (invocation, traces) in &traces_per_invocation {
        assert_eq!(
            traces.len(),
            1,
            "invocation {invocation} shows up under {} trace ids ({traces:?}); contexts crossed between the concurrent requests",
            traces.len()
        );
    }
    assert_eq!(
        traces_per_invocation.len(),
        REQUESTS * SERVICES,
        "expected {} distinct invocation ids ({REQUESTS} requests x {SERVICES} services), got {}",
        REQUESTS * SERVICES,
        traces_per_invocation.len()
    );
    for (trace_id, group) in groups {
        let per_service: BTreeMap<&str, BTreeSet<&str>> =
            group.iter().fold(BTreeMap::new(), |mut acc, span| {
                acc.entry(span.service.as_str())
                    .or_default()
                    .insert(span.invocation_id.as_str());
                acc
            });
        assert_eq!(
            per_service.len(),
            SERVICES,
            "trace {trace_id} carries records from {} services ({per_service:?}), expected {SERVICES}",
            per_service.len()
        );
        for (service, invocations) in &per_service {
            assert_eq!(
                invocations.len(),
                1,
                "{service} minted {} invocation ids inside trace {trace_id}: a service serves one request per context, so two means a retry reused or crossed a context",
                invocations.len()
            );
        }
    }
}

/// Monotonic chronology per trace, read off the values that arrived rather
/// than off the order they arrived in: `service_a` inbound, then its
/// outbound call, then `service_b` inbound. Arrival order across services
/// is not deterministic and must never be mistaken for event order.
///
/// Records are paired by `(service, probe)`, not by position in the
/// payload, because the probe name is what says which event is the
/// inbound one. A check that only compared sorted values would pass a
/// stream whose inbound and outbound timestamps had been swapped: both
/// sides would sort to the same numbers.
fn assert_monotonic_chronology(groups: &BTreeMap<String, Vec<&ReceivedSpan>>) {
    let sites = [
        (SERVICE_A, PROBE_IN),
        (SERVICE_A, PROBE_CALL),
        (SERVICE_B, PROBE_IN),
    ];
    for (trace_id, group) in groups {
        let mut by_site: BTreeMap<(&str, &str), u64> = BTreeMap::new();
        for span in group {
            let site = (span.service.as_str(), span.probe.as_str());
            assert!(
                by_site.insert(site, span.monotonic_ns).is_none(),
                "trace {trace_id} carries two records for {site:?}; the causal order is not a single chain"
            );
        }
        let mut previous = 0u64;
        for (index, site) in sites.iter().enumerate() {
            let observed = by_site.get(site).unwrap_or_else(|| {
                panic!("trace {trace_id} carries no record for {site:?}, only {by_site:?}")
            });
            assert_eq!(
                *observed,
                EXPECTED_CHRONOLOGY_NS[index],
                "trace {trace_id}: {site:?} carries {observed} ns instead of the monotonic value it was given, so the transport reordered or rewrote the clock domain"
            );
            assert!(
                *observed > previous,
                "trace {trace_id} is out of causal order at {site:?}: {observed} ns does not follow {previous} ns"
            );
            previous = *observed;
        }
    }
}

/// M6.5 redaction, applied per service inside the pipeline: the marker
/// crossed the wire, the secret did not, and the secret-bearing attribute
/// was actually present so the check cannot pass by the field having
/// vanished.
fn assert_secrets_redacted_per_service(spans: &[ReceivedSpan]) {
    let marker = RedactionPolicy::default_secrets().redaction_marker;
    for (field, secret, service) in [
        (SECRET_FIELD_A, SECRET_A, SERVICE_A),
        (SECRET_FIELD_B, SECRET_B, SERVICE_B),
    ] {
        assert!(
            spans
                .iter()
                .any(|span| span.attributes.iter().any(|(key, _)| key == field)
                    && span.service == service),
            "{service} never delivered a {field} attribute, so redaction proved nothing"
        );
        for span in spans.iter().filter(|span| span.service == service) {
            for (key, value) in &span.attributes {
                if key == field {
                    assert_ne!(value, secret, "{field} reached the consumer unredacted");
                    assert_eq!(
                        value,
                        &marker,
                        "{field} was rewritten to '{value}' rather than to the policy marker, on {}",
                        span.line
                    );
                }
            }
        }
    }
    for secret in [SECRET_A, SECRET_B] {
        assert!(
            !spans.iter().any(|span| span.line.contains(secret)),
            "the raw secret {secret} is present in the evidence that reached the consumer"
        );
    }
}

// ---------------------------------------------------------------------------
// The scenario
// ---------------------------------------------------------------------------

/// M6.6 end-to-end over HTTP: two concurrent requests, three services, one
/// evidence channel.
///
/// The sandbox runs its integration tests under tokio, so the entry point
/// follows that convention. The servers and clients below are std threads
/// with blocking, deadline-bounded waits, so nothing depends on the
/// runtime and no `.await` is involved.
#[tokio::test]
async fn cross_service_pipeline_keeps_two_concurrent_traces_separated_over_http() {
    let (evidence_tx, evidence_rx): (Sender<String>, Receiver<String>) = mpsc::channel();

    // Only the resolved ports travel into the handlers: a `Server` guard
    // captured by a closure would drop inside another server's accept
    // loop, which is a hard shutdown order to reason about.
    let consumer = Server::spawn("consumer", move |stream| {
        consumer(stream, evidence_tx.clone())
    });
    let consumer_port = consumer.port();
    let service_b = Server::spawn("service_b", move |stream| service_b(stream, consumer_port));
    let service_b_port = service_b.port();
    let gate = Arc::new(ArrivalGate::new());
    let gate_for_server = Arc::clone(&gate);
    let service_a = Server::spawn("service_a", move |stream| {
        service_a(
            stream,
            Downstream {
                service_b: service_b_port,
                consumer: consumer_port,
            },
            Arc::clone(&gate_for_server),
        )
    });

    // Both callers connect unconditionally and both requests carry their
    // own context, so nothing here has to be serialised to keep the run
    // reproducible. Both client threads are joined below, and each is
    // bounded by SOCKET_TIMEOUT on its socket.
    let client_a = spawn_client(service_a.port(), TRACE_A);
    let client_b = spawn_client(service_a.port(), TRACE_B);

    let response_a = join_client(client_a, "A");
    let response_b = join_client(client_b, "B");

    assert!(
        response_a.starts_with("HTTP/1.1 200 OK"),
        "service_a refused request A: {}",
        status_line(&response_a)
    );
    assert!(
        response_b.starts_with("HTTP/1.1 200 OK"),
        "service_a refused request B: {}",
        status_line(&response_b)
    );
    // Each caller is answered with its own context. A service that
    // answered both callers from one context would already have mixed the
    // requests before a single span reached the consumer.
    assert!(
        response_a.contains(TRACE_A) && !response_a.contains(TRACE_B),
        "the answer to request A does not carry A's traceparent alone: {}",
        status_line(&response_a)
    );
    assert!(
        response_b.contains(TRACE_B) && !response_b.contains(TRACE_A),
        "the answer to request B does not carry B's traceparent alone: {}",
        status_line(&response_b)
    );

    // Every producer waited for its acknowledgement, so all the evidence
    // is already in the channel. Read exactly the expected count with a
    // deadline, then prove there is no surplus.
    let mut records = Vec::with_capacity(EXPECTED_RECORDS);
    for index in 0..EXPECTED_RECORDS {
        match evidence_rx.recv_timeout(EVIDENCE_TIMEOUT) {
            Ok(line) => records.push(line),
            Err(mpsc::RecvTimeoutError::Timeout) => panic!(
                "the consumer handed over {} of {EXPECTED_RECORDS} expected records; record {index} never arrived within {EVIDENCE_TIMEOUT:?}. Propagation or export lost it upstream.",
                records.len()
            ),
            Err(mpsc::RecvTimeoutError::Disconnected) => panic!(
                "the consumer dropped the evidence channel after {} of {EXPECTED_RECORDS} records",
                records.len()
            ),
        }
    }
    match evidence_rx.try_recv() {
        Err(TryRecvError::Empty) => {}
        Ok(extra) => panic!("the consumer received more than {EXPECTED_RECORDS} records: {extra}"),
        Err(TryRecvError::Disconnected) => {}
    }

    let spans: Vec<ReceivedSpan> = records.iter().map(|line| parse_record(line)).collect();
    let groups = group_by_trace(&spans);

    // The two requests were served at the same time, not merely issued at
    // the same time: a service that answered them one after the other
    // would still emit correct evidence, and only the gate's high-water
    // mark tells the two apart.
    assert_eq!(
        gate.peak(),
        REQUESTS,
        "{SERVICE_A} never had more than {} request(s) in flight at once; the two concurrent requests were served serially, so nothing here exercised concurrent context",
        gate.peak()
    );

    assert_one_span_per_record(&spans);
    assert_trace_ids_match_requests(&groups);
    assert_no_context_crossing(&spans, &groups);
    assert_monotonic_chronology(&groups);
    assert_secrets_redacted_per_service(&spans);

    // Servers are dropped here: stop flag set, accept loop joined,
    // per-connection workers joined, listeners closed.
    drop(service_a);
    drop(service_b);
    drop(consumer);
}

fn spawn_client(port: u16, traceparent: &str) -> JoinHandle<Result<String, String>> {
    let traceparent = traceparent.to_string();
    thread::Builder::new()
        .name("cross-service-client".to_string())
        .spawn(move || http_get(port, &format!("/forward?traceparent={traceparent}")))
        .unwrap_or_else(|e| panic!("spawn a client thread: {e}"))
}

fn join_client(client: JoinHandle<Result<String, String>>, label: &str) -> String {
    let response = client
        .join()
        .unwrap_or_else(|_| panic!("the client thread for request {label} panicked"))
        .unwrap_or_else(|e| panic!("request {label} never completed: {e}"));
    response
}
