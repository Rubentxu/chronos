//! Integration tests for `chronos-domain::otlp::ingest` (M6.2 lift).
//!
//! Tests are adapted from the durable M6.2 spike
//! (`/home/rubentxu/m6-spikes/m6.2-otlp-adapter/tests/cases.rs`). Only the
//! header-level cases are lifted; the four `http_ingest_*` cases that
//! exercise the TCP server remain spike-only fixtures because the
//! product-side HTTP transport is deferred to the operator's choice
//! (ADR-0033 §2.2).

use chronos_domain::otlp::ingest::*;
use chronos_domain::otlp::parse::parse_traceparent;

const W3C_TP: &str = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";

#[test]
fn adapter_with_traceparent_records() {
    let mut h = Headers::default();
    h.entries
        .push(("traceparent".to_string(), W3C_TP.to_string()));
    let outcome = adapter(&h);
    assert!(matches!(outcome, IngestOutcome::Recorded(_)));
    assert!(outcome.is_ok());
    assert_eq!(outcome.status_code(), 200);
}

#[test]
fn adapter_without_traceparent_internal_only() {
    let h = Headers::default();
    let outcome = adapter(&h);
    assert!(matches!(outcome, IngestOutcome::InternalOnly(_)));
    assert!(outcome.is_ok());
    assert_eq!(outcome.status_code(), 200);
}

#[test]
fn adapter_with_bad_traceparent_returns_400() {
    let mut h = Headers::default();
    h.entries.push((
        "traceparent".to_string(),
        "not-a-valid-traceparent".to_string(),
    ));
    let outcome = adapter(&h);
    assert!(matches!(outcome, IngestOutcome::BadTraceparent { .. }));
    assert!(!outcome.is_ok());
    assert_eq!(outcome.status_code(), 400);
}

#[test]
fn adapter_with_all_zero_traceparent_fails() {
    let mut h = Headers::default();
    h.entries.push((
        "traceparent".to_string(),
        "00-00000000000000000000000000000000-00f067aa0ba902b7-01".to_string(),
    ));
    let outcome = adapter(&h);
    assert!(matches!(outcome, IngestOutcome::BadTraceparent { .. }));
}

#[test]
fn adapter_with_traceparent_and_tracestate() {
    let mut h = Headers::default();
    h.entries
        .push(("traceparent".to_string(), W3C_TP.to_string()));
    h.entries.push((
        "tracestate".to_string(),
        "congo=t61rcWkgMzE,rojo=00f067aa0ba902b7".to_string(),
    ));
    let outcome = adapter(&h);
    assert!(matches!(outcome, IngestOutcome::Recorded(_)));
}

#[test]
fn adapter_with_bad_tracestate_returns_400() {
    let mut h = Headers::default();
    h.entries
        .push(("traceparent".to_string(), W3C_TP.to_string()));
    h.entries
        .push(("tracestate".to_string(), "UPPERCASE=bad".to_string()));
    let outcome = adapter(&h);
    assert!(matches!(outcome, IngestOutcome::BadTracestate { .. }));
    assert_eq!(outcome.status_code(), 400);
}

#[test]
fn adapter_case_insensitive_header_lookup() {
    // W3C headers are case-insensitive (RFC 7230 §3.2). The spike uses
    // parse_block for header injection because push() preserves the
    // raw key — parse_block is the case-normalising path.
    let block = format!("TraceParent: {}\r\n", W3C_TP);
    let h = Headers::parse_block(&block);
    let outcome = adapter(&h);
    assert!(
        matches!(outcome, IngestOutcome::Recorded(_)),
        "case-insensitive lookup must succeed; got {:?}",
        outcome
    );
}

// --- Additional tests beyond the M6.2 spike (defensive coverage) ---

#[test]
fn headers_parse_block_handles_crlf_separated_lines() {
    let block = "Host: localhost\r\n\
                 traceparent: 00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01\r\n\
                 Connection: close\r\n";
    let h = Headers::parse_block(block);
    assert_eq!(h.get("host"), Some("localhost"));
    assert!(h.get("traceparent").unwrap().starts_with("00-"));
    assert_eq!(h.get("connection"), Some("close"));
}

#[test]
fn headers_parse_block_skips_empty_lines_and_bad_lines() {
    let block = "First: one\r\n\
                 \r\n\
                 no_colon_here\r\n\
                 Second: two\r\n";
    let h = Headers::parse_block(block);
    assert_eq!(h.get("first"), Some("one"));
    assert_eq!(h.get("second"), Some("two"));
    assert_eq!(h.entries.len(), 2);
}

#[test]
fn headers_parse_block_trims_whitespace_around_values() {
    let block = "  key  :   value with spaces   \r\n";
    let h = Headers::parse_block(block);
    assert_eq!(h.get("key"), Some("value with spaces"));
}

#[test]
fn render_response_200_for_recorded_includes_body_and_content_length() {
    let mut h = Headers::default();
    h.entries
        .push(("traceparent".to_string(), W3C_TP.to_string()));
    let outcome = adapter(&h);
    let response = render_response(&outcome);
    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(response.contains("Content-Type: text/plain"));
    assert!(response.contains("Content-Length: "));
    // Body shape: "recorded\n<rec>\n" — first line is exactly "recorded".
    let body_start = response.find("\r\n\r\n").expect("headers/body separator") + 4;
    let body = &response[body_start..];
    let first_line = body.lines().next().expect("body has at least one line");
    assert_eq!(
        first_line, "recorded",
        "first body line is the outcome label"
    );
    assert!(body.contains(W3C_TP));
    assert!(response.contains("Connection: close"));
}

#[test]
fn render_response_400_for_bad_traceparent_includes_raw_value() {
    let mut h = Headers::default();
    h.entries
        .push(("traceparent".to_string(), "GARBAGE".to_string()));
    let outcome = adapter(&h);
    let response = render_response(&outcome);
    assert!(response.starts_with("HTTP/1.1 400 Bad Request\r\n"));
    assert!(response.contains("bad-traceparent"));
    assert!(response.contains("GARBAGE"));
}

#[test]
fn render_response_200_for_internal_only() {
    let h = Headers::default();
    let outcome = adapter(&h);
    let response = render_response(&outcome);
    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(response.contains("internal-only"));
}

#[test]
fn adapter_each_call_yields_distinct_invocation_id() {
    // Defensive: the adapter mints a fresh OtlpInvocationId per call.
    let mut h = Headers::default();
    h.entries
        .push(("traceparent".to_string(), W3C_TP.to_string()));
    let out1 = adapter(&h);
    let out2 = adapter(&h);
    let id1 = match &out1 {
        IngestOutcome::Recorded(r) => r.invocation_id,
        _ => panic!("expected Recorded"),
    };
    let id2 = match &out2 {
        IngestOutcome::Recorded(r) => r.invocation_id,
        _ => panic!("expected Recorded"),
    };
    assert_ne!(id1, id2, "each adapter call must mint a fresh id");
}

#[test]
fn render_response_content_length_matches_body_byte_length() {
    let mut h = Headers::default();
    h.entries
        .push(("traceparent".to_string(), W3C_TP.to_string()));
    let outcome = adapter(&h);
    let response = render_response(&outcome);
    // Extract Content-Length and verify it equals the body byte length.
    let body_start = response.find("\r\n\r\n").expect("headers/body separator") + 4;
    let body = &response[body_start..];
    let cl_line = response
        .lines()
        .find(|l| l.to_ascii_lowercase().starts_with("content-length:"))
        .expect("content-length header");
    let cl: usize = cl_line
        .split(':')
        .nth(1)
        .expect("content-length value")
        .trim()
        .parse()
        .expect("content-length is a number");
    assert_eq!(cl, body.len(), "content-length must equal body byte length");
}

#[test]
fn ingest_outcome_status_code_matches_outcome_kind() {
    // Defensive: status_code() and is_ok() are consistent.
    let mut h = Headers::default();
    h.entries
        .push(("traceparent".to_string(), W3C_TP.to_string()));
    let recorded = adapter(&h);
    assert_eq!(recorded.status_code(), 200);
    assert!(recorded.is_ok());

    let internal = adapter(&Headers::default());
    assert_eq!(internal.status_code(), 200);
    assert!(internal.is_ok());

    let mut bad = Headers::default();
    bad.entries
        .push(("traceparent".to_string(), "X".to_string()));
    let bad = adapter(&bad);
    assert_eq!(bad.status_code(), 400);
    assert!(!bad.is_ok());
}

#[test]
fn adapter_round_trip_via_render_response_preserves_traceparent() {
    // End-to-end: parse a traceparent, attach via adapter, render the
    // response, confirm the original traceparent appears verbatim.
    let parsed = parse_traceparent(W3C_TP).expect("parse");
    let mut h = Headers::default();
    h.entries
        .push(("traceparent".to_string(), W3C_TP.to_string()));
    let outcome = adapter(&h);
    let response = render_response(&outcome);
    assert!(response.contains(W3C_TP));
    assert!(response.contains(&parsed.to_traceparent()));
}
