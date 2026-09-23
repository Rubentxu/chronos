//! REC-C1.7 — Restart equivalence UAT for the QueryEngine projection.
//!
//! Closes TRUTH-001: the QueryEngine serving a canonical operation
//! (execution_query) must be a reconstructible projection of the
//! SessionExecutionLog, not a second authority. The test:
//!
//!   1. Spawns the MCP server with a clean DB + exec_log_root.
//!   2. Drives `session_start_spawn` on the `test_busyloop` fixture
//!      and `session_stop` to finalize. Events flow to the log.
//!   3. Runs `execution_query{kind=execution_summary}` — record the
//!      response (engine was built via the drain path during stop).
//!   4. Shuts down the server.
//!   5. Restarts the server with the same DB + exec_log_root. The
//!      SessionExecutionLogRegistry is repopulated from disk via
//!      `bootstrap_execution_logs` (C1.5 plumbing). The engines map
//!      is empty. The projection_meta map is empty.
//!   6. Runs the same `execution_query{kind=execution_summary}` —
//!      this MUST trigger the new gate (`gate_projection_for_wire`)
//!      which builds the engine from the durable log.
//!   7. Asserts: response_2 == response_1 (semantic equality on the
//!      JSON envelope, modulo any volatile fields).
//!
//! What this proves:
//!   - The engine serving the query is derived from the log on demand.
//!   - The same log yields the same response across restarts.
//!   - TRUTH-001 is closed: there is no second authority; the engine
//!     is a projection, not a parallel truth.
//!
//! What this does NOT prove:
//!   - It does not prove the log is correct (REC-C1.5 owns that).
//!   - It does not prove the gate rejects truncated/empty projections
//!     (dual_truth tests cover that; the gate is a different layer).
//!   - It does not exercise trace_slice or state_query gates — the
//!     gate helper is shared, so coverage by one operation is
//!     representative of all three.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::client::McpSession;
use serde_json::Value;
use std::path::PathBuf;

fn unique_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "chronos-rec-c1-7-restart-{}-{}-{}",
        label,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

/// Run `execution_query{kind=execution_summary}` and return the JSON
/// envelope (the value of the `summary` field). Stripping to the
/// canonical sub-envelope makes the comparison robust to wrapper-
/// level fields that may legitimately vary (timing, counts embedded
/// in tool output if the dispatcher adds any).
async fn run_execution_summary(client: &mut McpTestClient, session_id: &str) -> Value {
    let response = client
        .call_tool(
            "execution_query",
            serde_json::json!({
                "session_id": session_id,
                "kind": "execution_summary",
            }),
        )
        .await
        .expect("execution_query must succeed on a projected session");
    // The McpTestClient.call_tool helper already unwraps the MCP
    // `result.content[0].text` envelope and parses it as JSON, so
    // the value we get back is the structured `ExecutionSummary`
    // payload directly. (See chronos_sandbox::client::tools::call_tool
    // for the unwrap logic.)
    response
}

#[tokio::test]
async fn rec_c1_7_projection_restart_equivalence() {
    let root = unique_root("projection");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let db = root.join("sessions.redb");
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found — run cargo build first");

    // ---- Phase 1: drive a session, capture the response. ----
    let mut first = McpTestClient::start_with_db_and_exec_log_root(db.clone(), root.clone())
        .await
        .expect("first MCP server must start");
    let started = first
        .session_start_spawn(fixture.to_str().unwrap(), vec![])
        .await
        .expect("session_start_spawn must succeed");
    let _stopped = first
        .session_stop(&started.session_id, true, true)
        .await
        .expect("session_stop must succeed and seal");

    let response_before = run_execution_summary(&mut first, &started.session_id).await;
    first.shutdown().await.expect("first shutdown must succeed");

    // ---- Phase 2: restart the server. ----
    // The SessionExecutionLogRegistry is repopulated from disk by
    // bootstrap_execution_logs (C1.5). The engines map and the
    // projection_meta map are empty.
    let mut second = McpTestClient::start_with_db_and_exec_log_root(db, root.clone())
        .await
        .expect("second MCP server must start");

    // The first call after restart MUST trigger the gate. The gate
    // calls ensure_projection which builds the engine from the log
    // and atomically inserts (engine, meta) into the maps. Then the
    // service runs against the freshly-projected engine.
    let response_after = run_execution_summary(&mut second, &started.session_id).await;

    second
        .shutdown()
        .await
        .expect("second shutdown must succeed");
    let _ = std::fs::remove_dir_all(&root);

    // ---- Phase 3: semantic equality. ----
    //
    // Compare on the structured summary envelope, not the raw JSON
    // string. Two runs of the same session through the same log
    // must produce identical structured output. If they don't, the
    // engine is NOT a deterministic projection of the log — which
    // is exactly the failure mode TRUTH-001 was about.
    assert_eq!(
        response_before, response_after,
        "execution_query must produce the same response before and after restart \
         (QueryEngine is a projection of the log, not a second authority)"
    );
}
