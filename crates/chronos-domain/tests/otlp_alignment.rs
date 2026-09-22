//! M7.2 contract tests: alignment by invocation/context.
//!
//! Lifted from `/home/rubentxu/m7-spikes/m7.2-invocation-alignment/tests/cases.rs` (525L, 22 tests).
//!
//! All tests are `--test-threads=1` safe (no global state, no time, no real network).

use chronos_domain::otlp::alignment::{
    align_sessions, AlignmentError, AlignmentKey, AlignmentKeyKind, EquivalenceSpec,
    InvocationWithEvents, Session,
};
use chronos_domain::otlp::correlation::{ChronosEvent, EventField};
use chronos_domain::otlp::equivalence::{equivalence_spec_default, hash_invocation_canonical};
use chronos_domain::otlp::parse::parse_traceparent;
use chronos_domain::otlp::{ExternalTraceContext, OtlpInvocationId, RecordedInvocation};

// =========================================================================
// Helpers
// =========================================================================

fn rec_with_inv(inv: OtlpInvocationId) -> RecordedInvocation {
    RecordedInvocation::internal_only(inv)
}

fn rec_with_external(inv: OtlpInvocationId, trace: [u8; 16]) -> RecordedInvocation {
    let trace_hex: String = trace.iter().map(|b| format!("{:02x}", b)).collect();
    let span_hex = "0000000000000001";
    let flags_hex = "01";
    let traceparent = format!("00-{}-{}-{}", trace_hex, span_hex, flags_hex);
    let ext: ExternalTraceContext = parse_traceparent(&traceparent).expect("valid traceparent");
    RecordedInvocation {
        invocation_id: inv,
        external: Some(ext),
    }
}

fn ev(ts: u64, probe: &str, kvs: Vec<(&str, EventField)>) -> ChronosEvent {
    ChronosEvent {
        ts_micros: ts,
        probe: probe.to_string(),
        fields: kvs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
    }
}

#[test]
fn two_empty_sessions_have_no_alignments() {
    let a: Session = Session::new();
    let b: Session = Session::new();
    let report = align_sessions(a, b, &equivalence_spec_default()).unwrap();
    assert_eq!(report.matched_count(), 0);
    assert_eq!(report.mismatched_count(), 0);
    assert_eq!(report.only_in_a_count(), 0);
    assert_eq!(report.only_in_b_count(), 0);
    assert_eq!(report.total_pairs(), 0);
}

#[test]
fn single_invocation_match_invocation_id() {
    let inv1 = OtlpInvocationId::new();
    let rec = rec_with_inv(inv1);
    let events = vec![
        ev(
            100,
            "checkout",
            vec![
                ("order", EventField::Str("ORD-001".into())),
                ("total", EventField::Int(3500)),
            ],
        ),
        ev(200, "payment", vec![("amount", EventField::Int(3500))]),
    ];
    let iwe = InvocationWithEvents::new(&rec, events);

    let mut a = Session::new();
    a.push(iwe.clone());
    let mut b = Session::new();
    b.push(iwe);

    let report = align_sessions(a, b, &equivalence_spec_default()).unwrap();
    assert_eq!(report.matched_count(), 1);
    assert_eq!(report.mismatched_count(), 0);
    assert_eq!(report.only_in_a_count(), 0);
    assert_eq!(report.only_in_b_count(), 0);
    assert_eq!(report.matched[0].hash_a, report.matched[0].hash_b);
    assert_eq!(report.matched[0].delta_event_count, Some(0));
}

#[test]
fn single_invocation_match_via_trace_id() {
    let trace_bytes = [1u8; 16];
    let inv1 = OtlpInvocationId::new();
    let inv2 = OtlpInvocationId::new();
    let rec_a = rec_with_external(inv1, trace_bytes);
    let rec_b = rec_with_external(inv2, trace_bytes);

    let events = vec![ev(100, "checkout", vec![("x", EventField::Int(1))])];
    let iwe_a = InvocationWithEvents::new(&rec_a, events.clone());
    let iwe_b = InvocationWithEvents::new(&rec_b, events);

    let mut a = Session::new();
    a.push(iwe_a);
    let mut b = Session::new();
    b.push(iwe_b);

    let report = align_sessions(a, b, &equivalence_spec_default()).unwrap();
    assert_eq!(report.matched_count(), 1);
    assert_eq!(report.mismatched_count(), 0);
    let key = &report.matched[0].key;
    match key {
        AlignmentKey::Trace(_) => {} // expected
        _ => panic!("expected Trace key, got {:?}", key),
    }
    assert_eq!(report.alignment_key_a, AlignmentKeyKind::Mixed);
    assert_eq!(report.alignment_key_b, AlignmentKeyKind::Mixed);
}

#[test]
fn mismatched_event_content_same_invocation() {
    let inv1 = OtlpInvocationId::new();
    let rec = rec_with_inv(inv1);
    let events_a = vec![ev(100, "checkout", vec![("total", EventField::Int(3500))])];
    let events_b = vec![ev(100, "checkout", vec![("total", EventField::Int(3150))])];
    let iwe_a = InvocationWithEvents::new(&rec, events_a);
    let iwe_b = InvocationWithEvents::new(&rec, events_b);

    let mut a = Session::new();
    a.push(iwe_a);
    let mut b = Session::new();
    b.push(iwe_b);

    let report = align_sessions(a, b, &equivalence_spec_default()).unwrap();
    assert_eq!(report.matched_count(), 0);
    assert_eq!(report.mismatched_count(), 1);
    assert_ne!(report.mismatched[0].hash_a, report.mismatched[0].hash_b);
}

#[test]
fn only_in_a_present() {
    let inv1 = OtlpInvocationId::new();
    let inv2 = OtlpInvocationId::new();
    let r1 = rec_with_inv(inv1);
    let r2 = rec_with_inv(inv2);

    let e = vec![ev(0, "p", vec![("k", EventField::Int(1))])];

    let mut a = Session::new();
    a.push(InvocationWithEvents::new(&r1, e.clone()));
    a.push(InvocationWithEvents::new(&r2, e.clone()));
    let mut b = Session::new();
    b.push(InvocationWithEvents::new(&r1, e));

    let report = align_sessions(a, b, &equivalence_spec_default()).unwrap();
    assert_eq!(report.matched_count(), 1);
    assert_eq!(report.only_in_a_count(), 1);
    assert_eq!(report.only_in_b_count(), 0);
}

#[test]
fn only_in_b_present() {
    let inv1 = OtlpInvocationId::new();
    let inv2 = OtlpInvocationId::new();
    let r1 = rec_with_inv(inv1);
    let r2 = rec_with_inv(inv2);

    let e = vec![ev(0, "p", vec![("k", EventField::Int(1))])];

    let mut a = Session::new();
    a.push(InvocationWithEvents::new(&r1, e.clone()));
    let mut b = Session::new();
    b.push(InvocationWithEvents::new(&r1, e.clone()));
    b.push(InvocationWithEvents::new(&r2, e));

    let report = align_sessions(a, b, &equivalence_spec_default()).unwrap();
    assert_eq!(report.matched_count(), 1);
    assert_eq!(report.only_in_a_count(), 0);
    assert_eq!(report.only_in_b_count(), 1);
}

#[test]
fn timestamp_drift_lenient_keeps_matched() {
    let inv1 = OtlpInvocationId::new();
    let rec = rec_with_inv(inv1);
    let e_a = vec![ev(100, "p", vec![("k", EventField::Int(1))])];
    let e_b = vec![ev(9_999_999_999, "p", vec![("k", EventField::Int(1))])];
    let iwe_a = InvocationWithEvents::new(&rec, e_a);
    let iwe_b = InvocationWithEvents::new(&rec, e_b);

    let mut a = Session::new();
    a.push(iwe_a);
    let mut b = Session::new();
    b.push(iwe_b);

    let report = align_sessions(a, b, &equivalence_spec_default()).unwrap();
    assert_eq!(report.matched_count(), 1);
}

#[test]
fn field_reorder_lenient_keeps_matched() {
    let inv1 = OtlpInvocationId::new();
    let rec = rec_with_inv(inv1);
    let e_a = vec![ev(
        0,
        "p",
        vec![("a", EventField::Int(1)), ("b", EventField::Int(2))],
    )];
    let e_b = vec![ev(
        0,
        "p",
        vec![("b", EventField::Int(2)), ("a", EventField::Int(1))],
    )];
    let iwe_a = InvocationWithEvents::new(&rec, e_a);
    let iwe_b = InvocationWithEvents::new(&rec, e_b);

    let mut a = Session::new();
    a.push(iwe_a);
    let mut b = Session::new();
    b.push(iwe_b);

    let report = align_sessions(a, b, &equivalence_spec_default()).unwrap();
    assert_eq!(report.matched_count(), 1);
}

#[test]
fn strict_spec_detects_timestamp_drift_as_mismatched() {
    let inv1 = OtlpInvocationId::new();
    let rec = rec_with_inv(inv1);
    let e_a = vec![ev(100, "p", vec![("k", EventField::Int(1))])];
    let e_b = vec![ev(9_999_999_999, "p", vec![("k", EventField::Int(1))])];
    let iwe_a = InvocationWithEvents::new(&rec, e_a);
    let iwe_b = InvocationWithEvents::new(&rec, e_b);

    let mut a = Session::new();
    a.push(iwe_a);
    let mut b = Session::new();
    b.push(iwe_b);

    let strict = EquivalenceSpec {
        ignore_timestamps: false,
        ignore_field_order: true,
    };
    let report = align_sessions(a, b, &strict).unwrap();
    assert_eq!(report.matched_count(), 0);
    assert_eq!(report.mismatched_count(), 1);
}

#[test]
fn cross_session_invocation_id_collision_returns_alignment_error() {
    let inv = OtlpInvocationId::new();
    let r1 = rec_with_inv(inv);
    let r2 = rec_with_inv(inv);

    let mut a = Session::new();
    a.push(InvocationWithEvents::new(&r1, vec![]));
    a.push(InvocationWithEvents::new(&r2, vec![]));
    let mut b = Session::new();
    b.push(InvocationWithEvents::new(&r1, vec![]));

    let result = align_sessions(a, b, &equivalence_spec_default());
    match result {
        Err(AlignmentError::AmbiguousInvocationId { occurrences, .. }) => {
            assert_eq!(occurrences, 2);
        }
        other => panic!("expected AmbiguousInvocationId, got {:?}", other),
    }
}

#[test]
fn trace_id_fallback_when_invocation_ids_differ() {
    let trace_bytes = [7u8; 16];
    let inv1 = OtlpInvocationId::new();
    let inv2 = OtlpInvocationId::new();
    let r1 = rec_with_external(inv1, trace_bytes);
    let r2 = rec_with_external(inv2, trace_bytes);
    let events = vec![ev(0, "p", vec![("k", EventField::Int(1))])];
    let mut a = Session::new();
    a.push(InvocationWithEvents::new(&r1, events.clone()));
    let mut b = Session::new();
    b.push(InvocationWithEvents::new(&r2, events));
    let report = align_sessions(a, b, &equivalence_spec_default()).unwrap();
    assert_eq!(report.matched_count(), 1);
    assert_eq!(report.alignment_key_a, AlignmentKeyKind::Mixed);
    assert_eq!(report.alignment_key_b, AlignmentKeyKind::Mixed);
}

#[test]
fn report_counts_invariant_every_invocation_accounted_for() {
    let inv1 = OtlpInvocationId::new();
    let inv2 = OtlpInvocationId::new();
    let inv3 = OtlpInvocationId::new();
    let r1 = rec_with_inv(inv1);
    let r2 = rec_with_inv(inv2);
    let r3 = rec_with_inv(inv3);

    let e = vec![ev(0, "p", vec![("k", EventField::Int(1))])];

    let mut a = Session::new();
    a.push(InvocationWithEvents::new(&r1, e.clone()));
    a.push(InvocationWithEvents::new(&r2, e.clone()));
    a.push(InvocationWithEvents::new(&r3, e.clone()));

    let mut b = Session::new();
    b.push(InvocationWithEvents::new(&r1, e.clone()));
    b.push(InvocationWithEvents::new(&r3, e.clone()));

    let report = align_sessions(a, b, &equivalence_spec_default()).unwrap();
    assert_eq!(report.total_pairs(), 3);
    assert_eq!(
        report.matched_count()
            + report.mismatched_count()
            + report.only_in_a_count()
            + report.only_in_b_count(),
        3
    );
}

#[test]
fn delta_event_count_nonzero_signals_drift() {
    let inv1 = OtlpInvocationId::new();
    let rec = rec_with_inv(inv1);
    let e_a = vec![ev(0, "p", vec![("k", EventField::Int(1))])];
    let e_b = vec![
        ev(0, "p", vec![("k", EventField::Int(1))]),
        ev(1, "p2", vec![("k", EventField::Int(2))]),
        ev(2, "p3", vec![("k", EventField::Int(3))]),
    ];
    let mut a = Session::new();
    a.push(InvocationWithEvents::new(&rec, e_a));
    let mut b = Session::new();
    b.push(InvocationWithEvents::new(&rec, e_b));
    let report = align_sessions(a, b, &equivalence_spec_default()).unwrap();
    assert_eq!(report.mismatched_count(), 1);
    assert_eq!(report.mismatched[0].delta_event_count, Some(2));
}

#[test]
fn delta_event_count_none_for_only_in() {
    let inv1 = OtlpInvocationId::new();
    let inv2 = OtlpInvocationId::new();
    let r1 = rec_with_inv(inv1);
    let r2 = rec_with_inv(inv2);
    let e = vec![ev(0, "p", vec![("k", EventField::Int(1))])];
    let mut a = Session::new();
    a.push(InvocationWithEvents::new(&r1, e.clone()));
    let mut b = Session::new();
    b.push(InvocationWithEvents::new(&r2, e));
    let report = align_sessions(a, b, &equivalence_spec_default()).unwrap();
    assert_eq!(report.only_in_b_count(), 1);
    assert_eq!(report.only_in_b[0].delta_event_count, None);
    assert!(report.only_in_b[0].hash_b.is_some());
    assert!(report.only_in_b[0].hash_a.is_none());
}

#[test]
fn alignment_key_kind_invocation_when_neither_uses_trace() {
    let inv1 = OtlpInvocationId::new();
    let r = rec_with_inv(inv1);
    let e = vec![ev(0, "p", vec![("k", EventField::Int(1))])];
    let mut a = Session::new();
    a.push(InvocationWithEvents::new(&r, e.clone()));
    let mut b = Session::new();
    b.push(InvocationWithEvents::new(&r, e));
    let report = align_sessions(a, b, &equivalence_spec_default()).unwrap();
    assert_eq!(report.alignment_key_a, AlignmentKeyKind::Invocation);
    assert_eq!(report.alignment_key_b, AlignmentKeyKind::Invocation);
}

#[test]
fn alignment_key_kind_trace_when_both_use_trace() {
    let trace_bytes = [9u8; 16];
    let inv1 = OtlpInvocationId::new();
    let inv2 = OtlpInvocationId::new();
    let r1 = rec_with_external(inv1, trace_bytes);
    let r2 = rec_with_external(inv2, trace_bytes);
    let e = vec![ev(0, "p", vec![("k", EventField::Int(1))])];
    let mut a = Session::new();
    a.push(InvocationWithEvents::new(&r1, e.clone()));
    let mut b = Session::new();
    b.push(InvocationWithEvents::new(&r2, e));
    let report = align_sessions(a, b, &equivalence_spec_default()).unwrap();
    assert_eq!(report.alignment_key_a, AlignmentKeyKind::Mixed);
    assert_eq!(report.alignment_key_b, AlignmentKeyKind::Mixed);
}

#[test]
fn alignment_key_kind_mixed_when_one_side_has_no_external() {
    let trace_bytes = [11u8; 16];
    let inv1 = OtlpInvocationId::new();
    let inv2 = OtlpInvocationId::new();
    let r1 = rec_with_inv(inv1); // no external
    let r2 = rec_with_external(inv2, trace_bytes); // has external
    let e = vec![ev(0, "p", vec![("k", EventField::Int(1))])];
    let mut a = Session::new();
    a.push(InvocationWithEvents::new(&r1, e.clone()));
    let mut b = Session::new();
    b.push(InvocationWithEvents::new(&r2, e));
    let report = align_sessions(a, b, &equivalence_spec_default()).unwrap();
    assert_eq!(report.only_in_a_count(), 1);
    assert_eq!(report.only_in_b_count(), 1);
    assert_eq!(report.alignment_key_a, AlignmentKeyKind::Invocation);
    assert_eq!(report.alignment_key_b, AlignmentKeyKind::Mixed);
}

#[test]
fn large_session_alignment_performance_smoke() {
    let recs: Vec<RecordedInvocation> = (0..1000)
        .map(|_| rec_with_inv(OtlpInvocationId::new()))
        .collect();
    let e = vec![ev(0, "p", vec![("k", EventField::Int(1))])];
    let mut a: Session = Session::new();
    let mut b: Session = Session::new();
    for r in &recs {
        a.push(InvocationWithEvents::new(r, e.clone()));
        b.push(InvocationWithEvents::new(r, e.clone()));
    }
    let t0 = std::time::Instant::now();
    let report = align_sessions(a, b, &equivalence_spec_default()).unwrap();
    let elapsed = t0.elapsed();
    assert_eq!(report.matched_count(), 1000);
    assert!(elapsed.as_millis() < 1000, "alignment took {:?}", elapsed);
}

#[test]
fn hash_invocation_canonical_propagates_through_alignment() {
    let inv1 = OtlpInvocationId::new();
    let rec = rec_with_inv(inv1);
    let e = vec![ev(0, "p", vec![("k", EventField::Int(42))])];
    let iwe = InvocationWithEvents::new(&rec, e.clone());
    let mut a = Session::new();
    a.push(iwe.clone());
    let mut b = Session::new();
    b.push(iwe);
    let report = align_sessions(a, b, &equivalence_spec_default()).unwrap();
    let expected = hash_invocation_canonical(&e, &equivalence_spec_default());
    assert_eq!(report.matched[0].hash_a, Some(expected));
    assert_eq!(report.matched[0].hash_b, Some(expected));
}

#[test]
fn summary_line_format_is_stable() {
    let inv1 = OtlpInvocationId::new();
    let r = rec_with_inv(inv1);
    let e = vec![ev(0, "p", vec![("k", EventField::Int(1))])];
    let mut a = Session::new();
    a.push(InvocationWithEvents::new(&r, e.clone()));
    let mut b = Session::new();
    b.push(InvocationWithEvents::new(&r, e));
    let report = align_sessions(a, b, &equivalence_spec_default()).unwrap();
    let line = report.summary_line();
    assert!(line.contains("matched=1"));
    assert!(line.contains("mismatched=0"));
    assert!(line.contains("only_in_a=0"));
    assert!(line.contains("only_in_b=0"));
    assert!(line.contains("key_kind=invocation"));
}

// =========================================================================
// Defensive: tighten contracts not covered by the spike suite.
// =========================================================================

/// `rec_with_external` must produce a non-zero trace_id and a non-zero
/// span_id per W3C §3.2.5 (sanity check that the parser path actually
/// survives the round-trip).
#[test]
fn defensive_rec_with_external_produces_nonzero_trace_id() {
    let trace_bytes = [42u8; 16];
    let inv = OtlpInvocationId::new();
    let rec = rec_with_external(inv, trace_bytes);
    let ext = rec.external.as_ref().expect("external set");
    assert_eq!(ext.trace_id().bytes(), &trace_bytes);
    // span_id is hardcoded to 0x...0001 in the helper.
    assert_eq!(*ext.span_id().bytes(), [0, 0, 0, 0, 0, 0, 0, 1]);
}

/// `AlignmentKey` Display impl must produce a stable, parseable-looking
/// string for both variants.
#[test]
fn defensive_alignment_key_display_stable_format() {
    let inv = OtlpInvocationId::new();
    let k1 = AlignmentKey::Invocation(inv.as_uuid());
    let s1 = format!("{}", k1);
    assert!(s1.starts_with("inv:"), "got {}", s1);

    let k2 = AlignmentKey::Trace(vec![0xde, 0xad, 0xbe, 0xef]);
    let s2 = format!("{}", k2);
    assert_eq!(s2, "tr:deadbeef");
}

/// `has_drift` returns `false` only when matched.len() > 0 and all other
/// categories are empty; empty report means no drift (consistent with
/// `is_clean_equivalent` requiring at least one match).
#[test]
fn defensive_has_drift_semantics() {
    let r = align_sessions(Session::new(), Session::new(), &equivalence_spec_default()).unwrap();
    assert!(!r.has_drift());
    // is_clean_equivalent() requires at least one match, so empty != clean.
    assert!(!r.is_clean_equivalent());
}
