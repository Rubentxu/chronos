//! G0.2 — observe uprobe migration tests.
//!
//! These tests verify that:
//!
//! 1. `observe` rejects unknown verbs with a typed semantic error
//!    (UAT-G0-02). The MCP wrapper translates serde's "unknown variant"
//!    into a stable error response — not a panic, not a 500.
//!
//! 2. `observe(verb=create, condition.kind=uprobe)` against a session
//!    that does not exist surfaces a typed error (UAT-G0-02: "errores
//!    semánticos de capacidad/no encontrado estables y tipados; no exigir
//!    prefijos v1").
//!
//! Both tests run against the real `chronos-mcp` binary, exercising the
//! JSON-RPC layer end-to-end (initialize → tools/list → tools/call).
//!
//! Error model used by the sandbox client (`call_tool` in
//! `chronos-sandbox/src/client/rpc.rs:153-165`):
//!
//!   - JSON-RPC `error` envelope failures (e.g. schema mismatch) are
//!     returned as `Err(McpSandboxError::RpcError)` because rmcp's
//!     transport layer converts them to RpcError before our `call_tool`
//!     wrapper sees them.
//!   - MCP `result.isError: true` failures (e.g. probe not found) are
//!     also returned as `Err(McpSandboxError::RpcError)` with the
//!     `content[0].text` as the error string.
//!
//! So the test asserts the error TYPE (RpcError) and that the diagnostic
//! text contains the expected discriminator fragments.
//!
//! Note: the legacy `client.probe_inject()` wrapper (migrated in m7-02
//! from `probe_inject` → `observe(verb=create, condition.kind=uprobe)`)
//! is already covered by pre-existing tests in
//! `chronos-sandbox/tests/probe_inject.rs::test_probe_inject_*`
//! (`without_root`, `nonexistent_session`, `invalid_symbol`,
//! `before_pid_known`). We intentionally do not duplicate that
//! coverage here — G0.2 focuses on the negative contract of `observe`
//! against the *direct* MCP tool surface.

use std::time::Duration;

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::McpSandboxError;

const FAKE_SESSION_ID: &str = "00000000-0000-0000-0000-000000000000";

#[tokio::test]
async fn test_observe_with_invalid_verb_returns_typed_error() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let result = client
        .call_tool(
            "observe",
            serde_json::json!({
                "verb": "frobnicate",
                "condition": {"kind": "uprobe", "binary_path": "/bin/ls", "symbol_name": "main"},
            }),
        )
        .await;

    // The sandbox client converts MCP errors to `McpSandboxError::RpcError`
    // before returning (see module docs). We must accept that here.
    let err_str = match result {
        Ok(value) => {
            panic!("expected RpcError for unknown verb, but call_tool returned Ok: {value}")
        }
        Err(McpSandboxError::RpcError(msg)) => msg,
        Err(e) => panic!("expected RpcError for unknown verb, got {e:?}"),
    };

    // UAT-G0-02: "errores semánticos ... estables y tipados".
    // The diagnostic must mention the offending verb and the valid set
    // (5 snake_case variants), matching the wire smoke G0.2 §11 evidence.
    assert!(
        err_str.contains("frobnicate") && err_str.contains("create"),
        "error message should mention the offending verb and the valid set; got: {err_str}"
    );

    // Give the server a moment to release the per-test store before the
    // next test starts — without this, `start()` for the next test can
    // race against the previous test's Drop.
    tokio::time::sleep(Duration::from_millis(50)).await;
}

#[tokio::test]
async fn test_observe_uprobe_against_nonexistent_session_returns_typed_error() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Issue observe(create, uprobe) against a session that does not
    // exist. The server must report a typed error (no panic), and the
    // error message must mention the missing session id so the operator
    // can diagnose.
    //
    // Wire shape uses the external tag discriminator
    // (`{"scope": "session", "session_id": "..."}`) that
    // `ObserveScopeWire` expects per its `#[serde(tag = "scope")]`.
    let result = client
        .call_tool(
            "observe",
            serde_json::json!({
                "verb": "create",
                "condition": {
                    "kind": "uprobe",
                    "binary_path": "/bin/ls",
                    "symbol_name": "main",
                },
                "action": "record",
                "retention": "drained",
                "scope": {"scope": "session", "session_id": FAKE_SESSION_ID},
            }),
        )
        .await;

    let err_str = match result {
        Ok(value) => {
            panic!("expected RpcError for missing session, but call_tool returned Ok: {value}")
        }
        Err(McpSandboxError::RpcError(msg)) => msg,
        Err(e) => panic!("expected RpcError for missing session, got {e:?}"),
    };

    // The server's diagnostic must reference the missing session id.
    assert!(
        err_str.contains(FAKE_SESSION_ID) || err_str.contains("not found"),
        "error must reference the missing session id or 'not found'; got: {err_str}"
    );

    // Give the server a moment to release the per-test store before the
    // next test starts — without this, `start()` for the next test can
    // race against the previous test's Drop.
    tokio::time::sleep(Duration::from_millis(50)).await;
}
