//! Integration tests for the `chronos-domain::otlp` module (M6.1 lift).
//!
//! Tests are adapted verbatim from the durable M6.1 spike
//! (`/home/rubentxu/m6-spikes/m6.1-skeleton/tests/cases.rs`) so the wire
//! contract is byte-identical between spike and product. Adding new cases
//! here is preferred over mutating the spike so that production tests are
//! the canonical source of truth.

use chronos_domain::otlp::parse::{
    parse_traceparent, parse_tracestate, TracestateParseError, TraceparentParseError,
};
use chronos_domain::otlp::*;

#[test]
fn otlp_invocation_id_generates_uuid_v4() {
    let id1 = OtlpInvocationId::new();
    let id2 = OtlpInvocationId::new();
    assert_ne!(id1, id2);
    let s = id1.to_string();
    assert_eq!(s.len(), 36);
}

#[test]
fn traceparent_w3c_example_parses() {
    let tp = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
    let etc = parse_traceparent(tp).expect("parse");
    assert_eq!(etc.to_traceparent(), tp);
}

#[test]
fn traceparent_flags_sampled_bit() {
    let tp = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
    let etc = parse_traceparent(tp).expect("parse");
    assert!(etc.flags().sampled());
}

#[test]
fn traceparent_can_attach_tracestate() {
    let tp = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
    let etc = parse_traceparent(tp).expect("parse");
    let ts = parse_tracestate("congo=t61rcWkgMzE,rojo=00f067aa0ba902b7").expect("parse ts");
    let etc2 = etc.with_tracestate(ts);
    assert_eq!(etc2.to_tracestate(), "congo=t61rcWkgMzE,rojo=00f067aa0ba902b7");
}

#[test]
fn traceparent_wrong_segment_count_fails() {
    let err = parse_traceparent("00-aa-bb-cc-dd").unwrap_err();
    assert_eq!(err, TraceparentParseError::WrongSegmentCount { actual: 5 });
}

#[test]
fn traceparent_unknown_version_fails() {
    let err =
        parse_traceparent("01-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01").unwrap_err();
    assert_eq!(
        err,
        TraceparentParseError::InvalidVersion {
            version: "01".to_string()
        }
    );
}

#[test]
fn traceparent_all_zero_trace_id_fails() {
    let err = parse_traceparent("00-00000000000000000000000000000000-00f067aa0ba902b7-01")
        .unwrap_err();
    assert_eq!(
        err,
        TraceparentParseError::InvalidAllZero {
            segment: "trace-id"
        }
    );
}

#[test]
fn traceparent_all_zero_span_id_fails() {
    let err =
        parse_traceparent("00-4bf92f3577b34da6a3ce929d0e0e4736-0000000000000000-01").unwrap_err();
    assert_eq!(
        err,
        TraceparentParseError::InvalidAllZero {
            segment: "parent-id"
        }
    );
}

#[test]
fn tracestate_empty_is_default() {
    let ts = parse_tracestate("").expect("empty ok");
    assert!(ts.is_empty());
}

#[test]
fn tracestate_too_many_entries_fails() {
    let raw: String = (0..33)
        .map(|i| format!("k{}=v", i))
        .collect::<Vec<_>>()
        .join(",");
    let err = parse_tracestate(&raw).unwrap_err();
    assert_eq!(err, TracestateParseError::TooManyEntries { count: 33 });
}

#[test]
fn tracestate_invalid_vendor_uppercase_fails() {
    let err = parse_tracestate("Congo=t61rcWkgMzE").unwrap_err();
    assert!(matches!(err, TracestateParseError::InvalidVendor { .. }));
}

#[test]
fn root_context_is_all_zero() {
    let root = ExternalTraceContext::root();
    assert!(root.is_root());
    assert_eq!(
        root.to_traceparent(),
        "00-00000000000000000000000000000000-0000000000000000-00"
    );
}

#[test]
fn record_external_combines_both_contracts() {
    let inv = OtlpInvocationId::new();
    let tp = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
    let ts = parse_tracestate("congo=t61rcWkgMzE").expect("parse");
    let etc = parse_traceparent(tp).expect("parse").with_tracestate(ts);
    let rec = inv.record_external(&etc);
    assert_eq!(rec.invocation_id, inv);
    assert!(rec.external.is_some());
}

#[test]
fn record_internal_only_omits_external() {
    let inv = OtlpInvocationId::new();
    let rec = RecordedInvocation::internal_only(inv);
    let s = rec.to_recorded();
    let parts: Vec<&str> = s.split('|').collect();
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0], inv.to_string());
    assert_eq!(parts[1], "");
    assert_eq!(parts[2], "");
}

// --- Additional tests beyond the M6.1 spike (defensive coverage) ---

#[test]
fn trace_id_is_all_zero_detects_invalid_id() {
    let tid = TraceId::from_bytes([0; 16]);
    assert!(tid.is_all_zero());
    let tid_ok = TraceId::from_bytes([1; 16]);
    assert!(!tid_ok.is_all_zero());
}

#[test]
fn span_id_from_bytes_round_trips() {
    let bytes: [u8; 8] = [0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89];
    let sid = SpanId::from_bytes(bytes);
    assert_eq!(sid.bytes(), &bytes);
    assert!(!sid.is_all_zero());
}

#[test]
fn trace_flags_bits_zero_means_not_sampled() {
    // Round-trip a non-sampled flag (00) to confirm bit-0 semantics.
    let tp = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-00";
    let etc = parse_traceparent(tp).expect("parse");
    assert!(!etc.flags().sampled());
    assert_eq!(etc.flags().bits(), 0x00);
}

#[test]
fn tracestate_entry_too_long_fails() {
    let long_value = "x".repeat(257);
    let raw = format!("vendor={}", long_value);
    let err = parse_tracestate(&raw).unwrap_err();
    assert!(matches!(err, TracestateParseError::EntryTooLong { .. }));
}

#[test]
fn tracestate_missing_equals_fails() {
    let err = parse_tracestate("vendor_no_value").unwrap_err();
    assert!(matches!(err, TracestateParseError::MissingEquals { .. }));
}

#[test]
fn tracestate_empty_vendor_fails() {
    let err = parse_tracestate("=value").unwrap_err();
    assert!(matches!(err, TracestateParseError::EmptyVendor));
}

#[test]
fn traceparent_invalid_hex_chars_fails() {
    let err = parse_traceparent("00-zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz-00f067aa0ba902b7-01")
        .unwrap_err();
    assert!(matches!(err, TraceparentParseError::InvalidHex { .. }));
}

#[test]
fn traceparent_wrong_trace_id_length_fails() {
    let err = parse_traceparent("00-4bf92f3577b34da6a-00f067aa0ba902b7-01").unwrap_err();
    assert!(matches!(
        err,
        TraceparentParseError::WrongLength {
            segment: "trace-id",
            ..
        }
    ));
}

#[test]
fn traceparent_wrong_flags_length_fails() {
    let err = parse_traceparent("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-1")
        .unwrap_err();
    assert!(matches!(
        err,
        TraceparentParseError::WrongLength {
            segment: "trace-flags",
            ..
        }
    ));
}

#[test]
fn recorded_to_recorded_format_is_three_pipe_segments() {
    let inv = OtlpInvocationId::new();
    let tp = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
    let ts = parse_tracestate("k=v").expect("parse");
    let etc = parse_traceparent(tp).expect("parse").with_tracestate(ts);
    let rec = inv.record_external(&etc);
    let s = rec.to_recorded();
    assert_eq!(s.matches('|').count(), 2, "must be 3 pipe-separated parts");
    assert!(s.starts_with(&inv.to_string()));
    assert!(s.contains(tp));
    assert!(s.contains("k=v"));
}
