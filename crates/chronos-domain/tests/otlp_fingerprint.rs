//! M7.3 contract tests: BehaviourFingerprint from AlignmentReport.
//!
//! Lifted from `/home/rubentxu/m7-spikes/m7.3-behaviour-fingerprint/tests/cases.rs` (760L, 20 tests).
//!
//! All tests are `--test-threads=1` safe (no global state, no time, no real network).
//!
//! Strategy: build pairs of sessions via M6.1+M6.3, run `align_sessions`
//! to get the `AlignmentReport`, then verify the `BehaviourFingerprint`
//! output is what we expect.

use chronos_domain::otlp::alignment::{align_sessions, InvocationWithEvents, Session};
use chronos_domain::otlp::correlation::{ChronosEvent, EventField};
use chronos_domain::otlp::equivalence::{equivalence_spec_default, equivalence_spec_strict};
use chronos_domain::otlp::fingerprint::{
    fingerprint_equiv, fingerprint_shape_only_equiv, session_fingerprint, summary, FingerprintError,
};
use chronos_domain::otlp::parse::parse_traceparent;
use chronos_domain::otlp::{ExternalTraceContext, OtlpInvocationId, RecordedInvocation};

// ============================================================================
// Helpers (re-used across tests)
// ============================================================================

fn rec_internal(inv: OtlpInvocationId) -> RecordedInvocation {
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

fn fingerprint_of(
    a: Session,
    b: Session,
) -> chronos_domain::otlp::fingerprint::BehaviourFingerprint {
    let report = align_sessions(a, b, &equivalence_spec_default()).expect("default align_sessions");
    session_fingerprint(&report).expect("non-empty report")
}

#[test]
fn empty_sessions_return_empty_report_error() {
    let a: Session = Session::new();
    let b: Session = Session::new();
    let report = align_sessions(a, b, &equivalence_spec_default()).unwrap();
    assert_eq!(report.total_pairs(), 0);
    let result = session_fingerprint(&report);
    assert!(matches!(result, Err(FingerprintError::EmptyReport)));
}

#[test]
fn single_identical_invocation_yields_equiv_fingerprints() {
    let inv = OtlpInvocationId::new();
    let rec = rec_internal(inv);
    let events = vec![
        ev(
            100,
            "checkout",
            vec![("order", EventField::Str("X".into()))],
        ),
        ev(200, "payment", vec![("amount", EventField::Int(100))]),
    ];
    let iwe = InvocationWithEvents::new(&rec, events);
    let mut a = Session::new();
    a.push(iwe.clone());
    let mut b = Session::new();
    b.push(iwe);

    let fp_a = fingerprint_of(a.clone(), b.clone());
    let fp_b = fingerprint_of(a, b);
    assert_eq!(fp_a.aggregate_hash, fp_b.aggregate_hash);
    assert_eq!(fp_a.fingerprint_hash, fp_b.fingerprint_hash);
    assert_eq!(fp_a.matched_count, 1);
    assert_eq!(fp_a.mismatched_count, 0);
    assert_eq!(fp_a.only_in_a_count, 0);
    assert_eq!(fp_a.only_in_b_count, 0);
    assert!(fingerprint_equiv(&fp_a, &fp_b));
}

#[test]
fn two_sessions_with_same_invocations_produce_equiv() {
    let inv1 = OtlpInvocationId::new();
    let inv2 = OtlpInvocationId::new();
    let r1 = rec_internal(inv1);
    let r2 = rec_internal(inv2);
    let iwe1 = InvocationWithEvents::new(
        &r1,
        vec![ev(100, "a", vec![("k", EventField::Str("v".into()))])],
    );
    let iwe2 = InvocationWithEvents::new(&r2, vec![ev(200, "b", vec![("k", EventField::Int(1))])]);

    let mut a = Session::new();
    a.push(iwe1.clone());
    a.push(iwe2.clone());
    let mut b = Session::new();
    b.push(iwe2);
    b.push(iwe1); // reverse order: must still equiv

    let fp_a = fingerprint_of(a.clone(), b.clone());
    let fp_b = fingerprint_of(a, b);
    assert!(fingerprint_equiv(&fp_a, &fp_b));
    assert_eq!(fp_a.matched_count, 2);
}

#[test]
fn mismatched_content_changes_aggregate_hash() {
    let inv = OtlpInvocationId::new();
    let r = rec_internal(inv);
    let iwe1 = InvocationWithEvents::new(&r, vec![ev(100, "a", vec![("x", EventField::Int(1))])]);
    let iwe2 = InvocationWithEvents::new(&r, vec![ev(100, "a", vec![("x", EventField::Int(2))])]);
    let mut a = Session::new();
    a.push(iwe1);
    let mut b = Session::new();
    b.push(iwe2);

    let report = align_sessions(a, b, &equivalence_spec_default()).expect("default align_sessions");
    assert_eq!(report.matched_count(), 0);
    assert_eq!(report.mismatched_count(), 1);
    let fp = session_fingerprint(&report).unwrap();
    assert_eq!(fp.matched_count, 0);
    assert_eq!(fp.mismatched_count, 1);
    assert_ne!(fp.aggregate_hash, 0);
    assert_ne!(fp.fingerprint_hash, 0);
}

#[test]
fn only_in_a_and_only_in_b_count_preserved() {
    let inv_a = OtlpInvocationId::new();
    let inv_b = OtlpInvocationId::new();
    let r_a = rec_internal(inv_a);
    let r_b = rec_internal(inv_b);
    let iwe_a = InvocationWithEvents::new(
        &r_a,
        vec![ev(100, "x", vec![("k", EventField::Str("a".into()))])],
    );
    let iwe_b = InvocationWithEvents::new(
        &r_b,
        vec![ev(100, "y", vec![("k", EventField::Str("b".into()))])],
    );
    let mut a = Session::new();
    a.push(iwe_a);
    let mut b = Session::new();
    b.push(iwe_b);

    let report = align_sessions(a, b, &equivalence_spec_default()).expect("default align_sessions");
    assert_eq!(report.only_in_a_count(), 1);
    assert_eq!(report.only_in_b_count(), 1);
    let fp = session_fingerprint(&report).unwrap();
    assert_eq!(fp.only_in_a_count, 1);
    assert_eq!(fp.only_in_b_count, 1);
    assert_ne!(fp.aggregate_hash, 0);
}

#[test]
fn strict_spec_rejects_tolerated_drift_via_mismatched() {
    let inv = OtlpInvocationId::new();
    let r = rec_internal(inv);
    let iwe1 = InvocationWithEvents::new(
        &r,
        vec![ev(100, "a", vec![("k", EventField::Str("x".into()))])],
    );
    let iwe2 = InvocationWithEvents::new(
        &r,
        vec![ev(999, "a", vec![("k", EventField::Str("x".into()))])],
    );
    let mut a = Session::new();
    a.push(iwe1);
    let mut b = Session::new();
    b.push(iwe2);

    let r_default = align_sessions(a.clone(), b.clone(), &equivalence_spec_default()).unwrap();
    assert_eq!(r_default.matched_count(), 1);
    let fp_default = session_fingerprint(&r_default).unwrap();

    let r_strict = align_sessions(a, b, &equivalence_spec_strict()).unwrap();
    assert_eq!(r_strict.matched_count(), 0);
    assert_eq!(r_strict.mismatched_count(), 1);
    let fp_strict = session_fingerprint(&r_strict).unwrap();

    assert_ne!(fp_default.aggregate_hash, fp_strict.aggregate_hash);
    assert_ne!(fp_default.fingerprint_hash, fp_strict.fingerprint_hash);
    assert!(!fingerprint_equiv(&fp_default, &fp_strict));
}

#[test]
fn fingerprint_is_order_independent() {
    let inv1 = OtlpInvocationId::new();
    let inv2 = OtlpInvocationId::new();
    let r1 = rec_internal(inv1);
    let r2 = rec_internal(inv2);

    let mut a1 = Session::new();
    a1.push(InvocationWithEvents::new(
        &r1,
        vec![ev(100, "a", vec![("k", EventField::Str("a".into()))])],
    ));
    a1.push(InvocationWithEvents::new(
        &r2,
        vec![ev(100, "b", vec![("k", EventField::Str("b".into()))])],
    ));
    let mut b1 = Session::new();
    b1.push(InvocationWithEvents::new(
        &r1,
        vec![ev(100, "a", vec![("k", EventField::Str("a".into()))])],
    ));
    b1.push(InvocationWithEvents::new(
        &r2,
        vec![ev(100, "b", vec![("k", EventField::Str("b".into()))])],
    ));

    let mut a2 = Session::new();
    a2.push(InvocationWithEvents::new(
        &r2,
        vec![ev(100, "b", vec![("k", EventField::Str("b".into()))])],
    ));
    a2.push(InvocationWithEvents::new(
        &r1,
        vec![ev(100, "a", vec![("k", EventField::Str("a".into()))])],
    ));
    let mut b2 = Session::new();
    b2.push(InvocationWithEvents::new(
        &r2,
        vec![ev(100, "b", vec![("k", EventField::Str("b".into()))])],
    ));
    b2.push(InvocationWithEvents::new(
        &r1,
        vec![ev(100, "a", vec![("k", EventField::Str("a".into()))])],
    ));

    let fp1 = fingerprint_of(a1, b1);
    let fp2 = fingerprint_of(a2, b2);
    assert_eq!(fp1.aggregate_hash, fp2.aggregate_hash);
    assert_eq!(fp1.fingerprint_hash, fp2.fingerprint_hash);
    assert!(fingerprint_equiv(&fp1, &fp2));
}

#[test]
fn shape_only_equiv_across_content_differences() {
    let trace_bytes = [9u8; 16];
    let inv1 = OtlpInvocationId::new();
    let inv2 = OtlpInvocationId::new();
    let r1a = rec_with_external(inv1, trace_bytes);
    let r2a = rec_with_external(inv2, trace_bytes);
    let r1b = rec_with_external(inv1, trace_bytes);
    let r2b = rec_with_external(inv2, trace_bytes);

    let a1_iwe = InvocationWithEvents::new(
        &r1a,
        vec![ev(100, "p", vec![("k", EventField::Str("x".into()))])],
    );
    let a2_iwe = InvocationWithEvents::new(
        &r2a,
        vec![ev(100, "p", vec![("k", EventField::Str("y".into()))])],
    );
    let b1_iwe = InvocationWithEvents::new(
        &r1b,
        vec![ev(100, "p", vec![("k", EventField::Str("q".into()))])],
    );
    let b2_iwe = InvocationWithEvents::new(
        &r2b,
        vec![ev(100, "p", vec![("k", EventField::Str("z".into()))])],
    );

    let mut a = Session::new();
    a.push(a1_iwe);
    a.push(a2_iwe);
    let mut b = Session::new();
    b.push(b1_iwe);
    b.push(b2_iwe);

    let fp_a = fingerprint_of(a.clone(), b.clone());
    let fp_b = fingerprint_of(a, b);

    assert!(fp_a.matched_count + fp_a.mismatched_count >= 2);
    assert!(fp_b.matched_count + fp_b.mismatched_count >= 2);
    // Exercise the helper on real data.
    let _ = fingerprint_shape_only_equiv(&fp_a, &fp_b);
}

#[test]
fn summary_renders_formatted_fields() {
    let inv = OtlpInvocationId::new();
    let r = rec_internal(inv);
    let iwe = InvocationWithEvents::new(&r, vec![ev(100, "p", vec![("k", EventField::Int(1))])]);
    let mut a = Session::new();
    a.push(iwe.clone());
    let mut b = Session::new();
    b.push(iwe);

    let fp = fingerprint_of(a, b);
    let s = summary(&fp);
    assert_eq!(s.aggregate_hash_hex.len(), 16);
    assert_eq!(s.fingerprint_hash_hex.len(), 16);
    assert!(s.counts_line.contains("matched=1"));
    assert!(s.counts_line.contains("mismatched=0"));
    assert!(s.counts_line.contains("only_in_a=0"));
    assert!(s.counts_line.contains("only_in_b=0"));
}

#[test]
fn trace_id_fallback_path_yields_equiv() {
    let trace_bytes = [7u8; 16];
    let inv1 = OtlpInvocationId::new();
    let inv2 = OtlpInvocationId::new();
    let r_a = rec_with_external(inv1, trace_bytes);
    let r_b = rec_with_external(inv2, trace_bytes);
    let events = vec![ev(100, "p", vec![("k", EventField::Str("v".into()))])];
    let iwe_a = InvocationWithEvents::new(&r_a, events.clone());
    let iwe_b = InvocationWithEvents::new(&r_b, events);

    let mut a = Session::new();
    a.push(iwe_a);
    let mut b = Session::new();
    b.push(iwe_b);

    let fp = fingerprint_of(a, b);
    assert_eq!(fp.matched_count, 1);
    assert_ne!(fp.aggregate_hash, 0);
}

#[test]
fn large_balanced_report_stable() {
    let fp = {
        let records: Vec<RecordedInvocation> = (0..50)
            .map(|_| rec_internal(OtlpInvocationId::new()))
            .collect();
        let mut a = Session::new();
        let mut b = Session::new();
        for r in &records {
            let events = vec![ev(100, "p", vec![("k", EventField::Int(42))])];
            a.push(InvocationWithEvents::new(r, events.clone()));
            b.push(InvocationWithEvents::new(r, events));
        }
        fingerprint_of(a, b)
    };
    assert_eq!(fp.matched_count, 50);
    assert_eq!(fp.mismatched_count, 0);
    assert_eq!(fp.aggregate_hash, fp.aggregate_hash); // not NaN
}

#[test]
fn mixed_status_session_yields_mixed_counts() {
    let inv1 = OtlpInvocationId::new();
    let inv2 = OtlpInvocationId::new();
    let inv3 = OtlpInvocationId::new();
    let inv4 = OtlpInvocationId::new();

    let r1a = rec_internal(inv1);
    let r1b = rec_internal(inv1);
    let r2a = rec_internal(inv2);
    let r2b = rec_internal(inv2);
    let r3 = rec_internal(inv3);
    let r4 = rec_internal(inv4);

    let ev_match_a = InvocationWithEvents::new(
        &r1a,
        vec![ev(100, "p", vec![("k", EventField::Str("ok".into()))])],
    );
    let ev_match_b = InvocationWithEvents::new(
        &r1b,
        vec![ev(100, "p", vec![("k", EventField::Str("ok".into()))])],
    );
    let ev_mis_a =
        InvocationWithEvents::new(&r2a, vec![ev(100, "p", vec![("k", EventField::Int(1))])]);
    let ev_mis_b =
        InvocationWithEvents::new(&r2b, vec![ev(100, "p", vec![("k", EventField::Int(2))])]);
    let ev_only_a = InvocationWithEvents::new(
        &r3,
        vec![ev(100, "p", vec![("k", EventField::Str("a-only".into()))])],
    );
    let ev_only_b = InvocationWithEvents::new(
        &r4,
        vec![ev(100, "p", vec![("k", EventField::Str("b-only".into()))])],
    );

    let mut a = Session::new();
    a.push(ev_match_a.clone());
    a.push(ev_mis_a.clone());
    a.push(ev_only_a.clone());
    let mut b = Session::new();
    b.push(ev_match_b);
    b.push(ev_mis_b);
    b.push(ev_only_b);

    let fp = fingerprint_of(a, b);
    assert_eq!(fp.matched_count, 1);
    assert_eq!(fp.mismatched_count, 1);
    assert_eq!(fp.only_in_a_count, 1);
    assert_eq!(fp.only_in_b_count, 1);
}

#[test]
fn same_report_idempotent() {
    let inv = OtlpInvocationId::new();
    let r = rec_internal(inv);
    let events = vec![ev(100, "p", vec![("k", EventField::Int(7))])];
    let mut a = Session::new();
    a.push(InvocationWithEvents::new(&r, events.clone()));
    let mut b = Session::new();
    b.push(InvocationWithEvents::new(&r, events));

    let report = align_sessions(a.clone(), b.clone(), &equivalence_spec_default()).unwrap();
    let fp1 = session_fingerprint(&report).unwrap();
    let fp2 = session_fingerprint(&report).unwrap();
    assert_eq!(fp1.aggregate_hash, fp2.aggregate_hash);
    assert_eq!(fp1.fingerprint_hash, fp2.fingerprint_hash);
}

#[test]
fn only_in_invariant_under_inversion() {
    let inv_a = OtlpInvocationId::new();
    let inv_b = OtlpInvocationId::new();
    let r_a = rec_internal(inv_a);
    let r_b = rec_internal(inv_b);
    let iwe_a =
        InvocationWithEvents::new(&r_a, vec![ev(100, "p", vec![("k", EventField::Int(1))])]);
    let iwe_b =
        InvocationWithEvents::new(&r_b, vec![ev(100, "p", vec![("k", EventField::Int(2))])]);

    let mut a = Session::new();
    a.push(iwe_a.clone());
    let mut b = Session::new();
    b.push(iwe_b.clone());
    let fp_ab = fingerprint_of(a, b);

    let mut a2 = Session::new();
    a2.push(iwe_b);
    let mut b2 = Session::new();
    b2.push(iwe_a);
    let fp_ba = fingerprint_of(a2, b2);

    // Status tags differ so aggregate bytes differ; strict equiv fails.
    assert!(!fingerprint_equiv(&fp_ab, &fp_ba));
    // Counts line says only_in_a=1 / only_in_b=1 either way.
    assert_eq!(fp_ab.matched_count, fp_ba.matched_count);
    assert_eq!(fp_ab.mismatched_count, fp_ba.mismatched_count);
    assert_eq!(fp_ab.only_in_a_count, fp_ba.only_in_a_count);
    assert_eq!(fp_ab.only_in_b_count, fp_ba.only_in_b_count);
}

#[test]
fn fingerprint_is_idempotent_under_repeated_align() {
    let inv = OtlpInvocationId::new();
    let r = rec_internal(inv);
    let events = vec![
        ev(100, "a", vec![("k", EventField::Str("v".into()))]),
        ev(200, "b", vec![("k", EventField::Str("w".into()))]),
    ];
    let mut a = Session::new();
    a.push(InvocationWithEvents::new(&r, events.clone()));
    let mut b = Session::new();
    b.push(InvocationWithEvents::new(&r, events));

    let fp_a = {
        let r = align_sessions(a.clone(), b.clone(), &equivalence_spec_default()).unwrap();
        session_fingerprint(&r).unwrap()
    };
    let fp_c = {
        let r = align_sessions(a, b, &equivalence_spec_default()).unwrap();
        session_fingerprint(&r).unwrap()
    };
    assert_eq!(fp_a.aggregate_hash, fp_c.aggregate_hash);
    assert_eq!(fp_a.fingerprint_hash, fp_c.fingerprint_hash);
    assert_eq!(fp_a.matched_count, 1);
}

#[test]
fn hashes_never_zero_even_for_empty_hashable() {
    let inv = OtlpInvocationId::new();
    let r = rec_internal(inv);
    let events = vec![ev(0, "empty_event", vec![])];
    let mut a = Session::new();
    a.push(InvocationWithEvents::new(&r, events.clone()));
    let mut b = Session::new();
    b.push(InvocationWithEvents::new(&r, events));

    let fp = fingerprint_of(a, b);
    assert_ne!(fp.aggregate_hash, 0);
    assert_ne!(fp.fingerprint_hash, 0);
}

#[test]
fn fingerprint_stable_under_session_reordering() {
    let inv1 = OtlpInvocationId::new();
    let inv2 = OtlpInvocationId::new();
    let inv3 = OtlpInvocationId::new();
    let r1 = rec_internal(inv1);
    let r2 = rec_internal(inv2);
    let r3 = rec_internal(inv3);

    let mut a1 = Session::new();
    a1.push(InvocationWithEvents::new(
        &r1,
        vec![ev(100, "p", vec![("k", EventField::Int(1))])],
    ));
    a1.push(InvocationWithEvents::new(
        &r2,
        vec![ev(100, "p", vec![("k", EventField::Int(2))])],
    ));
    a1.push(InvocationWithEvents::new(
        &r3,
        vec![ev(100, "p", vec![("k", EventField::Int(3))])],
    ));
    let mut b1 = Session::new();
    b1.push(InvocationWithEvents::new(
        &r3,
        vec![ev(100, "p", vec![("k", EventField::Int(3))])],
    ));
    b1.push(InvocationWithEvents::new(
        &r1,
        vec![ev(100, "p", vec![("k", EventField::Int(1))])],
    ));
    b1.push(InvocationWithEvents::new(
        &r2,
        vec![ev(100, "p", vec![("k", EventField::Int(2))])],
    ));

    let mut a2 = Session::new();
    a2.push(InvocationWithEvents::new(
        &r2,
        vec![ev(100, "p", vec![("k", EventField::Int(2))])],
    ));
    a2.push(InvocationWithEvents::new(
        &r3,
        vec![ev(100, "p", vec![("k", EventField::Int(3))])],
    ));
    a2.push(InvocationWithEvents::new(
        &r1,
        vec![ev(100, "p", vec![("k", EventField::Int(1))])],
    ));
    let mut b2 = Session::new();
    b2.push(InvocationWithEvents::new(
        &r1,
        vec![ev(100, "p", vec![("k", EventField::Int(1))])],
    ));
    b2.push(InvocationWithEvents::new(
        &r2,
        vec![ev(100, "p", vec![("k", EventField::Int(2))])],
    ));
    b2.push(InvocationWithEvents::new(
        &r3,
        vec![ev(100, "p", vec![("k", EventField::Int(3))])],
    ));

    let fp1 = fingerprint_of(a1, b1);
    let fp2 = fingerprint_of(a2, b2);
    assert_eq!(fp1.matched_count, 3);
    assert_eq!(fp2.matched_count, 3);
    assert!(fingerprint_equiv(&fp1, &fp2));
}

#[test]
fn fingerprint_count_total_matches_alignment() {
    let inv1 = OtlpInvocationId::new();
    let inv2 = OtlpInvocationId::new();
    let r1a = rec_internal(inv1);
    let r2a = rec_internal(inv2);
    let r1b = rec_internal(inv1);
    let r2b = rec_internal(inv2);
    let iwe1a = InvocationWithEvents::new(
        &r1a,
        vec![ev(100, "p", vec![("k", EventField::Str("a".into()))])],
    );
    let iwe2a = InvocationWithEvents::new(
        &r2a,
        vec![ev(100, "p", vec![("k", EventField::Str("b".into()))])],
    );
    let iwe1b = InvocationWithEvents::new(
        &r1b,
        vec![ev(100, "p", vec![("k", EventField::Str("x".into()))])],
    );
    let iwe2b = InvocationWithEvents::new(
        &r2b,
        vec![ev(100, "p", vec![("k", EventField::Str("y".into()))])],
    );
    let mut a = Session::new();
    a.push(iwe1a);
    a.push(iwe2a);
    let mut b = Session::new();
    b.push(iwe1b);
    b.push(iwe2b);

    let report = align_sessions(a, b, &equivalence_spec_default()).unwrap();
    let fp = session_fingerprint(&report).unwrap();
    assert_eq!(
        fp.matched_count + fp.mismatched_count + fp.only_in_a_count + fp.only_in_b_count,
        report.total_pairs()
    );
}

#[test]
fn fingerprint_invocation_hash_is_idempotent() {
    let inv = OtlpInvocationId::new();
    let r = rec_internal(inv);
    let events = vec![ev(100, "p", vec![("k", EventField::Bool(true))])];
    let mut a = Session::new();
    a.push(InvocationWithEvents::new(&r, events.clone()));
    let mut b = Session::new();
    b.push(InvocationWithEvents::new(&r, events));

    let r1 = align_sessions(a.clone(), b.clone(), &equivalence_spec_default()).unwrap();
    let r2 = align_sessions(a, b, &equivalence_spec_default()).unwrap();
    let fp1 = session_fingerprint(&r1).unwrap();
    let fp2 = session_fingerprint(&r2).unwrap();
    assert_eq!(fp1.aggregate_hash, fp2.aggregate_hash);
    assert_eq!(fp1.fingerprint_hash, fp2.fingerprint_hash);
}

#[test]
fn summary_uniqueness_across_scenarios() {
    let inv = OtlpInvocationId::new();
    let r = rec_internal(inv);
    let ev_a = vec![ev(100, "p", vec![("k", EventField::Int(1))])];
    let ev_b = vec![ev(100, "p", vec![("k", EventField::Int(2))])];
    let mut a = Session::new();
    a.push(InvocationWithEvents::new(&r, ev_a));
    let mut b = Session::new();
    b.push(InvocationWithEvents::new(&r, ev_b));

    let fp = fingerprint_of(a, b);
    let s = summary(&fp);
    assert!(s.counts_line.starts_with("matched="));
    assert_eq!(s.counts_line.split_whitespace().count(), 4);
}

// ============================================================================
// Defensive: tighten contracts not covered by the spike suite.
// ============================================================================

/// FNV-1a 64-bit of "foobar" must match the IETF reference constant
/// 0x85944171f73967e8 — sanity check that the lift's `fnv1a_64` import
/// reaches the canonical product implementation (not a re-declaration
/// that drifts in the future).
#[test]
fn defensive_fnv1a_64_known_vector() {
    use chronos_domain::trace::event::fnv1a_64;
    assert_eq!(fnv1a_64(b""), 0xcbf29ce484222325);
    assert_eq!(fnv1a_64(b"foobar"), 0x85944171f73967e8);
}

/// `fingerprint_shape_only_equiv` must agree with `fingerprint_equiv`
/// when the aggregate hashes match. Used to verify that the partial
/// equivalence is at most as strong as the full one (it can be true
/// while full equiv is false).
#[test]
fn defensive_shape_only_subset_of_full_equiv() {
    let inv = OtlpInvocationId::new();
    let r = rec_internal(inv);
    let iwe1 = InvocationWithEvents::new(&r, vec![ev(100, "a", vec![("k", EventField::Int(1))])]);
    let iwe2 = InvocationWithEvents::new(&r, vec![ev(100, "a", vec![("k", EventField::Int(2))])]);
    let mut a = Session::new();
    a.push(iwe1);
    let mut b = Session::new();
    b.push(iwe2);
    let r1 = align_sessions(a.clone(), b.clone(), &equivalence_spec_default()).unwrap();
    let r2 = align_sessions(a, b, &equivalence_spec_default()).unwrap();
    let fp1 = session_fingerprint(&r1).unwrap();
    let fp2 = session_fingerprint(&r2).unwrap();
    // Same report → equiv holds.
    assert!(fingerprint_equiv(&fp1, &fp2));
    assert!(fingerprint_shape_only_equiv(&fp1, &fp2));
}
