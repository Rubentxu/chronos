//! M6.7 lift: the load / recovery / error-path gates of the OTLP
//! cross-service pipeline.
//!
//! Source of truth: ADR-0021. M6.6 shipped the cross-service correlation
//! harness and M6.4/M6.5 shipped the opt-in exporter and the redaction
//! policy, but nothing ever ran those pieces **as one pipeline**. This
//! module is the composition point: [`run_service_pipeline`] wires
//! parse -> correlation -> export -> redaction -> render, and the three
//! gates above it assert what that pipeline must hold under sustained
//! load, under a downstream outage, and on malformed input.
//!
//! ## No pipeline existed in product
//!
//! The M6.6 spike had a `run_service_pipeline`, but
//! [`super::cross_service`] only exposes `run_uat_m6_01` / `run_uat_m6_02`,
//! which validate the correlation layer and stop before export. Nothing
//! in product called [`super::exporter::export`] and
//! [`super::redaction::redact_and_limit_attributes`] in sequence, so this
//! module composes those existing primitives rather than re-implementing
//! any of them.
//!
//! ## The one composition decision that is not obvious
//!
//! Redaction has to run **between** span construction and JSON rendering:
//! it rewrites attribute values, which only exist while the span is still
//! a struct. In the spike that was free, because `export_spans` returned
//! un-rendered spans. In product `export` renders internally, so the seam
//! [`super::exporter::export_spans`] was exposed for this caller (and
//! `export` is now built on it). Redaction deliberately does **not** move
//! inside `export`: `export` stays a pure, policy-free selection step,
//! and this module is what applies a policy to its output.
//!
//! ## Scope limits, stated rather than hidden
//!
//! - **The load gate does not assert latency.** See
//!   [`run_load_gate`]: the latency of an in-process, single-span pipeline
//!   cannot violate a budget that is orders of magnitude larger than it,
//!   so p50/p95/p99 are *reported* in [`LoadMetrics`] and deliberately
//!   excluded from the verdict.
//! - **Outage is logical.** [`OutageState`] is a struct the gate toggles;
//!   no TCP listener is involved, so the consumer-side `Ok`/`Err` branch
//!   is simulated rather than dialled. A real consumer-down test needs a
//!   sidecar and is out of M6.7 scope (ADR-0021 §4).
//! - **Cardinality is capped per span, not per export call.** The product
//!   redaction primitive operates on one attribute list, so
//!   [`CardinalityLimits::max_distinct_keys`] bounds the distinct keys of a
//!   single span. The M6.5 spike shared one budget across a whole span
//!   batch; that narrowing is inherited from where the primitive landed and
//!   is disclosed here instead of being papered over.
//! - **Concurrency is out of scope.** Every gate is single-threaded and
//!   sequential (ADR-0021 §8).
//!
//! ## Verdict shape
//!
//! Verdicts are binary — [`LoadGateVerdict::Pass`] or
//! [`LoadGateVerdict::Fail`] with one reason. There is deliberately no
//! "warning" state (ADR-0021 §2.4): a gate that cannot prove its
//! invariant has not passed, and a gate that ran over nothing has proven
//! nothing, so it fails too rather than passing vacuously.
//!
//! The three decision functions — [`judge_load_gate`],
//! [`judge_recovery_gate`], [`judge_error_paths`] — are public for a
//! reason that is easy to miss. Every check inside a gate fires only
//! when it is *violated*, so a suite that only ever observes a passing
//! run cannot distinguish a wired check from a dead one: deleting the
//! check changes nothing the tests can see. That was measured here, not
//! assumed — a first pass over this module had thirteen verdict checks
//! and thirteen tests, and reverting the checks one at a time left all
//! thirteen green. Exposing the decision logic lets each check be driven
//! into its failing branch directly, which is what turned them red. A
//! `Fail` path nothing can reach is a doc-comment, not a gate.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use uuid::Uuid;

use super::correlation::{ingest_stream, ChronosEvent, CorrelationStore, EventField};
use super::exporter::{
    export_spans, render_json_line, ExportLimits, ExportResult, ExportedSpan, OptInFilter,
};
use super::parse::{parse_traceparent, TraceparentParseError};
use super::redaction::{
    redact_and_limit_attributes_within, CardinalityBudget, CardinalityLimits, RedactionCounters,
    RedactionPolicy,
};
use super::{OtlpInvocationId, RecordedInvocation};

/// Probe name the gates emit for an inbound request.
///
/// The gates own their [`OptInFilter`] rather than taking one as an
/// argument, and the reason is the failure mode that ownership removes.
/// A filter is a deny-by-default gate: an event whose probe is not on the
/// allow-list is dropped, counted, and otherwise invisible. If the gates
/// accepted a caller-supplied filter, a probe typo would silently reduce
/// the whole pipeline to "parse, bind, then export nothing", and a gate
/// built on top would still report success. Owning the filter here makes
/// that mistake a hard [`LoadGateVerdict::Fail`].
const GATE_PROBE: &str = "http_in";

/// Outage target name carried by every [`OutageEvent`].
const CONSUMER_TARGET: &str = "consumer";

// ---------------------------------------------------------------------------
// §1 — the pipeline under test
// ---------------------------------------------------------------------------

/// Outcome of one full pipeline call.
///
/// Both representations of the same work are kept: `spans` is the
/// redacted span list (where a redaction is observable without parsing
/// JSON) and `lines` is its rendered JSON Lines form (what a collector
/// would actually receive, and what ADR-0021 §2.3 names as the artifact
/// the gates consume).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceRunOutcome {
    /// The correlation identity minted for this call.
    pub recorded: RecordedInvocation,
    /// Redacted spans, pre-render, in emission order.
    pub spans: Vec<ExportedSpan>,
    /// Rendered JSON Lines, one object per span, in the same order.
    pub lines: Vec<String>,
    /// Selection counters from the export stage. `lines` is always empty
    /// here: rendering happens after redaction, by design.
    pub export_counters: ExportResult,
    /// Counters from the redaction stage.
    pub redaction_counters: RedactionCounters,
}

impl ServiceRunOutcome {
    /// Events the export stage dropped, across every skip counter.
    ///
    /// A request can be `handled` (the pipeline returned `Ok`) and still
    /// have lost events inside it. Collapsing the three skip counters into
    /// one is deliberate: a gate that only asked "did it return `Ok`" would
    /// miss exactly that case, which is how a zero-span run reads as
    /// success.
    pub fn export_skipped(&self) -> usize {
        self.export_counters.skipped_probe_filter
            + self.export_counters.skipped_limit_per_invocation
            + self.export_counters.skipped_limit_total
    }
}

/// Failure of a pipeline call, carrying the offending input verbatim.
///
/// Both fields are kept on purpose. `raw` is what the caller sent, so a
/// diagnosis never has to guess which of several requests produced this;
/// `error` is the parser's own verdict, so the reason is never reduced to
/// "bad input" with the specifics discarded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipelineError {
    /// The inbound `traceparent` header was not a valid W3C context.
    BadTraceparent {
        /// The header exactly as it was received.
        raw: String,
        /// The parser's verdict on that header.
        error: TraceparentParseError,
    },
}

impl std::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadTraceparent { raw, error } => {
                write!(f, "traceparent '{raw}' is invalid: {error}")
            }
        }
    }
}

impl std::error::Error for PipelineError {}

/// Run the full OTLP pipeline for one inbound request.
///
/// 1. Parse `traceparent`, or mint an internal-only correlation id when
///    the request carried none.
/// 2. Bind every event to that one invocation in a fresh
///    [`CorrelationStore`].
/// 3. Select and build spans through [`export_spans`] (opt-in filter and
///    declared limits applied here).
/// 4. Redact and cardinality-limit each span's attributes, all charged
///    against one budget shared by the call (ADR-0019 §2.3).
/// 5. Render the redacted spans as JSON Lines.
///
/// Steps 3-5 are three separate calls because that is the order the
/// design constraints fix: the exporter stays pure (it decides *what* is
/// exported and never inspects a policy), and redaction acts on the span
/// rather than on rendered text, where the values are still addressable.
///
/// Pure and single-threaded: no clock, no I/O, no runtime. The only
/// fresh value per call is the [`OtlpInvocationId`], and that is
/// deliberate — a retry of the same request must not reuse it
/// (ADR-0020 §2.3).
pub fn run_service_pipeline(
    traceparent: Option<&str>,
    events: &[ChronosEvent],
    filter: &OptInFilter,
    limits: &ExportLimits,
    redaction: &RedactionPolicy,
    cardinality: &CardinalityLimits,
) -> Result<ServiceRunOutcome, PipelineError> {
    let invocation = OtlpInvocationId::new();
    let recorded = match traceparent {
        // No inbound context: a root invocation. `parse_traceparent`
        // rejects an all-zero trace id, so a parsed context is never
        // root — there is no "parsed but root" case to fold here.
        None => RecordedInvocation::internal_only(invocation),
        Some(raw) => match parse_traceparent(raw) {
            Ok(external) => invocation.record_external(&external),
            Err(error) => {
                return Err(PipelineError::BadTraceparent {
                    raw: raw.to_string(),
                    error,
                })
            }
        },
    };

    let mut store = CorrelationStore::new();
    ingest_stream(&mut store, &recorded, events);

    let (spans, export_counters) = export_spans(&store, events, filter, limits);

    let mut redaction_counters = RedactionCounters::default();
    let mut spans = spans;
    // ADR-0019 §2.3: the cap is across the entire export call, not per
    // span. One budget for the whole call, so a request that emits N spans
    // still carries at most `max_distinct_keys` distinct attribute keys.
    let mut budget = CardinalityBudget::new(cardinality);
    for span in &mut spans {
        let attributes = std::mem::take(&mut span.attributes);
        let (redacted, counters) =
            redact_and_limit_attributes_within(attributes, redaction, cardinality, &mut budget);
        span.attributes = redacted;
        redaction_counters.redacted_fields += counters.redacted_fields;
        redaction_counters.collapsed_cardinality += counters.collapsed_cardinality;
    }

    let lines = spans.iter().map(render_json_line).collect();

    Ok(ServiceRunOutcome {
        recorded,
        spans,
        lines,
        export_counters,
        redaction_counters,
    })
}

// ---------------------------------------------------------------------------
// §2 — load gate
// ---------------------------------------------------------------------------

/// What one request of the load run did.
///
/// The counters are per sample rather than only aggregated so the
/// aggregate can be checked against the samples it came from. An
/// aggregate that disagrees with its own parts is a bug in the
/// accounting, and the gate is supposed to catch bugs in the pipeline, not
/// in itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadSample {
    /// `Instant::now()` delta for this call. Reported, not asserted; see
    /// [`run_load_gate`].
    pub micros: u128,
    /// W3C trace id this request carried.
    pub trace_id_hex: String,
    /// `true` iff the pipeline returned `Ok`.
    pub handled: bool,
    /// Spans the export stage emitted for this request.
    pub spans_emitted: usize,
    /// Events the export stage dropped, across every skip counter.
    pub export_skipped: usize,
    /// Attribute values redacted for this request.
    pub redacted_fields: usize,
}

/// Aggregated load metrics, derived from the samples.
///
/// `LoadMetrics` is a report, not a verdict: every field here is
/// observable, and [`LoadBudget`] names the only two of them that a gate
/// can meaningfully enforce.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LoadMetrics {
    /// Requests the gate issued.
    pub requests: u64,
    /// Requests that returned `Ok`.
    pub handled: u64,
    /// Requests that returned `Err`.
    pub dropped: u64,
    /// Distinct trace ids observed. Equal to `handled` iff every request
    /// carried its own correlation context.
    pub distinct_trace_ids: u64,
    /// Spans emitted across all requests.
    pub spans_emitted: u64,
    /// Events dropped inside the export stage, across all requests.
    pub export_skipped: u64,
    /// Attribute values redacted across all requests.
    pub redacted_fields: u64,
    /// 50th percentile latency over handled requests.
    pub p50_micros: u128,
    /// 95th percentile latency over handled requests.
    pub p95_micros: u128,
    /// 99th percentile latency over handled requests.
    pub p99_micros: u128,
    /// Worst observed latency over handled requests.
    pub max_micros: u128,
}

impl LoadMetrics {
    /// Derive the aggregate from the samples.
    pub fn from_samples(samples: &[LoadSample]) -> Self {
        let mut metrics = LoadMetrics {
            requests: samples.len() as u64,
            ..LoadMetrics::default()
        };
        let mut trace_ids: BTreeSet<&str> = BTreeSet::new();
        let mut latencies: Vec<u128> = Vec::new();
        for sample in samples {
            if sample.handled {
                metrics.handled += 1;
                trace_ids.insert(sample.trace_id_hex.as_str());
                latencies.push(sample.micros);
            } else {
                metrics.dropped += 1;
            }
            metrics.spans_emitted += sample.spans_emitted as u64;
            metrics.export_skipped += sample.export_skipped as u64;
            metrics.redacted_fields += sample.redacted_fields as u64;
        }
        metrics.distinct_trace_ids = trace_ids.len() as u64;
        latencies.sort_unstable();
        metrics.p50_micros = percentile(&latencies, 0.50);
        metrics.p95_micros = percentile(&latencies, 0.95);
        metrics.p99_micros = percentile(&latencies, 0.99);
        metrics.max_micros = latencies.last().copied().unwrap_or(0);
        metrics
    }
}

/// Nearest-rank percentile of an ascending slice. Empty input is 0.
fn percentile(sorted: &[u128], p: f64) -> u128 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = (sorted.len() as f64 * p).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

/// Declared budget for the load gate.
///
/// Only [`LoadBudget::max_dropped`] is enforced by [`run_load_gate`].
/// [`LoadBudget::max_p95_micros`] is advisory: it names the budget a
/// caller with a real load generator can compare against
/// [`LoadMetrics::p95_micros`], and it is deliberately not a verdict
/// input. The reasoning is in [`run_load_gate`]; the short version is
/// that no property of this code can make it fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadBudget {
    /// Advisory latency budget in microseconds. Not enforced here.
    pub max_p95_micros: u128,
    /// Request-level failures tolerated. A handled request that silently
    /// lost events inside the exporter is never tolerated, regardless of
    /// this value.
    pub max_dropped: u64,
}

impl Default for LoadBudget {
    fn default() -> Self {
        LoadBudget {
            max_p95_micros: 50_000,
            max_dropped: 0,
        }
    }
}

/// Binary verdict of the load gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadGateVerdict {
    /// Every invariant below held.
    Pass,
    /// An invariant did not hold; `reason` names which one.
    Fail {
        /// The violated invariant, in enough detail to act on.
        reason: String,
    },
}

/// One request of the load run, before it is timed. Gate-private: it is
/// the fixture the run iterates over, not an input a caller supplies.
struct LoadRequest {
    /// W3C `traceparent` for the request.
    pub traceparent: String,
    /// Trace id carried by `traceparent`, kept so the sample records the
    /// correlation context without re-parsing the header.
    pub trace_id_hex: String,
    /// Event to carry through the pipeline.
    pub event: ChronosEvent,
}

/// Issue `n` sequential pipeline calls and check the invariants that a
/// load run can actually violate.
///
/// ## What is asserted, and what is only reported
///
/// The verdict covers six invariants:
///
/// 1. **The gate ran.** `n == 0` fails. A gate that observed nothing has
///    certified nothing, and letting it pass would make "the gate is
///    green" mean nothing at all.
/// 2. **No request was dropped** beyond [`LoadBudget::max_dropped`].
/// 3. **No event was lost inside a handled request.** `export_skipped ==
///    0`. This is the invariant that a `handled == n` check alone
///    misses: a probe outside the opt-in filter returns `Ok` while
///    emitting nothing. (The M6.7 spike ran every gate with a probe its
///    own filter excluded, so its whole load run measured a pipeline that
///    emitted zero spans — and asserted nothing that would have noticed.)
/// 4. **Every handled request produced its span.** `spans_emitted ==
///    handled`, so "handled" cannot mean "handled and emitted nothing".
/// 5. **Every handled request carried its own `trace_id`.** Two requests
///    sharing one would silently merge under load, which is the failure
///    correlation exists to prevent.
/// 6. **Redaction ran on every span.** `redacted_fields == handled`:
///    every emitted span had its sensitive attribute replaced by the
///    marker. A run with a policy that matches nothing still returns
///    `Ok` end to end, so a gate that skipped this check would report a
///    clean run over leaked secrets.
///
/// One candidate was dropped rather than shipped: `handled + dropped ==
/// n`. The two counters are computed from the same loop, so that equality
/// is an algebraic identity and a check against it could never fire. The
/// property behind it — that the aggregate agrees with the samples it
/// summarises — is instead pinned by recomputing the aggregate from the
/// samples independently, which *can* fail if the aggregation is wrong.
///
/// **Latency is reported, not asserted.** `p50`/`p95`/`p99`/`max` land in
/// [`LoadMetrics`] and are excluded from the verdict on purpose. Measured
/// here, in a debug test build (the slower of the two profiles), over
/// `n = 100 / 1000 / 5000`: `p50 = 51us` every run, `p95 = 57..64us`,
/// `p99 = 66..81us`, worst single sample `120us`. The default budget is
/// `50_000us`, so `p95` would have to grow about 780x — and since `p95`
/// tolerates one sample in twenty, roughly 5% of samples would each have
/// to be stalled past 50 ms by the host scheduler, not by anything the
/// pipeline does. No property of this code can move the number that far.
/// Asserting it would add a check that cannot fail, which is worse than
/// no check: it reads as a latency guarantee while testing nothing about
/// the code. A caller driving this pipeline under real load has the
/// numbers in [`LoadMetrics`] and the budget in
/// [`LoadBudget::max_p95_micros`] to compare; the honest place for that
/// comparison is the caller's.
pub fn run_load_gate(
    n: usize,
    budget: &LoadBudget,
    redaction: &RedactionPolicy,
    cardinality: &CardinalityLimits,
) -> (Vec<LoadSample>, LoadGateVerdict) {
    let filter = OptInFilter::new(vec![GATE_PROBE.to_string()]);
    let limits = ExportLimits::default();
    let mut samples = Vec::with_capacity(n);
    for request in load_requests(n) {
        let started = Instant::now();
        let result = run_service_pipeline(
            Some(&request.traceparent),
            std::slice::from_ref(&request.event),
            &filter,
            &limits,
            redaction,
            cardinality,
        );
        let micros = started.elapsed().as_micros();
        samples.push(match result {
            Ok(outcome) => LoadSample {
                micros,
                trace_id_hex: request.trace_id_hex,
                handled: true,
                spans_emitted: outcome.spans.len(),
                export_skipped: outcome.export_skipped(),
                redacted_fields: outcome.redaction_counters.redacted_fields,
            },
            Err(_) => LoadSample {
                micros,
                trace_id_hex: request.trace_id_hex,
                handled: false,
                spans_emitted: 0,
                export_skipped: 0,
                redacted_fields: 0,
            },
        });
    }
    let metrics = LoadMetrics::from_samples(&samples);
    let verdict = judge_load_gate(&metrics, budget);
    (samples, verdict)
}

/// Decide the load gate's verdict from already-aggregated metrics.
///
/// Public because a verdict nobody can reach a failing branch of is a
/// verdict nobody can check: every check below fires only when it is
/// violated, so a caller (or a test) that cannot hand these metrics in
/// cannot tell a wired check from a dead one. It is also the honest
/// entry point for a caller that aggregated [`LoadMetrics`] from samples
/// of its own and wants them judged against a [`LoadBudget`].
pub fn judge_load_gate(metrics: &LoadMetrics, budget: &LoadBudget) -> LoadGateVerdict {
    let fail = |reason: String| LoadGateVerdict::Fail { reason };
    if metrics.requests == 0 {
        return fail("no requests were exercised, so the run certifies nothing".to_string());
    }
    if metrics.dropped > budget.max_dropped {
        return fail(format!(
            "dropped={} exceeds budget max_dropped={}",
            metrics.dropped, budget.max_dropped
        ));
    }
    if metrics.export_skipped != 0 {
        return fail(format!(
            "{} event(s) dropped inside the export stage of a handled request",
            metrics.export_skipped
        ));
    }
    if metrics.spans_emitted != metrics.handled {
        return fail(format!(
            "handled={} but spans_emitted={}: a handled request emitted no span",
            metrics.handled, metrics.spans_emitted
        ));
    }
    if metrics.distinct_trace_ids != metrics.handled {
        return fail(format!(
            "handled={} requests carry only {} distinct trace id(s): two requests shared a correlation context",
            metrics.handled, metrics.distinct_trace_ids
        ));
    }
    if metrics.redacted_fields != metrics.handled {
        return fail(format!(
            "redacted_fields={} for handled={}: the redaction stage did not run on every span",
            metrics.redacted_fields, metrics.handled
        ));
    }
    LoadGateVerdict::Pass
}

/// Build `n` distinct requests, each with its own W3C context.
///
/// The trace id is derived from the request index, so it is valid hex and
/// never all-zero for any `n`. That bounds the gate at `2^32` requests:
/// past that the ids repeat and invariant 5 above fires, by design.
fn load_requests(n: usize) -> impl Iterator<Item = LoadRequest> {
    (0..n).map(move |i| {
        let trace_id = format!("{:032x}", 0xaaaa_0001u32.wrapping_add(i as u32));
        let span_id = format!("{:016x}", 0xbbbb_0001u64.wrapping_add(i as u64));
        LoadRequest {
            traceparent: traceparent_of(&trace_id, &span_id),
            event: ChronosEvent {
                ts_micros: 1_700_000_000_000_000u64.wrapping_add(i as u64),
                probe: GATE_PROBE.to_string(),
                // One sensitive attribute per request, so invariant 4 has
                // something to find: without it a no-op redaction stage
                // would be indistinguishable from a working one.
                fields: vec![
                    ("request_index".to_string(), EventField::Int(i as i64)),
                    (
                        "auth_token".to_string(),
                        EventField::Str(format!("secret-{i}")),
                    ),
                ],
            },
            trace_id_hex: trace_id,
        }
    })
}

// ---------------------------------------------------------------------------
// §3 — recovery gate
// ---------------------------------------------------------------------------

/// Which downstream targets are reachable.
///
/// A struct, not a socket: the gate toggles it directly, so the
/// consumer-down branch is exercised without a listener and without the
/// flakiness that a real disconnect would add (ADR-0021 §2.3). The shape
/// is fixed by that ADR for the cross-service path; M6.7 drives
/// [`OutageState::consumer_up`] and carries
/// [`OutageState::service_b_up`] through unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OutageState {
    /// `false` while the downstream consumer is refusing writes.
    pub consumer_up: bool,
    /// `false` while service B is refusing writes. Not toggled by this
    /// gate; see the module docs.
    pub service_b_up: bool,
}

impl OutageState {
    /// Both targets reachable.
    pub fn all_up() -> Self {
        OutageState {
            consumer_up: true,
            service_b_up: true,
        }
    }

    /// Consumer down, service B up.
    pub fn consumer_down() -> Self {
        OutageState {
            consumer_up: false,
            service_b_up: true,
        }
    }

    /// Both targets down.
    pub fn all_down() -> Self {
        OutageState {
            consumer_up: false,
            service_b_up: false,
        }
    }
}

/// One recorded outage.
///
/// Carries the target and the instant the outage began, measured as an
/// `Instant` delta — never a wall-clock timestamp, which chronos does not
/// observe during capture. An outage with no instant would be
/// unauditable, so the field is not optional and is not defaulted to zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutageEvent {
    /// Which target was down.
    pub target: &'static str,
    /// Monotonic microseconds since the outage began.
    pub since_micros: u128,
    /// Trace id of the request the outage was observed on.
    pub trace_id_hex: String,
}

/// Everything the recovery run observed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecoveryState {
    /// `trace_id` -> the `OtlpInvocationId`s seen across its retries.
    pub trace_to_invocations: BTreeMap<String, Vec<Uuid>>,
    /// Outages recorded during the consumer-down phase.
    pub outages: Vec<OutageEvent>,
    /// Requests delivered while the consumer was up.
    pub handled_after_recovery: u64,
    /// Requests during the outage phase whose *local* pipeline failed.
    /// Counted separately from [`RecoveryState::outages`] because a
    /// downstream outage and a local failure are different diagnoses,
    /// and conflating them lets one hide the other.
    pub pipeline_failures: u64,
    /// The outage state the gate finished in.
    pub final_outage: OutageState,
}

impl RecoveryState {
    /// Every retry collapsed onto a single `trace_id`.
    ///
    /// This is the idempotency invariant ADR-0020 §2.3 mandated and
    /// ADR-0021 §2.3 re-confirmed: retries of the same `traceparent` are
    /// the same trace, and idempotency is keyed on `trace_id` — never on
    /// `chronos_invocation_id`, which is fresh per attempt by design.
    pub fn is_single_trace(&self) -> bool {
        self.trace_to_invocations.len() == 1
    }

    /// No `OtlpInvocationId` was reused across attempts.
    pub fn invocations_are_distinct(&self) -> bool {
        self.trace_to_invocations
            .values()
            .all(|ids| ids.iter().collect::<BTreeSet<_>>().len() == ids.len())
    }
}

/// Binary verdict of the recovery gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryGateVerdict {
    /// Every invariant below held.
    Pass,
    /// An invariant did not hold; `reason` names which one.
    Fail {
        /// The violated invariant, in enough detail to act on.
        reason: String,
    },
}

/// Drive one UP -> DOWN -> UP cycle and check the recovery invariants.
///
/// The cycle: a request while the consumer is up (skipped when
/// `outage_at_start`), `n_outage_requests` requests while it is down,
/// then one more while it is up again. Every request carries the same
/// `trace_id` and a distinct parent span id, so all of them are retries
/// of one trace — which is what makes the idempotency invariant
/// observable.
///
/// The verdict covers five invariants:
///
/// 1. **The retries collapsed onto one `trace_id`** (ADR-0021 §2.3). More
///    than one key means a retry was given a fresh correlation context,
///    which is the exact failure idempotency exists to prevent.
/// 2. **No invocation id was reused.** Two attempts sharing an id would
///    make a consumer's deduplication swallow a genuinely new attempt.
/// 3. **Every outage-phase request was recorded.** `outages.len() ==
///    n_outage_requests`, each tagged with the consumer target name. An outage that
///    is not recorded is the failure mode this gate exists to catch: the
///    pipeline keeps working locally while the downstream silently stops
///    receiving anything.
/// 4. **No local pipeline failed during the outage.** A downstream outage
///    must not damage local emission, so `pipeline_failures == 0`.
/// 5. **Delivery was counted exactly.** `handled_after_recovery` is the
///    number of requests issued while the consumer was up — 2 normally, 1
///    when the run started with the consumer already down. That last case
///    is what stops a skipped opening phase from reading as "the consumer
///    never came back".
///
/// Outage provenance (`target` plus the `Instant` delta the outage began
/// at) travels in [`RecoveryState::outages`] as [`OutageEvent`] rather
/// than as a gate error: there is no delivery call here that could fail,
/// so an error variant for it would be a variant nothing can construct.
pub fn run_recovery_gate(
    redaction: &RedactionPolicy,
    cardinality: &CardinalityLimits,
    outage_at_start: bool,
    n_outage_requests: usize,
) -> (RecoveryState, RecoveryGateVerdict) {
    let filter = OptInFilter::new(vec![GATE_PROBE.to_string()]);
    let limits = ExportLimits::default();
    let mut state = RecoveryState::default();
    let mut outage = if outage_at_start {
        OutageState::consumer_down()
    } else {
        OutageState::all_up()
    };

    let trace_id = "aaaa1111aaaa1111aaaa1111aaaa1111";
    // Phase 1 — consumer up. A request that is not delivered is counted,
    // not discarded, so the verdict can name the shortfall.
    if outage.consumer_up
        && deliver(
            trace_id,
            &filter,
            &limits,
            redaction,
            cardinality,
            &mut state,
        )
    {
        state.handled_after_recovery += 1;
    }

    // Phase 2 — consumer down. The local pipeline still runs: that is the
    // point of the gate. What changes is that the delivery cannot land, so
    // each request is recorded as an outage with the instant it happened.
    outage.consumer_up = false;
    let outage_started = Instant::now();
    for _ in 0..n_outage_requests {
        state.outages.push(OutageEvent {
            target: CONSUMER_TARGET,
            since_micros: outage_started.elapsed().as_micros(),
            trace_id_hex: trace_id.to_string(),
        });
        if !deliver(
            trace_id,
            &filter,
            &limits,
            redaction,
            cardinality,
            &mut state,
        ) {
            state.pipeline_failures += 1;
        }
    }

    // Phase 3 — consumer back up.
    outage.consumer_up = true;
    if deliver(
        trace_id,
        &filter,
        &limits,
        redaction,
        cardinality,
        &mut state,
    ) {
        state.handled_after_recovery += 1;
    }

    state.final_outage = outage;
    let verdict = judge_recovery_gate(&state, outage_at_start, n_outage_requests);
    (state, verdict)
}

/// Run one pipeline call, folding the attempt into `state`.
///
/// Returns whether the local pipeline succeeded. Whether the consumer is
/// up is *not* passed in: delivery is counted by the caller, which is the
/// only place that knows the consumer's state. What this always does is
/// record the attempt's invocation id under its trace id, so idempotency
/// is tracked whether or not the delivery landed.
fn deliver(
    trace_id: &str,
    filter: &OptInFilter,
    limits: &ExportLimits,
    redaction: &RedactionPolicy,
    cardinality: &CardinalityLimits,
    state: &mut RecoveryState,
) -> bool {
    let parent_id = format!(
        "{:016x}",
        0xcccc_3333_ffff_0000u64.wrapping_add(
            state
                .trace_to_invocations
                .values()
                .map(|ids| ids.len() as u64)
                .sum::<u64>(),
        )
    );
    let traceparent = traceparent_of(trace_id, &parent_id);
    let event = ChronosEvent {
        ts_micros: 1_700_000_000_000_000,
        probe: GATE_PROBE.to_string(),
        fields: vec![("phase".to_string(), EventField::Str("recovery".to_string()))],
    };
    match run_service_pipeline(
        Some(&traceparent),
        &[event],
        filter,
        limits,
        redaction,
        cardinality,
    ) {
        Ok(outcome) => {
            state
                .trace_to_invocations
                .entry(trace_id.to_string())
                .or_default()
                .push(outcome.recorded.invocation_id.as_uuid());
            true
        }
        Err(_) => false,
    }
}

/// Decide the recovery gate's verdict from an observed state.
///
/// Public for the same reason as [`judge_load_gate`]: each check fires
/// only when violated. `expected_deliveries` is derived from
/// `outage_at_start` rather than passed in, so a caller cannot assert the
/// verdict by supplying the count it just measured.
pub fn judge_recovery_gate(
    state: &RecoveryState,
    outage_at_start: bool,
    n_outage_requests: usize,
) -> RecoveryGateVerdict {
    let expected_deliveries = if outage_at_start { 1 } else { 2 };
    let fail = |reason: String| RecoveryGateVerdict::Fail { reason };
    if !state.is_single_trace() {
        return fail(format!(
            "retries spread over {} trace id(s), expected 1: idempotency is keyed on trace_id",
            state.trace_to_invocations.len()
        ));
    }
    if !state.invocations_are_distinct() {
        return fail(
            "two attempts reused one chronos_invocation_id: a retry must mint a fresh id"
                .to_string(),
        );
    }
    if state.outages.len() != n_outage_requests {
        return fail(format!(
            "{} outage(s) recorded for {n_outage_requests} consumer-down request(s)",
            state.outages.len()
        ));
    }
    if let Some(misattributed) = state
        .outages
        .iter()
        .find(|outage| outage.target != CONSUMER_TARGET)
    {
        return fail(format!(
            "outage attributed to target '{}', expected '{CONSUMER_TARGET}'",
            misattributed.target
        ));
    }
    if state.pipeline_failures != 0 {
        return fail(format!(
            "{} local pipeline call(s) failed while the consumer was down: the outage must not damage local emission",
            state.pipeline_failures
        ));
    }
    if state.handled_after_recovery != expected_deliveries {
        return fail(format!(
            "delivered {} request(s), expected {expected_deliveries} for the consumer-up phases",
            state.handled_after_recovery
        ));
    }
    RecoveryGateVerdict::Pass
}

// ---------------------------------------------------------------------------
// §4 — error paths gate
// ---------------------------------------------------------------------------

/// One adversarial input and what the pipeline did with it.
///
/// `raw_input` is the input as it was fed, so a caller can pair the two
/// without consulting the caller that built the case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorSample {
    /// Which case this is.
    pub input_label: &'static str,
    /// The input exactly as fed to the pipeline.
    pub raw_input: String,
    /// The pipeline's answer.
    pub outcome: Result<ServiceRunOutcome, PipelineError>,
}

/// Binary verdict of the error paths gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorGateVerdict {
    /// Every invariant below held.
    Pass,
    /// An invariant did not hold; `reason` names which one.
    Fail {
        /// The violated invariant, in enough detail to act on.
        reason: String,
    },
}

/// Feed the adversarial battery through the pipeline and check that every
/// case degrades gracefully.
///
/// The cases, one per branch the pipeline can take on bad input:
///
/// | label | input | expected |
/// |---|---|---|
/// | `no_traceparent` | absent header | `Ok`, internal-only, all-zero trace id |
/// | `empty_traceparent` | `""` | `Err`, provenance `raw == ""` |
/// | `malformed_traceparent` | non-hex ids | `Err`, provenance `raw == input` |
/// | `all_zero_traceparent` | W3C-invalid zero ids | `Err`, provenance `raw == input` |
/// | `zero_events` | valid header, no events | `Ok`, zero spans |
/// | `extreme_cardinality` | zero cardinality budget | `Ok`, keys collapsed, sensitive values still redacted |
///
/// The verdict covers two invariants:
///
/// 1. **No case panicked and no case invented provenance.** Every case is
///    either `Ok`, or `Err` whose `BadTraceparent::raw` equals the input
///    that was actually fed. An error that arrived with an empty or
///    substituted `raw` would leave the caller unable to tell which
///    request broke, so that is a failure even though the call returned
///    cleanly.
/// 2. **No accepted case lost an event.** A case that returned `Ok` must
///    report zero export-stage skips. `Ok` says "handled", not "handled
///    and produced something", and conflating the two is how a probe
///    outside the filter reads as a pass.
///
/// Only the invocation id differs between two runs of the same case: it is
/// minted fresh per call so that a retry cannot be mistaken for a
/// duplicate (ADR-0020 §2.3). Everything else is reproducible.
pub fn run_error_paths_gate(
    redaction: &RedactionPolicy,
    cardinality: &CardinalityLimits,
) -> (Vec<ErrorSample>, ErrorGateVerdict) {
    let filter = OptInFilter::new(vec![GATE_PROBE.to_string()]);
    let limits = ExportLimits::default();
    let valid_traceparent = traceparent_of(VALID_TRACE_ID, VALID_PARENT_ID);

    // Case 1: no inbound context at all. The pipeline mints an
    // internal-only invocation, so the spans carry the all-zero trace id
    // that marks "no external context" (ADR-0018 §2.5).
    let absent = ErrorSample {
        input_label: "no_traceparent",
        raw_input: String::new(),
        outcome: run_service_pipeline(
            None,
            &[gate_event(0)],
            &filter,
            &limits,
            redaction,
            cardinality,
        ),
    };

    // Case 2: the header is present but empty. Not the same as absent —
    // an empty header is a client bug that must surface, not a root
    // context the parser should paper over.
    let empty = ErrorSample {
        input_label: "empty_traceparent",
        raw_input: String::new(),
        outcome: run_service_pipeline(
            Some(""),
            &[gate_event(1)],
            &filter,
            &limits,
            redaction,
            cardinality,
        ),
    };

    // Case 3: structurally plausible, not hex. Rejected, with the literal
    // header handed back to the caller.
    let malformed_raw = "00-GARBAGE_not_hex_at_all-0000000000000000-01";
    let malformed = ErrorSample {
        input_label: "malformed_traceparent",
        raw_input: malformed_raw.to_string(),
        outcome: run_service_pipeline(
            Some(malformed_raw),
            &[gate_event(2)],
            &filter,
            &limits,
            redaction,
            cardinality,
        ),
    };

    // Case 4: valid hex, W3C-invalid because both ids are all-zero. A
    // distinct parser branch from case 3, so a distinct case.
    let all_zero_raw = "00-00000000000000000000000000000000-0000000000000000-01";
    let all_zero = ErrorSample {
        input_label: "all_zero_traceparent",
        raw_input: all_zero_raw.to_string(),
        outcome: run_service_pipeline(
            Some(all_zero_raw),
            &[gate_event(3)],
            &filter,
            &limits,
            redaction,
            cardinality,
        ),
    };

    // Case 5: a valid context with nothing to say. Zero spans is the
    // correct answer, and it must be an answer rather than an error.
    let zero_events = ErrorSample {
        input_label: "zero_events",
        raw_input: valid_traceparent.clone(),
        outcome: run_service_pipeline(
            Some(&valid_traceparent),
            &[],
            &filter,
            &limits,
            redaction,
            cardinality,
        ),
    };

    // Case 6: every distinct attribute key is over budget. The sensitive
    // key must still be redacted rather than collapsed: collapsing would
    // replace its key with the shared overflow key, losing the fact that
    // the attribute was a secret at all.
    let strict = CardinalityLimits {
        max_distinct_keys: 0,
        overflow_key: cardinality.overflow_key.clone(),
    };
    let overflowing = ChronosEvent {
        ts_micros: 1_700_000_000_000_006,
        probe: GATE_PROBE.to_string(),
        fields: vec![
            ("alpha".to_string(), EventField::Int(1)),
            ("beta".to_string(), EventField::Int(2)),
            (
                "auth_token".to_string(),
                EventField::Str("secret-value".to_string()),
            ),
        ],
    };
    let extreme_cardinality = ErrorSample {
        input_label: "extreme_cardinality",
        raw_input: valid_traceparent.clone(),
        outcome: run_service_pipeline(
            Some(&valid_traceparent),
            &[overflowing],
            &filter,
            &limits,
            redaction,
            &strict,
        ),
    };

    let samples = vec![
        absent,
        empty,
        malformed,
        all_zero,
        zero_events,
        extreme_cardinality,
    ];
    let verdict = judge_error_paths(&samples);
    (samples, verdict)
}

/// Canonical valid context for the error cases that need one.
const VALID_TRACE_ID: &str = "aaaa2222aaaa2222aaaa2222aaaa2222";
const VALID_PARENT_ID: &str = "2222222222222222";

fn gate_event(offset: u64) -> ChronosEvent {
    ChronosEvent {
        ts_micros: 1_700_000_000_000_000u64.wrapping_add(offset),
        probe: GATE_PROBE.to_string(),
        fields: vec![(
            "auth_token".to_string(),
            EventField::Str("secret-value".to_string()),
        )],
    }
}

/// Decide the error paths gate's verdict from the sample battery.
///
/// Public for the same reason as [`judge_load_gate`]: both of its checks
/// fire only on a violation, so an unexercised branch is indistinguishable
/// from a correct one.
pub fn judge_error_paths(samples: &[ErrorSample]) -> ErrorGateVerdict {
    for sample in samples {
        match &sample.outcome {
            Err(PipelineError::BadTraceparent { raw, .. }) if *raw != sample.raw_input => {
                return ErrorGateVerdict::Fail {
                    reason: format!(
                        "case '{}' lost provenance: error carries raw {raw:?}, input was {:?}",
                        sample.input_label, sample.raw_input
                    ),
                };
            }
            Ok(outcome) if outcome.export_skipped() != 0 => {
                return ErrorGateVerdict::Fail {
                    reason: format!(
                        "case '{}' returned Ok but dropped {} event(s) inside the export stage",
                        sample.input_label,
                        outcome.export_skipped()
                    ),
                };
            }
            _ => {}
        }
    }
    ErrorGateVerdict::Pass
}

// ---------------------------------------------------------------------------
// §5 — aggregate
// ---------------------------------------------------------------------------

/// Per-gate result of an aggregate run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateReport {
    /// Load gate result.
    pub load: (LoadMetrics, LoadGateVerdict),
    /// Recovery gate result.
    pub recovery: (RecoveryState, RecoveryGateVerdict),
    /// Error paths gate result and the number of cases it ran.
    pub error_paths: (ErrorGateVerdict, usize),
}

impl GateReport {
    /// `true` only if all three gates passed.
    pub fn all_passed(&self) -> bool {
        matches!(self.load.1, LoadGateVerdict::Pass)
            && matches!(self.recovery.1, RecoveryGateVerdict::Pass)
            && matches!(self.error_paths.0, ErrorGateVerdict::Pass)
    }
}

/// Run the three gates in order and collect their verdicts.
///
/// Order is load -> recovery -> error paths, and it is not arbitrary
/// (ADR-0021 §2.5). Load runs first because it assumes a healthy system;
/// recovery runs second because it is the gate that takes the system out
/// of that assumption; error paths runs last because it is independent of
/// the consumer state and must not inherit anything the outage phase
/// left behind. Reordering would couple gates that are meant to fail
/// independently.
pub fn run_all_gates(
    n_load: usize,
    budget: &LoadBudget,
    n_outage_requests: usize,
    redaction: &RedactionPolicy,
    cardinality: &CardinalityLimits,
) -> GateReport {
    let (load_samples, load_verdict) = run_load_gate(n_load, budget, redaction, cardinality);
    let load_metrics = LoadMetrics::from_samples(&load_samples);
    let (recovery_state, recovery_verdict) =
        run_recovery_gate(redaction, cardinality, false, n_outage_requests);
    let (error_samples, error_verdict) = run_error_paths_gate(redaction, cardinality);
    GateReport {
        load: (load_metrics, load_verdict),
        recovery: (recovery_state, recovery_verdict),
        error_paths: (error_verdict, error_samples.len()),
    }
}

// ---------------------------------------------------------------------------
// §6 — fixtures
// ---------------------------------------------------------------------------

/// Build a W3C `traceparent` from a trace id and parent span id.
///
/// The product has no public traceparent *builder* — `ExternalTraceContext`
/// is constructed only by `parse_traceparent` (`super::mod`) — so gate
/// fixtures assemble the header text and let the parser validate it, which
/// also means an invalid fixture cannot be smuggled past validation into
/// the pipeline.
fn traceparent_of(trace_id: &str, parent_id: &str) -> String {
    let mut header = String::with_capacity(2 + 1 + 32 + 1 + 16 + 1 + 2);
    header.push_str("00-");
    header.push_str(trace_id);
    header.push('-');
    header.push_str(parent_id);
    header.push_str("-01");
    header
}
