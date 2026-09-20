//! Native probe tools — sandbox coverage for the new MCP tool surface introduced
//! by REC-C3.3.3 (Tren B).
//!
//! These tests exercise the three new tool surface additions landed across
//! TASK-TB-F (`capture_session`) and TASK-TB-G (`probe_advance`, `probe_step`).
//! They live in their own file so the smoke_subset can be selected by name
//! (`native_probe_tools`) without dragging in heavier bucket-C suites.
//!
//! **Slice B (this file) is honest by construction (B9):** the three new tool
//! names (`capture_session`, `probe_advance`, `probe_step`) are NOT yet wired
//! into `chronos-mcp::server` at the time this scaffold is committed. Each
//! test therefore goes through `McpTestClient::call_tool` directly (via the
//! `DerefMut` impl that lets the test client be used as a mutable `McpSession`)
//! and asserts the *current* observable state — a method-not-found /
//! unknown-tool error — rather than a fabricated success. Once TASK-TB-F/G
//! land, the assertions below are extended to cover the real success and
//! error paths; the file itself remains additive (new `#[tokio::test]`
//! functions, never destructive edits to the existing RED assertions).
//!
//! Why `call_tool` and not typed wrappers? `McpTestClient` has typed wrappers
//! for `probe_start`, `probe_stop`, etc., but the three new tools have no
//! wrappers yet (the wrappers ship with TASK-TB-F/G alongside the server
//! handlers, per the task-graph "concrete impls ship in TASK-TB-F/G"
//! contract). Using `call_tool` keeps this scaffold compile-clean before the
//! wrappers exist.

use chronos_sandbox::client::tools::McpTestClient;

const TOOL_CAPTURE_SESSION: &str = "capture_session";
const TOOL_PROBE_ADVANCE: &str = "probe_advance";
const TOOL_PROBE_STEP: &str = "probe_step";

/// Boot the harness against the current binary. Returns the client or panics
/// with the harness's own diagnostic (the harness already prints CHRONOS_MCP_PATH
/// resolution steps and a clear "SpawnFailed" error when the binary is missing).
async fn boot_harness() -> McpTestClient {
    McpTestClient::start()
        .await
        .expect("Failed to start MCP server for native_probe_tools suite")
}

/// RED (slice B): the `capture_session` tool name must NOT yet be present in
/// the server's tool list. This proves the scaffold is honest about the
/// current state — once TASK-TB-F lands, this test is removed and replaced by
/// real success-path coverage in `capture_session_against_controlled_target`.
#[tokio::test]
async fn capture_session_tool_not_yet_wired_in_slice_b() {
    let mut client = boot_harness().await;

    // The server rejects unknown tools at the JSON-RPC layer with a
    // "method not found" / -32601 error code, surfaced by call_tool as
    // McpSandboxError. We only need to assert the call fails — not
    // pin the exact error code (that's an MCP protocol concern, not this
    // slice's contract).
    let result = client
        .call_tool(
            TOOL_CAPTURE_SESSION,
            serde_json::json!({
                "program": "/bin/true",
                "args": [],
            }),
        )
        .await;

    assert!(
        result.is_err(),
        "capture_session tool must NOT be wired at slice B (got Ok: {:?})",
        result.ok()
    );

    client.shutdown().await.ok();
}

/// RED (slice B): `probe_advance` is not yet wired. Same rationale as above;
/// replaced by real coverage in TASK-TB-G (`probe_advance_against_paused_succeeds`).
#[tokio::test]
async fn probe_advance_tool_not_yet_wired_in_slice_b() {
    let mut client = boot_harness().await;

    let result = client
        .call_tool(
            TOOL_PROBE_ADVANCE,
            serde_json::json!({
                "session_id": "non-existent-session-id",
            }),
        )
        .await;

    assert!(
        result.is_err(),
        "probe_advance tool must NOT be wired at slice B (got Ok: {:?})",
        result.ok()
    );

    client.shutdown().await.ok();
}

/// RED (slice B): `probe_step` is not yet wired. Same rationale as above;
/// replaced by real coverage in TASK-TB-G (`probe_step_against_paused_advances_pc`).
#[tokio::test]
async fn probe_step_tool_not_yet_wired_in_slice_b() {
    let mut client = boot_harness().await;

    let result = client
        .call_tool(
            TOOL_PROBE_STEP,
            serde_json::json!({
                "session_id": "non-existent-session-id",
            }),
        )
        .await;

    assert!(
        result.is_err(),
        "probe_step tool must NOT be wired at slice B (got Ok: {:?})",
        result.ok()
    );

    client.shutdown().await.ok();
}

/// Regression guard: existing probe tools continue to respond through the
/// harness. Independent of slice B's RED assertions — proves the harness
/// integration did not break the previously-shipped tool surface. Becomes
/// redundant once the suite grows real coverage but stays as a fast smoke.
#[tokio::test]
async fn existing_probe_status_tool_still_responds() {
    let mut client = boot_harness().await;

    // probe_status with a non-existent session is a legal call shape; the
    // server returns a structured error rather than an RPC-level unknown-tool
    // error. We only assert "did not fail at the dispatch layer".
    let result = client
        .call_tool(
            "probe_status",
            serde_json::json!({
                "session_id": "non-existent-session-id",
            }),
        )
        .await;

    // The server may legitimately error (probe_status against a missing
    // session returns a ServiceError, which the server maps to an
    // isError=true CallToolResult). We only assert the RPC dispatch
    // succeeded, which means the tool is wired.
    match result {
        Ok(_) => {}
        Err(e) => {
            // An RPC-level error (-32601 method not found, transport
            // failure, etc.) is the failure mode we care about. A
            // well-formed error payload from the server is Ok at the
            // RPC layer (call_tool returns Ok(value) where value.is_error).
            let s = format!("{e:?}");
            assert!(
                !s.contains("method not found") && !s.contains("-32601"),
                "probe_status must remain wired (got RPC-level error: {s})"
            );
        }
    }

    client.shutdown().await.ok();
}
