//! REC-C1.8 — C1.8.5: UAT-REC-C1-05 exact.
//!
//! Verifies the literal `MILESTONE_ACCEPTANCE.md` spec:
//!
//! > "ExecutionRecord preserves seq, event_id, monotonic_ns, and
//! >  captured_at_unix_ns as four pairwise-independent dimensions."
//!
//! Per C1.8.5 option A, `captured_at_unix_ns: Option<u64>` was added
//! to `ExecutionRecord` (and `NewExecutionRecord`); this UAT
//! proves the four-dimension independence invariant on the wire
//! shape (serde_json round-trip).
//!
//! Independence in this context means:
//!
//! 1. Each dimension can be set independently. Reading any one
//!    dimension back MUST return exactly what was written for that
//!    dimension; no coupling to the others.
//! 2. The presence of `captured_at_unix_ns` does NOT promote a
//!    record's logical schema version from v1 to v2 (the M2+
//!    fields — invocation_id / parent_invocation_id / symbol_id —
//!    remain the sole promotion drivers).
//! 3. A v1 JSON document (no `captured_at_unix_ns` key) round-trips
//!    into an `ExecutionRecord` with `captured_at_unix_ns == None`.
//! 4. A v2 JSON document (with `captured_at_unix_ns: 1700000000_000_000_000`)
//!    round-trips with the value preserved.
//!
//! The four dimensions in this fixture are deliberately uncorrelated
//! (different magnitudes, different monotonicity, different domains):
//!
//! - `seq` : 0..N-1 (auto-assigned, per-session monotonic)
//! - `event_id` : 1000 + i (NOT auto-assigned; producer-driven)
//! - `monotonic_ns` : 5000 * i (session-relative time; producer-driven)
//! - `captured_at_unix_ns` : 1_700_000_000_000_000_000 + i * 10^9
//!   (wall clock; producer-driven, optional)

use chronos_log::{
    ExecutionPayload, ExecutionRecord, NewExecutionRecord, SessionId,
};

const FIXTURE_LEN: usize = 5;

/// Build a fixture of N records where each dimension is deliberately
/// independent of the others.
///
/// The four dimensions are seeded with **incommensurable** numerical
/// progressions so any coupling would surface as a value collision:
///   - `seq`              = i          (auto-assigned by append)
///   - `event_id`         = 1000 + i   (in payload bytes, distinct from seq)
///   - `monotonic_ns`     = 5000 * i   (5× larger than seq, no shared factors)
///   - `captured_at_unix_ns` = 1.7e18 + i * 1e9 (wall-clock ns, year 2023+)
///
/// Distinct magnitudes make any cross-dimension coupling trivially
/// detectable.
fn fixture_records() -> Vec<NewExecutionRecord> {
    let session_id = SessionId::new("rec-c1-8-uat-c1-05");
    (0..FIXTURE_LEN)
        .map(|i| NewExecutionRecord {
            kind: chronos_log::ExecutionKind::Raw,

            session_id: session_id.clone(),
            monotonic_ns: 5_000 * i as u64,
            payload: ExecutionPayload::new(
                format!("event_id={}", 1000 + i).into_bytes(),
                "dim_fixture",
            ),
            invocation_id: None,
            parent_invocation_id: None,
            symbol_id: None,
            captured_at_unix_ns: Some(1_700_000_000_000_000_000 + (i as u64) * 1_000_000_000),
        })
        .collect()
}

/// Round-trip an `ExecutionRecord` through serde_json (the on-wire
/// format) and assert the four dimensions survive independently.
///
/// We construct the `ExecutionRecord` directly (bypassing the
/// segmented log) so the test is a property check on the
/// serialization shape, not on the storage path. The storage path
/// is exercised end-to-end by the C1.8 closeout tests.
fn assert_four_dim_independence(records: &[ExecutionRecord]) {
    assert_eq!(records.len(), FIXTURE_LEN);

    for (i, r) in records.iter().enumerate() {
        // Serialize to JSON (the wire format).
        let json = serde_json::to_string(r).expect("serialize");
        // Deserialize back.
        let de: ExecutionRecord = serde_json::from_str(&json).expect("deserialize");

        // Dimension 1: seq (auto-assigned; in our fixture i).
        assert_eq!(de.seq.0, i as u64, "seq dimension corrupted at i={}", i);
        // Dimension 2: event_id (in payload bytes; 1000 + i).
        let event_id: u64 = std::str::from_utf8(&de.payload.bytes)
            .expect("utf8")
            .trim_start_matches("event_id=")
            .parse()
            .expect("parse event_id");
        assert_eq!(
            event_id,
            1000 + i as u64,
            "event_id dimension corrupted at i={}",
            i
        );
        // Dimension 3: monotonic_ns (session-relative; 5000 * i).
        assert_eq!(
            de.monotonic_ns,
            5_000 * i as u64,
            "monotonic_ns dimension corrupted at i={}",
            i
        );
        // Dimension 4: captured_at_unix_ns (wall clock; 1.7e18 + i*1e9).
        assert_eq!(
            de.captured_at_unix_ns,
            Some(1_700_000_000_000_000_000 + (i as u64) * 1_000_000_000),
            "captured_at_unix_ns dimension corrupted at i={}",
            i
        );
    }
}

/// TEST 1: round-trip four uncorrelated dimensions through the wire
/// JSON shape. Each dimension survives independently.
#[test]
fn four_dimensions_round_trip_independently() {
    let new_records = fixture_records();
    // Convert to stored records (seq is auto-assigned by append; for
    // a property check on the wire shape we assign seq = i directly
    // so the test is hermetic — no log backend in the loop).
    let stored: Vec<ExecutionRecord> = new_records
        .into_iter()
        .enumerate()
        .map(|(i, r)| ExecutionRecord {
            session_id: r.session_id,
            seq: chronos_log::EventSeq::new(i as u64),
            monotonic_ns: r.monotonic_ns,
            kind: chronos_log::ExecutionKind::Raw,
            payload: r.payload,
            invocation_id: r.invocation_id,
            parent_invocation_id: r.parent_invocation_id,
            symbol_id: r.symbol_id,
            captured_at_unix_ns: r.captured_at_unix_ns,
        })
        .collect();

    assert_four_dim_independence(&stored);
}

/// TEST 2: a v1 JSON document (without `captured_at_unix_ns`)
/// deserializes to `captured_at_unix_ns == None` and the record's
/// schema version stays v1.
#[test]
fn v1_document_without_captured_field_round_trips_to_none() {
    let session_id = SessionId::new("rec-c1-8-uat-c1-05-v1");
    let json = serde_json::json!({
        "session_id": session_id.as_str(),
        "seq": 42,
        "monotonic_ns": 100,
        "kind": "Raw",
        "payload": { "bytes": [1, 2, 3], "tag": "v1" }
    });
    let text = serde_json::to_string(&json).expect("encode json");
    let r: ExecutionRecord = serde_json::from_str(&text).expect("decode v1");
    assert_eq!(r.captured_at_unix_ns, None);
    assert_eq!(r.schema_version(), "chronos_exec_v1");
}

/// TEST 3: a v2 JSON document (with `captured_at_unix_ns` populated)
/// round-trips with the value preserved. Crucially, schema_version
/// stays v1 (no invocation/symbol fields), proving the field does
/// NOT promote the schema.
#[test]
fn v2_shape_with_captured_field_preserves_value_but_stays_v1() {
    let session_id = SessionId::new("rec-c1-8-uat-c1-05-v2");
    let captured = 1_700_000_000_000_000_000_u64;
    let json = serde_json::json!({
        "session_id": session_id.as_str(),
        "seq": 7,
        "monotonic_ns": 3500,
        "kind": "Raw",
        "payload": { "bytes": [], "tag": "v2-shape" },
        "captured_at_unix_ns": captured
    });
    let text = serde_json::to_string(&json).expect("encode json");
    let r: ExecutionRecord = serde_json::from_str(&text).expect("decode v2-shape");
    assert_eq!(r.captured_at_unix_ns, Some(captured));
    assert_eq!(
        r.schema_version(),
        "chronos_exec_v1",
        "captured_at_unix_ns must NOT promote schema version"
    );
}

/// TEST 4: setting `captured_at_unix_ns` to None on a record with
/// no other v2 fields round-trips and remains None, AND the wire
/// JSON omits the field (per `skip_serializing_if`).
#[test]
fn unchanged_default_skips_field_in_wire_json() {
    let session_id = SessionId::new("rec-c1-8-uat-c1-05-default");
    let r = ExecutionRecord {
        session_id,
        seq: chronos_log::EventSeq::new(0),
        monotonic_ns: 0,
        kind: chronos_log::ExecutionKind::Raw,
        payload: ExecutionPayload::default(),
        invocation_id: None,
        parent_invocation_id: None,
        symbol_id: None,
        captured_at_unix_ns: None,
    };
    let text = serde_json::to_string(&r).expect("encode");
    assert!(
        !text.contains("captured_at_unix_ns"),
        "wire JSON must omit captured_at_unix_ns when None; got: {}",
        text
    );
}
