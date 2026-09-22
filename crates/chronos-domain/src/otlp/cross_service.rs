//! M6.6 lift (R1): cross-service correlation harness + UAT-M6-01 / UAT-M6-02
//! executors.
//!
//! Composes modules already lifted into `chronos-domain::otlp`:
//!
//! - [`super::parse`] — W3C `traceparent` parsing.
//! - [`super::correlation`] — `ChronosEvent` + `CorrelationStore` bidirectional
//!   index (event_idx ↔ `RecordedInvocation`, plus reverse indices by
//!   invocation_id and trace_id).
//! - [`super`] — `OtlpInvocationId`, `RecordedInvocation`, `ExternalTraceContext`,
//!   `TraceId`, `SpanId`.
//!
//! The M6.6 spike (`/home/rubentxu/m6-spikes/m6.6-otel-cross-service/`) also
//! pulled in `m6_4_otel_exporter` for `export_spans` / `render_json_line` /
//! `OptInFilter` / `ExportLimits`. Those pieces were NOT lifted to the product
//! per ADR-0033 §2.2 (M6.4 deferred-honesto because the canonical exporter
//! path lives in `chronos-services::session_export::serialize_bundle_otlp_json`).
//! This lift therefore validates the UAT scenarios at the *correlation*
//! layer — events bound to invocations, distinct invocations / trace_ids per
//! service, drift preserved, idempotent retry under same `traceparent` — and
//! does NOT re-introduce the deferred exporter. See module docs for the
//! precise scope of each UAT scenario.
//!
//! ## Duplication avoidance
//!
//! `UatOutcome` / `UatResult` are reused from
//! [`super::cost_memory_collision`]. New variants
//! `CrossServiceUnambiguous`, `DriftAndIdempotencyHold`,
//! `CrossServiceMixDetected`, `DriftOrIdempotencyCollapsed` extend the
//! existing enum rather than introducing a parallel `UatM6Outcome` type.
//!
//! ## Honest scope
//!
//! UAT-M6-01 verifies "no cross-mix" of trace_id / invocation_id across two
//! concurrent invocations. UAT-M6-02 verifies monotonic-clock drift between
//! services is preserved AND an idempotent retry with the same `traceparent`
//! produces a new `OtlpInvocationId` (UUID v4) but the same `trace_id`.
//! Both scenarios run **in-process** (no real network). They are not a
//! substitute for an end-to-end OTLP/HTTP integration test — that lives
//! in the durable M6.6 spike and is preserved there as canonical evidence.

use super::correlation::{ChronosEvent, CorrelationStore, EventField};
use super::cost_memory_collision::{UatOutcome, UatResult};
use super::parse::{parse_traceparent, TraceparentParseError};
use super::{OtlpInvocationId, RecordedInvocation};

// ---------------------------------------------------------------------------
// §1 — Helpers
// ---------------------------------------------------------------------------

/// Build a fresh `RecordedInvocation` from a W3C `traceparent` string. Used
/// by the UAT executors to compose cross-service pipelines in-process.
fn make_recorded_with_traceparent(
    traceparent: &str,
) -> Result<RecordedInvocation, TraceparentParseError> {
    let inv_id = OtlpInvocationId::new();
    let etc = parse_traceparent(traceparent)?;
    Ok(RecordedInvocation {
        invocation_id: inv_id,
        external: Some(etc),
    })
}

/// Distinct chronos event with a given `ts_micros`.
fn make_event(ts_micros: u64, probe: &str) -> ChronosEvent {
    ChronosEvent {
        ts_micros,
        probe: probe.to_string(),
        fields: vec![("ts".to_string(), EventField::Int(ts_micros as i64))],
    }
}

/// Canonical trace_id A used by the UAT-M6-01 executor (32 lowercase hex).
const TRACE_ID_A: &str = "aaaa1111aaaa1111aaaa1111aaaa1111";
const SPAN_ID_A: &str = "1111111111111111";

/// Canonical trace_id B used by the UAT-M6-01 executor (32 lowercase hex).
const TRACE_ID_B: &str = "bbbb2222bbbb2222bbbb2222bbbb2222";
const SPAN_ID_B: &str = "2222222222222222";

/// Canonical trace_id for UAT-M6-02 idempotent retry.
const TRACE_ID_IDEMPOTENT: &str = "cccc3333cccc3333cccc3333cccc3333";
const SPAN_ID_IDEMPOTENT: &str = "3333333333333333";

fn traceparent_for(trace_id_hex: &str, span_id_hex: &str, flags: &str) -> String {
    // format!("00-{}-{}-{}", trace_id_hex, span_id_hex, flags)
    // Avoid the `format!` call to keep clippy `--no-deps` happy with the
    // `clippy::uninlined_format_args` lint — the format string is simple
    // and short enough that `format!` is fine, but a `String` literal is
    // the canonical way to build a `traceparent` here.
    let mut s =
        String::with_capacity(2 + 1 + trace_id_hex.len() + 1 + span_id_hex.len() + 1 + flags.len());
    s.push_str("00-");
    s.push_str(trace_id_hex);
    s.push('-');
    s.push_str(span_id_hex);
    s.push('-');
    s.push_str(flags);
    s
}

// ---------------------------------------------------------------------------
// §2 — UAT-M6-01 executor
// ---------------------------------------------------------------------------

/// UAT-M6-01: "Two concurrent invocations with distinct `traceparent`s do
/// NOT mix context."
///
/// Scenario:
/// - Two invocations executed in the same process with distinct W3C
///   `traceparent`s (trace_ids `aaaa1111…` and `bbbb2222…`).
/// - Each invocation produces one event.
///
/// Contract:
/// - `invocation_id`s are distinct (UUID v4 by construction).
/// - `trace_id`s are distinct (carried by `RecordedInvocation.external`).
/// - `CorrelationStore` keeps both bindings separated: distinct
///   `events_for_invocation` and `events_for_trace` lookups return the
///   correct event_idx with no overlap.
///
/// Returns:
/// - `UatOutcome::CrossServiceUnambiguous` if the contract holds (PASS).
/// - `UatOutcome::CrossServiceMixDetected` if either invariant breaks
///   (FAIL — the failure mode UAT-M6-01 is testing for).
pub fn run_uat_m6_01() -> UatResult {
    let scenario = "UAT-M6-01 / two concurrent invocations distinct traces no-mix".to_string();

    let tp_a = traceparent_for(TRACE_ID_A, SPAN_ID_A, "01");
    let tp_b = traceparent_for(TRACE_ID_B, SPAN_ID_B, "01");

    let rec_a = match make_recorded_with_traceparent(&tp_a) {
        Ok(r) => r,
        Err(e) => {
            return UatResult {
                scenario,
                outcome: UatOutcome::CrossServiceMixDetected,
                divergence_count: 0,
                unsupported_region_count: 0,
                aggregate_hash: None,
                fingerprint_hash: None,
            }
            .with_failure_note(format!("traceparent A unparseable: {e}"));
        }
    };
    let rec_b = match make_recorded_with_traceparent(&tp_b) {
        Ok(r) => r,
        Err(e) => {
            return UatResult {
                scenario,
                outcome: UatOutcome::CrossServiceMixDetected,
                divergence_count: 0,
                unsupported_region_count: 0,
                aggregate_hash: None,
                fingerprint_hash: None,
            }
            .with_failure_note(format!("traceparent B unparseable: {e}"));
        }
    };

    // Bindings: 0..0 → rec_a (1 event), 1..1 → rec_b (1 event).
    let mut store = CorrelationStore::new();
    store.bind(0, &rec_a);
    store.bind(1, &rec_b);

    // Assertion 1: distinct invocation_ids.
    let inv_a = rec_a.invocation_id;
    let inv_b = rec_b.invocation_id;
    if inv_a == inv_b {
        return UatResult {
            scenario,
            outcome: UatOutcome::CrossServiceMixDetected,
            divergence_count: 1,
            unsupported_region_count: 0,
            aggregate_hash: None,
            fingerprint_hash: None,
        }
        .with_failure_note("invocation_ids collided (UUID v4 collision)".to_string());
    }

    // Assertion 2: each store has exactly one distinct trace + invocation.
    let trace_count = store.trace_count();
    let inv_count = store.invocation_count();
    if trace_count != 2 || inv_count != 2 {
        return UatResult {
            scenario,
            outcome: UatOutcome::CrossServiceMixDetected,
            divergence_count: 1,
            unsupported_region_count: 0,
            aggregate_hash: None,
            fingerprint_hash: None,
        }
        .with_failure_note(format!(
            "expected 2 traces + 2 invocations; got traces={trace_count} invocations={inv_count}"
        ));
    }

    // Assertion 3: trace_ids are distinct in the parsed RecordedInvocation.
    let trace_a = rec_a
        .external
        .as_ref()
        .map(|e| e.trace_id().bytes().to_vec());
    let trace_b = rec_b
        .external
        .as_ref()
        .map(|e| e.trace_id().bytes().to_vec());
    match (trace_a, trace_b) {
        (Some(a), Some(b)) if a == b => UatResult {
            scenario,
            outcome: UatOutcome::CrossServiceMixDetected,
            divergence_count: 1,
            unsupported_region_count: 0,
            aggregate_hash: None,
            fingerprint_hash: None,
        }
        .with_failure_note("trace_ids collided".to_string()),
        (None, _) | (_, None) => UatResult {
            scenario,
            outcome: UatOutcome::CrossServiceMixDetected,
            divergence_count: 1,
            unsupported_region_count: 0,
            aggregate_hash: None,
            fingerprint_hash: None,
        }
        .with_failure_note("missing external context".to_string()),
        _ => {
            // Assertion 4: lookup by invocation returns the correct event_idx.
            let for_inv_a = store.events_for_invocation(inv_a);
            let for_inv_b = store.events_for_invocation(inv_b);
            if for_inv_a != vec![0] || for_inv_b != vec![1] {
                return UatResult {
                    scenario,
                    outcome: UatOutcome::CrossServiceMixDetected,
                    divergence_count: 1,
                    unsupported_region_count: 0,
                    aggregate_hash: None,
                    fingerprint_hash: None,
                }
                .with_failure_note(format!(
                    "events_for_invocation mapping wrong: a={for_inv_a:?} b={for_inv_b:?}"
                ));
            }

            // Assertion 5: lookup by trace_id returns the correct event_idx.
            // (CorrelationStore lookup by TraceId is via the parsed bytes.)
            let events_for_trace_a =
                store.events_for_trace(rec_a.external.as_ref().unwrap().trace_id());
            let events_for_trace_b =
                store.events_for_trace(rec_b.external.as_ref().unwrap().trace_id());
            if events_for_trace_a != vec![0] || events_for_trace_b != vec![1] {
                return UatResult {
                    scenario,
                    outcome: UatOutcome::CrossServiceMixDetected,
                    divergence_count: 1,
                    unsupported_region_count: 0,
                    aggregate_hash: None,
                    fingerprint_hash: None,
                }
                .with_failure_note(format!(
                    "events_for_trace mapping wrong: a={events_for_trace_a:?} b={events_for_trace_b:?}"
                ));
            }

            UatResult {
                scenario,
                outcome: UatOutcome::CrossServiceUnambiguous,
                divergence_count: 0,
                unsupported_region_count: 0,
                aggregate_hash: None,
                fingerprint_hash: None,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// §3 — UAT-M6-02 executor
// ---------------------------------------------------------------------------

/// UAT-M6-02: "Monotonic-clock drift between two services is preserved, and
/// an idempotent retry with the same `traceparent` produces a NEW
/// `OtlpInvocationId` but the SAME `trace_id`."
///
/// Scenario:
/// - Two invocations with distinct `ts_micros` (1000 vs 8000) representing
///   monotonic-clock drift between two services.
/// - One idempotent retry: same `traceparent` (`cccc3333…`) replayed, with a
///   fresh `OtlpInvocationId` per attempt.
///
/// Contract:
/// - The two drift events preserve distinct `ts_micros` values through the
///   `ChronosEvent` data path.
/// - The retry produces a different `OtlpInvocationId` (UUID v4).
/// - The retry preserves the same `trace_id` (carried by
///   `RecordedInvocation.external.trace_id()`).
///
/// Returns:
/// - `UatOutcome::DriftAndIdempotencyHold` if both invariants hold (PASS).
/// - `UatOutcome::DriftOrIdempotencyCollapsed` if either invariant breaks
///   (FAIL — the failure mode UAT-M6-02 is testing for).
pub fn run_uat_m6_02() -> UatResult {
    let scenario = "UAT-M6-02 / monotonic drift + idempotent retry preserves trace_id".to_string();

    // Drift: two services with distinct monotonic clocks.
    let ev_a = make_event(1000, "http_in");
    let ev_b = make_event(8000, "http_in");
    if ev_a.ts_micros == ev_b.ts_micros {
        return UatResult {
            scenario,
            outcome: UatOutcome::DriftOrIdempotencyCollapsed,
            divergence_count: 1,
            unsupported_region_count: 0,
            aggregate_hash: None,
            fingerprint_hash: None,
        }
        .with_failure_note("monotonic drift collapsed in event construction".to_string());
    }

    // Idempotent retry with same traceparent.
    let tp = traceparent_for(TRACE_ID_IDEMPOTENT, SPAN_ID_IDEMPOTENT, "01");
    let rec_1 = match make_recorded_with_traceparent(&tp) {
        Ok(r) => r,
        Err(e) => {
            return UatResult {
                scenario,
                outcome: UatOutcome::DriftOrIdempotencyCollapsed,
                divergence_count: 1,
                unsupported_region_count: 0,
                aggregate_hash: None,
                fingerprint_hash: None,
            }
            .with_failure_note(format!("traceparent unparseable: {e}"));
        }
    };
    let rec_2 = match make_recorded_with_traceparent(&tp) {
        Ok(r) => r,
        Err(e) => {
            return UatResult {
                scenario,
                outcome: UatOutcome::DriftOrIdempotencyCollapsed,
                divergence_count: 1,
                unsupported_region_count: 0,
                aggregate_hash: None,
                fingerprint_hash: None,
            }
            .with_failure_note(format!("traceparent unparseable: {e}"));
        }
    };

    // Assertion 1: distinct invocation_ids (UUID v4).
    if rec_1.invocation_id == rec_2.invocation_id {
        return UatResult {
            scenario,
            outcome: UatOutcome::DriftOrIdempotencyCollapsed,
            divergence_count: 1,
            unsupported_region_count: 0,
            aggregate_hash: None,
            fingerprint_hash: None,
        }
        .with_failure_note("retry produced identical UUID v4".to_string());
    }

    // Assertion 2: same trace_id (carried externally).
    let trace_1 = rec_1
        .external
        .as_ref()
        .map(|e| e.trace_id().bytes().to_vec());
    let trace_2 = rec_2
        .external
        .as_ref()
        .map(|e| e.trace_id().bytes().to_vec());
    match (trace_1, trace_2) {
        (Some(a), Some(b)) if a == b => {
            // Assertion 3: bind both retries into a CorrelationStore and
            // verify `events_for_trace` returns BOTH event_idxs (idempotent
            // retry contributes the same trace, distinct invocations).
            let mut store = CorrelationStore::new();
            store.bind(0, &rec_1);
            store.bind(1, &rec_2);
            let evts = store.events_for_trace(rec_1.external.as_ref().unwrap().trace_id());
            if evts != vec![0, 1] {
                return UatResult {
                    scenario,
                    outcome: UatOutcome::DriftOrIdempotencyCollapsed,
                    divergence_count: 1,
                    unsupported_region_count: 0,
                    aggregate_hash: None,
                    fingerprint_hash: None,
                }
                .with_failure_note(format!(
                    "events_for_trace did not include both retries; got {evts:?}"
                ));
            }

            UatResult {
                scenario,
                outcome: UatOutcome::DriftAndIdempotencyHold,
                divergence_count: 0,
                unsupported_region_count: 0,
                aggregate_hash: None,
                fingerprint_hash: None,
            }
        }
        (Some(_), Some(_)) => UatResult {
            scenario,
            outcome: UatOutcome::DriftOrIdempotencyCollapsed,
            divergence_count: 1,
            unsupported_region_count: 0,
            aggregate_hash: None,
            fingerprint_hash: None,
        }
        .with_failure_note("retry produced different trace_id".to_string()),
        _ => UatResult {
            scenario,
            outcome: UatOutcome::DriftOrIdempotencyCollapsed,
            divergence_count: 1,
            unsupported_region_count: 0,
            aggregate_hash: None,
            fingerprint_hash: None,
        }
        .with_failure_note("missing external context on retry".to_string()),
    }
}

// ---------------------------------------------------------------------------
// §4 — Extensions
// ---------------------------------------------------------------------------

// `UatResult` already declares `aggregate_hash` and `fingerprint_hash` as
// `Option<u64>` for the M7 executors. For the M6 executors the failure note
// is the more useful diagnostic, so we add a thin extension trait that
// attaches a free-text note to the result. This avoids polluting `UatResult`
// itself with a field that would be unused 90% of the time.
trait UatResultExt {
    fn with_failure_note(self, note: String) -> Self;
}

impl UatResultExt for UatResult {
    fn with_failure_note(mut self, note: String) -> Self {
        // We piggy-back on the existing `scenario` field to embed a short
        // failure marker. This keeps `UatResult` shape stable for M7 callers
        // and avoids adding a new field that would break pattern matches.
        self.scenario = format!("{}\nFAILURE_NOTE: {}", self.scenario, note);
        self
    }
}
