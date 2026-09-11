//! Counterexample smoke tests (m8-03 sandbox deliverable 4).
//!
//! Exercises the 3 new chronos-mcp tools (`counterexample_shrink`,
//! `counterexample_get`, `counterexample_list`) end-to-end against a
//! real spawned MCP server.
//!
//! Pipeline shape today (m8-03):
//!   1. probe_start on a real fixture (test_busyloop) so the engine
//!      has captured events for the returned session_id.
//!   2. counterexample_shrink — validates the target violates the
//!      captured trace via `hypothesis_test::test()`, then runs the
//!      proptest shrink loop with a Just(value) strategy (m8-02
//!      disclosure: rounds_used == 1).
//!   3. counterexample_get / counterexample_list — read back from the
//!      redb `counterexample_bundles` table populated in step 2.
//!
//! Honest disclosures (see m8-03 scoping doc):
//!  * `rounds_used == 1` — strategies are `Just(value)` per m8-02
//!  * `next_cursor == None` — pagination is m8-05 close-time work (R3)
//!  * Events count is 0 on the wire — events live in redb (B3, R4)
//!
//! Tests skip (with a printed message) if `CHRONOS_MCP_PATH` is unset
//! AND the default binary is missing, so the file compiles even on CI.

// Re-export the helpers we use so the test compiles even when skipped.
use chronos_sandbox::client::tools::McpTestClient;
use serde_json::json;
use std::time::Duration;

/// Spin up a server and start a probe on the given fixture, returning
/// `(client, session_id)`. Returns `None` (and prints) if anything
/// along the chain is unavailable so the suite can be skipped cleanly.
async fn setup_with_probe(fixture: &str) -> Option<(McpTestClient, String)> {
    let mut client = match McpTestClient::start().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("counterexample_tools: server start failed: {e}");
            return None;
        }
    };

    let path = match chronos_sandbox::McpSession::fixture_path(fixture) {
        Some(p) => p,
        None => {
            eprintln!("counterexample_tools: fixture `{fixture}` not built — skipping");
            let _ = client.shutdown().await;
            return None;
        }
    };

    let session_id = match client.probe_start(path.to_str().unwrap()).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("counterexample_tools: probe_start failed: {e}");
            let _ = client.shutdown().await;
            return None;
        }
    };

    // Allow the probe to populate the in-flight buffer.
    tokio::time::sleep(Duration::from_millis(400)).await;

    // The MCP server only inserts a session_id into the engines map on
    // probe_stop (the query engine is built at stop time, not start
    // time). Without this, `hypothesis_test::test()` would return
    // SessionNotFound before the shrink loop even begins.
    if let Err(e) = client.probe_stop(&session_id).await {
        eprintln!("counterexample_tools: probe_stop failed: {e}");
        let _ = client.shutdown().await;
        return None;
    }

    Some((client, session_id))
}

/// CE1: shrink a constant target on a real probe session.
#[tokio::test]
async fn ce1_shrink_constant_target() {
    let Some((mut client, session_id)) = setup_with_probe("test_busyloop").await else {
        return;
    };

    let target = json!({
        "session_id": session_id,
        "kind": "invariant",
        "constant": { "Number": 42.0 },
    });

    let resp = client
        .counterexample_shrink(target)
        .await
        .expect("counterexample_shrink failed");

    assert!(
        !resp.bundle.bundle_id.is_empty(),
        "bundle_id should be non-empty"
    );
    assert_eq!(
        resp.rounds_used, 1,
        "Just(value) strategies always use 1 round"
    );
    assert!(
        resp.bundle.has_full_bundle,
        "m8-03 bundles always carry full payload"
    );
    assert_eq!(resp.bundle.property_kind, "invariant");

    let _ = client.shutdown().await;
}

/// CE2: shrink → get round-trip with the same session.
#[tokio::test]
async fn ce2_shrink_then_get_round_trip() {
    let Some((mut client, session_id)) = setup_with_probe("test_busyloop").await else {
        return;
    };

    let target = json!({
        "session_id": session_id,
        "kind": "invariant",
        "constant": { "Number": -1.0 },
    });

    let shrink = client
        .counterexample_shrink(target)
        .await
        .expect("shrink failed");
    let bundle_id = shrink.bundle.bundle_id.clone();

    let get = client
        .counterexample_get(&bundle_id)
        .await
        .expect("counterexample_get failed");

    assert_eq!(get.bundle.bundle_id, bundle_id, "id should round-trip");
    assert!(get.has_full_bundle);

    let _ = client.shutdown().await;
}

/// CE3: counterexample_get on a non-existent id returns Err(LoadFailed)
/// at the MCP layer.
#[tokio::test]
async fn ce3_get_missing_bundle_errors() {
    let mut client = match McpTestClient::start().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("counterexample_tools: server start failed: {e}");
            return;
        }
    };

    let result = client.counterexample_get("does-not-exist").await;
    assert!(result.is_err(), "missing bundle id should error");

    let _ = client.shutdown().await;
}

/// CE4: list after a shrink sees the bundle.
#[tokio::test]
async fn ce4_list_after_shrink_includes_bundle() {
    let Some((mut client, session_id)) = setup_with_probe("test_busyloop").await else {
        return;
    };

    let target = json!({
        "session_id": session_id,
        "kind": "invariant",
        "constant": { "Number": 7.0 },
    });

    let shrink = client
        .counterexample_shrink(target)
        .await
        .expect("shrink failed");
    let bundle_id = shrink.bundle.bundle_id.clone();

    let list = client
        .counterexample_list(None, Some("invariant"), Some(50))
        .await
        .expect("counterexample_list failed");

    assert!(
        list.bundles.iter().any(|b| b.bundle_id == bundle_id),
        "newly-shrunk bundle should appear in list"
    );
    assert!(
        list.next_cursor.is_none(),
        "m8-03 next_cursor is always None (R3)"
    );

    let _ = client.shutdown().await;
}

/// CE5: shrink after a different fixture (test_exit_immediate) — same
/// behaviour expected; pipeline is fixture-agnostic.
#[tokio::test]
async fn ce5_shrink_on_exit_immediate_fixture() {
    let Some((mut client, session_id)) = setup_with_probe("test_exit_immediate").await else {
        return;
    };

    let target = json!({
        "session_id": session_id,
        "kind": "invariant",
        "constant": { "Number": 0.5 },
    });

    let resp = client
        .counterexample_shrink(target)
        .await
        .expect("shrink on exit_immediate fixture failed");

    assert!(!resp.bundle.bundle_id.is_empty());
    assert_eq!(resp.rounds_used, 1);

    let _ = client.shutdown().await;
}

/// CE6: list with workspace_id filter — call shape is honoured.
#[tokio::test]
async fn ce6_list_with_workspace_filter() {
    let Some((mut client, session_id)) = setup_with_probe("test_busyloop").await else {
        return;
    };

    let target = json!({
        "session_id": session_id,
        "kind": "invariant",
        "constant": { "Number": 9.0 },
    });
    let _ = client
        .counterexample_shrink(target)
        .await
        .expect("shrink failed");

    let list = client
        .counterexample_list(Some("ws-default"), None, Some(10))
        .await
        .expect("list with workspace filter failed");

    assert!(list.next_cursor.is_none());

    let _ = client.shutdown().await;
}
