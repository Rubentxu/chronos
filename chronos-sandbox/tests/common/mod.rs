//! Shared scaffolding for the 1M-event scale lanes.
//!
//! Two integration test targets in this package need the same expensive
//! fixture: a real 1M-event execution log on disk, plus an MCP server that
//! reopens it from what is on disk alone.
//!
//!   * `scale_execution_log_1m.rs` — the read path serves a million events
//!   * `scale_aggregate_p95_1m.rs`  — the *distribution* of aggregate cost
//!
//! Integration test targets cannot import each other, so without this module
//! the second lane would have carried a second copy of the seeder — and a
//! second copy of the fixture is a second copy of the thing that decides
//! whether the numbers mean anything. One seeder, one fixture definition.
//!
//! `chronos-log` is a dev-dependency of this crate, so none of this could live
//! in `src/` even in principle. It is test scaffolding by construction and it
//! stays here.
//!
//! # Scratch space
//!
//! These fixtures write tens of megabytes — a 1M-event log is ~27 MB of
//! segments — and they go wherever [`std::env::temp_dir`] points. On a host
//! where `/tmp` is a quota'd tmpfs rather than a plain directory, seeding dies
//! with `Disk quota exceeded (os error 122)` while `df` still reports free
//! space, because a quota is not a space check. That failure reads as a product
//! bug and is not one, so the fix is the environment's: point `TMPDIR` at a
//! filesystem that can hold the fixture.
//!
//!     TMPDIR=/path/with/room cargo test -p chronos-sandbox --test <lane> -- --ignored

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use chronos_domain::trace::TraceEvent;
use chronos_domain::{EventData, EventType, MonotonicNs, SourceLocation};
use chronos_log::{
    ExecutionKind, ExecutionPayload, NewExecutionRecord, SegmentedConfig, SegmentedExecutionLog,
    SessionId,
};
use chronos_sandbox::client::tools::McpTestClient;
use serde_json::{json, Value};

pub const SESSION: &str = "scale-1m";
pub const EVENTS: u64 = 1_000_000;
/// One thread, so `rollup` has a single bucket and the arithmetic is exact.
pub const THREAD: u64 = 1;
/// Generous ceiling for the aggregate calls, used only to measure the real
/// cost. `McpTestClient::call_tool` hardcodes 30s, which is shorter than the
/// aggregate takes at this size, so the aggregate modes are called through
/// `call_with_timeout` instead.
pub const AGGREGATE_TIMEOUT_SECS: u64 = 600;

/// A fresh, empty directory for one lane's fixture.
///
/// The pid and nanosecond timestamp are in the name so two lanes running
/// concurrently — or a rerun after an aborted one — cannot collide on a
/// directory the previous run left half-seeded.
pub fn temp_root(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "chronos-scale-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("create temp root");
    p
}

/// A minimal but well-formed `TraceEvent`.
///
/// The reader decodes every record into a `TraceEvent` and an undecodable
/// record fails the whole read closed, so the seeded payload must be a real
/// event under the canonical `"trace_event"` tag. Arbitrary bytes would make
/// the log unreadable rather than merely large.
pub fn trace_event(event_id: u64) -> TraceEvent {
    TraceEvent {
        event_id,
        timestamp_ns: MonotonicNs::from(event_id * 1_000),
        thread_id: THREAD,
        event_type: EventType::FunctionEntry,
        location: SourceLocation {
            function: Some("scale_work".to_string()),
            ..SourceLocation::default()
        },
        data: EventData::Function {
            name: "scale_work".to_string(),
            signature: None,
            symbol_id: None,
            invocation_id: None,
            parent_invocation_id: None,
        },
    }
}

/// How many records the current fixture actually wrote.
///
/// The budget stop's anchor has to point inside the session, and the session
/// is whatever the lane seeded — which is not always [`EVENTS`], because one
/// lane deliberately seeds a session one event *over* the ceiling. A bound
/// written against the constant would be false for that lane and vacuous for
/// the rest, so the fixture records what it wrote and the assertion uses it.
static SEEDED_EVENTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Events written by the most recent [`seed_with`] call.
pub fn seeded_events() -> u64 {
    SEEDED_EVENTS.load(std::sync::atomic::Ordering::Relaxed)
}

/// Seed a real 1M-event log with the production writer, and return the seed
/// seconds it took.
///
/// Nothing here hand-writes a manifest, so the segments and retention
/// metadata on disk are exactly what the server will reopen. The log is
/// dropped before the server starts: the server must be able to open it from
/// what is on disk alone.
pub fn seed(root: &Path) -> f64 {
    seed_with(root, EVENTS)
}

/// Seed `events` records instead of [`EVENTS`].
///
/// The size is a parameter because one lane needs a session that is *one
/// event over* a ceiling rather than exactly on it, and a fixed constant
/// cannot express both without lying about one of them.
pub fn seed_with(root: &Path, events: u64) -> f64 {
    let started = Instant::now();
    let dir = root.join(SESSION);
    std::fs::create_dir_all(&dir).expect("create execution-log dir");

    let session_id = SessionId::new(SESSION);
    let log = SegmentedExecutionLog::open(session_id.clone(), SegmentedConfig::with_dir(&dir))
        .expect("open execution log for seeding");

    // Each record is encoded for real, because `event_id` and the timestamp
    // vary per event and the reader decodes them. A reused payload would make
    // this a test of a log where every record claims the same identity.
    for i in 1..=events {
        log.append(NewExecutionRecord {
            session_id: session_id.clone(),
            kind: ExecutionKind::Raw,
            monotonic_ns: i * 1_000,
            payload: ExecutionPayload::new(
                serde_json::to_vec(&trace_event(i)).expect("encode trace event"),
                "trace_event",
            ),
            ..Default::default()
        })
        .expect("append event");
    }
    log.flush().expect("flush seeded log");
    drop(log);

    // The log must really be on disk and really be that large, otherwise
    // every assertion downstream could pass against a log that was never
    // written. The fixture checks itself rather than trusting the loop above.
    let seg_bytes: u64 = std::fs::read_dir(&dir)
        .expect("segment dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("seg"))
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum();
    assert!(
        seg_bytes > 0,
        "no segment bytes were written; every assertion downstream would be vacuous"
    );

    let secs = started.elapsed().as_secs_f64();
    SEEDED_EVENTS.store(events, std::sync::atomic::Ordering::Relaxed);
    eprintln!("seeded {events} events in {secs:.1}s ({seg_bytes} segment bytes on disk)");
    secs
}

/// Start an MCP server against a seeded execution-log root.
///
/// `CHRONOS_EXECUTION_LOG_DIR` is the only route to a genuinely readable
/// session: no MCP tool registers one on demand, and the server bootstraps
/// its registry from the environment at startup. So seeding before spawn is
/// not an optimisation, it is the only way the fixture can exist.
pub async fn start_server(root: &Path) -> McpTestClient {
    let mcp_path = McpTestClient::resolve_mcp_path();
    let mut env = HashMap::new();
    env.insert(
        "CHRONOS_EXECUTION_LOG_DIR".to_string(),
        root.to_string_lossy().to_string(),
    );
    McpTestClient::start_with_env(&mcp_path, &env)
        .await
        .unwrap_or_else(|e| {
            panic!("MCP server must start against a {EVENTS}-event log root, got: {e}")
        })
}

/// Unwrap a raw `tools/call` envelope into the tool's own JSON payload.
///
/// `McpTestClient::call_tool` does this internally; `call_with_timeout` does
/// not — it returns the JSON-RPC envelope verbatim, because the point of
/// taking an explicit timeout is to reach the aggregate modes whose cost the
/// client's hardcoded 30s default would cut short.
///
/// Indexing that envelope as if it were the payload is a mistake this code
/// path used to make: `summary["total_events"]` on
/// `{"id":..,"jsonrpc":..,"result":{..}}` yields `None`, which read as a
/// missing count rather than as a wrong index.
///
/// # Two content blocks, and which one is which
///
/// A **success** envelope carries one content block and it is the JSON.
///
/// A **failure** envelope that carries structured facts carries **two**: the
/// prose first, the JSON second. Both the D2 budget stop and the
/// `cursor_stale` re-anchor do this, and `wire_retention_facts.rs` pins it
/// explicitly — `content[0]` is the text, preserved verbatim because other
/// consumers parse it, and `content[1]` is the JSON.
///
/// So the first block is **not** a reliable place to look for the payload.
/// This helper took `content.first()` and required it to parse as JSON, which
/// works for every success and breaks on every structured failure — and it
/// broke the moment a lane actually hit a budget stop, which is exactly the
/// path the 1M lane documents and had never exercised. Reading the wrong block
/// does not fail loudly: the prose is not JSON, so the symptom is a parse
/// error quoting the human-readable message, which reads like a product bug
/// and is a bug in the reader.
///
/// Hence: first block that parses as JSON wins, whatever its position. If none
/// does, the envelope had no structured payload, and saying so — with the
/// blocks quoted — is more useful than guessing.
pub fn unwrap_tool_envelope(raw: &Value) -> Value {
    let result = raw
        .get("result")
        .unwrap_or_else(|| panic!("tools/call must answer with a result, got {raw}"));
    let content = result
        .get("content")
        .and_then(|c| c.as_array())
        .unwrap_or_else(|| panic!("result must carry a content array, got {raw}"));
    assert!(
        !content.is_empty(),
        "result must carry at least one content block, got {raw}"
    );

    for block in content {
        let Some(text) = block.get("text").and_then(|t| t.as_str()) else {
            continue;
        };
        if let Ok(value) = serde_json::from_str::<Value>(text) {
            return value;
        }
    }

    let blocks: Vec<&str> = content
        .iter()
        .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
        .collect();
    panic!(
        "no content block carried a JSON payload; this envelope is prose-only. Blocks: {blocks:?}. \
         A structured failure carries the JSON in a later block, so a failure to parse block 0 \
         is not by itself a product fault."
    );
}

/// What one whole-log aggregate call returned.
///
/// The two variants are the only honest outcomes after D2, and both lanes
/// need to tell them apart: a *complete* aggregate is a measurement, a
/// *stopped* one is a budget event. Collapsing them would let a p95 be
/// computed partly from walks that never finished.
#[derive(Debug)]
pub enum AggregateOutcome {
    /// The walk finished inside its ceiling and accounted for the log.
    Complete {
        total_events: u64,
        /// The tool's own payload, for the lanes that pin fields beyond the
        /// total. Read by the 1M lane; the p95 lane only needs the cost, so
        /// this field is unused in that target.
        #[allow(dead_code)]
        payload: Value,
        secs: f64,
    },
    /// The read-path budget stopped the walk and offered a resume anchor.
    Stopped {
        reason: String,
        next_seq: u64,
        secs: f64,
    },
}

/// The two whole-log aggregation modes of `execution_log_read`.
#[derive(Debug, Clone, Copy)]
pub enum AggregationOp {
    Summarize,
    Rollup,
}

impl AggregationOp {
    pub fn name(self) -> &'static str {
        match self {
            Self::Summarize => "summarize",
            Self::Rollup => "rollup",
        }
    }

    fn mode(self) -> &'static str {
        self.name()
    }

    /// Run one whole-log aggregation against the seeded 1M session and
    /// measure it.
    pub async fn once(self, client: &mut McpTestClient) -> AggregateOutcome {
        self.once_from(client, None).await
    }

    /// The same call, optionally resuming from a cursor.
    ///
    /// `cursor` is passed through verbatim, so the caller owns the encoding.
    /// It exists because a budget stop hands back a `next_seq`, and the only
    /// way to spend that anchor is a second call carrying a cursor at that
    /// position — which is a contract the wire does not document, and the lane
    /// that resumes says so where it does it.
    pub async fn once_from(
        self,
        client: &mut McpTestClient,
        cursor: Option<&str>,
    ) -> AggregateOutcome {
        let started = Instant::now();
        let mut arguments = json!({ "session_id": SESSION, "mode": self.mode() });
        if let Some(c) = cursor {
            arguments["cursor"] = json!(c);
        }
        let raw = client
            .call_with_timeout(
                "tools/call",
                json!({ "name": "execution_log_read", "arguments": arguments }),
                Duration::from_secs(AGGREGATE_TIMEOUT_SECS),
            )
            .await
            .unwrap_or_else(|e| {
                panic!("{} over {EVENTS} events must answer, got: {e}", self.name())
            });
        let secs = started.elapsed().as_secs_f64();
        let payload = unwrap_tool_envelope(&raw);

        match payload.get("error") {
            None => {
                let total_events = payload
                    .get("total_events")
                    .and_then(Value::as_u64)
                    .unwrap_or_else(|| {
                        panic!(
                            "a complete {} must report total_events, got {payload}",
                            self.name()
                        )
                    });
                AggregateOutcome::Complete {
                    total_events,
                    payload,
                    secs,
                }
            }
            Some(err) => {
                let reason = err.as_str().unwrap_or("<not a string>").to_string();
                assert!(
                    reason.starts_with("read_path_"),
                    "a {} failure must name its reason from the read-path budget contract, \
                     got {reason:?} in {payload}",
                    self.name()
                );
                let next_seq = payload
                    .get("next_seq")
                    .and_then(Value::as_u64)
                    .unwrap_or_else(|| {
                        panic!("a budget stop must carry the resume anchor, got {payload}")
                    });
                assert!(
                    next_seq > 0 && next_seq <= seeded_events(),
                    "the resume anchor must point inside the session ({} events seeded), \
                     got {next_seq}",
                    seeded_events()
                );
                assert_eq!(
                    payload.get("partial"),
                    Some(&json!(true)),
                    "a budget stop must be marked partial, never presented as complete: {payload}"
                );
                AggregateOutcome::Stopped {
                    reason,
                    next_seq,
                    secs,
                }
            }
        }
    }
}
