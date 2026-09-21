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

const FAKE_SESSION_ID: &str = "00000000-0000-0000-0000-000000000000";

#[tokio::test]
async fn test_observe_with_invalid_verb_returns_typed_error() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let response = client
        .call_tool(
            "observe",
            serde_json::json!({
                "verb": "frobnicate",
                "condition": {"kind": "uprobe", "binary_path": "/bin/ls", "symbol_name": "main"},
            }),
        )
        .await
        .expect("call_tool must return Ok (the error is in the body, not the RPC envelope)");

    // UAT-G0-02: "errores semánticos ... estables y tipados".
    // The server must report a JSON-RPC -32602 (invalid params) error,
    // not an empty result or a panic.
    let code = response
        .get("error")
        .and_then(|e| e.get("code"))
        .and_then(|c| c.as_i64());
    assert_eq!(
        code,
        Some(-32602),
        "expected -32602 (invalid params) for unknown verb, got {response}"
    );

    let msg = response
        .get("error")
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())
        .unwrap_or("");
    assert!(
        msg.contains("frobnicate") && msg.contains("create"),
        "error message should mention the offending verb and the valid set; got: {msg}"
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
    let response = client
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
        .await
        .expect("call_tool must return Ok; the error is in the body");

    // The error path here depends on the dispatcher implementation:
    // it can be either a JSON-RPC -32602 (invalid params: no such
    // session) or a typed body error. Both are acceptable — what is
    // NOT acceptable is an Ok result that silently registered nothing,
    // or a panic.
    let has_error = response.get("error").is_some();
    let has_status = response
        .get("is_error")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    assert!(
        has_error || has_status,
        "observe against missing session must surface a typed error; got {response}"
    );

    let err_str = response.to_string();
    assert!(
        err_str.contains(FAKE_SESSION_ID)
            || err_str.contains("not found")
            || err_str.contains("session"),
        "error must reference the missing session id or 'not found'; got: {err_str}"
    );

    // Give the server a moment to release the per-test store before the
    // next test starts — without this, `start()` for the next test can
    // race against the previous test's Drop.
    tokio::time::sleep(Duration::from_millis(50)).await;
}
