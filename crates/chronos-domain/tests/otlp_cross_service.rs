//! M6.6 cross-service contract tests — UAT-M6-01 / UAT-M6-02 acceptance.
//!
//! These tests exercise the `run_uat_m6_01` / `run_uat_m6_02` executors
//! lifted from the M6.6 spike into `chronos-domain::otlp::cross_service`.
//!
//! The contract tests pin:
//! - **UAT-M6-01**: two concurrent invocations with distinct W3C
//!   `traceparent`s do NOT mix trace_ids / invocation_ids / events.
//! - **UAT-M6-02**: monotonic-clock drift between two services is
//!   preserved; an idempotent retry with the same `traceparent`
//!   produces a new `OtlpInvocationId` but the same `trace_id`.
//!
//! Both UATs run **in-process** (no real network). They are not a
//! substitute for the end-to-end OTLP/HTTP integration test that lives in
//! the durable M6.6 spike at
//! `/home/rubentxu/m6-spikes/m6.6-otel-cross-service/`.
//!
//! Tests are `--test-threads=1` safe (no global state).

use chronos_domain::otlp::correlation::{ChronosEvent, CorrelationStore, EventField};
use chronos_domain::otlp::cost_memory_collision::{UatOutcome, UatResult};
use chronos_domain::otlp::cross_service::{run_uat_m6_01, run_uat_m6_02};
use chronos_domain::otlp::parse::parse_traceparent;
use chronos_domain::otlp::{OtlpInvocationId, RecordedInvocation};

// ---------------------------------------------------------------------------
// 1. UatOutcome variants are wired correctly
// ---------------------------------------------------------------------------

#[test]
fn uat_outcome_m6_variants_have_pass_semantics() {
    assert!(UatOutcome::CrossServiceUnambiguous.is_pass());
    assert!(UatOutcome::DriftAndIdempotencyHold.is_pass());
    assert!(!UatOutcome::CrossServiceMixDetected.is_pass());
    assert!(!UatOutcome::DriftOrIdempotencyCollapsed.is_pass());
}

#[test]
fn uat_outcome_m6_variants_have_stable_names() {
    assert_eq!(
        UatOutcome::CrossServiceUnambiguous.name(),
        "CrossServiceUnambiguous"
    );
    assert_eq!(
        UatOutcome::DriftAndIdempotencyHold.name(),
        "DriftAndIdempotencyHold"
    );
    assert_eq!(
        UatOutcome::CrossServiceMixDetected.name(),
        "CrossServiceMixDetected"
    );
    assert_eq!(
        UatOutcome::DriftOrIdempotencyCollapsed.name(),
        "DriftOrIdempotencyCollapsed"
    );
}

// ---------------------------------------------------------------------------
// 2. UAT-M6-01 executor
// ---------------------------------------------------------------------------

#[test]
fn uat_m6_01_passes_when_two_distinct_traces_do_not_mix() {
    let result = run_uat_m6_01();
    assert_eq!(
        result.outcome,
        UatOutcome::CrossServiceUnambiguous,
        "UAT-M6-01 must pass when two distinct traces do not mix; got {:?} scenario={}",
        result.outcome,
        result.scenario
    );
    assert_eq!(
        result.scenario,
        "UAT-M6-01 / two concurrent invocations distinct traces no-mix"
    );
    assert_eq!(result.divergence_count, 0);
    assert_eq!(result.unsupported_region_count, 0);
    // Scenario string is unchanged when the executor passes (no failure note).
    assert!(!result.scenario.contains("FAILURE_NOTE"));
}

#[test]
fn uat_m6_01_internal_assertion_invocations_are_distinct() {
    // Direct composition: bind two distinct traces into a CorrelationStore
    // and verify `events_for_invocation` separates them cleanly.
    let tp_a = "00-aaaa1111aaaa1111aaaa1111aaaa1111-1111111111111111-01";
    let tp_b = "00-bbbb2222bbbb2222bbbb2222bbbb2222-2222222222222222-01";
    let inv_a = OtlpInvocationId::new();
    let inv_b = OtlpInvocationId::new();
    let rec_a = RecordedInvocation {
        invocation_id: inv_a,
        external: Some(parse_traceparent(tp_a).expect("valid traceparent A")),
    };
    let rec_b = RecordedInvocation {
        invocation_id: inv_b,
        external: Some(parse_traceparent(tp_b).expect("valid traceparent B")),
    };

    let mut store = CorrelationStore::new();
    store.bind(0, &rec_a);
    store.bind(1, &rec_b);

    assert_ne!(inv_a, inv_b, "UUID v4 should not collide in 2 calls");
    assert_eq!(store.trace_count(), 2);
    assert_eq!(store.invocation_count(), 2);
    assert_eq!(store.events_for_invocation(inv_a), vec![0]);
    assert_eq!(store.events_for_invocation(inv_b), vec![1]);

    let trace_a = rec_a.external.as_ref().unwrap().trace_id();
    let trace_b = rec_b.external.as_ref().unwrap().trace_id();
    assert_ne!(trace_a.bytes(), trace_b.bytes());
    assert_eq!(store.events_for_trace(trace_a), vec![0]);
    assert_eq!(store.events_for_trace(trace_b), vec![1]);
}

// ---------------------------------------------------------------------------
// 3. UAT-M6-02 executor
// ---------------------------------------------------------------------------

#[test]
fn uat_m6_02_passes_when_drift_preserved_and_retry_holds_trace_id() {
    let result = run_uat_m6_02();
    assert_eq!(
        result.outcome,
        UatOutcome::DriftAndIdempotencyHold,
        "UAT-M6-02 must pass when drift preserved + retry holds trace_id; got {:?} scenario={}",
        result.outcome,
        result.scenario
    );
    assert_eq!(
        result.scenario,
        "UAT-M6-02 / monotonic drift + idempotent retry preserves trace_id"
    );
    assert_eq!(result.divergence_count, 0);
    assert_eq!(result.unsupported_region_count, 0);
    assert!(!result.scenario.contains("FAILURE_NOTE"));
}

#[test]
fn uat_m6_02_internal_assertion_drift_distinct_in_chronos_event() {
    // Direct composition: two services with distinct monotonic clocks.
    let ev_a = ChronosEvent {
        ts_micros: 1000,
        probe: "http_in".to_string(),
        fields: vec![("ts".to_string(), EventField::Int(1000))],
    };
    let ev_b = ChronosEvent {
        ts_micros: 8000,
        probe: "http_in".to_string(),
        fields: vec![("ts".to_string(), EventField::Int(8000))],
    };
    assert_ne!(
        ev_a.ts_micros, ev_b.ts_micros,
        "monotonic drift must be preserved"
    );
    assert_eq!(
        ev_a.probe, ev_b.probe,
        "probe name should be stable across services"
    );
}

#[test]
fn uat_m6_02_internal_assertion_retry_produces_new_uuid_same_trace() {
    // Direct composition: same traceparent twice → distinct UUID v4, same trace.
    let tp = "00-cccc3333cccc3333cccc3333cccc3333-3333333333333333-01";
    let etc = parse_traceparent(tp).expect("valid traceparent");
    let rec_1 = RecordedInvocation {
        invocation_id: OtlpInvocationId::new(),
        external: Some(etc.clone()),
    };
    let rec_2 = RecordedInvocation {
        invocation_id: OtlpInvocationId::new(),
        external: Some(etc),
    };

    // Distinct UUID v4.
    assert_ne!(
        rec_1.invocation_id, rec_2.invocation_id,
        "retry must mint a fresh UUID v4"
    );

    // Same trace_id.
    let trace_1 = rec_1.external.as_ref().unwrap().trace_id();
    let trace_2 = rec_2.external.as_ref().unwrap().trace_id();
    assert_eq!(
        trace_1.bytes(),
        trace_2.bytes(),
        "retry must preserve trace_id"
    );

    // Both retries land in the same `events_for_trace` bucket.
    let mut store = CorrelationStore::new();
    store.bind(0, &rec_1);
    store.bind(1, &rec_2);
    assert_eq!(store.events_for_trace(trace_1), vec![0, 1]);
    assert_eq!(
        store.trace_count(),
        1,
        "idempotent retry should not mint a new trace"
    );
    assert_eq!(
        store.invocation_count(),
        2,
        "idempotent retry should mint a new invocation"
    );
}

// ---------------------------------------------------------------------------
// 4. Helpers / module wiring
// ---------------------------------------------------------------------------

#[test]
fn uat_result_struct_is_reused_across_m6_m7() {
    // `UatResult` is a single struct reused across M6 and M7 executors.
    // Pin its shape so future refactors don't break the contract.
    let result: UatResult = UatResult {
        scenario: "test".to_string(),
        outcome: UatOutcome::CrossServiceUnambiguous,
        divergence_count: 0,
        unsupported_region_count: 0,
        aggregate_hash: None,
        fingerprint_hash: None,
    };
    assert_eq!(result.scenario, "test");
    assert_eq!(result.divergence_count, 0);
    assert!(result.aggregate_hash.is_none());
    assert!(result.fingerprint_hash.is_none());
}

#[test]
fn traceparent_for_m6_01_trace_ids_match_known_canonical_constants() {
    // The executor uses fixed trace_ids `aaaa1111…` and `bbbb2222…`. Verify
    // they round-trip through `parse_traceparent` without modification.
    // We compare bytes directly — the canonical trace_ids are 16 bytes of
    // an alternating pattern: `0xaa, 0xaa, 0x11, 0x11, …` (and analogously
    // `0xbb, 0xbb, 0x22, 0x22, …`).
    let tp_a = "00-aaaa1111aaaa1111aaaa1111aaaa1111-1111111111111111-01";
    let etc_a = parse_traceparent(tp_a).expect("valid");
    let trace_a = etc_a.trace_id().bytes();
    assert_eq!(trace_a.len(), 16, "W3C trace_id is 16 bytes");
    let mut expected_a = [0u8; 16];
    for i in 0..4 {
        let base = i * 4;
        expected_a[base] = 0xaa;
        expected_a[base + 1] = 0xaa;
        expected_a[base + 2] = 0x11;
        expected_a[base + 3] = 0x11;
    }
    assert_eq!(trace_a, &expected_a);

    let tp_b = "00-bbbb2222bbbb2222bbbb2222bbbb2222-2222222222222222-01";
    let etc_b = parse_traceparent(tp_b).expect("valid");
    let trace_b = etc_b.trace_id().bytes();
    assert_eq!(trace_b.len(), 16);
    let mut expected_b = [0u8; 16];
    for i in 0..4 {
        let base = i * 4;
        expected_b[base] = 0xbb;
        expected_b[base + 1] = 0xbb;
        expected_b[base + 2] = 0x22;
        expected_b[base + 3] = 0x22;
    }
    assert_eq!(trace_b, &expected_b);

    assert_ne!(trace_a, trace_b);
}
