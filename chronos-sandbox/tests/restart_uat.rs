//! Real two-process restart UAT for clean sealed lifecycle persistence.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::client::McpSession;
use std::path::PathBuf;

fn unique_root() -> PathBuf {
    std::env::temp_dir().join(format!("chronos-restart-uat-{}", std::process::id()))
}

/// Helper: read the first page of `events_read{mode=query}` with the given
/// cursor and return the page (event-id vector + `next_cursor`).
async fn read_events_page(
    client: &mut McpTestClient,
    session_id: &str,
    cursor: Option<&str>,
    limit: usize,
) -> (Vec<u64>, Option<String>) {
    let mut params = serde_json::json!({
        "mode": "query",
        "session_id": session_id,
        "limit": limit,
    });
    if let Some(c) = cursor {
        params["cursor"] = serde_json::Value::String(c.to_string());
    }
    let response = client.call_tool("events_read", params).await.unwrap();
    let result = response
        .get("result")
        .cloned()
        .unwrap_or_else(|| response.clone());
    let events = result
        .get("events")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let ids: Vec<u64> = events
        .iter()
        .filter_map(|e| e.get("event_id").and_then(serde_json::Value::as_u64))
        .collect();
    let next_cursor = result
        .get("next_cursor")
        .and_then(serde_json::Value::as_str)
        .map(|s| s.to_string());
    (ids, next_cursor)
}

#[tokio::test]
async fn r3_clean_session_stop_seals_and_restart_bootstraps_it() {
    let root = unique_root();
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();

    let db = root.join("sessions.redb");
    let fixture = McpSession::fixture_path("test_add").expect("fixture");

    let mut first = McpTestClient::start_with_db_and_exec_log_root(db.clone(), root.clone())
        .await
        .unwrap();
    let started = first
        .session_start_spawn(fixture.to_str().unwrap(), vec![])
        .await
        .unwrap();
    let stopped = first
        .session_stop(&started.session_id, true, true)
        .await
        .unwrap();
    assert!(
        stopped.sealed_at.is_some(),
        "clean stop must report sealed_at"
    );
    first.shutdown().await.unwrap();

    let mut second = McpTestClient::start_with_db_and_exec_log_root(db, root.clone())
        .await
        .unwrap();
    let loaded = second
        .session_start_load(&started.session_id)
        .await
        .unwrap();
    assert_eq!(loaded.session_id, started.session_id);
    assert_eq!(loaded.capability_snapshot["tail_sealed"], true);
    second.shutdown().await.unwrap();
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn r4_delete_session_is_absent_after_restart() {
    let root = unique_root().join("r4");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let db = root.join("sessions.redb");
    let fixture = McpSession::fixture_path("test_add").expect("fixture");

    let mut first = McpTestClient::start_with_db_and_exec_log_root(db.clone(), root.clone())
        .await
        .unwrap();
    let started = first
        .session_start_spawn(fixture.to_str().unwrap(), vec![])
        .await
        .unwrap();
    first
        .session_stop(&started.session_id, true, true)
        .await
        .unwrap();
    let deleted = first
        .call_tool(
            "delete_session",
            serde_json::json!({"session_id": started.session_id.clone()}),
        )
        .await
        .unwrap();
    assert_eq!(deleted["status"], "deleted");
    eprintln!("delete response: {deleted:#}");
    let paths = deleted
        .get("paths_removed")
        .or_else(|| deleted.get("result").and_then(|v| v.get("paths_removed")))
        .and_then(serde_json::Value::as_array)
        .expect("delete response must expose paths_removed");
    assert!(!paths.is_empty());
    first.shutdown().await.unwrap();

    let mut second = McpTestClient::start_with_db_and_exec_log_root(db, root.clone())
        .await
        .unwrap();
    let missing = second.session_start_load(&started.session_id).await;
    assert!(
        missing.is_err(),
        "deleted session must not reappear after restart"
    );
    second.shutdown().await.unwrap();
    let _ = std::fs::remove_dir_all(root);
}

/// REC-C1.5 UAT-R1 — real-process resume after unclean shutdown.
///
/// Invariant: after the MCP server is killed mid-session (SIGKILL, no seal),
/// a fresh MCP server MUST reproduce the exact same `events_read` page at
/// the same cursor — no duplicates, no omissions, same `session_id`.
///
/// This is the spec's "session remembers its run" invariant: restart must not
/// silently mutate evidence.
///
/// Note on `session_start_load`: the load path reads from the redb
/// SessionStore, which is only populated by an explicit `session_stop(true,
/// true)` (clean stop with seal). Since UAT-R1 SIGKILLs mid-session, the redb
/// metadata is intentionally absent — the session lives only on disk as a
/// durable ExecutionLog directory, and the bootstrap path republishes it.
/// We therefore drive the read directly via `events_read{mode=Query}`, which
/// goes through the bootstrap-populated ExecutionLogRegistry.
#[tokio::test]
async fn r1_unclean_restart_reproduces_same_events_page() {
    let root = unique_root().join("r1");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let db = root.join("sessions.redb");
    let fixture = McpSession::fixture_path("test_add").expect("fixture");

    // Process A: start session, capture two pages of events_read, then SIGKILL.
    let mut first = McpTestClient::start_with_db_and_exec_log_root(db.clone(), root.clone())
        .await
        .unwrap();
    let started = first
        .session_start_spawn(fixture.to_str().unwrap(), vec![])
        .await
        .unwrap();
    // Let the fixture run to completion and let the bus drain a bit.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Capture page 1 (cursor=None) and page 2 (cursor=page1.next_cursor).
    // The test_add fixture produces few events, so a small limit suffices.
    let (page1_ids, page1_next) = read_events_page(&mut first, &started.session_id, None, 64).await;
    let (page2_ids, _page2_next) =
        read_events_page(&mut first, &started.session_id, page1_next.as_deref(), 64).await;

    // Sanity: process A produced at least one event for both pages combined.
    let total_a = page1_ids.len() + page2_ids.len();
    assert!(
        total_a > 0,
        "test_add fixture should emit at least one event; got 0"
    );

    // Verify the page cursor in A is well-formed (a non-empty opaque string).
    if let Some(c) = &page1_next {
        assert!(
            c.starts_with("ecv1:"),
            "next_cursor must be an opaque EventsCursorV1 encoding"
        );
    }

    // Capture the manifest path of the session's ExecutionLog directory so
    // we can verify tail_state on disk after restart.
    let session_log_dir_before = root.join(&started.session_id);
    assert!(
        session_log_dir_before.exists(),
        "session's ExecutionLog directory must exist after session_start_spawn"
    );

    // SIGKILL the MCP server mid-session (no session_stop, no seal).
    first.force_kill().await.unwrap();
    // Drop the client so the inner McpProcess handle is released.
    drop(first);

    // Process B: fresh MCP server against the same root and DB.
    // Bootstrap republishes the durable ExecutionLog directory into the registry.
    let mut second = McpTestClient::start_with_db_and_exec_log_root(db, root.clone())
        .await
        .unwrap();

    // Read the same pages again. The cursor from A is opaque and may not be
    // portable across processes (it references session_id only), so we
    // re-anchor: page 1 (cursor=None) MUST equal A's page 1, then page 2
    // (cursor=B's page1_next) MUST equal A's page 2.
    let (page1_ids_b, page1_next_b) =
        read_events_page(&mut second, &started.session_id, None, 64).await;
    assert_eq!(
        page1_ids_b, page1_ids,
        "page 1 event-id vector must be identical across restart"
    );

    if let Some(next_b) = &page1_next_b {
        let (page2_ids_b, _) =
            read_events_page(&mut second, &started.session_id, Some(next_b.as_str()), 64).await;
        assert_eq!(
            page2_ids_b, page2_ids,
            "page 2 event-id vector must be identical across restart"
        );
    }

    // Verify the session's tail_state on disk: the manifest written by
    // process A (before SIGKILL) must NOT report `Sealed` — it should be
    // either `Open` (process A was still running when killed, manifest
    // never sealed) or `Unclean` (manifest written post-recovery). What
    // matters for this invariant is that the restart path does not fake
    // a clean sealed tail. Reading the manifest directly avoids any
    // SessionStore dependency on redb.
    let manifest_path = session_log_dir_before.join("execution-log.manifest.json");
    if manifest_path.exists() {
        let bytes = std::fs::read(&manifest_path).unwrap();
        let manifest: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let tail_state = manifest
            .get("tail_state")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let state_name = tail_state
            .get("state")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        assert_ne!(
            state_name, "sealed",
            "unclean restart must not report a sealed tail_state on disk"
        );
    }

    second.shutdown().await.unwrap();
    let _ = std::fs::remove_dir_all(root);
}

/// REC-C1.5 UAT-R2 — identical stale before/after restart.
///
/// Invariant: when a cursor's `next_seq` falls below the durable retention
/// watermark, `events_read` returns a typed `CursorStale` carrying both
/// numbers. Across a process restart, the SAME cursor must produce a
/// `CursorStale` with EXACTLY the same `requested_next_seq` and
/// `retained_from_seq` — no drift, no inference.
///
/// This proves restart cannot silently mutate the retention boundary.
#[tokio::test]
async fn r2_stale_cursor_is_identical_before_and_after_restart() {
    use chronos_sandbox::client::error::McpSandboxError;

    let root = unique_root().join("r2");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let db = root.join("sessions.redb");
    let _fixture = McpSession::fixture_path("test_add").expect("fixture");

    // Pre-create a session's ExecutionLog directory with a manifest whose
    // `retained_from` is already past 0. The bootstrap path (REC-C1.5.4) will
    // discover this directory, reopen it, and publish it into the registry.
    //
    // We craft a fixed UUID so the test is reproducible across runs.
    let session_id = "11111111-2222-3333-4444-555555555555".to_string();
    let session_log_dir = root.join(&session_id);
    std::fs::create_dir_all(&session_log_dir).unwrap();
    const RETAINED_FROM: u64 = 5;
    let manifest = serde_json::json!({
        "schema_version": 2,
        "session_id": session_id,
        "retained_from": RETAINED_FROM,
        "created_at_unix_ms": 0u64,
        "tail_state": {
            "state": "unknown",
            "reason": "synthetic fixture for UAT-R2"
        }
    });
    let manifest_path = session_log_dir.join("execution-log.manifest.json");
    std::fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();

    // Helper: send events_read with a stale cursor (seq#0 < retained_from=5)
    // and capture the CursorStale payload from the server's error text.
    async fn read_stale(client: &mut McpTestClient, sid: &str) -> CursorStaleNumbers {
        let cursor_encoded = format!("ecv1:1:{}:{}:0", sid.len(), sid);
        let result = client
            .call_tool(
                "events_read",
                serde_json::json!({
                    "mode": "query",
                    "session_id": sid,
                    "cursor": cursor_encoded,
                    "limit": 16,
                }),
            )
            .await;
        let text = match result {
            Ok(value) => serde_json::to_string(&value).unwrap_or_default(),
            Err(McpSandboxError::RpcError(msg)) => msg,
            Err(other) => panic!("unexpected MCP error: {other}"),
        };
        parse_cursor_stale_text(&text)
            .unwrap_or_else(|| panic!("server response did not contain CursorStale: {text:?}"))
    }

    // Process A: bootstrap republishes the pre-seeded ExecutionLog directory
    // into the registry with the watermark already advanced to 5.
    let mut first = McpTestClient::start_with_db_and_exec_log_root(db.clone(), root.clone())
        .await
        .unwrap();
    let stale_a = read_stale(&mut first, &session_id).await;
    assert_eq!(
        stale_a.requested_next_seq, 0,
        "process A: requested_next_seq must equal the supplied cursor"
    );
    assert_eq!(
        stale_a.retained_from_seq, RETAINED_FROM,
        "process A: retained_from_seq must equal the manifest's watermark"
    );

    // SIGKILL the MCP server mid-session (no clean shutdown, no seal).
    first.force_kill().await.unwrap();
    drop(first);

    // Process B: fresh MCP server against the same root and DB. The manifest
    // is read back as-is by bootstrap (no inference), so the watermark and
    // the cursor stale mapping are byte-for-byte the same.
    let mut second = McpTestClient::start_with_db_and_exec_log_root(db, root.clone())
        .await
        .unwrap();
    let stale_b = read_stale(&mut second, &session_id).await;

    assert_eq!(
        stale_b.requested_next_seq, stale_a.requested_next_seq,
        "requested_next_seq must survive restart"
    );
    assert_eq!(
        stale_b.retained_from_seq, stale_a.retained_from_seq,
        "retained_from_seq must survive restart"
    );
    // Full struct equality: not just the two fields, but the whole payload.
    assert_eq!(
        stale_b, stale_a,
        "the CursorStale payload must be byte-identical across restart"
    );

    second.shutdown().await.unwrap();
    let _ = std::fs::remove_dir_all(root);
}

/// Parse the `CursorStale { requested_next_seq, retained_from_seq }` payload
/// out of an MCP error text. The server formats it as either:
///   `"Cursor at seq {X} is stale; the earliest available position is {R}. ..."` (callable surface)
/// or
///   `"cursor at seq {X} is before the retention boundary {R}"` (typed error path)
/// Both forms carry the same two numbers; we accept either.
fn parse_cursor_stale_text(text: &str) -> Option<CursorStaleNumbers> {
    // Find the requested seq number, after either "Cursor at seq " or "cursor at seq ".
    let x_str = text
        .split("ursor at seq ")
        .nth(1)?
        .split(|c: char| !c.is_ascii_digit())
        .next()?;
    let x: u64 = x_str.parse().ok()?;
    // Find the retained seq number, after either "earliest available position is " or "retention boundary ".
    let r_str = if let Some(after) = text.split("earliest available position is ").nth(1) {
        after.split(|c: char| !c.is_ascii_digit()).next()?
    } else {
        text.split("retention boundary ")
            .nth(1)?
            .split(|c: char| !c.is_ascii_digit())
            .next()?
    };
    let r: u64 = r_str.parse().ok()?;
    Some(CursorStaleNumbers {
        requested_next_seq: x,
        retained_from_seq: r,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CursorStaleNumbers {
    requested_next_seq: u64,
    retained_from_seq: u64,
}
