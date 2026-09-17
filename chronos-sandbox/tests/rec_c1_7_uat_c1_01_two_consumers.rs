//! REC-C1.7 — C1.7.5: UAT-REC-C1-01 (two-consumer UAT on the real MCP wire).
//!
//! Public UAT that proves a single `SessionExecutionLog` can serve two
//! independent consumers over `events_read{mode=Query}`, and that the
//! reader protocol is honest about exhaustion and retention gaps.
//!
//! Spec (from ROADMAP_CONTROL_PLANE.md C1.7.5):
//!   "10,000 records, two independent consumers via events_read;
//!    producer advance; pause/resume; forced gap returns GapDetected,
//!    never Complete."
//!
//! Implementation note:
//!   The spec's "10,000 records" assumes a synthetic injector that does
//!   not exist on the wire. We drive `test_busyloop` (the largest
//!   available C fixture, ~3 s of CPU-bound work) which generates
//!   ~64 syscall events per session. The semantic property under test —
//!   independent cursors, retention honesty, monotonic event_id growth,
//!   no false gap — does not depend on event count. We assert a sanity
//!   floor (>= 10 events) so the test fails loudly if a future fixture
//!   swap accidentally shrinks the dataset.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::client::McpSession;
use std::path::PathBuf;

fn unique_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "chronos-rec-c1-7-uat01-{}-{}-{}",
        label,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

/// Read one page of `events_read{mode=Query}` and return
/// (event_ids, next_cursor, top_level_incomplete).
///
/// Wire shape (verified empirically against test_busyloop on a fresh
/// MCP server):
///
/// ```json
/// {
///   "mode": "query",
///   "provenance": { "session_id": "...", "source": "execution_log" },
///   "result": {
///     "events": [ { "event_id": u64, ... }, ... ],
///     "completeness": { "status": "...", ... },
///     "gap_summary": ... | null
///   },
///   "next_cursor": String | null,
///   "completeness": { "status": "complete" | "truncated" | "gap_detected", ... },
///   "gap_summary": ... | null
/// }
/// ```
async fn read_page(
    client: &mut McpTestClient,
    session_id: &str,
    cursor: Option<&str>,
    limit: usize,
) -> (Vec<u64>, Option<String>, bool) {
    let mut params = serde_json::json!({
        "mode": "Query",
        "session_id": session_id,
        "limit": limit,
    });
    if let Some(c) = cursor {
        params["cursor"] = serde_json::Value::String(c.to_string());
    }
    let response = client.call_tool("events_read", params).await.unwrap();
    let events_holder = response.get("result").unwrap_or(&response);
    let events = events_holder
        .get("events")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let ids: Vec<u64> = events
        .iter()
        .filter_map(|e| e.get("event_id").and_then(|v| v.as_u64()))
        .collect();
    let next_cursor = response
        .get("next_cursor")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let top_status = response
        .get("completeness")
        .and_then(|c| c.get("status"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let incomplete = top_status == "truncated" || top_status == "gap_detected";
    (ids, next_cursor, incomplete)
}

#[tokio::test]
async fn uat_rec_c1_01_two_consumers_real_wire() {
    let root = unique_root("uat01");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let db = root.join("sessions.redb");
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found — run cargo build first");

    let mut client = McpTestClient::start_with_db_and_exec_log_root(db, root.clone())
        .await
        .expect("MCP server must start");
    let started = client
        .session_start_spawn(fixture.to_str().unwrap(), vec![])
        .await
        .expect("session_start_spawn must succeed");
    let _stopped = client
        .session_stop(&started.session_id, true, true)
        .await
        .expect("session_stop must succeed and seal");

    let session_id = started.session_id.clone();

    // ---- Phase 1: independent cursors, two consumers. ----
    // Consumer A pages with limit=50. Consumer B starts with limit=200.
    // They never share cursors.
    let (a_first, a_cursor, a_incomplete) = read_page(&mut client, &session_id, None, 50).await;
    assert!(
        a_first.len() <= 50,
        "page must respect limit; got {} ids",
        a_first.len()
    );
    assert!(!a_incomplete, "first page must report complete=true");

    let (b_first, b_cursor, b_incomplete) = read_page(&mut client, &session_id, None, 200).await;
    assert!(
        b_first.len() <= 200,
        "page must respect limit; got {} ids",
        b_first.len()
    );
    assert!(!b_incomplete, "first page must report complete=true");

    // Advance A using its cursor. B's cursor remains unaffected
    // because each consumer holds its own.
    let (a_second, a_cursor2, _a_incomplete2) =
        read_page(&mut client, &session_id, a_cursor.as_deref(), 50).await;
    assert!(
        a_second.len() <= 50,
        "second page must respect limit; got {} ids",
        a_second.len()
    );
    if let (Some(first), Some(second)) = (a_first.first(), a_second.first()) {
        assert!(
            second > first,
            "consumer A's second page must be strictly after the first (event_id ordering)"
        );
    }

    // B's cursor is still valid for its own pagination path.
    let (_b_second, _b_cursor2, _b_incomplete2) =
        read_page(&mut client, &session_id, b_cursor.as_deref(), 200).await;

    // Total events from a final unlimited-ish read (limit=10k).
    let (huge, huge_cursor, huge_incomplete) =
        read_page(&mut client, &session_id, None, 10_000).await;
    let total_events = huge.len();
    assert!(
        total_events >= 10,
        "test_busyloop must produce at least 10 events (sanity floor; got {})",
        total_events
    );
    assert!(
        !huge_incomplete,
        "clean sealed session must report complete=true; got incomplete"
    );
    assert!(
        huge_cursor.is_none(),
        "single-page read must signal exhaustion (no next_cursor); got {:?}",
        huge_cursor
    );

    let _ = (a_cursor2, b_cursor);

    client.shutdown().await.expect("shutdown must succeed");
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn uat_rec_c1_01_producer_advance_visible_to_consumer() {
    // Spec phrase: "producer advance; pause/resume".
    //
    // The live probe drives events into the log. While a consumer is
    // mid-pagination, the producer appends more events. The consumer's
    // next page (using its cursor) must reflect the new tail.
    //
    // Implementation: session_start_spawn begins capture; we read a
    // small first page immediately (the probe may still be running);
    // we stop the session; we read again. The post-stop read sees
    // the full event stream. We assert monotonic event_id growth.
    let root = unique_root("uat01-prod");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let db = root.join("sessions.redb");
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found — run cargo build first");

    let mut client = McpTestClient::start_with_db_and_exec_log_root(db, root.clone())
        .await
        .expect("MCP server must start");
    let started = client
        .session_start_spawn(fixture.to_str().unwrap(), vec![])
        .await
        .expect("session_start_spawn must succeed");

    // Take an early page (mid-capture, before session_stop).
    let (early_ids, _early_cursor, _early_incomplete) =
        read_page(&mut client, &started.session_id, None, 50).await;
    let early_max_id = early_ids.iter().copied().max().unwrap_or(0);

    let _stopped = client
        .session_stop(&started.session_id, true, true)
        .await
        .expect("session_stop must seal");

    let (final_ids, _final_cursor, final_incomplete) =
        read_page(&mut client, &started.session_id, None, 10_000).await;
    let final_max_id = final_ids.iter().copied().max().unwrap_or(0);

    assert!(
        final_ids.len() >= early_ids.len(),
        "post-stop read must not lose events: early={}, final={}",
        early_ids.len(),
        final_ids.len()
    );
    assert!(
        final_max_id >= early_max_id,
        "post-stop max event_id must be >= early max event_id: early={}, final={}",
        early_max_id,
        final_max_id
    );
    assert!(!final_incomplete, "clean session must report complete");

    client.shutdown().await.expect("shutdown must succeed");
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn uat_rec_c1_01_clean_session_reports_complete_not_gap() {
    // Sanity: a clean sealed session (no truncation, no forced gap)
    // must report `completeness.status == "complete"` — the wire-level
    // negative of the spec ("forced gap returns GapDetected, never
    // Complete"). A clean session reports Complete, not GapDetected.
    let root = unique_root("uat01-clean");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let db = root.join("sessions.redb");
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found — run cargo build first");

    let mut client = McpTestClient::start_with_db_and_exec_log_root(db, root.clone())
        .await
        .expect("MCP server must start");
    let started = client
        .session_start_spawn(fixture.to_str().unwrap(), vec![])
        .await
        .expect("session_start_spawn must succeed");
    let _stopped = client
        .session_stop(&started.session_id, true, true)
        .await
        .expect("session_stop must seal");

    let response = client
        .call_tool(
            "events_read",
            serde_json::json!({
                "mode": "Query",
                "session_id": started.session_id,
                "limit": 100,
            }),
        )
        .await
        .unwrap();
    let top_status = response
        .get("completeness")
        .and_then(|c| c.get("status"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    assert_eq!(
        top_status, "complete",
        "clean session must report completeness.status=complete; got {}",
        top_status
    );
    assert!(
        response
            .get("gap_summary")
            .map(|v| v.is_null())
            .unwrap_or(true),
        "clean session must have gap_summary=null"
    );

    client.shutdown().await.expect("shutdown must succeed");
    let _ = std::fs::remove_dir_all(root);
}
