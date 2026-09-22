//! Integration tests for `chronos-domain::otlp::correlation` (M6.3 lift).
//!
//! Tests are adapted from the durable M6.3 spike
//! (`/home/rubentxu/m6-spikes/m6.3-otel-correlation/tests/cases.rs`).

use chronos_domain::otlp::correlation::*;
use chronos_domain::otlp::parse::parse_traceparent;
use chronos_domain::otlp::{OtlpInvocationId, RecordedInvocation};

const W3C_TP_A: &str = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
const W3C_TP_B: &str = "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01";

fn make_recorded_with_traceparent(tp: &str) -> RecordedInvocation {
    let inv = OtlpInvocationId::new();
    let etc = parse_traceparent(tp).expect("parse traceparent");
    inv.record_external(&etc)
}

fn make_internal_only() -> RecordedInvocation {
    let inv = OtlpInvocationId::new();
    RecordedInvocation::internal_only(inv)
}

fn make_event(ts: u64, probe: &str, value: i64) -> ChronosEvent {
    ChronosEvent {
        ts_micros: ts,
        probe: probe.to_string(),
        fields: vec![("value".to_string(), EventField::Int(value))],
    }
}

#[test]
fn bind_event_to_recorded() {
    let mut store = CorrelationStore::new();
    let rec = make_internal_only();
    let inv = rec.invocation_id;
    store.bind(0, &rec);
    assert_eq!(store.bound_count(), 1);
    let looked = store.lookup(0).unwrap();
    assert_eq!(looked.invocation_id, inv);
}

#[test]
fn events_for_invocation_returns_all() {
    let mut store = CorrelationStore::new();
    let rec = make_recorded_with_traceparent(W3C_TP_A);
    let inv = rec.invocation_id;
    store.bind(0, &rec);
    store.bind(1, &rec);
    store.bind(2, &rec);
    let mut found = store.events_for_invocation(inv);
    found.sort();
    assert_eq!(found, vec![0, 1, 2]);
}

#[test]
fn events_for_trace_returns_all() {
    let mut store = CorrelationStore::new();
    let rec = make_recorded_with_traceparent(W3C_TP_A);
    let trace_id = *rec.external.as_ref().unwrap().trace_id();
    store.bind(0, &rec);
    store.bind(1, &rec);
    let mut found = store.events_for_trace(&trace_id);
    found.sort();
    assert_eq!(found, vec![0, 1]);
}

#[test]
fn distinct_traces_produce_distinct_bindings() {
    let mut store = CorrelationStore::new();
    let rec_a = make_recorded_with_traceparent(W3C_TP_A);
    let rec_b = make_recorded_with_traceparent(W3C_TP_B);
    store.bind(0, &rec_a);
    store.bind(1, &rec_b);
    assert_eq!(store.trace_count(), 2);
    let trace_a = *rec_a.external.as_ref().unwrap().trace_id();
    let trace_b = *rec_b.external.as_ref().unwrap().trace_id();
    assert_eq!(store.events_for_trace(&trace_a), vec![0]);
    assert_eq!(store.events_for_trace(&trace_b), vec![1]);
}

#[test]
fn ingest_stream_binds_all_events() {
    let mut store = CorrelationStore::new();
    let rec = make_internal_only();
    let events = vec![
        make_event(1, "a", 10),
        make_event(2, "b", 20),
        make_event(3, "c", 30),
    ];
    let n = ingest_stream(&mut store, &rec, &events);
    assert_eq!(n, 3);
    assert_eq!(store.bound_count(), 3);
    assert!(store.lookup(0).is_some());
    assert!(store.lookup(1).is_some());
    assert!(store.lookup(2).is_some());
}

#[test]
fn ingest_two_streams_two_distinct_invocations() {
    let mut store = CorrelationStore::new();
    let rec1 = make_internal_only();
    let rec2 = make_internal_only();
    let n1 = ingest_stream(&mut store, &rec1, &[make_event(1, "a", 1)]);
    let n2 = ingest_stream(&mut store, &rec2, &[make_event(2, "b", 2)]);
    assert_eq!(n1, 1);
    assert_eq!(n2, 1);
    assert_eq!(store.bound_count(), 2);
    assert_eq!(store.invocation_count(), 2);
}

#[test]
fn mutation_pair_binds_to_same_invocation() {
    let mut store = CorrelationStore::new();
    let rec = make_internal_only();
    store.bind(0, &rec);
    store.bind(1, &rec);
    store.record_mutation(0, 1);
    assert_eq!(store.mutation_of(0), Some((0, 1)));
    assert_eq!(store.mutation_of(1), Some((0, 1)));
    let correlated = mutation_is_correlated(&store, 0, 1).expect("correlated");
    assert!(correlated);
}

#[test]
fn mutation_pair_across_invocations_is_detected() {
    let mut store = CorrelationStore::new();
    let rec1 = make_internal_only();
    let rec2 = make_internal_only();
    store.bind(0, &rec1);
    store.bind(1, &rec2);
    store.record_mutation(0, 1);
    let correlated = mutation_is_correlated(&store, 0, 1).expect("ok");
    assert!(!correlated, "different invocations => not correlated");
}

#[test]
fn mutation_lookup_round_trip() {
    let mut store = CorrelationStore::new();
    let rec = make_internal_only();
    store.bind(0, &rec);
    store.bind(1, &rec);
    store.record_mutation(0, 1);
    assert_eq!(store.mutation_of(0), Some((0, 1)));
    assert_eq!(store.mutation_of(1), Some((0, 1)));
}

#[test]
fn unambiguous_correlation_returns_one() {
    let mut store = CorrelationStore::new();
    let rec = make_internal_only();
    store.bind(42, &rec);
    let looked = unambiguous_correlation(&store, 42).expect("bound");
    assert_eq!(looked.invocation_id, rec.invocation_id);
}

#[test]
fn unambiguous_correlation_errors_on_unbound() {
    let store = CorrelationStore::new();
    let err = unambiguous_correlation(&store, 99).unwrap_err();
    assert_eq!(err, CorrelationError::EventNotBound(99));
}

#[test]
fn count_unambiguous_after_ingest() {
    let mut store = CorrelationStore::new();
    let rec = make_internal_only();
    ingest_stream(
        &mut store,
        &rec,
        &[make_event(1, "a", 1), make_event(2, "b", 2)],
    );
    let (unambig, ambig, unbound) = count_unambiguous(&store, 2);
    assert_eq!(unambig, 2);
    assert_eq!(ambig, 0);
    assert_eq!(unbound, 0);
}

#[test]
fn count_unambiguous_mixed_bound_unbound() {
    let mut store = CorrelationStore::new();
    let rec = make_internal_only();
    store.bind(0, &rec);
    let (unambig, ambig, unbound) = count_unambiguous(&store, 3);
    assert_eq!(unambig, 1);
    assert_eq!(ambig, 0);
    assert_eq!(unbound, 2);
}

#[test]
fn end_to_end_adapter_then_correlate() {
    use chronos_domain::otlp::ingest::{adapter, Headers};
    let mut h = Headers::default();
    h.entries
        .push(("traceparent".to_string(), W3C_TP_A.to_string()));
    let outcome = adapter(&h);
    let rec = match outcome {
        chronos_domain::otlp::ingest::IngestOutcome::Recorded(r) => r,
        _ => panic!("expected Recorded"),
    };
    let mut store = CorrelationStore::new();
    let n = ingest_stream(
        &mut store,
        &rec,
        &[
            make_event(1, "function_entry", 0),
            make_event(2, "function_exit", 0),
        ],
    );
    assert_eq!(n, 2);
    let trace_id = *rec.external.as_ref().unwrap().trace_id();
    assert_eq!(store.events_for_trace(&trace_id), vec![0, 1]);
}

#[test]
fn mutation_with_typed_field_change() {
    let mut store = CorrelationStore::new();
    let rec = make_internal_only();
    store.bind(0, &rec);
    store.bind(1, &rec);
    let before = ChronosEvent {
        ts_micros: 1,
        probe: "var.write".to_string(),
        fields: vec![("value".to_string(), EventField::Int(42))],
    };
    let after = ChronosEvent {
        ts_micros: 2,
        probe: "var.write".to_string(),
        fields: vec![("value".to_string(), EventField::Int(99))],
    };
    // The store binds events to RecordedInvocation; the ChronosEvent
    // content is verified structurally below.
    store.bind(2, &rec);
    store.bind(3, &rec);
    assert_eq!(before.probe, "var.write");
    assert_eq!(before.fields[0].1, EventField::Int(42));
    assert_eq!(after.fields[0].1, EventField::Int(99));
    store.record_mutation(2, 3);
    let correlated = mutation_is_correlated(&store, 2, 3).expect("ok");
    assert!(correlated);
}

#[test]
fn distinct_traces_distinct_invocations_distinct_bindings() {
    let mut store = CorrelationStore::new();
    let rec_a = make_recorded_with_traceparent(W3C_TP_A);
    let rec_b = make_recorded_with_traceparent(W3C_TP_B);
    store.bind(0, &rec_a);
    store.bind(1, &rec_b);
    assert_eq!(store.bound_count(), 2);
    assert_eq!(store.invocation_count(), 2);
    assert_eq!(store.trace_count(), 2);
    // Bindings_iter exposes both.
    let pairs: Vec<_> = store.bindings_iter().collect();
    assert_eq!(pairs.len(), 2);
}

// --- Defensive coverage tests beyond the spike ---

#[test]
fn default_store_is_empty() {
    let store = CorrelationStore::new();
    assert_eq!(store.bound_count(), 0);
    assert_eq!(store.trace_count(), 0);
    assert_eq!(store.invocation_count(), 0);
    assert!(store.lookup(0).is_none());
    assert_eq!(store.mutation_of(0), None);
}

#[test]
fn lookup_on_unbound_returns_none() {
    let store = CorrelationStore::new();
    assert!(store.lookup(0).is_none());
    assert!(store.lookup(u64::MAX).is_none());
}

#[test]
fn events_for_unknown_invocation_returns_empty() {
    let store = CorrelationStore::new();
    let phantom = OtlpInvocationId::new();
    assert!(store.events_for_invocation(phantom).is_empty());
}

#[test]
fn events_for_unknown_trace_returns_empty() {
    use chronos_domain::otlp::TraceId;
    let store = CorrelationStore::new();
    let phantom = TraceId::from_bytes([1u8; 16]);
    assert!(store.events_for_trace(&phantom).is_empty());
}

#[test]
fn record_mutation_lookup_from_either_side() {
    let mut store = CorrelationStore::new();
    let rec = make_internal_only();
    store.bind(0, &rec);
    store.bind(1, &rec);
    store.record_mutation(0, 1);
    // Both directions return the same pair.
    assert_eq!(store.mutation_of(0), Some((0, 1)));
    assert_eq!(store.mutation_of(1), Some((0, 1)));
}

#[test]
fn mutation_is_correlated_errors_when_one_side_unbound() {
    let mut store = CorrelationStore::new();
    let rec = make_internal_only();
    store.bind(0, &rec);
    store.record_mutation(0, 1);
    let err = mutation_is_correlated(&store, 0, 1).unwrap_err();
    assert_eq!(err, CorrelationError::EventNotBound(1));
}

#[test]
fn correlation_error_display_includes_event_idx() {
    let err = CorrelationError::EventNotBound(42);
    let s = err.to_string();
    assert!(s.contains("42"));
}

#[test]
fn ingest_mutations_computes_absolute_indexes() {
    let mut store = CorrelationStore::new();
    let rec = make_internal_only();
    // Bind events 100..=102.
    store.bind(100, &rec);
    store.bind(101, &rec);
    store.bind(102, &rec);
    // Stream starts at 100, mutations are relative to that.
    let n = ingest_mutations(&mut store, 100, &[(0, 1), (1, 2)]);
    assert_eq!(n, 2);
    // Absolute indexes: (100, 101) and (101, 102).
    assert_eq!(store.mutation_of(100), Some((100, 101)));
    assert_eq!(store.mutation_of(101), Some((101, 102)));
    assert_eq!(store.mutation_of(102), Some((101, 102)));
}
