//! End-to-end acceptance for the M10 Execution Explorer read path.
//!
//! These tests drive the REAL production surface: they spawn the `chronos-mcp`
//! binary as a child process and talk JSON-RPC over stdio, exactly as an agent
//! would. This is deliberately not a unit test of the handler — the handler can
//! be correct while the wiring (registration, schema, routing, serialization)
//! is broken at the transport boundary.
//!
//! The in-crate tests in `server.rs` call `execution_log_read` as a Rust
//! method. That proves the logic. It does not prove an agent can reach it.
//!
//! Wire contract, as implemented by `RpcClient::call_tool`:
//!
//!   - `isError: true`  -> `Err(McpSandboxError::RpcError(<first text block>))`
//!   - `isError: false` -> `Ok(<first text block parsed as JSON>)`
//!
//! So "the call failed closed" is observed as an `Err` whose message is the
//! registry's own reason.
//!
//! STRUCTURE: one `#[tokio::test]`, one server, many assertions. This is
//! deliberate on two counts.
//!
//!   1. Every `#[tokio::test]` gets its OWN runtime. A server booted in test A
//!      has its stdio tasks bound to A's runtime, so once A finishes, that
//!      runtime is dropped and any later test reusing the process observes a
//!      dead transport. Sharing a process across `#[tokio::test]` functions is
//!      therefore unsound, not merely inefficient.
//!   2. Booting a fresh `chronos-mcp` per test costs seconds and hundreds of
//!      MB. Six concurrent boots exhausted the box and produced spurious 120s
//!      `initialize` timeouts that read as product failures and were not.
//!
//! One process, one runtime, sequential assertions: no cross-runtime reuse,
//! no resource contention, ~1s of startup total. Every assertion still runs,
//! and the first failure still panics with its own message.

use chronos_sandbox::client::tools::McpTestClient;
use serde_json::{json, Value};

/// Fetch the advertised tool list. `McpTestClient` derefs to `McpSession`,
/// which owns the raw RPC client.
async fn tools_list(client: &mut McpTestClient) -> Vec<Value> {
    client
        .list_tools()
        .await
        .expect("tools/list must succeed over the transport")
}

#[tokio::test]
async fn execution_log_read_over_the_wire() {
    let mut client = McpTestClient::start().await.expect("MCP server must start");

    // ---------------------------------------------------------------- discovery
    // The tool must be discoverable through `tools/list`, the only way an agent
    // learns it exists. A tool that routes but is not listed is invisible, and
    // invoking it directly would never reveal that.
    let tools = tools_list(&mut client).await;
    let names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
        .collect();
    assert!(
        names.contains(&"execution_log_read"),
        "execution_log_read must appear in tools/list; {} tools listed",
        names.len()
    );

    // ------------------------------------------------------------------- schema
    // The declared input schema must document the contract an agent codes
    // against. A mismatch between the Rust params struct and the published
    // schema makes a well-formed agent call fail at the transport, which is
    // invisible to every in-crate test.
    let entry = tools
        .iter()
        .find(|t| t.get("name").and_then(|n| n.as_str()) == Some("execution_log_read"))
        .expect("execution_log_read must be listed");
    let schema = entry
        .get("inputSchema")
        .cloned()
        .expect("every tool must publish an inputSchema");
    let props = schema
        .get("properties")
        .and_then(|p| p.as_object())
        .expect("inputSchema must be an object with properties");

    for required in ["session_id", "mode", "bucket_size_ns", "limit"] {
        assert!(
            props.contains_key(required),
            "inputSchema must document '{required}'; documents: {:?}",
            props.keys().collect::<Vec<_>>()
        );
    }

    // The `mode` discriminator must advertise the four documented views, so an
    // agent can select one without parsing description prose.
    let mode_text = format!("{}", props.get("mode").expect("mode property"));
    for variant in ["poll", "summarize", "rollup", "causality"] {
        assert!(
            mode_text.contains(variant),
            "inputSchema must document mode='{variant}'; mode schema: {mode_text}"
        );
    }

    // ------------------------------------------------------------- fail-closed
    // The single most important acceptance property of the read path: an agent
    // that sees an empty batch with no error concludes the session is quiet,
    // which is a false negative about evidence.
    match client
        .call_tool(
            "execution_log_read",
            json!({ "session_id": "no-such-session-anywhere", "mode": "poll" }),
        )
        .await
    {
        Ok(v) => panic!("unknown session must fail closed over the wire, got success: {v}"),
        Err(e) => {
            let msg = format!("{e}");
            assert!(
                !msg.contains("\"event_count\":0"),
                "fail-closed response must not masquerade as an empty success: {msg}"
            );
            assert!(
                msg.contains("unavailable") && msg.contains("no ExecutionLog"),
                "fail-closed response must carry the registry's own reason: {msg}"
            );
        }
    }

    // ------------------------------------------------------------- mode routing
    // All four modes must be routable. With no log present every mode must fail
    // closed, including `causality`: the service requires a readable log before
    // it will assert anything about causality.
    for mode in ["poll", "summarize", "rollup", "causality"] {
        let outcome = client
            .call_tool(
                "execution_log_read",
                json!({ "session_id": "no-such-session-anywhere", "mode": mode }),
            )
            .await;
        assert!(
            outcome.is_err(),
            "mode '{mode}' without a log must fail closed, got: {:?}",
            outcome.unwrap_or(Value::Null)
        );
    }

    // ------------------------------------------------------------ input rejection
    // An unknown mode must be rejected, never silently defaulted. Silent
    // defaulting would let an agent believe it received a view it never asked
    // for and did not get.
    assert!(
        client
            .call_tool(
                "execution_log_read",
                json!({ "session_id": "whatever", "mode": "definitely-not-a-mode" }),
            )
            .await
            .is_err(),
        "an unknown mode must not produce a success envelope"
    );

    // A cursor minted for one session must not be silently reinterpreted
    // against another. Replaying it would anchor the agent to the wrong log and
    // return plausible-looking but wrong evidence.
    //
    // The wire format is `ecv1:{version}:{session_len}:{session}:{seq}`, so a
    // genuine cross-session cursor is well-formed and clears every structural
    // check in `parse`. It is rejected ONLY by the session-binding check in
    // `decode_for_session`. A malformed string would be rejected earlier, in
    // `parse`, and would prove nothing about binding — so this must be a real
    // encoding, or the test would pass for the wrong reason.
    //
    // "session-a" is 9 bytes, which is what the length field encodes.
    let foreign_cursor = "ecv1:1:9:session-a:0";
    let outcome = client
        .call_tool(
            "execution_log_read",
            json!({
                "session_id": "session-b",
                "mode": "poll",
                "cursor": foreign_cursor,
            }),
        )
        .await;
    match outcome {
        Ok(v) => panic!("a cursor minted for another session must be refused, got: {v}"),
        Err(e) => {
            let msg = format!("{e}");
            // Both session ids must appear: that is what distinguishes a
            // binding rejection from a structural parse failure or a generic
            // "no such log" error, which would name neither.
            assert!(
                msg.contains("session-a") && msg.contains("session-b"),
                "cross-session rejection must name both sessions, otherwise \
                 the test cannot tell binding rejection from a parse or \
                 missing-log error: {msg}"
            );
        }
    }
}
