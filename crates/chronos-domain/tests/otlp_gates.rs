//! Integration tests for `chronos_domain::otlp::gates` (M6.7 lift).
//!
//! Every test here pins one invariant of one gate, and each invariant is
//! named in the failure message so a red run says which contract broke.
//! The shape follows the existing `otlp_*` test files: product types
//! built from literals, no fixtures shared between gates except the two
//! policy defaults.
//!
//! ## What these tests do not claim
//!
//! The load gate reports latency instead of asserting it (ADR-0021 and
//! the `run_load_gate` docs carry the measurement and the reasoning). No
//! test here compares a percentile against
//! [`LoadBudget::max_p95_micros`]: doing so would reintroduce exactly the
//! check that cannot fail, just one file further away.

use std::collections::BTreeSet;

use uuid::Uuid;

use chronos_domain::otlp::correlation::{ChronosEvent, EventField};
use chronos_domain::otlp::exporter::{ExportLimits, OptInFilter};
use chronos_domain::otlp::gates::*;
use chronos_domain::otlp::parse::TraceparentParseError;
use chronos_domain::otlp::redaction::{CardinalityLimits, RedactionPolicy};

/// All-zero W3C trace id: what an internal-only invocation must emit.
const ZERO_TRACE_ID: &str = "00000000000000000000000000000000";

/// The trace id the recovery gate retries, taken from the gate docs.
const RETRIED_TRACE_ID: &str = "aaaa1111aaaa1111aaaa1111aaaa1111";

fn redaction() -> RedactionPolicy {
    RedactionPolicy::default_secrets()
}

fn cardinality() -> CardinalityLimits {
    CardinalityLimits::default()
}

/// The filter the gates own. Not accepted from a caller on purpose: see
/// `GATE_PROBE` in the module docs.
fn gate_filter() -> OptInFilter {
    OptInFilter::new(vec!["http_in".to_string()])
}

fn event_with_secret(probe: &str) -> ChronosEvent {
    ChronosEvent {
        ts_micros: 1_700_000_000_000_000,
        probe: probe.to_string(),
        fields: vec![
            ("request_index".to_string(), EventField::Int(7)),
            (
                "auth_token".to_string(),
                EventField::Str("super-secret-value".to_string()),
            ),
        ],
    }
}

// ---------------------------------------------------------------------------
// The composed pipeline
// ---------------------------------------------------------------------------

/// The composition order is load-bearing: redaction must act on the span
/// before it becomes text, so a secret that reaches a rendered line means
/// the pipeline ran the stages in the wrong order or skipped redaction.
#[test]
fn pipeline_redacts_before_rendering() {
    let outcome = run_service_pipeline(
        Some("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"),
        &[event_with_secret("http_in")],
        &gate_filter(),
        &ExportLimits::default(),
        &redaction(),
        &cardinality(),
    )
    .expect("valid traceparent and event must succeed");

    assert_eq!(
        outcome.redaction_counters.redacted_fields, 1,
        "the sensitive attribute must be redacted exactly once"
    );
    assert_eq!(outcome.spans.len(), 1, "one event in, one span out");
    assert_eq!(
        outcome.lines.len(),
        1,
        "one span must produce exactly one rendered line"
    );

    let span = &outcome.spans[0];
    let rendered = &outcome.lines[0];
    assert_eq!(
        rendered,
        &chronos_domain::otlp::exporter::render_json_line(span)
    );

    let token = span
        .attributes
        .iter()
        .find(|(key, _)| key == "auth_token")
        .expect("the redacted attribute keeps its key");
    assert_eq!(token.1, "[REDACTED]", "sensitive value replaced by marker");
    assert_eq!(
        outcome.redaction_counters.collapsed_cardinality, 0,
        "4 attributes are well inside the default cardinality budget"
    );
    assert!(
        !rendered.contains("super-secret-value"),
        "the secret must never reach the rendered line, got {rendered}"
    );
    assert!(
        rendered.contains("[REDACTED]"),
        "the marker must be visible in the rendered line, got {rendered}"
    );
}

/// `Ok` does not mean "produced something". A probe outside the opt-in
/// filter is dropped, counted, and the call still succeeds — which is why
/// the gates assert on `export_skipped`, not only on the variant.
#[test]
fn pipeline_reports_export_stage_drops_instead_of_failing() {
    let excluding = OptInFilter::new(vec!["some_other_probe".to_string()]);
    let outcome = run_service_pipeline(
        Some("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"),
        &[event_with_secret("http_in")],
        &excluding,
        &ExportLimits::default(),
        &redaction(),
        &cardinality(),
    )
    .expect("a filtered-out event is not a pipeline failure");

    assert_eq!(
        outcome.export_counters.skipped_probe_filter, 1,
        "the drop must be attributed to the probe-filter counter"
    );
    assert_eq!(
        outcome.export_skipped(),
        1,
        "export_skipped() must aggregate every skip counter"
    );
    assert!(outcome.spans.is_empty(), "nothing was exported");
    assert!(outcome.lines.is_empty(), "so nothing was rendered");
}

// ---------------------------------------------------------------------------
// Load gate
// ---------------------------------------------------------------------------

#[test]
fn load_gate_passes_with_nothing_dropped() {
    let n = 250usize;
    let (samples, verdict) = run_load_gate(n, &LoadBudget::default(), &redaction(), &cardinality());
    assert_eq!(verdict, LoadGateVerdict::Pass);

    let metrics = LoadMetrics::from_samples(&samples);
    assert_eq!(metrics.requests, n as u64);
    assert_eq!(metrics.handled, n as u64, "no request may fail");
    assert_eq!(metrics.dropped, 0, "zero events dropped under load");
    assert_eq!(metrics.spans_emitted, n as u64);
    assert_eq!(
        metrics.export_skipped, 0,
        "no event may be dropped inside a handled request"
    );
    assert_eq!(
        metrics.redacted_fields, n as u64,
        "every emitted span must pass through redaction"
    );
}

/// The gate must not be able to certify nothing. `n = 0` is the case that
/// a purely aggregate check would report as a perfect run.
#[test]
fn load_gate_fails_when_no_request_was_exercised() {
    let (samples, verdict) = run_load_gate(0, &LoadBudget::default(), &redaction(), &cardinality());
    assert!(samples.is_empty());
    match verdict {
        LoadGateVerdict::Fail { reason } => assert!(
            reason.contains("certifies nothing"),
            "the reason must say the run proved nothing, got {reason}"
        ),
        LoadGateVerdict::Pass => panic!("a gate that observed nothing must not pass"),
    }
}

/// The per-sample breakdown must add up to the aggregate it produced. An
/// aggregate that disagrees with its own parts is a bug in the
/// accounting, and a gate has to catch bugs in itself too.
#[test]
fn load_gate_counters_break_down_per_sample() {
    let n = 64usize;
    let (samples, _) = run_load_gate(n, &LoadBudget::default(), &redaction(), &cardinality());
    assert_eq!(samples.len(), n);

    let summed = LoadMetrics {
        requests: 0,
        handled: samples.iter().filter(|s| s.handled).count() as u64,
        dropped: samples.iter().filter(|s| !s.handled).count() as u64,
        spans_emitted: samples.iter().map(|s| s.spans_emitted as u64).sum(),
        export_skipped: samples.iter().map(|s| s.export_skipped as u64).sum(),
        redacted_fields: samples.iter().map(|s| s.redacted_fields as u64).sum(),
        ..LoadMetrics::default()
    };
    let metrics = LoadMetrics::from_samples(&samples);
    assert_eq!(metrics.handled, summed.handled);
    assert_eq!(metrics.dropped, summed.dropped);
    assert_eq!(metrics.spans_emitted, summed.spans_emitted);
    assert_eq!(metrics.export_skipped, summed.export_skipped);
    assert_eq!(metrics.redacted_fields, summed.redacted_fields);

    for (i, sample) in samples.iter().enumerate() {
        assert!(sample.handled, "sample {i} was dropped");
        assert_eq!(
            sample.spans_emitted, 1,
            "sample {i} did not emit exactly one span"
        );
        assert_eq!(sample.export_skipped, 0, "sample {i} lost an event");
        assert_eq!(sample.redacted_fields, 1, "sample {i} skipped redaction");
    }
}

/// Each request must be its own correlation context; two requests sharing
/// a trace id would silently merge under load.
#[test]
fn load_gate_gives_every_request_a_distinct_trace_id() {
    let n = 300usize;
    let (samples, _) = run_load_gate(n, &LoadBudget::default(), &redaction(), &cardinality());
    let distinct: BTreeSet<&str> = samples.iter().map(|s| s.trace_id_hex.as_str()).collect();
    assert_eq!(distinct.len(), n, "each request needs its own trace id");
    assert_eq!(
        LoadMetrics::from_samples(&samples).distinct_trace_ids,
        n as u64
    );
}

/// Latency is a reported measurement, not a verdict input. What is
/// asserted is that the measurement is actually taken and internally
/// consistent — a p95 with no data behind it would be worse than none.
///
/// No comparison against `LoadBudget::max_p95_micros` appears here on
/// purpose. Measured p95 is ~60us against a 50_000us budget; a check
/// there cannot fail and would only look like a guarantee.
#[test]
fn load_gate_fails_when_redaction_does_not_run() {
    // An empty policy matches no key, so nothing is redacted. Every
    // pipeline call still succeeds and still emits its span, which is
    // exactly the shape a gate that only checked "did it return Ok" would
    // read as a clean run. This is the load gate failing on a real
    // pipeline property rather than on a scheduling accident.
    let (samples, verdict) = run_load_gate(
        32,
        &LoadBudget::default(),
        &RedactionPolicy::empty(),
        &cardinality(),
    );
    let metrics = LoadMetrics::from_samples(&samples);
    assert_eq!(metrics.handled, 32, "every request still succeeds");
    assert_eq!(metrics.dropped, 0);
    assert_eq!(
        metrics.spans_emitted, 32,
        "every request still emits its span"
    );
    assert_eq!(metrics.redacted_fields, 0, "nothing was redacted");

    match verdict {
        LoadGateVerdict::Fail { reason } => assert!(
            reason.contains("redaction"),
            "the reason must name the redaction stage, got {reason}"
        ),
        LoadGateVerdict::Pass => {
            panic!("a run whose secrets left the process must not pass the load gate")
        }
    }
}

#[test]
fn load_gate_reports_a_consistent_latency_profile() {
    let (samples, verdict) =
        run_load_gate(200, &LoadBudget::default(), &redaction(), &cardinality());
    assert_eq!(verdict, LoadGateVerdict::Pass);

    let metrics = LoadMetrics::from_samples(&samples);
    assert!(
        metrics.p50_micros > 0,
        "each handled request must be timed, got p50=0"
    );
    assert!(
        metrics.p50_micros <= metrics.p95_micros
            && metrics.p95_micros <= metrics.p99_micros
            && metrics.p99_micros <= metrics.max_micros,
        "percentiles must be ordered: p50={} p95={} p99={} max={}",
        metrics.p50_micros,
        metrics.p95_micros,
        metrics.p99_micros,
        metrics.max_micros
    );
    assert_eq!(LoadBudget::default().max_dropped, 0);
}

// ---------------------------------------------------------------------------
// Recovery gate
// ---------------------------------------------------------------------------

#[test]
fn recovery_gate_passes_and_records_every_outage() {
    let (state, verdict) = run_recovery_gate(&redaction(), &cardinality(), false, 7);
    assert_eq!(verdict, RecoveryGateVerdict::Pass);
    assert_eq!(
        state.outages.len(),
        7,
        "one outage per consumer-down request; a silent outage is the failure this gate exists to catch"
    );
    assert_eq!(state.pipeline_failures, 0);
    assert_eq!(
        state.handled_after_recovery, 2,
        "one delivery before the outage and one after recovery"
    );
    assert!(state.final_outage.consumer_up, "the consumer came back up");
}

/// Idempotency is keyed on `trace_id` (ADR-0020 §2.3, re-confirmed in
/// ADR-0021 §2.3): every retry lands under one trace key, and each
/// attempt gets its own invocation id.
#[test]
fn recovery_gate_idempotency_is_keyed_on_trace_id() {
    let n_outage = 10usize;
    let (state, _) = run_recovery_gate(&redaction(), &cardinality(), false, n_outage);

    assert_eq!(
        state.trace_to_invocations.len(),
        1,
        "11 retries of one traceparent must produce one trace key"
    );
    let invocations = state
        .trace_to_invocations
        .get(RETRIED_TRACE_ID)
        .expect("the retried trace id must be the only key");
    assert_eq!(
        invocations.len(),
        n_outage + 2,
        "2 consumer-up deliveries plus {n_outage} outage attempts each mint an invocation id"
    );
    let distinct: BTreeSet<_> = invocations.iter().collect();
    assert_eq!(
        distinct.len(),
        invocations.len(),
        "a retry must not reuse a chronos_invocation_id"
    );
    assert!(state.is_single_trace());
    assert!(state.invocations_are_distinct());
}

#[test]
fn recovery_gate_outage_at_start_skips_the_opening_delivery() {
    let (state, verdict) = run_recovery_gate(&redaction(), &cardinality(), true, 4);
    assert_eq!(verdict, RecoveryGateVerdict::Pass);
    assert_eq!(
        state.handled_after_recovery, 1,
        "the consumer never delivered before the outage, so only the post-recovery delivery counts"
    );
    assert_eq!(state.outages.len(), 4);
    assert_eq!(state.trace_to_invocations.len(), 1);
}

/// Every outage must carry the target it applies to and the monotonic
/// instant it began at. Ordering is asserted, not strict monotonicity:
/// two outages inside the same microsecond legitimately share an instant.
#[test]
fn recovery_gate_outage_events_carry_target_and_instant() {
    let (state, _) = run_recovery_gate(&redaction(), &cardinality(), false, 5);
    assert!(!state.outages.is_empty());
    for (i, outage) in state.outages.iter().enumerate() {
        assert_eq!(
            outage.target, "consumer",
            "outage {i} attributed to the wrong target"
        );
        assert_eq!(
            outage.trace_id_hex, RETRIED_TRACE_ID,
            "outage {i} must name the trace it was observed on"
        );
        if i > 0 {
            let previous = state.outages[i - 1].since_micros;
            assert!(
                outage.since_micros >= previous,
                "outage {i} instant {} went backwards from {previous}",
                outage.since_micros
            );
        }
    }
}

#[test]
fn outage_state_transitions_carry_both_targets() {
    assert!(OutageState::all_up().consumer_up);
    assert!(OutageState::all_up().service_b_up);

    let down = OutageState::consumer_down();
    assert!(!down.consumer_up);
    assert!(
        down.service_b_up,
        "taking the consumer down must not imply service B is down"
    );

    let all = OutageState::all_down();
    assert!(!all.consumer_up);
    assert!(!all.service_b_up);
}

// ---------------------------------------------------------------------------
// Error paths gate
// ---------------------------------------------------------------------------

#[test]
fn error_paths_gate_passes_on_the_full_battery() {
    let (samples, verdict) = run_error_paths_gate(&redaction(), &cardinality());
    assert_eq!(verdict, ErrorGateVerdict::Pass);
    assert_eq!(
        samples.len(),
        6,
        "one sample per adversarial case the pipeline can meet"
    );
    let labels: Vec<&str> = samples.iter().map(|s| s.input_label).collect();
    for expected in [
        "no_traceparent",
        "empty_traceparent",
        "malformed_traceparent",
        "all_zero_traceparent",
        "zero_events",
        "extreme_cardinality",
    ] {
        assert!(labels.contains(&expected), "missing case {expected}");
    }
}

/// Provenance is checked against the literal inputs this test file owns,
/// not against the gate's own `raw_input` bookkeeping, so the assertion
/// cannot be satisfied by the two being wrong in the same way.
#[test]
fn error_paths_preserve_the_offending_input_verbatim() {
    let (samples, _) = run_error_paths_gate(&redaction(), &cardinality());
    let sample = |label: &str| {
        samples
            .iter()
            .find(|s| s.input_label == label)
            .unwrap_or_else(|| panic!("missing case {label}"))
    };

    let malformed = sample("malformed_traceparent");
    match &malformed.outcome {
        Err(PipelineError::BadTraceparent { raw, error }) => {
            assert_eq!(raw, "00-GARBAGE_not_hex_at_all-0000000000000000-01");
            assert!(
                matches!(error, TraceparentParseError::WrongLength { .. }),
                "the parser's own verdict must be kept alongside the raw input, got {error}"
            );
        }
        other => panic!("malformed traceparent must be rejected, got {other:?}"),
    }

    // An empty header is a client bug, not a root context. It is rejected,
    // and the rejected value is the empty string the client actually sent.
    let empty = sample("empty_traceparent");
    match &empty.outcome {
        Err(PipelineError::BadTraceparent { raw, .. }) => {
            assert_eq!(
                raw, "",
                "provenance must be the literal input, not a placeholder"
            )
        }
        other => panic!("an empty traceparent must be rejected, got {other:?}"),
    }

    // Valid hex, W3C-invalid: a different parser branch, so a different
    // case, and still an error rather than a silent root context.
    let all_zero = sample("all_zero_traceparent");
    match &all_zero.outcome {
        Err(PipelineError::BadTraceparent { raw, error }) => {
            assert_eq!(
                raw,
                "00-00000000000000000000000000000000-0000000000000000-01"
            );
            assert!(
                matches!(error, TraceparentParseError::InvalidAllZero { .. }),
                "expected the all-zero branch, got {error}"
            );
        }
        other => panic!("an all-zero trace id must be rejected, got {other:?}"),
    }
}

#[test]
fn absent_traceparent_yields_an_internal_only_invocation() {
    let (samples, _) = run_error_paths_gate(&redaction(), &cardinality());
    let sample = samples
        .iter()
        .find(|s| s.input_label == "no_traceparent")
        .expect("case present");
    let outcome = sample
        .outcome
        .as_ref()
        .expect("absent header is not an error");
    assert!(
        outcome.recorded.external.is_none(),
        "no inbound context means an internal-only invocation"
    );
    assert_eq!(
        outcome.spans[0].trace_id_hex, ZERO_TRACE_ID,
        "an internal-only invocation emits the all-zero trace id"
    );
    assert_eq!(
        outcome.recorded.to_recorded().split('|').nth(1),
        Some(""),
        "no traceparent is recorded for an internal-only invocation"
    );
}

#[test]
fn zero_events_yields_zero_spans_not_an_error() {
    let (samples, _) = run_error_paths_gate(&redaction(), &cardinality());
    let sample = samples
        .iter()
        .find(|s| s.input_label == "zero_events")
        .expect("case present");
    let outcome = sample.outcome.as_ref().expect("no events is not an error");
    assert!(outcome.spans.is_empty());
    assert!(outcome.lines.is_empty());
    assert_eq!(outcome.export_counters.emitted, 0);
    assert_eq!(
        outcome.export_skipped(),
        0,
        "nothing was dropped: nothing existed"
    );
}

/// With a zero cardinality budget every non-sensitive key collapses to the
/// shared overflow key — but the sensitive one must still be redacted.
/// Collapsing it would replace its key too, losing the fact that the
/// attribute was a secret at all.
#[test]
fn extreme_cardinality_collapses_keys_but_still_redacts() {
    let (samples, _) = run_error_paths_gate(&redaction(), &cardinality());
    let sample = samples
        .iter()
        .find(|s| s.input_label == "extreme_cardinality")
        .expect("case present");
    let outcome = sample
        .outcome
        .as_ref()
        .expect("cardinality is not an error");
    assert_eq!(outcome.spans.len(), 1);

    let span = &outcome.spans[0];
    let keys: Vec<&str> = span.attributes.iter().map(|(k, _)| k.as_str()).collect();
    assert!(
        !keys.contains(&"alpha") && !keys.contains(&"beta"),
        "over-budget keys must collapse, got {keys:?}"
    );
    assert_eq!(
        outcome.redaction_counters.collapsed_cardinality,
        keys.len() - 1,
        "every non-sensitive attribute collapses under a zero budget"
    );

    let token = span
        .attributes
        .iter()
        .find(|(key, _)| key == "auth_token")
        .expect("the sensitive attribute keeps its own key");
    assert_eq!(token.1, "[REDACTED]");
    assert_eq!(outcome.redaction_counters.redacted_fields, 1);
    assert!(
        !outcome.lines[0].contains("secret-value"),
        "a collapsed span must not leak the secret into the rendered line"
    );
    assert!(outcome.lines[0].contains("[cardinality-collapsed]"));
}

// ---------------------------------------------------------------------------
// Aggregate
// ---------------------------------------------------------------------------

#[test]
fn run_all_gates_runs_all_three() {
    let report = run_all_gates(200, &LoadBudget::default(), 6, &redaction(), &cardinality());
    assert!(report.all_passed(), "report: {report:?}");
    assert_eq!(report.load.0.handled, 200);
    assert_eq!(report.recovery.0.outages.len(), 6);
    assert_eq!(
        report.error_paths.1, 6,
        "the aggregate reports how many error cases it ran"
    );
}

/// One gate failing must be visible in the aggregate without disturbing
/// the other two. Here the load gate is the one that fails: it is handed
/// an empty run, which the gate itself refuses to certify.
#[test]
fn run_all_gates_surfaces_a_failing_gate_without_coupling_the_others() {
    let report = run_all_gates(0, &LoadBudget::default(), 3, &redaction(), &cardinality());
    assert!(!report.all_passed());
    assert!(
        matches!(report.load.1, LoadGateVerdict::Fail { .. }),
        "the empty load run must fail"
    );
    assert_eq!(
        report.recovery.1,
        RecoveryGateVerdict::Pass,
        "the recovery gate must be unaffected by the load gate's input"
    );
    assert_eq!(
        report.error_paths.0,
        ErrorGateVerdict::Pass,
        "the error gate must be unaffected by the load gate's input"
    );
}

// ---------------------------------------------------------------------------
// Discriminating the verdict checks
// ---------------------------------------------------------------------------
//
// Every check inside a `judge_*` function fires only when it is violated,
// so a suite that only ever sees a passing run cannot tell a wired check
// from a dead one — and a `Fail` path nothing exercises is decoration with
// a doc-comment. These tests drive each check into its failing branch
// directly, by handing the judge hand-built state.

/// Baseline metrics: one request, handled, emitting its span, redacted,
/// carrying its own trace id. Each case below breaks exactly one field.
fn healthy_metrics() -> LoadMetrics {
    LoadMetrics {
        requests: 1,
        handled: 1,
        dropped: 0,
        distinct_trace_ids: 1,
        spans_emitted: 1,
        export_skipped: 0,
        redacted_fields: 1,
        ..LoadMetrics::default()
    }
}

fn expect_load_fail(metrics: &LoadMetrics, budget: &LoadBudget, expected: &str) {
    match judge_load_gate(metrics, budget) {
        LoadGateVerdict::Fail { reason } => assert!(
            reason.contains(expected),
            "the Fail reason must name the violated invariant ({expected}), got {reason}"
        ),
        LoadGateVerdict::Pass => {
            panic!("judge_load_gate passed metrics that violate '{expected}': {metrics:?}")
        }
    }
}

#[test]
fn load_gate_verdict_fails_on_each_violated_invariant() {
    let budget = LoadBudget::default();
    assert_eq!(
        judge_load_gate(&healthy_metrics(), &budget),
        LoadGateVerdict::Pass,
        "the healthy baseline must pass, or the cases below prove nothing"
    );

    let vacuous = LoadMetrics::default();
    expect_load_fail(&vacuous, &budget, "certifies nothing");

    let mut dropped = healthy_metrics();
    dropped.dropped = 1;
    expect_load_fail(&dropped, &budget, "exceeds budget");

    let mut lost_event = healthy_metrics();
    lost_event.export_skipped = 1;
    expect_load_fail(&lost_event, &budget, "export stage");

    let mut no_span = healthy_metrics();
    no_span.spans_emitted = 0;
    expect_load_fail(&no_span, &budget, "emitted no span");

    let mut merged = healthy_metrics();
    merged.handled = 2;
    merged.spans_emitted = 2;
    merged.redacted_fields = 2;
    merged.distinct_trace_ids = 1;
    expect_load_fail(&merged, &budget, "distinct trace id");

    let mut unredacted = healthy_metrics();
    unredacted.redacted_fields = 0;
    expect_load_fail(&unredacted, &budget, "redaction");
}

/// A tolerated drop must stop being tolerated when the budget says so;
/// otherwise `max_dropped` is not a budget.
#[test]
fn load_gate_budget_can_tolerate_a_drop_it_declares() {
    let metrics = LoadMetrics {
        dropped: 1,
        ..healthy_metrics()
    };
    let strict = LoadBudget::default();
    let lenient = LoadBudget {
        max_dropped: 1,
        ..LoadBudget::default()
    };
    expect_load_fail(&metrics, &strict, "exceeds budget");
    assert_eq!(
        judge_load_gate(&metrics, &lenient),
        LoadGateVerdict::Pass,
        "a drop inside the declared budget must not fail the gate"
    );
}

fn healthy_recovery_state(outage_at_start: bool, n_outage: usize) -> RecoveryState {
    let state = run_recovery_gate(&redaction(), &cardinality(), outage_at_start, n_outage).0;
    assert_eq!(
        judge_recovery_gate(&state, outage_at_start, n_outage),
        RecoveryGateVerdict::Pass,
        "the healthy baseline must pass, or the cases below prove nothing"
    );
    state
}

fn expect_recovery_fail(
    state: &RecoveryState,
    outage_at_start: bool,
    n_outage: usize,
    expected: &str,
) {
    match judge_recovery_gate(state, outage_at_start, n_outage) {
        RecoveryGateVerdict::Fail { reason } => assert!(
            reason.contains(expected),
            "the Fail reason must name the violated invariant ({expected}), got {reason}"
        ),
        RecoveryGateVerdict::Pass => {
            panic!("judge_recovery_gate passed state that violates '{expected}': {state:?}")
        }
    }
}

#[test]
fn recovery_gate_verdict_fails_on_each_violated_invariant() {
    let n_outage = 3usize;
    let base = healthy_recovery_state(false, n_outage);

    let mut split_trace = base.clone();
    split_trace.trace_to_invocations.insert(
        "bbbb2222bbbb2222bbbb2222bbbb2222".to_string(),
        vec![Uuid::new_v4()],
    );
    expect_recovery_fail(&split_trace, false, n_outage, "trace id");

    let mut reused_id = base.clone();
    let first = base.trace_to_invocations[RETRIED_TRACE_ID][0];
    reused_id
        .trace_to_invocations
        .get_mut(RETRIED_TRACE_ID)
        .expect("key present")
        .push(first);
    expect_recovery_fail(&reused_id, false, n_outage, "chronos_invocation_id");

    let mut silent_outage = base.clone();
    silent_outage.outages.pop();
    expect_recovery_fail(&silent_outage, false, n_outage, "outage(s) recorded");

    let mut misattributed = base.clone();
    misattributed.outages[0].target = "service_b";
    expect_recovery_fail(&misattributed, false, n_outage, "target");

    let mut local_failure = base.clone();
    local_failure.pipeline_failures = 1;
    expect_recovery_fail(&local_failure, false, n_outage, "local pipeline call");

    let mut undelivered = base;
    undelivered.handled_after_recovery = 1;
    expect_recovery_fail(&undelivered, false, n_outage, "delivered");
}

/// The delivery count is judged against the phases the run actually had,
/// so a run that started with the consumer already down is held to one
/// delivery, not two.
#[test]
fn recovery_gate_judges_deliveries_against_the_phases_that_ran() {
    let n_outage = 2usize;
    let state = healthy_recovery_state(true, n_outage);
    assert_eq!(state.handled_after_recovery, 1);

    let mut two_when_one_ran = state.clone();
    two_when_one_ran.handled_after_recovery = 2;
    expect_recovery_fail(&two_when_one_ran, true, n_outage, "delivered");

    let mut one_when_two_ran = state;
    one_when_two_ran.handled_after_recovery = 0;
    expect_recovery_fail(&one_when_two_ran, true, n_outage, "delivered");
}

fn expect_error_fail(samples: &[ErrorSample], expected: &str) {
    match judge_error_paths(samples) {
        ErrorGateVerdict::Fail { reason } => assert!(
            reason.contains(expected),
            "the Fail reason must name the violated invariant ({expected}), got {reason}"
        ),
        ErrorGateVerdict::Pass => {
            panic!("judge_error_paths passed samples that violate '{expected}'")
        }
    }
}

#[test]
fn error_paths_verdict_fails_on_each_violated_invariant() {
    let (healthy, _) = run_error_paths_gate(&redaction(), &cardinality());
    assert_eq!(
        judge_error_paths(&healthy),
        ErrorGateVerdict::Pass,
        "the healthy battery must pass, or the cases below prove nothing"
    );

    // An error that arrives without the input it came from cannot be
    // attributed to a request, however cleanly the call returned.
    let mut laundered = healthy.clone();
    laundered[1].outcome = Err(PipelineError::BadTraceparent {
        raw: "00-some-other-request-aaaaaaaa11111111-01".to_string(),
        error: TraceparentParseError::WrongSegmentCount { actual: 4 },
    });
    expect_error_fail(&laundered, "lost provenance");

    // `Ok` that quietly dropped the event is the other failure mode: the
    // call looks healthy and the data is gone.
    let excluding = OptInFilter::new(vec!["some_other_probe".to_string()]);
    let dropped_outcome = run_service_pipeline(
        Some("00-aaaa2222aaaa2222aaaa2222aaaa2222-2222222222222222-01"),
        &[event_with_secret("http_in")],
        &excluding,
        &ExportLimits::default(),
        &redaction(),
        &cardinality(),
    )
    .expect("a filtered-out event is not a pipeline failure");
    let lossy = vec![ErrorSample {
        input_label: "silent_loss",
        raw_input: String::new(),
        outcome: Ok(dropped_outcome),
    }];
    expect_error_fail(&lossy, "returned Ok but dropped");
}
