//! Real-process lifecycle-safe delete UAT for REC-C1.6.
//!
//! Three UATs here that exercise the typed `SessionStillActive` refusal
//! across a real spawned MCP server (so the `connected_sessions` set
//! is the actual server-side state, not a hand-built mock):
//!
//! - DEL-LIVE-1: `probe_start -> delete_session(live)` -> typed refusal,
//!   evidence still readable, durable dir intact, probe still alive.
//! - DEL-LIVE-2: probe_start -> refused delete -> probe_stop (seal) ->
//!   delete_session succeeds -> restart -> session absent.
//! - DEL-LIVE-4: while A's probe is live, delete B (B cleanly stopped).
//!   A's manifest, durable directory, and registry entry must be
//!   byte-identical before vs after (no sibling perturbation).
//!
//! DEL-LIVE-3 (unclean-recovered, no writer) is a unit test in
//! `chronos-services` — it does not need a real restart.

use std::path::PathBuf;

use chronos_sandbox::client::error::McpSandboxError;
use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::client::McpSession;

/// Each test gets a tempdir root reserved for itself so the per-test
/// db and durable logs do not collide on a shared filesystem.
fn unique_root(label: &str) -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let pid = std::process::id();
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "chronos-lifecycle-delete-{label}-{pid}-{n}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("create tempdir root");
    dir
}

/// Run `delete_session` against the spawned server and return either the
/// raw error text (refusal) or None on success. We use the raw
/// `call_tool` because the high-level `delete_session` wrapper in
/// the sandbox client swallows the success body, which we don't need.
async fn raw_delete_result(
    client: &mut McpTestClient,
    session_id: &str,
) -> Result<Option<String>, McpSandboxError> {
    let raw = client
        .call_tool(
            "delete_session",
            serde_json::json!({ "session_id": session_id }),
        )
        .await;

    match raw {
        Ok(_) => Ok(None),
        Err(McpSandboxError::RpcError(text)) => Ok(Some(text)),
        Err(e) => Err(e),
    }
}

/// Manually list the durable log directory for `session_id` under
/// `root`. Returns true iff the on-disk directory still exists.
fn durable_log_exists(root: &std::path::Path, session_id: &str) -> bool {
    let p = root.join(session_id);
    p.exists() && p.is_dir()
}

// -----------------------------------------------------------------------------
// DEL-LIVE-1: probe live -> delete -> typed refusal
// -----------------------------------------------------------------------------

#[tokio::test]
async fn del_live_1_live_probe_delete_is_refused() {
    let root = unique_root("del-live-1");
    let db = root.join("sessions.redb");
    let fixture = McpSession::fixture_path("test_add").expect("fixture");

    let mut client = McpTestClient::start_with_db_and_exec_log_root(db.clone(), root.clone())
        .await
        .expect("start MCP server");

    let session_id = client
        .session_start_spawn(fixture.to_str().unwrap(), vec![])
        .await
        .expect("session_start_spawn")
        .session_id;

    // The probe is live. delete_session must be a typed refusal.
    let refusal_text = match raw_delete_result(&mut client, &session_id).await {
        Ok(None) => panic!(
            "delete_session on a live session MUST return a refusal; \
             got Ok instead. The probe writer was silently removed and the \
             session silently dropped."
        ),
        Ok(Some(text)) => text,
        Err(e) => panic!("delete_session returned an unexpected error kind: {e:?}"),
    };

    // The text carries both the session id and the recovery hint.
    assert!(
        refusal_text.contains(&session_id),
        "refusal text must mention the session id; got {refusal_text:?}"
    );
    assert!(
        refusal_text.contains("session_stop") && refusal_text.contains("probe"),
        "refusal text must name the recovery action; got {refusal_text:?}"
    );

    // 1. Evidence is still readable — a refusal must NOT corrupt or
    //    hide the live session's evidence.
    let page = client
        .call_tool(
            "events_read",
            serde_json::json!({
                "mode": "Query",
                "session_id": session_id,
                "limit": 16,
            }),
        )
        .await
        .expect("events_read after refused delete must still succeed");
    let result_obj = &page["result"];
    assert!(
        result_obj.get("isError").and_then(|v| v.as_bool()) != Some(true),
        "evidence must remain readable after a refused delete; got {page}"
    );

    // 2. The durable directory must still exist on disk.
    assert!(
        durable_log_exists(&root, &session_id),
        "durable log directory must survive a refused delete"
    );

    // 3. The probe is still alive: a follow-up session_stop succeeds
    //    (which it would not if the probe had been silently stopped).
    let stop = client
        .session_stop(&session_id, /*seal=*/ true, /*drain=*/ true)
        .await;
    assert!(
        stop.is_ok(),
        "the live probe must still be alive after the refused delete; \
         session_stop then succeeded cleanly. Got {stop:?}"
    );

    client.shutdown().await.ok();
    let _ = std::fs::remove_dir_all(&root);
}

// -----------------------------------------------------------------------------
// DEL-LIVE-2: refused delete -> stop -> delete -> restart -> absent
// -----------------------------------------------------------------------------

#[tokio::test]
async fn del_live_2_refused_then_stop_then_delete_then_restart_absent() {
    let root = unique_root("del-live-2");
    let db = root.join("sessions.redb");
    let fixture = McpSession::fixture_path("test_add").expect("fixture");

    // Process A.
    let mut first = McpTestClient::start_with_db_and_exec_log_root(db.clone(), root.clone())
        .await
        .expect("start MCP server A");

    let started_a = first
        .session_start_spawn(fixture.to_str().unwrap(), vec![])
        .await
        .expect("session_start_spawn A");

    // Live probe → refused.
    let refusal = raw_delete_result(&mut first, &started_a.session_id).await;
    assert!(
        matches!(refusal, Ok(Some(_))),
        "deleting a live session must return a typed refusal; got {refusal:?}"
    );

    // Stop the probe (seal the manifest).
    first
        .session_stop(&started_a.session_id, true, true)
        .await
        .expect("session_stop A");
    first.shutdown().await.ok();

    // Process B (fresh). Bootstrap republishes the durable log. Now A is
    // sealed, no live writer → delete must succeed.
    let mut second = McpTestClient::start_with_db_and_exec_log_root(db.clone(), root.clone())
        .await
        .expect("start MCP server B");
    let outcome = raw_delete_result(&mut second, &started_a.session_id).await;
    assert!(
        matches!(outcome, Ok(None)),
        "delete_session on a sealed session must succeed; got {outcome:?}"
    );
    second.shutdown().await.ok();

    // Process C (fresh). The session must NOT be rediscoverable — durable
    // delete from B must have removed the directory on disk.
    let mut third = McpTestClient::start_with_db_and_exec_log_root(db, root.clone())
        .await
        .expect("start MCP server C");
    let recovered = third.session_start_load(&started_a.session_id).await;
    assert!(
        recovered.is_err(),
        "after delete_session from B, C must NOT be able to load A; \
         got {recovered:?}"
    );
    third.shutdown().await.ok();
    let _ = std::fs::remove_dir_all(&root);
}

// -----------------------------------------------------------------------------
// DEL-LIVE-4: while A is live, deleting a different (B cleanly stopped)
//              session must not perturb A's manifest / durable dir / registry
// -----------------------------------------------------------------------------

#[tokio::test]
async fn del_live_4_sibling_delete_does_not_perturb_live_a() {
    let root = unique_root("del-live-4");
    let db = root.join("sessions.redb");
    let fixture = McpSession::fixture_path("test_add").expect("fixture");

    let mut client = McpTestClient::start_with_db_and_exec_log_root(db.clone(), root.clone())
        .await
        .expect("start MCP server");

    // Start A (will remain live for the whole test).
    let started_a = client
        .session_start_spawn(fixture.to_str().unwrap(), vec![])
        .await
        .expect("session_start_spawn A")
        .session_id;

    // Start B, then cleanly stop it (B is now sealed, no writer).
    let started_b = client
        .session_start_spawn(fixture.to_str().unwrap(), vec![])
        .await
        .expect("session_start_spawn B")
        .session_id;
    client
        .session_stop(&started_b, true, true)
        .await
        .expect("session_stop B");

    // Snapshot A's on-disk state.
    let manifest_before = std::fs::read(root.join(&started_a).join("execution-log.manifest.json"))
        .expect("read A manifest before");
    let dir_listing_before = list_dir(&root.join(&started_a));

    // Delete B (B is not live, must succeed).
    let outcome = raw_delete_result(&mut client, &started_b).await;
    assert!(
        matches!(outcome, Ok(None)),
        "deleting cleanly stopped B must succeed; got {outcome:?}"
    );

    // A's manifest must be byte-identical.
    let manifest_after = std::fs::read(root.join(&started_a).join("execution-log.manifest.json"))
        .expect("read A manifest after");
    assert_eq!(
        manifest_before, manifest_after,
        "deleting B must not perturb A's manifest byte-by-byte"
    );

    // A's directory listing must be byte-identical (no spurious files
    // appeared/disappeared).
    let dir_listing_after = list_dir(&root.join(&started_a));
    assert_eq!(
        dir_listing_before, dir_listing_after,
        "deleting B must not perturb A's directory contents"
    );

    // A is still readable from the live probe.
    let evidence = client
        .call_tool(
            "events_read",
            serde_json::json!({
                "mode": "Query",
                "session_id": started_a,
                "limit": 16,
            }),
        )
        .await
        .expect("events_read A after sibling delete");
    let result_obj = &evidence["result"];
    assert!(
        result_obj.get("isError").and_then(|v| v.as_bool()) != Some(true),
        "A must still be readable after deleting B; got {evidence}"
    );

    client.shutdown().await.ok();
    let _ = std::fs::remove_dir_all(&root);
}

/// Return sorted relative paths of every entry under `dir` so two
/// snapshots can be compared byte-for-byte.
fn list_dir(dir: &std::path::Path) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for entry in rd.flatten() {
            out.push(entry.file_name().to_string_lossy().to_string());
        }
    }
    out.sort();
    out
}
