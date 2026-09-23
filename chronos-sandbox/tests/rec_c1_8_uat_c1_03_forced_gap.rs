//! REC-C1.8 — C1.8.4: UAT-REC-C1-03 exact.
//!
//! Public UAT that proves the literal MILESTONE_ACCEPTANCE.md spec:
//!
//! > "Force bounded retention loss. The read crossing the lost interval
//! >  returns an explicit gap/incomplete state and MUST NOT report
//! >  'complete'."
//!
//! STATUS (2026-09-17): both arms GREEN. Both the negative arm
//! (`complete` for a clean session) and the positive arm (forced-gap
//! `gap_detected`) run on the real MCP wire. The positive arm used to
//! be `#[ignore]`-marked because a durable segment containing a `Gap`
//! could not be reopened (FIND-C1.8-01). The `m1-gap-segment-accounting-repair`
//! cycle fixed that: the header count is now unambiguously the
//! persisted `SegmentEntry` count, shared by writer, header and replay.
//! The fixture therefore uses the direct `record_gap` path again.
//!
//! Wire shape (verified empirically against `rec_c1_7_uat_c1_01_two_consumers.rs`
//! and `EventsReadOutput::Query` at `crates/chronos-services/src/output.rs:1581`):
//!
//! ```json
//! {
//!   "result": { "events": [...] },
//!   "next_cursor": String | null,
//!   "completeness": { "status": "complete" | "truncated" | "gap_detected",
//!                     "scope": "examined_range",
//!                     "from_seq": u64,
//!                     "to_seq_exclusive": u64 },
//!   "gap_summary": ... | null,
//!   "retention": { ... }
//! }
//! ```
//!
//! The gap range is encoded in `completeness.from_seq` and
//! `completeness.to_seq_exclusive`; `gap_summary` is `null` in the
//! current wire envelope (always `None` per the m7-01 disclosure at
//! `crates/chronos-services/src/lib.rs:21`).

use std::path::PathBuf;

use chronos_domain::MonotonicNs;
use chronos_domain::{EventData, EventType, SourceLocation, TraceEvent};
use chronos_log::{
    EventSeq, ExecutionPayload, Gap, GapReason, NewExecutionRecord, SegmentedConfig,
    SegmentedExecutionLog, SessionId,
};
use chronos_sandbox::client::tools::McpTestClient;

/// Records at seq 0..GAP_FROM.
const RECORDS_BEFORE_GAP: u64 = 100;

/// Seq at which the gap starts (first_missing).
const GAP_FROM: u64 = 100;

/// Seq at which the gap ends (last_missing).
const GAP_TO: u64 = 200;

/// Records after the gap. The first record after `record_gap([100,200])`
/// gets seq `GAP_TO + 1 = 201`, so we append `RECORDS_AFTER_GAP` records
/// at seqs 201..(201+RECORDS_AFTER_GAP).
const RECORDS_AFTER_GAP: u64 = 100;

/// Total records in the fixture (excluding the gap).
const TOTAL_RECORDS: u64 = RECORDS_BEFORE_GAP + RECORDS_AFTER_GAP;

fn unique_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "chronos-rec-c1-8-uat03-{}-{}-{}",
        label,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

/// Build a deterministic `TraceEvent` for seq `i`. Uncorrelated triple.
fn trace_event_for(i: u64) -> TraceEvent {
    TraceEvent::new(
        40 + i,
        MonotonicNs::from(10_000_500 + i * 1_000),
        1,
        EventType::FunctionEntry,
        SourceLocation::from_address(0),
        EventData::Empty,
    )
}

/// Seed records `0..RECORDS_BEFORE_GAP`, record an explicit gap at
/// `[GAP_FROM, GAP_TO]`, then append `RECORDS_AFTER_GAP` records starting
/// at `GAP_TO + 1`.
///
/// The recorded gap is the contract: the `events_log_read::completeness_for`
/// semantics at `crates/chronos-services/src/events_log_read.rs:480` only
/// report `gap_detected` when the examined range intersects a `Gap` entry.
/// An absent range without a recorded gap is correctly reported as
/// `complete` (no evidence was used to reach that verdict; see the
/// "An empty examined range used no evidence" comment at line 494).
///
/// `record_gap` consumes the whole `[GAP_FROM, GAP_TO]` seq range, so the
/// first post-gap append lands at `GAP_TO + 1`.
fn seed_log_with_gap(exec_log_root: &std::path::Path, session_id: &str) {
    let session_id_typed = SessionId::new(session_id);
    let session_dir = exec_log_root.join(session_id);
    let mut cfg = SegmentedConfig::with_dir(&session_dir);
    // Keep the fixture in a single segment so reopen exercises one
    // header; the accounting fix applies to any layout regardless.
    cfg.flush_threshold = std::num::NonZeroUsize::new(4096).expect("non-zero");
    let log = SegmentedExecutionLog::open(session_id_typed.clone(), cfg).expect("open log");

    for i in 0..RECORDS_BEFORE_GAP {
        let ev = trace_event_for(i);
        let bytes = serde_json::to_vec(&ev).expect("encode");
        log.append(NewExecutionRecord {
            kind: chronos_log::ExecutionKind::Raw,

            session_id: session_id_typed.clone(),
            monotonic_ns: 10_000_500 + i * 1_000,
            payload: ExecutionPayload::new(bytes, "trace_event"),
            ..Default::default()
        })
        .expect("append");
    }

    log.record_gap(Gap::new(
        EventSeq::new(GAP_FROM),
        EventSeq::new(GAP_TO),
        GapReason::AdapterBufferOverflow,
        "rec-c1-8-uat03-fixture",
    ))
    .expect("record_gap");

    for i in 0..RECORDS_AFTER_GAP {
        let seq = GAP_TO + 1 + i;
        let ev = trace_event_for(seq);
        let bytes = serde_json::to_vec(&ev).expect("encode");
        log.append(NewExecutionRecord {
            kind: chronos_log::ExecutionKind::Raw,

            session_id: session_id_typed.clone(),
            monotonic_ns: 10_000_500 + seq * 1_000,
            payload: ExecutionPayload::new(bytes, "trace_event"),
            ..Default::default()
        })
        .expect("append");
    }
    log.flush().expect("flush");
}

/// Read a single `events_read{mode=query}` page and return the parsed
/// `completeness` envelope plus the events array.
async fn read_for_completeness(
    client: &mut McpTestClient,
    session_id: &str,
    cursor: Option<&str>,
    limit: usize,
) -> (Vec<serde_json::Value>, serde_json::Value, Option<String>) {
    let mut params = serde_json::json!({
        // R9.7 (drift #12): EventsReadKind is snake_case on the wire
        // (`"query"` lowercase, see `crates/chronos-services/src/output.rs:1631`
        // and contract tests at `crates/chronos-services/tests/events_read_kind.rs`).
        // The pre-C5.3.2 test used `"Query"` PascalCase, which the v2
        // `EventsReadParams` deserializer correctly rejects with:
        //   `unknown variant `Query`, expected `query` or `by_id``
        "mode": "query",
        "session_id": session_id,
        "limit": limit,
    });
    if let Some(c) = cursor {
        params["cursor"] = serde_json::Value::String(c.to_string());
    }
    let response = client.call_tool("events_read", params).await.expect("call");
    let events_holder = response.get("result").unwrap_or(&response);
    let events = events_holder
        .get("events")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let completeness = response
        .get("completeness")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let next_cursor = response
        .get("next_cursor")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    (events, completeness, next_cursor)
}

/// UAT-REC-C1-03 — exact.
///
/// GREEN since `m1-gap-segment-accounting-repair` (FIND-C1.8-01 fixed):
/// a durable segment carrying a `Gap` reopens cleanly, so this fixture
/// seeds the gap with the direct `record_gap` path and reads it back
/// over the MCP wire.
#[tokio::test]
async fn uat_rec_c1_03_forced_gap_reports_gap_detected() {
    let root = unique_root("uat03-gap");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create root");
    let db = root.join("sessions.redb");
    let session_id = "rec-c1-8-uat03-gap";

    seed_log_with_gap(&root, session_id);

    let mut client = McpTestClient::start_with_db_and_exec_log_root(db, root.clone())
        .await
        .expect("MCP server must start over pre-seeded log");

    // ---- Phase 1: first page (limit=100, before the gap). ----
    let (events_1, completeness_1, cursor_1) =
        read_for_completeness(&mut client, session_id, None, 100).await;
    assert_eq!(
        events_1.len(),
        100,
        "first page must contain 100 records (0..99); got {}",
        events_1.len()
    );
    // Page 1 covers only the pre-gap region; completeness.status
    // depends on whether the engine considers the gap to be in the
    // examined range. With limit=100 the engine's examined range is
    // [0, 100) — that does NOT cross the gap. Page 1's completeness
    // status may be `complete` for that range. We assert it is NOT
    // gap_detected at this point.
    if let Some(status) = completeness_1.get("status").and_then(|s| s.as_str()) {
        assert_ne!(
            status, "gap_detected",
            "page 1 (limit=100, range [0, 100)) must not report gap_detected; got {status}"
        );
    }

    // ---- Phase 2: a paginated read that DOES cross the gap. ----
    //
    // Reading page-2 from the page-1 cursor at limit=100 lands entirely
    // inside the gap (range [100, 100) — empty), and the
    // `completeness_for` semantics at
    // `crates/chronos-services/src/events_log_read.rs:480` correctly
    // report `complete` for an empty examined range that uses no
    // evidence ("An empty examined range used no evidence, so no gap
    // can contaminate it"). To exercise the gap, we issue a read whose
    // examined range spans the gap with a `from_seq` BEFORE the gap
    // and a `to_seq_exclusive` AFTER the gap. The wire API does not
    // accept explicit `from_seq`; we exercise the gap by reading
    // page-1 (records 0..99) plus a resumed page that lands on the
    // post-gap region (records 200..299). The transition between
    // page-1 and page-2 is what crosses the gap, and that crossing
    // is reflected in `completeness` on the page that contains the
    // post-gap records.
    let cursor_1 = cursor_1.expect("page 1 must have a next_cursor (more records after seq 99)");

    // Resume from page-1 cursor with limit=100. The engine reads
    // forward from seq 100, skips the empty gap range (100..200),
    // returns records 200..299 (the post-gap region). The examined
    // range covers [100, 200+) so the recorded gap at
    // [100, 200) intersects → `gap_detected`. This is the assertion
    // that exercises UAT-REC-C1-03 on the wire.
    let (events_resumed, completeness_resumed, cursor_after_resume) =
        read_for_completeness(&mut client, session_id, Some(&cursor_1), 100).await;
    assert_eq!(
        events_resumed.len(),
        RECORDS_AFTER_GAP as usize,
        "resume page must skip the gap and return {RECORDS_AFTER_GAP} post-gap records; got {}",
        events_resumed.len()
    );
    let status_resumed = completeness_resumed
        .get("status")
        .and_then(|s| s.as_str())
        .unwrap_or("");
    assert_eq!(
        status_resumed, "gap_detected",
        "resume page (range crosses the gap) must report gap_detected; got {status_resumed}"
    );
    let from_seq = completeness_resumed
        .get("from_seq")
        .and_then(|v| v.as_u64())
        .expect("completeness.from_seq must be a u64");
    let to_seq = completeness_resumed
        .get("to_seq_exclusive")
        .and_then(|v| v.as_u64())
        .expect("completeness.to_seq_exclusive must be a u64");
    assert!(
        from_seq <= GAP_FROM,
        "completeness.from_seq ({from_seq}) must be <= GAP_FROM ({GAP_FROM})"
    );
    assert!(
        to_seq >= GAP_TO,
        "completeness.to_seq_exclusive ({to_seq}) must be >= GAP_TO ({GAP_TO})"
    );
    assert!(
        cursor_after_resume.is_none(),
        "resume page (post-gap, dense, no more records after) must signal exhaustion (no next_cursor); got {:?}",
        cursor_after_resume
    );

    // ---- Phase 3: cursor non-destructive across the gap. ----
    // Re-read page 1 with the original (None) cursor and assert it
    // still returns the same 100 records. The gap never affected A's
    // first-page view.
    let (events_1_again, _, _) = read_for_completeness(&mut client, session_id, None, 100).await;
    assert_eq!(
        events_1.len(),
        events_1_again.len(),
        "page 1 must remain identical across the gap read"
    );

    // ---- Phase 4: explicit full-read with limit > total records. ----
    // A single huge read covers [0, 300) — that crosses the gap.
    // Status MUST be gap_detected, NEVER complete.
    let (events_all, completeness_all, cursor_all) =
        read_for_completeness(&mut client, session_id, None, 1_000).await;
    let status_all = completeness_all
        .get("status")
        .and_then(|s| s.as_str())
        .unwrap_or("");
    assert_ne!(
        status_all, "complete",
        "a single huge read covering the gap range MUST NOT report complete; got {status_all}"
    );
    assert_eq!(
        status_all, "gap_detected",
        "the full-read across the gap must report gap_detected; got {status_all}"
    );
    // The full-read events array should contain the pre-gap and
    // post-gap records, totalling TOTAL_RECORDS = 200.
    assert_eq!(
        events_all.len() as u64,
        TOTAL_RECORDS,
        "full-read across the gap must contain {} records (pre-gap + post-gap); got {}",
        TOTAL_RECORDS,
        events_all.len()
    );
    assert!(
        cursor_all.is_none(),
        "single full-read must signal exhaustion (no next_cursor); got {:?}",
        cursor_all
    );

    client.shutdown().await.expect("shutdown must succeed");
}

/// Negative-shape complement: a clean (no-gap) read of the same
/// 200-record range reports `complete`. This is the negative of
/// UAT-REC-C1-03 and the assertion the C1.7 third test made; we
/// keep it explicitly so the contrast is visible in the test corpus.
///
/// NOTE: this test uses the SAME session_id string but a different
/// `root` directory, so the durable logs are independent.
#[tokio::test]
async fn uat_rec_c1_03_clean_session_reports_complete_negative() {
    let root = unique_root("uat03-clean");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create root");
    let db = root.join("sessions.redb");
    let session_id = "rec-c1-8-uat03-clean";

    // Seed only the pre-gap range (no gap, no post-gap records).
    let session_id_typed = SessionId::new(session_id);
    let session_dir = root.join(session_id);
    let cfg = SegmentedConfig::with_dir(&session_dir);
    let log = SegmentedExecutionLog::open(session_id_typed.clone(), cfg).expect("open log");
    for i in 0..TOTAL_RECORDS {
        let ev = trace_event_for(i);
        let bytes = serde_json::to_vec(&ev).expect("encode");
        log.append(NewExecutionRecord {
            kind: chronos_log::ExecutionKind::Raw,

            session_id: session_id_typed.clone(),
            monotonic_ns: 10_000_500 + i * 1_000,
            payload: ExecutionPayload::new(bytes, "trace_event"),
            ..Default::default()
        })
        .expect("append");
    }
    log.flush().expect("flush");

    let mut client = McpTestClient::start_with_db_and_exec_log_root(db, root.clone())
        .await
        .expect("MCP server must start over clean pre-seeded log");

    let (events_all, completeness_all, _) =
        read_for_completeness(&mut client, session_id, None, 1_000).await;
    assert_eq!(
        events_all.len() as u64,
        TOTAL_RECORDS,
        "clean read must return all {} records; got {}",
        TOTAL_RECORDS,
        events_all.len()
    );
    let status = completeness_all
        .get("status")
        .and_then(|s| s.as_str())
        .unwrap_or("");
    assert_eq!(
        status, "complete",
        "clean session (no gap) MUST report complete (negative of UAT-REC-C1-03); got {status}"
    );

    client.shutdown().await.expect("shutdown must succeed");
}
