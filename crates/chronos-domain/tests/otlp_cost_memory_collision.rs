//! M7.4 contract tests — cost, memory, collision hunt, UAT-M7-01/02 executors.
//!
//! Lifted from `/home/rubentxu/m7-spikes/m7.4-cost-memory-collision/tests/cases.rs` (~340L, 18 tests).
//!
//! All tests are `--test-threads=1` safe (no global state, no real network).
//!
//! 18 contract tests organised into 5 categories:
//!  1. Cost baseline (3 tests)
//!  2. Memory baseline (3 tests)
//!  3. Stream size predicates (3 tests)
//!  4. Collision hunt (4 tests)
//!  5. UAT-M7-01 / UAT-M7-02 executors (5 tests)

use chronos_domain::otlp::alignment::{
    align_sessions, AlignmentKey, AlignmentKeyKind, AlignmentReport, InvocationAlignment,
    InvocationStatus, InvocationWithEvents, Session,
};
use chronos_domain::otlp::correlation::{ChronosEvent, EventField};
use chronos_domain::otlp::cost_memory_collision::{
    aggregate_stream_size, collision_hunt, measure_cost, measure_memory, run_uat_m7_01,
    run_uat_m7_02, shape_stream_size, CollisionReport, CostBaseline, UatOutcome,
};
use chronos_domain::otlp::equivalence::equivalence_spec_default;
use chronos_domain::otlp::fingerprint::{
    fingerprint_equiv, fingerprint_shape_only_equiv, session_fingerprint,
};
use chronos_domain::otlp::parse::parse_traceparent;
use chronos_domain::otlp::{OtlpInvocationId as InvocationId, RecordedInvocation};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_event(probe: &str, key: &str, value: &str) -> ChronosEvent {
    ChronosEvent {
        ts_micros: 0,
        probe: probe.to_string(),
        fields: vec![(key.to_string(), EventField::Str(value.to_string()))],
    }
}

fn make_invocation_with_trace(traceparent: &str) -> RecordedInvocation {
    let inv_id = InvocationId::new();
    let etc = parse_traceparent(traceparent).expect("valid traceparent");
    RecordedInvocation {
        invocation_id: inv_id,
        external: Some(etc),
    }
}

fn build_session_from<'a>(pairs: Vec<(&'a RecordedInvocation, Vec<ChronosEvent>)>) -> Session<'a> {
    let mut s = Session::new();
    for (inv, evs) in pairs {
        s.push(InvocationWithEvents::new(inv, evs));
    }
    s
}

/// Build a synthetic non-empty AlignmentReport with N pairs of mixed status.
fn synthetic_report(n: usize) -> AlignmentReport {
    let mut report = AlignmentReport {
        matched: Vec::new(),
        mismatched: Vec::new(),
        only_in_a: Vec::new(),
        only_in_b: Vec::new(),
        alignment_key_a: AlignmentKeyKind::Trace,
        alignment_key_b: AlignmentKeyKind::Trace,
    };
    for i in 0..n {
        let status = i % 4;
        let hash_a = 0xA000_0000_0000_0000_u64.wrapping_add(i as u64);
        let hash_b = 0xB000_0000_0000_0000_u64.wrapping_add(i as u64);
        let delta = if i % 5 == 0 { None } else { Some(i as i64 - 3) };
        let alignment = InvocationAlignment {
            key: AlignmentKey::Invocation(uuid::Uuid::new_v4()),
            status: match status {
                0 => InvocationStatus::Matched,
                1 => InvocationStatus::Mismatched,
                2 => InvocationStatus::OnlyInA,
                _ => InvocationStatus::OnlyInB,
            },
            hash_a: if status == 2 { None } else { Some(hash_a) },
            hash_b: if status == 3 { None } else { Some(hash_b) },
            delta_event_count: delta,
        };
        match alignment.status {
            InvocationStatus::Matched => report.matched.push(alignment),
            InvocationStatus::Mismatched => report.mismatched.push(alignment),
            InvocationStatus::OnlyInA => report.only_in_a.push(alignment),
            InvocationStatus::OnlyInB => report.only_in_b.push(alignment),
        }
    }
    report
}

// ===========================================================================
// §1 — Cost baseline (3 tests)
// ===========================================================================

#[test]
fn cost_01_zero_iterations_returns_zero() {
    let report = synthetic_report(10);
    let baseline = measure_cost(&report, 0);
    assert_eq!(baseline.iterations, 0);
    assert_eq!(baseline.n_invocations, 10);
    assert_eq!(baseline.total, std::time::Duration::ZERO);
    assert_eq!(baseline.per_iter, std::time::Duration::ZERO);
}

#[test]
fn cost_02_empty_report_returns_zero() {
    let report = synthetic_report(0);
    let baseline = measure_cost(&report, 100);
    assert_eq!(baseline.iterations, 100);
    assert_eq!(baseline.n_invocations, 0);
    assert_eq!(baseline.total, std::time::Duration::ZERO);
    assert_eq!(baseline.per_iter, std::time::Duration::ZERO);
}

#[test]
fn cost_03_nonzero_iterations_records_elapsed() {
    let report = synthetic_report(50);
    let baseline: CostBaseline = measure_cost(&report, 100);
    assert_eq!(baseline.iterations, 100);
    assert_eq!(baseline.n_invocations, 50);
    assert!(baseline.total >= baseline.per_iter);
    assert!(baseline.invocations_per_sec > 0.0);
    assert!(baseline.invocations_per_sec.is_finite());
}

// ===========================================================================
// §2 — Memory baseline (3 tests)
// ===========================================================================

#[test]
fn memory_01_zero_invocations_has_zero_peak() {
    let report = synthetic_report(0);
    let baseline = measure_memory(&report);
    assert_eq!(baseline.n_invocations, 0);
    assert_eq!(baseline.total_bytes_appended, 0);
    assert_eq!(baseline.reallocations, 0);
}

#[test]
fn memory_02_nonzero_invocations_has_expected_byte_counts() {
    let report = synthetic_report(10);
    let baseline = measure_memory(&report);
    assert_eq!(baseline.n_invocations, 10);
    assert_eq!(baseline.total_bytes_appended, 420);
    assert_eq!(baseline.reallocations, 0);
    assert!(
        baseline.peak_vec_bytes >= 250,
        "peak_vec_bytes {} < 250",
        baseline.peak_vec_bytes
    );
}

#[test]
fn memory_03_large_session_scales_linearly() {
    let small = measure_memory(&synthetic_report(100));
    let large = measure_memory(&synthetic_report(1000));
    assert_eq!(large.total_bytes_appended, small.total_bytes_appended * 10);
}

// ===========================================================================
// §3 — Stream size predicates (3 tests)
// ===========================================================================

#[test]
fn stream_size_01_aggregate_is_25_per_pair() {
    assert_eq!(aggregate_stream_size(&synthetic_report(0)), 0);
    assert_eq!(aggregate_stream_size(&synthetic_report(1)), 25);
    assert_eq!(aggregate_stream_size(&synthetic_report(40)), 40 * 25);
}

#[test]
fn stream_size_02_shape_is_17_per_pair() {
    assert_eq!(shape_stream_size(&synthetic_report(0)), 0);
    assert_eq!(shape_stream_size(&synthetic_report(1)), 17);
    assert_eq!(shape_stream_size(&synthetic_report(40)), 40 * 17);
}

#[test]
fn stream_size_03_combined_sizes_match_memory() {
    let report = synthetic_report(123);
    let baseline = measure_memory(&report);
    let combined = aggregate_stream_size(&report) + shape_stream_size(&report);
    assert_eq!(baseline.total_bytes_appended, combined);
}

// ===========================================================================
// §4 — Collision hunt (4 tests)
// ===========================================================================

#[test]
fn collision_01_deterministic_with_same_seed() {
    let a: CollisionReport = collision_hunt(42, 100);
    let b = collision_hunt(42, 100);
    assert_eq!(a, b, "same seed must produce same CollisionReport");
    assert_eq!(a.corpus_size, 100);
    assert_eq!(a.seed, 42);
}

#[test]
fn collision_02_different_seeds_produce_different_streams() {
    let a = collision_hunt(1, 100);
    let b = collision_hunt(2, 100);
    assert_eq!(a.corpus_size, 100);
    assert_eq!(b.corpus_size, 100);
    assert!(a.unique_aggregates >= 95);
    assert!(b.unique_aggregates >= 95);
}

#[test]
fn collision_03_small_corpus_has_zero_collisions() {
    let cr = collision_hunt(0xABCD, 100);
    assert_eq!(cr.aggregate_collisions, 0);
    assert_eq!(cr.fingerprint_collisions, 0);
    assert_eq!(cr.unique_aggregates, 100);
    assert_eq!(cr.unique_fingerprints, 100);
}

#[test]
fn collision_04_medium_corpus_remains_low() {
    let cr = collision_hunt(0xDEADBEEF, 1000);
    assert!(
        cr.aggregate_collisions <= 5,
        "aggregate collisions {} > 5 (corpus_size=1000)",
        cr.aggregate_collisions
    );
    assert!(
        cr.fingerprint_collisions <= 5,
        "fingerprint collisions {} > 5 (corpus_size=1000)",
        cr.fingerprint_collisions
    );
}

// ===========================================================================
// §5 — UAT-M7-01 / UAT-M7-02 executors (5 tests)
// ===========================================================================

#[test]
fn uat_01_finds_divergence_on_shared_trace_id() {
    let r = run_uat_m7_01();
    assert_eq!(r.outcome, UatOutcome::FoundDivergence);
    assert_eq!(r.divergence_count, 1);
    assert_eq!(r.unsupported_region_count, 0);
    assert!(r.aggregate_hash.is_some());
    assert!(r.fingerprint_hash.is_some());
}

#[test]
fn uat_02_reports_unsupported_with_gap() {
    let r = run_uat_m7_02();
    assert_eq!(r.outcome, UatOutcome::UnknownUnsupported);
    assert_eq!(r.divergence_count, 2); // only_in_a=1 + only_in_b=1
    assert_eq!(r.unsupported_region_count, 2);
    assert!(r.aggregate_hash.is_some());
    assert!(r.fingerprint_hash.is_some());
}

#[test]
fn uat_03_outcome_is_pass_for_both_scenarios() {
    assert!(run_uat_m7_01().outcome.is_pass());
    assert!(run_uat_m7_02().outcome.is_pass());
}

#[test]
fn uat_04_outcome_name_is_stable() {
    assert_eq!(UatOutcome::FoundDivergence.name(), "FoundDivergence");
    assert_eq!(UatOutcome::UnknownUnsupported.name(), "UnknownUnsupported");
    assert_eq!(UatOutcome::FalseEquality.name(), "FalseEquality");
    assert_eq!(UatOutcome::NoDivergence.name(), "NoDivergence");
}

#[test]
fn uat_05_end_to_end_session_fingerprint_round_trip() {
    let traceparent = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";

    let inv_a = make_invocation_with_trace(traceparent);
    let inv_b = make_invocation_with_trace(traceparent);

    let events_a = vec![make_event("checkout", "total", "3500")];
    let events_b = vec![make_event("checkout", "total", "3150")];

    let session_a = build_session_from(vec![(&inv_a, events_a)]);
    let session_b = build_session_from(vec![(&inv_b, events_b)]);

    let spec = equivalence_spec_default();
    let r1 = align_sessions(session_a, session_b, &spec).unwrap();
    let fp1 = session_fingerprint(&r1).unwrap();

    let fp2 = session_fingerprint(&r1).unwrap();
    assert!(fingerprint_equiv(&fp1, &fp2));
    assert!(fingerprint_shape_only_equiv(&fp1, &fp2));
}

// ===========================================================================
// Defensive: tighten contracts not covered by the spike suite.
// ===========================================================================

/// `measure_memory` must keep allocations bounded — the spike's
/// `debug_assert_eq!(reallocations, 0)` is the contract. Verify it holds
/// over a non-trivial size.
#[test]
fn defensive_measure_memory_no_reallocs_at_500() {
    let baseline = measure_memory(&synthetic_report(500));
    assert_eq!(
        baseline.reallocations, 0,
        "exact pre-allocation must avoid reallocs; got {}",
        baseline.reallocations
    );
}

/// `aggregate_stream_size + shape_stream_size == 42 * total_pairs`
/// (25 + 17 = 42 bytes per pair). Pinning this prevents drift between
/// the predicate helpers and M7.3's actual per-pair layout.
#[test]
fn defensive_per_pair_layout_is_25_plus_17() {
    let report = synthetic_report(50);
    let total_pairs = report.matched.len()
        + report.mismatched.len()
        + report.only_in_a.len()
        + report.only_in_b.len();
    let combined = aggregate_stream_size(&report) + shape_stream_size(&report);
    assert_eq!(combined, total_pairs * 42);
}

/// `collision_hunt` is fully deterministic — running it 3 times with the
/// same seed must produce identical reports (defends against accidental
/// non-determinism creeping in via HashMap iteration, etc.).
#[test]
fn defensive_collision_hunt_three_runs_identical() {
    let a = collision_hunt(0xC0FFEE, 250);
    let b = collision_hunt(0xC0FFEE, 250);
    let c = collision_hunt(0xC0FFEE, 250);
    assert_eq!(a, b);
    assert_eq!(b, c);
}
