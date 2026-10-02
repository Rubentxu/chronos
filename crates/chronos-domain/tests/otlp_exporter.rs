//! Integration tests for `chronos-domain::otlp::exporter` (M6.4 lift).
//!
//! Adapted from the durable M6.4 spike
//! (`/home/rubentxu/m6-spikes/m6.4-otel-exporter/tests/cases.rs`). The 16
//! spike cases are preserved one-for-one in intent, and the cases the spike
//! structurally could not have are added on top:
//!
//!   - **filtering on a real `EventType` name.** The spike passed ad-hoc
//!     probe literals (`"tool_call"`, `"p1"`). The product has no
//!     `probe` field on `TraceEvent`; its name channel is
//!     [`EventType`]'s canonical snake_case spelling. These tests derive
//!     probe names from real `TraceEvent`s so the filter is exercised
//!     against names the product can actually produce.
//!   - **the real temporal domain.** ADR-0018 §2.3 claims
//!     `start_time_unix_nano = ts_micros * 1000` is Unix-epoch time
//!     "because the stream producer sets the epoch". It does not:
//!     `ChronosEvent::ts_micros` is session-monotonic. Product convention
//!     (commit `32fb5ab0`) emits `start_time_unix_nano = 0` and discloses
//!     the real monotonic value. These tests pin that disclosure.

use chronos_domain::otlp::correlation::{ChronosEvent, CorrelationStore, EventField};
use chronos_domain::otlp::exporter::*;
use chronos_domain::otlp::parse::parse_traceparent;
use chronos_domain::otlp::{OtlpInvocationId, RecordedInvocation};
use chronos_domain::trace::{EventData, EventType, MonotonicNs, SourceLocation, TraceEvent};

const W3C_TP_A: &str = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
const W3C_TP_B: &str = "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01";

/// All-zero W3C trace id, the value an internal-only invocation must emit.
const ZERO_TRACE_ID: &str = "00000000000000000000000000000000";

fn recorded_with_traceparent(tp: &str) -> RecordedInvocation {
    let etc = parse_traceparent(tp).expect("parse traceparent");
    OtlpInvocationId::new().record_external(&etc)
}

fn recorded_internal() -> RecordedInvocation {
    RecordedInvocation::internal_only(OtlpInvocationId::new())
}

fn event(ts_micros: u64, probe: &str, value: i64) -> ChronosEvent {
    ChronosEvent {
        ts_micros,
        probe: probe.to_string(),
        fields: vec![("value".to_string(), EventField::Int(value))],
    }
}

fn event_with_str(ts_micros: u64, probe: &str, value: String) -> ChronosEvent {
    ChronosEvent {
        ts_micros,
        probe: probe.to_string(),
        fields: vec![("data".to_string(), EventField::Str(value))],
    }
}

/// A real `TraceEvent` built from product domain types: a function frame
/// whose payload names a symbol. `payload_name` differs from the event kind
/// on purpose — it is what a naive exporter would mistake for a probe name.
fn real_trace_event(
    event_id: u64,
    timestamp_ns: u64,
    kind: EventType,
    payload: &str,
) -> TraceEvent {
    TraceEvent::new(
        event_id,
        MonotonicNs(timestamp_ns),
        1,
        kind,
        SourceLocation {
            function: Some(payload.to_string()),
            ..Default::default()
        },
        EventData::Function {
            name: payload.to_string(),
            signature: None,
            symbol_id: None,
            invocation_id: None,
            parent_invocation_id: None,
        },
    )
}

/// Project a real `TraceEvent` into the exporter's event view.
fn project(te: &TraceEvent) -> ChronosEvent {
    ChronosEvent {
        ts_micros: te.timestamp_ns.as_u64() / 1000,
        probe: probe_name(te),
        fields: vec![
            (
                "chronos.event_id".to_string(),
                EventField::Int(te.event_id as i64),
            ),
            (
                "chronos.thread_id".to_string(),
                EventField::Int(te.thread_id as i64),
            ),
        ],
    }
}

// ----- OptInFilter -----

#[test]
fn filter_allows_matching_probe_names() {
    let f = OptInFilter::new(vec!["tool_call".to_string(), "session_state".to_string()]);
    assert!(f.allows("tool_call"));
    assert!(f.allows("session_state"));
    assert!(!f.allows("debug_log"));
}

#[test]
fn empty_filter_allows_nothing() {
    let f = OptInFilter::empty();
    assert!(!f.allows("anything"));
    assert_eq!(f, OptInFilter::default(), "Default must also allow nothing");
}

// ----- Basic export -----

#[test]
fn export_emits_one_json_line_per_event() {
    let mut store = CorrelationStore::new();
    let rec = recorded_with_traceparent(W3C_TP_A);
    let events = vec![
        event(100, "tool_call", 1),
        event(200, "tool_call", 2),
        event(300, "session_state", 3),
    ];
    for i in 0..events.len() {
        store.bind(i as u64, &rec);
    }
    let filter = OptInFilter::new(vec!["tool_call".to_string(), "session_state".to_string()]);
    let result = export(&store, &events, &filter, &ExportLimits::default());
    assert_eq!(result.emitted, 3);
    assert_eq!(result.skipped_probe_filter, 0);
    assert_eq!(result.lines.len(), 3);
}

#[test]
fn export_includes_chronos_invocation_id() {
    let mut store = CorrelationStore::new();
    let rec = recorded_internal();
    let events = vec![event(100, "tool_call", 1)];
    store.bind(0, &rec);
    let filter = OptInFilter::new(vec!["tool_call".to_string()]);
    let result = export(&store, &events, &filter, &ExportLimits::default());
    assert_eq!(result.emitted, 1);
    assert!(
        result.lines[0].contains(&rec.invocation_id.to_string()),
        "line must carry the chronos invocation id: {}",
        result.lines[0]
    );
}

#[test]
fn export_includes_trace_id_when_external() {
    let mut store = CorrelationStore::new();
    let rec = recorded_with_traceparent(W3C_TP_A);
    let events = vec![event(100, "tool_call", 1)];
    store.bind(0, &rec);
    let filter = OptInFilter::new(vec!["tool_call".to_string()]);
    let result = export(&store, &events, &filter, &ExportLimits::default());
    assert_eq!(result.emitted, 1);
    assert!(
        result.lines[0].contains("\"trace_id\":\"4bf92f3577b34da6a3ce929d0e0e4736\""),
        "W3C trace id must survive byte-exact, without dashes: {}",
        result.lines[0]
    );
}

#[test]
fn export_internal_only_uses_zero_trace_id() {
    let mut store = CorrelationStore::new();
    let rec = recorded_internal();
    let events = vec![event(100, "tool_call", 1)];
    store.bind(0, &rec);
    let filter = OptInFilter::new(vec!["tool_call".to_string()]);
    let result = export(&store, &events, &filter, &ExportLimits::default());
    assert!(
        result.lines[0].contains(&format!("\"trace_id\":\"{ZERO_TRACE_ID}\"")),
        "internal-only invocation must emit the all-zero trace id: {}",
        result.lines[0]
    );
    assert!(
        result.lines[0].contains(&rec.invocation_id.to_string()),
        "internal-only still stays correlatable via chronos_invocation_id"
    );
}

// ----- Opt-in filter -----

#[test]
fn export_skips_filtered_probes() {
    let mut store = CorrelationStore::new();
    let rec = recorded_internal();
    let events = vec![
        event(100, "tool_call", 1),
        event(200, "debug_log", 2),
        event(300, "tool_call", 3),
    ];
    for (i, _) in events.iter().enumerate() {
        store.bind(i as u64, &rec);
    }
    let filter = OptInFilter::new(vec!["tool_call".to_string()]);
    let result = export(&store, &events, &filter, &ExportLimits::default());
    assert_eq!(result.emitted, 2);
    assert_eq!(result.skipped_probe_filter, 1);
    assert_eq!(result.skipped_limit_per_invocation, 0);
    assert_eq!(result.skipped_limit_total, 0);
}

// ----- Limits: max_events_per_invocation -----

#[test]
fn export_respects_max_events_per_invocation() {
    let mut store = CorrelationStore::new();
    let rec = recorded_internal();
    let events: Vec<ChronosEvent> = (0..10)
        .map(|i| event(i * 100, "tool_call", i as i64))
        .collect();
    for (i, _) in events.iter().enumerate() {
        store.bind(i as u64, &rec);
    }
    let filter = OptInFilter::new(vec!["tool_call".to_string()]);
    let limits = ExportLimits {
        max_events_per_invocation: 3,
        ..ExportLimits::default()
    };
    let result = export(&store, &events, &filter, &limits);
    assert_eq!(result.emitted, 3);
    assert_eq!(result.skipped_limit_per_invocation, 7);
    assert_eq!(result.skipped_limit_total, 0);
    assert_eq!(result.lines.len(), 3);
}

// ----- Limits: max_events_total -----

#[test]
fn export_respects_max_events_total() {
    let mut store = CorrelationStore::new();
    let rec_a = recorded_internal();
    let rec_b = recorded_internal();
    let events: Vec<ChronosEvent> = (0..10).map(|i| event(i * 100, "p1", i as i64)).collect();
    for i in 0..5 {
        store.bind(i, &rec_a);
        store.bind(i + 5, &rec_b);
    }
    let filter = OptInFilter::new(vec!["p1".to_string()]);
    let limits = ExportLimits {
        max_events_total: 6,
        ..ExportLimits::default()
    };
    let result = export(&store, &events, &filter, &limits);
    assert_eq!(result.emitted, 6);
    assert_eq!(result.skipped_limit_total, 4);
    assert_eq!(result.skipped_limit_per_invocation, 0);
}

// ----- Limits: max_field_chars -----

#[test]
fn export_truncates_long_string_fields() {
    let mut store = CorrelationStore::new();
    let rec = recorded_internal();
    let long = "x".repeat(2000);
    let events = vec![event_with_str(100, "tool_call", long)];
    store.bind(0, &rec);
    let filter = OptInFilter::new(vec!["tool_call".to_string()]);
    let limits = ExportLimits {
        max_field_chars: 100,
        ..ExportLimits::default()
    };
    let result = export(&store, &events, &filter, &limits);
    assert_eq!(result.emitted, 1);
    assert_eq!(result.truncated_fields, 1);
    let line = &result.lines[0];
    assert!(
        !line.contains(&"x".repeat(2000)),
        "the full 2000-char value must not survive"
    );
    assert!(
        line.contains("truncated"),
        "truncation must be disclosed: {}",
        line
    );
    assert!(
        line.contains("2000"),
        "the suffix must report the original length: {}",
        line
    );
}

// ----- JSON validity / escaping -----

#[test]
fn export_lines_parse_as_json_objects() {
    let mut store = CorrelationStore::new();
    let rec = recorded_with_traceparent(W3C_TP_A);
    let events = vec![event(100, "tool_call", 1), event(200, "session_state", 2)];
    store.bind(0, &rec);
    store.bind(1, &rec);
    let filter = OptInFilter::new(vec!["tool_call".to_string(), "session_state".to_string()]);
    let result = export(&store, &events, &filter, &ExportLimits::default());
    assert_eq!(result.lines.len(), 2);
    for line in &result.lines {
        let v: serde_json::Value =
            serde_json::from_str(line).unwrap_or_else(|e| panic!("invalid JSON line: {e}: {line}"));
        let obj = v.as_object().expect("line must be a JSON object");
        for key in [
            "trace_id",
            "span_id",
            "name",
            "start_time_unix_nano",
            "chronos_invocation_id",
            "attributes",
        ] {
            assert!(obj.contains_key(key), "missing {key} in {line}");
        }
        assert!(!line.contains('\n'), "JSON Lines must stay single-line");
    }
}

#[test]
fn to_jsonl_concatenates_lines_with_newlines() {
    let mut store = CorrelationStore::new();
    let rec = recorded_internal();
    let events = vec![event(100, "p", 1), event(200, "p", 2)];
    store.bind(0, &rec);
    store.bind(1, &rec);
    let filter = OptInFilter::new(vec!["p".to_string()]);
    let result = export(&store, &events, &filter, &ExportLimits::default());
    let jsonl = to_jsonl(&result);
    assert!(jsonl.ends_with('\n'), "JSONL payload ends with a newline");
    let lines: Vec<&str> = jsonl.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lines.len(), 2);
    for line in &lines {
        serde_json::from_str::<serde_json::Value>(line).expect("each JSONL line parses");
    }
}

#[test]
fn to_jsonl_of_an_empty_export_is_empty() {
    let store = CorrelationStore::new();
    let result = export(
        &store,
        &[],
        &OptInFilter::new(vec!["p".to_string()]),
        &ExportLimits::default(),
    );
    assert_eq!(result.emitted, 0);
    assert_eq!(to_jsonl(&result), "");
}

#[test]
fn export_json_escapes_special_chars() {
    let mut store = CorrelationStore::new();
    let rec = recorded_internal();
    let events = vec![ChronosEvent {
        ts_micros: 100,
        probe: "tool_call".to_string(),
        fields: vec![
            (
                "with_quote".to_string(),
                EventField::Str("hello \"world\"".to_string()),
            ),
            (
                "with_newline".to_string(),
                EventField::Str("line1\nline2".to_string()),
            ),
            (
                "with_backslash".to_string(),
                EventField::Str("path\\to\\file".to_string()),
            ),
            ("with_tab".to_string(), EventField::Str("a\tb".to_string())),
            (
                // Rust has no `\f` escape; form feed is \u{c}.
                "with_cr_bs_ff".to_string(),
                EventField::Str("a\rb\u{8}c\u{c}d".to_string()),
            ),
            (
                "with_control".to_string(),
                EventField::Str("a\u{1}b".to_string()),
            ),
        ],
    }];
    store.bind(0, &rec);
    let filter = OptInFilter::new(vec!["tool_call".to_string()]);
    let result = export(&store, &events, &filter, &ExportLimits::default());
    let line = &result.lines[0];
    assert!(line.contains(r#"\"world\""#), "quote escaped: {line}");
    assert!(line.contains(r#"line1\nline2"#), "newline escaped: {line}");
    assert!(
        line.contains(r#"path\\to\\file"#),
        "backslash escaped: {line}"
    );
    assert!(line.contains(r#"a\tb"#), "tab escaped: {line}");
    assert!(line.contains(r#"a\rb"#), "carriage return escaped: {line}");
    assert!(
        line.contains(r#"c\fd"#),
        "form feed uses the short escape (ADR-0018 §2.4): {line}"
    );
    assert!(
        line.contains(r#"b\bc"#),
        "backspace uses the short escape (ADR-0018 §2.4): {line}"
    );
    assert!(
        line.contains(r#"a\u0001b"#),
        "control char below 0x20 escaped as \\uXXXX: {line}"
    );
    serde_json::from_str::<serde_json::Value>(line).expect("escaped line still parses as JSON");
}

// ----- Multiple invocations -----

#[test]
fn export_groups_events_by_invocation() {
    let mut store = CorrelationStore::new();
    let rec_a = recorded_with_traceparent(W3C_TP_A);
    let rec_b = recorded_with_traceparent(W3C_TP_B);
    let events = vec![
        event(100, "p", 1),
        event(200, "p", 2),
        event(300, "p", 3),
        event(400, "p", 4),
    ];
    store.bind(0, &rec_a);
    store.bind(1, &rec_a);
    store.bind(2, &rec_b);
    store.bind(3, &rec_b);
    let filter = OptInFilter::new(vec!["p".to_string()]);
    let limits = ExportLimits {
        max_events_per_invocation: 1,
        ..ExportLimits::default()
    };
    let result = export(&store, &events, &filter, &limits);
    // Each invocation contributes its first event only.
    assert_eq!(result.emitted, 2);
    assert_eq!(result.skipped_limit_per_invocation, 2);
    let has_a = result
        .lines
        .iter()
        .any(|l| l.contains("4bf92f3577b34da6a3ce929d0e0e4736"));
    let has_b = result
        .lines
        .iter()
        .any(|l| l.contains("0af7651916cd43dd8448eb211c80319c"));
    assert!(has_a && has_b, "both invocations emit one span each");
    // Within a group the emitted event is the lowest event_idx of that group,
    // and the groups themselves are ordered by that same lowest event_idx —
    // so the sequence is now pinned, not merely the set. Pinning it by the
    // literal ids would restate the seed, so this asserts the property:
    // two distinct identities, neither of them a stream position.
    let ids = span_ids_in_order(&result);
    assert_eq!(ids.len(), 2, "one span per invocation: {:?}", result.lines);
    assert_ne!(
        ids[0], ids[1],
        "one span per invocation means one identity each: {:?}",
        result.lines
    );
    assert!(
        ids.iter().all(|id| id != "0000000000000000"),
        "a span id must not be the bare stream position: {:?}",
        result.lines
    );
}

// ----- Empty filter -----

#[test]
fn export_with_empty_filter_emits_nothing() {
    let mut store = CorrelationStore::new();
    let rec = recorded_internal();
    let events = vec![event(100, "p", 1)];
    store.bind(0, &rec);
    let result = export(
        &store,
        &events,
        &OptInFilter::empty(),
        &ExportLimits::default(),
    );
    assert_eq!(result.emitted, 0);
    assert_eq!(result.skipped_probe_filter, 1);
    assert!(result.lines.is_empty());
}

// ----- Span identity -----

/// Two services in one trace must not mint the same span id.
///
/// `run_service_pipeline` builds a fresh `CorrelationStore` per call, so
/// `event_idx` restarts at 0 in every service. When two services share a
/// traceparent, the old `event_idx`-only seed gave both their first event
/// `span_id 0000000000000000` — the same OTel identity for two different
/// spans, and a collector absorbs the second as a duplicate and loses it.
///
/// Discriminating: with the previous seed both sides are byte-identical
/// here, so the assertion below passes trivially.
#[test]
fn two_services_sharing_a_traceparent_do_not_emit_the_same_span_id() {
    let filter = OptInFilter::new(vec!["p".to_string()]);
    let limits = ExportLimits::default();
    let events = [event(100, "p", 1)];

    // Same inbound traceparent, different services, both at event_idx 0.
    let mut service_a = CorrelationStore::new();
    service_a.bind(0, &recorded_with_traceparent(W3C_TP_A));
    let mut service_b = CorrelationStore::new();
    service_b.bind(0, &recorded_with_traceparent(W3C_TP_A));

    let a = export(&service_a, &events, &filter, &limits);
    let b = export(&service_b, &events, &filter, &limits);

    assert_eq!(a.emitted, 1);
    assert_eq!(b.emitted, 1);
    assert_ne!(
        span_ids_in_order(&a),
        span_ids_in_order(&b),
        "the same trace, the same position and the same event produced \
         one identity for two services: {:?} vs {:?}",
        a.lines,
        b.lines
    );
}

/// Control: what ADR-0018 §2.3 was actually protecting survives the
/// change. Re-exporting the same stream under the same trace and the same
/// invocation reproduces the same ids — the reproducibility is scoped, not
/// dropped.
#[test]
fn re_exporting_the_same_stream_reproduces_the_same_span_id() {
    let mut store = CorrelationStore::new();
    let rec = recorded_internal();
    let events = vec![event(100, "p", 1)];
    store.bind(0, &rec);
    let filter = OptInFilter::new(vec!["p".to_string()]);
    let limits = ExportLimits::default();

    let first = export(&store, &events, &filter, &limits);
    let second = export(&store, &events, &filter, &limits);

    assert_eq!(
        span_ids_in_order(&first),
        span_ids_in_order(&second),
        "the same event re-exported under the same trace and invocation \
         must reproduce its span id: {:?} vs {:?}",
        first.lines,
        second.lines
    );
}

#[test]
fn export_span_id_follows_the_stream_position_not_the_payload() {
    let mut store = CorrelationStore::new();
    let rec = recorded_internal();
    let events = vec![event(100, "p", 7), event(200, "p", 7)];
    store.bind(0, &rec);
    store.bind(1, &rec);
    let filter = OptInFilter::new(vec!["p".to_string()]);
    let result = export(&store, &events, &filter, &ExportLimits::default());
    let ids = span_ids_in_order(&result);
    assert_ne!(
        ids[0], ids[1],
        "two positions in the stream must be two identities even when the \
         payload is identical: {}",
        result.lines[0]
    );
    assert_ne!(
        ids[0], "0000000000000000",
        "the span id must no longer be the raw stream position, which two \
         services in one trace would both hand out: {}",
        result.lines[0]
    );
}

#[test]
fn repeated_calls_on_one_store_are_byte_identical() {
    let mut store = CorrelationStore::new();
    let rec = recorded_with_traceparent(W3C_TP_A);
    let events: Vec<ChronosEvent> = (0..5).map(|i| event(i * 100, "p", i as i64)).collect();
    for (i, _) in events.iter().enumerate() {
        store.bind(i as u64, &rec);
    }
    let filter = OptInFilter::new(vec!["p".to_string()]);
    let first = export(&store, &events, &filter, &ExportLimits::default());
    let second = export(&store, &events, &filter, &ExportLimits::default());
    assert_eq!(
        first, second,
        "same store + same events must export byte-identical"
    );
}

/// Two invocations ordered by their uuid bytes.
///
/// `OtlpInvocationId` is v4 random, so a test cannot pick ids that happen to
/// sort a given way — it draws fresh ids until it holds a pair whose uuid
/// order it knows, and then places them against the stream on purpose.
fn uuid_ordered_pair() -> (RecordedInvocation, RecordedInvocation) {
    for _ in 0..64 {
        let a = recorded_internal();
        let b = recorded_internal();
        if a.invocation_id.as_uuid() < b.invocation_id.as_uuid() {
            return (a, b);
        }
    }
    panic!("no uuid-ordered pair in 64 draws");
}

#[test]
fn export_is_reproducible_across_stores_with_the_same_stream() {
    // ADR-0018 §3.1 promises deterministic output. The promise is about runs,
    // not about calls, so the two stores here are separate and hold different
    // v4 invocation ids bound to the same event_idx ranges — swapped, so that
    // uuid order and stream order disagree in both stores. Ordering by
    // invocation id would emit the groups in opposite orders; ordering by each
    // group's lowest event_idx must not.
    let events: Vec<ChronosEvent> = (0..4).map(|i| event(i * 100, "p", i as i64)).collect();
    let filter = OptInFilter::new(vec!["p".to_string()]);

    let (lower_uuid, higher_uuid) = uuid_ordered_pair();

    let mut store_a = CorrelationStore::new();
    store_a.bind(0, &lower_uuid);
    store_a.bind(1, &lower_uuid);
    store_a.bind(2, &higher_uuid);
    store_a.bind(3, &higher_uuid);

    let mut store_b = CorrelationStore::new();
    store_b.bind(0, &higher_uuid);
    store_b.bind(1, &higher_uuid);
    store_b.bind(2, &lower_uuid);
    store_b.bind(3, &lower_uuid);

    let a = export(&store_a, &events, &filter, &ExportLimits::default());
    let b = export(&store_b, &events, &filter, &ExportLimits::default());

    assert_eq!(a.emitted, 4);
    assert_eq!(b.emitted, 4);
    // The whole stream comes out ascending, because groups are ordered by
    // their lowest event_idx and are themselves ascending. Under invocation-id
    // ordering store_a would emit [0,1,2,3] and store_b [2,3,0,1].
    //
    // What is compared is the payload, which is the stream position, and
    // not the span id: spans are derived from the invocation id as well as
    // the position, so two stores that hand the invocations to different
    // event ranges produce different identities for the same position. That
    // is the property that stops two services in one trace from colliding.
    let a_order = payload_values_in_order(&a);
    let b_order = payload_values_in_order(&b);
    assert_eq!(
        a_order,
        vec![0, 1, 2, 3],
        "groups follow the stream, not the random invocation ids: {:?}",
        a.lines
    );
    assert_eq!(
        a_order, b_order,
        "the same stream must export in the same order regardless of which invocation id owns which event range"
    );
    // The two stores really did disagree about ownership, so the assertion
    // above is not vacuous: the invocation sequences are the mirror image.
    assert_ne!(
        invocation_ids_in_order(&a),
        invocation_ids_in_order(&b),
        "the stores assign the invocations to opposite halves; if they did \
         not, comparing payload order would prove nothing"
    );
    assert_eq!(
        a.skipped_limit_total, b.skipped_limit_total,
        "counters must not depend on invocation id assignment either"
    );
}

/// Span ids in emission order.
///
/// Compared *within one store*, where the trace and the invocation are
/// held fixed, so a difference means a real difference in stream
/// position. Across two stores that bound different invocation ids the
/// ids deliberately differ for the same position — see
/// [`payload_values_in_order`] for what is comparable across stores, and
/// [`invocation_ids_in_order`] for the mirror-image check that keeps that
/// comparison from going vacuous.
fn span_ids_in_order(result: &ExportResult) -> Vec<String> {
    result
        .lines
        .iter()
        .map(|line| {
            let v: serde_json::Value = serde_json::from_str(line).expect("valid JSON");
            v["span_id"].as_str().expect("span_id").to_string()
        })
        .collect()
}

/// The `value` attribute of each emitted line, in emission order.
///
/// This is the stream position the exporter promises to emit in, and it
/// is what this test asserts on. Span ids and invocation ids cannot:
/// spans are derived from the invocation id (so two stores that assign
/// the invocations differently produce different identities for the same
/// position), and the invocation id is a random v4 on purpose.
fn payload_values_in_order(result: &ExportResult) -> Vec<i64> {
    result
        .lines
        .iter()
        .map(|line| {
            let v: serde_json::Value = serde_json::from_str(line).expect("valid JSON");
            let attrs = v["attributes"].as_array().expect("attributes array");
            let entry = attrs
                .iter()
                .find(|a| a["key"] == "value")
                .expect("the value attribute");
            entry["value"]
                .as_str()
                .expect("a string on the wire")
                .parse()
                .expect("an integer payload")
        })
        .collect()
}

/// The invocation each emitted line belongs to, in emission order.
///
/// Separate from [`span_ids_in_order`] because the span id is derived from
/// the invocation, so the two answer different questions: which span is
/// this, and which group does it belong to.
fn invocation_ids_in_order(result: &ExportResult) -> Vec<String> {
    result
        .lines
        .iter()
        .map(|line| {
            let v: serde_json::Value = serde_json::from_str(line).expect("valid JSON");
            v["chronos_invocation_id"]
                .as_str()
                .expect("chronos_invocation_id")
                .to_string()
        })
        .collect()
}

// ----- Declared defaults -----

#[test]
fn export_limits_defaults_are_exact() {
    let l = ExportLimits::default();
    assert_eq!(l.max_events_per_invocation, 100);
    assert_eq!(l.max_events_total, 1000);
    assert_eq!(l.max_field_chars, 1024);
}

// ----- Limit priority (ADR-0018 §2.2) -----

#[test]
fn filter_precedes_the_caps() {
    // max_events_per_invocation = 0 caps everything; the non-allowed probe
    // must still be attributed to the filter, not to the cap.
    let mut store = CorrelationStore::new();
    let rec = recorded_internal();
    let events = vec![event(100, "denied", 1)];
    store.bind(0, &rec);
    let limits = ExportLimits {
        max_events_per_invocation: 0,
        max_events_total: 0,
        ..ExportLimits::default()
    };
    let result = export(
        &store,
        &events,
        &OptInFilter::new(vec!["allowed".to_string()]),
        &limits,
    );
    assert_eq!(result.skipped_probe_filter, 1);
    assert_eq!(result.skipped_limit_per_invocation, 0);
    assert_eq!(result.skipped_limit_total, 0);
}

#[test]
fn per_invocation_cap_precedes_the_total_cap() {
    // Both caps would trip on the same event; the per-invocation counter
    // must win, so the total cap must report zero skips.
    let mut store = CorrelationStore::new();
    let rec = recorded_internal();
    let events: Vec<ChronosEvent> = (0..4).map(|i| event(i, "p", i as i64)).collect();
    for (i, _) in events.iter().enumerate() {
        store.bind(i as u64, &rec);
    }
    let limits = ExportLimits {
        max_events_per_invocation: 1,
        max_events_total: 1,
        ..ExportLimits::default()
    };
    let result = export(
        &store,
        &events,
        &OptInFilter::new(vec!["p".to_string()]),
        &limits,
    );
    assert_eq!(result.emitted, 1);
    assert_eq!(result.skipped_limit_per_invocation, 3);
    assert_eq!(result.skipped_limit_total, 0);
}

// ----- Wire format -----

#[test]
fn wire_field_order_is_fixed() {
    let mut store = CorrelationStore::new();
    let rec = recorded_with_traceparent(W3C_TP_A);
    let events = vec![event(100, "p", 1)];
    store.bind(0, &rec);
    let result = export(
        &store,
        &events,
        &OptInFilter::new(vec!["p".to_string()]),
        &ExportLimits::default(),
    );
    let line = &result.lines[0];
    let order = [
        "\"trace_id\":",
        "\"span_id\":",
        "\"name\":",
        "\"start_time_unix_nano\":",
        "\"chronos_invocation_id\":",
        "\"attributes\":",
    ];
    let mut previous = 0usize;
    for key in order {
        let at = line
            .find(key)
            .unwrap_or_else(|| panic!("{key} missing from {line}"));
        assert!(at >= previous, "{key} out of order in {line}");
        previous = at;
    }
    assert!(line.contains("\"name\":\"chronos.event.p\""), "{line}");
}

#[test]
fn attribute_values_are_always_strings() {
    let mut store = CorrelationStore::new();
    let rec = recorded_internal();
    let events = vec![ChronosEvent {
        ts_micros: 100,
        probe: "p".to_string(),
        fields: vec![
            ("an_int".to_string(), EventField::Int(-42)),
            ("a_bool".to_string(), EventField::Bool(true)),
            (
                "a_ref".to_string(),
                EventField::DomainRef("session:7".to_string()),
            ),
        ],
    }];
    store.bind(0, &rec);
    let result = export(
        &store,
        &events,
        &OptInFilter::new(vec!["p".to_string()]),
        &ExportLimits::default(),
    );
    let v: serde_json::Value = serde_json::from_str(&result.lines[0]).expect("valid JSON");
    let attrs = v["attributes"].as_array().expect("attributes array");
    let rendered: Vec<(String, String)> = attrs
        .iter()
        .map(|a| {
            (
                a["key"].as_str().expect("key").to_string(),
                a["value"]
                    .as_str()
                    .expect("value must be a string")
                    .to_string(),
            )
        })
        .collect();
    assert!(rendered.contains(&("an_int".to_string(), "-42".to_string())));
    assert!(rendered.contains(&("a_bool".to_string(), "true".to_string())));
    assert!(rendered.contains(&("a_ref".to_string(), "ref:session:7".to_string())));
    assert!(
        rendered.contains(&("probe".to_string(), "p".to_string())),
        "ADR-0018 §3.2: every span carries its probe: {rendered:?}"
    );
}

// ----- Real temporal domain (ADR-0018 §2.3 divergence) -----

#[test]
fn start_time_unix_nano_is_zero_and_monotonic_value_is_disclosed() {
    let mut store = CorrelationStore::new();
    let rec = recorded_internal();
    let events = vec![event(1500, "p", 1)];
    store.bind(0, &rec);
    let result = export(
        &store,
        &events,
        &OptInFilter::new(vec!["p".to_string()]),
        &ExportLimits::default(),
    );
    let v: serde_json::Value = serde_json::from_str(&result.lines[0]).expect("valid JSON");
    assert_eq!(
        v["start_time_unix_nano"], 0,
        "unix-epoch ns are unknown: emitting a monotonic value here would be a lie"
    );
    let attrs = v["attributes"].as_array().expect("attributes array");
    let get = |key: &str| -> String {
        attrs
            .iter()
            .find(|a| a["key"] == key)
            .unwrap_or_else(|| panic!("attribute {key} missing"))
            .get("value")
            .and_then(|x| x.as_str())
            .expect("value is a string")
            .to_string()
    };
    assert_eq!(
        get("chronos.timestamp_domain"),
        TIMESTAMP_DOMAIN,
        "the domain must be disclosed so a collector does not read 0 as a real epoch"
    );
    assert_eq!(
        get("chronos.timestamp_monotonic_ns"),
        "1500000",
        "the real session-monotonic nanoseconds must be carried as an attribute"
    );
}

#[test]
fn monotonic_attribute_saturates_instead_of_wrapping() {
    let mut store = CorrelationStore::new();
    let rec = recorded_internal();
    let events = vec![event(u64::MAX, "p", 1)];
    store.bind(0, &rec);
    let result = export(
        &store,
        &events,
        &OptInFilter::new(vec!["p".to_string()]),
        &ExportLimits::default(),
    );
    let v: serde_json::Value = serde_json::from_str(&result.lines[0]).expect("valid JSON");
    let attrs = v["attributes"].as_array().expect("attributes array");
    let monotonic = attrs
        .iter()
        .find(|a| a["key"] == "chronos.timestamp_monotonic_ns")
        .and_then(|a| a["value"].as_str())
        .expect("monotonic attribute present");
    assert_eq!(monotonic, u64::MAX.to_string(), "saturated, not wrapped");
}

// ----- Real event-type name channel (the spike had no equivalent) -----

#[test]
fn probe_name_is_the_event_type_spelling() {
    let te = real_trace_event(0, 1_000, EventType::FunctionEntry, "main");
    assert_eq!(probe_name(&te), "function_entry");
    assert_eq!(
        EventType::from_snake_case(&probe_name(&te)),
        Some(EventType::FunctionEntry),
        "the probe name must round-trip through the domain's own parser"
    );
}

#[test]
fn probe_name_ignores_the_payload_name() {
    // EventData::Function { name } and SourceLocation::function both name the
    // subject ("main"), not the trigger. Folding them into the probe name
    // would make the opt-in list unbounded (one entry per function) and would
    // let a payload value choose what the caller opted into.
    let te = real_trace_event(0, 1_000, EventType::FunctionEntry, "main");
    let other = real_trace_event(1, 2_000, EventType::FunctionEntry, "helper");
    assert_eq!(probe_name(&te), probe_name(&other));
    assert_eq!(probe_name(&te), "function_entry");
}

#[test]
fn probe_name_covers_every_real_event_type_spelling() {
    for kind in [
        EventType::SyscallEnter,
        EventType::FunctionExit,
        EventType::VariableWrite,
        EventType::MemoryAlloc,
        EventType::SignalDelivered,
        EventType::BreakpointHit,
        EventType::ThreadCreate,
        EventType::ExceptionThrown,
        EventType::InvocationIncomplete,
        EventType::Unknown,
    ] {
        let te = real_trace_event(0, 1_000, kind, "subject");
        let name = probe_name(&te);
        assert_eq!(
            EventType::from_snake_case(&name),
            Some(kind),
            "{name} must round-trip for {kind:?}"
        );
    }
}

#[test]
fn filter_selects_on_real_event_type_names() {
    // Everything here comes from real domain types: the filter operates on the
    // `EventType` spelling, because `TraceEvent` has no probe field.
    let te_function = real_trace_event(0, 1_000_000, EventType::FunctionEntry, "main");
    let te_syscall = real_trace_event(1, 2_000_000, EventType::SyscallEnter, "read");
    let te_memory = real_trace_event(2, 3_000_000, EventType::MemoryWrite, "0x1000");
    let events = vec![
        project(&te_function),
        project(&te_syscall),
        project(&te_memory),
    ];
    let mut store = CorrelationStore::new();
    let rec = recorded_with_traceparent(W3C_TP_A);
    for (i, _) in events.iter().enumerate() {
        store.bind(i as u64, &rec);
    }
    let filter = OptInFilter::new(vec![
        EventType::FunctionEntry.to_string(),
        EventType::MemoryWrite.to_string(),
    ]);
    let result = export(&store, &events, &filter, &ExportLimits::default());
    assert_eq!(result.emitted, 2);
    assert_eq!(result.skipped_probe_filter, 1);
    let names: Vec<String> = result
        .lines
        .iter()
        .map(|l| {
            let v: serde_json::Value = serde_json::from_str(l).expect("valid JSON");
            v["name"].as_str().expect("name").to_string()
        })
        .collect();
    assert!(names.contains(&"chronos.event.function_entry".to_string()));
    assert!(names.contains(&"chronos.event.memory_write".to_string()));
    assert!(!names.contains(&"chronos.event.syscall_enter".to_string()));
}

#[test]
fn event_type_filter_and_monotonic_domain_hold_together() {
    let tes: Vec<TraceEvent> = (0..3)
        .map(|i| real_trace_event(i, (i + 1) * 1_000_000, EventType::FunctionEntry, "main"))
        .collect();
    let events: Vec<ChronosEvent> = tes.iter().map(project).collect();
    let mut store = CorrelationStore::new();
    let rec = recorded_internal();
    for (i, _) in events.iter().enumerate() {
        store.bind(i as u64, &rec);
    }
    let filter = OptInFilter::new(vec![EventType::FunctionEntry.to_string()]);
    let result = export(&store, &events, &filter, &ExportLimits::default());
    assert_eq!(result.emitted, 3);
    assert_eq!(result.skipped_probe_filter, 0);
    for (i, line) in result.lines.iter().enumerate() {
        let v: serde_json::Value = serde_json::from_str(line).expect("valid JSON");
        assert_eq!(v["start_time_unix_nano"], 0);
        let expected_ns = ((i as u64 + 1) * 1_000_000).to_string();
        let attrs = v["attributes"].as_array().expect("attributes array");
        let monotonic = attrs
            .iter()
            .find(|a| a["key"] == "chronos.timestamp_monotonic_ns")
            .and_then(|a| a["value"].as_str())
            .expect("monotonic attribute");
        assert_eq!(monotonic, expected_ns);
    }
}

// ----- Truncation edge cases the spike could not reach -----

#[test]
fn truncation_never_splits_a_multibyte_character() {
    let mut store = CorrelationStore::new();
    let rec = recorded_internal();
    // Two-byte characters: a byte-index cut can land inside one, which is a
    // panic, not a wrong value. cap 26 leaves a 3-byte budget, so the cut
    // would start mid-character without the boundary walk-back.
    let events = vec![event_with_str(100, "p", "é".repeat(20))];
    store.bind(0, &rec);
    let limits = ExportLimits {
        max_field_chars: 26,
        ..ExportLimits::default()
    };
    let result = export(
        &store,
        &events,
        &OptInFilter::new(vec!["p".to_string()]),
        &limits,
    );
    assert_eq!(result.emitted, 1, "truncation must not panic");
    assert_eq!(result.truncated_fields, 1);
    let v: serde_json::Value =
        serde_json::from_str(&result.lines[0]).expect("valid JSON after truncation");
    let attrs = v["attributes"].as_array().expect("attributes array");
    let value = attrs
        .iter()
        .find(|a| a["key"] == "data")
        .and_then(|a| a["value"].as_str())
        .expect("data attribute");
    assert_eq!(
        value.chars().next(),
        Some('é'),
        "the kept payload must be whole characters: {value:?}"
    );
    assert!(
        value.contains("truncated"),
        "disclosure suffix must survive: {value}"
    );
    assert!(
        !value.contains('\u{fffd}'),
        "no replacement character may appear: {value}"
    );
    assert!(
        value.len() <= limits.max_field_chars,
        "rendered value stays within the cap: {} > {}",
        value.len(),
        limits.max_field_chars
    );
}

#[test]
fn truncation_keeps_the_rendered_value_within_the_declared_cap() {
    let mut store = CorrelationStore::new();
    let rec = recorded_internal();
    let events = vec![event_with_str(100, "p", "y".repeat(500))];
    store.bind(0, &rec);
    let limits = ExportLimits {
        max_field_chars: 64,
        ..ExportLimits::default()
    };
    let result = export(
        &store,
        &events,
        &OptInFilter::new(vec!["p".to_string()]),
        &limits,
    );
    let v: serde_json::Value = serde_json::from_str(&result.lines[0]).expect("valid JSON");
    let attrs = v["attributes"].as_array().expect("attributes array");
    let value = attrs
        .iter()
        .find(|a| a["key"] == "data")
        .and_then(|a| a["value"].as_str())
        .expect("data attribute");
    assert!(
        value.len() <= limits.max_field_chars,
        "declared cap must bound the rendered value: {} > {}",
        value.len(),
        limits.max_field_chars
    );
}

#[test]
fn truncation_disclosure_survives_a_cap_smaller_than_the_suffix() {
    let mut store = CorrelationStore::new();
    let rec = recorded_internal();
    let events = vec![event_with_str(100, "p", "z".repeat(500))];
    store.bind(0, &rec);
    let limits = ExportLimits {
        max_field_chars: 4,
        ..ExportLimits::default()
    };
    let result = export(
        &store,
        &events,
        &OptInFilter::new(vec!["p".to_string()]),
        &limits,
    );
    assert_eq!(result.truncated_fields, 1);
    let line = &result.lines[0];
    assert!(
        line.contains("truncated"),
        "a cap too small for the payload must still disclose the truncation: {line}"
    );
    serde_json::from_str::<serde_json::Value>(line).expect("valid JSON");
}
