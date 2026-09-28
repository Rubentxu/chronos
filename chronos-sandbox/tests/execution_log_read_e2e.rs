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
//! STRUCTURE: one `#[tokio::test]` per server, many assertions. Sharing one
//! process across `#[tokio::test]` functions is unsound, not merely
//! inefficient: each test gets its OWN runtime, a server booted in test A has
//! its stdio bound to A's runtime, and that runtime is dropped when A finishes,
//! so a later test reusing the process observes a dead transport. Booting a
//! fresh `chronos-mcp` per test also costs seconds and hundreds of MB, and six
//! concurrent boots exhausted the box with spurious 120s `initialize`
//! timeouts that read as product failures and were not.
//!
//! The two tests below therefore cover complementary halves, each with its own
//! server: `execution_log_read_over_the_wire` for the contract surface and
//! fail-closed behaviour, `execution_log_read_serves_a_real_log_over_the_wire`
//! for the success path against persisted evidence.

use chronos_domain::trace::TraceEvent;
use chronos_domain::{EventData, EventType, MonotonicNs, SourceLocation};
use chronos_log::{
    ExecutionKind, ExecutionPayload, NewExecutionRecord, SegmentedConfig, SegmentedExecutionLog,
    SessionId,
};
use chronos_sandbox::client::tools::McpTestClient;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const SESSION: &str = "e2e-read-path-session";

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

/// Write a real, persisted execution log for `SESSION` under `root`.
///
/// Uses the production writer rather than hand-writing a manifest, so the
/// segments and retention metadata on disk are exactly what the server will
/// reopen. A hand-crafted manifest parses but leaves the log with no segments,
/// which is a different thing from a session that genuinely recorded events.
fn seed_execution_log(root: &Path, session: &str, events: u64) {
    let dir = root.join(session);
    std::fs::create_dir_all(&dir).expect("create execution-log dir");

    let session_id = SessionId::new(session);
    let log = SegmentedExecutionLog::open(session_id.clone(), SegmentedConfig::with_dir(&dir))
        .expect("open execution log for seeding");
    for i in 1..=events {
        log.append(NewExecutionRecord {
            session_id: session_id.clone(),
            kind: ExecutionKind::Raw,
            monotonic_ns: i * 1_000,
            // The reader decodes every record into a `TraceEvent`, and an
            // undecodable record fails the whole read closed rather than being
            // skipped. So the seeded payload must be a real `TraceEvent` under
            // the canonical `"trace_event"` tag; arbitrary bytes here would
            // make the log unreadable rather than empty.
            payload: ExecutionPayload::new(
                serde_json::to_vec(&trace_event(i)).expect("encode trace event"),
                "trace_event",
            ),
            ..Default::default()
        })
        .expect("append event");
    }
    log.flush().expect("flush seeded log");
    // Dropping `log` here releases the handle. The server must be able to
    // reopen the log purely from what is on disk; if it needed a live handle
    // from this process, the bootstrap would be doing nothing.
}

/// A minimal but well-formed `TraceEvent` for the seeded log.
fn trace_event(event_id: u64) -> TraceEvent {
    TraceEvent {
        event_id,
        timestamp_ns: MonotonicNs::from(event_id * 1_000),
        thread_id: 1,
        event_type: EventType::FunctionEntry,
        location: SourceLocation {
            function: Some("e2e_work".to_string()),
            ..SourceLocation::default()
        },
        data: EventData::Function {
            name: "e2e_work".to_string(),
            signature: None,
            symbol_id: None,
            invocation_id: None,
            parent_invocation_id: None,
        },
    }
}

fn temp_root(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "chronos-e2e-read-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("create temp root");
    p
}

/// The success path, against a session that genuinely recorded events.
///
/// Everything above exercises refusal. This exercises delivery: the four modes
/// must each return their own real result for the same persisted log, and
/// `poll` must advance its cursor so a second call resumes instead of
/// replaying. A read path that only ever fails closed is not a read path.
#[tokio::test]
async fn execution_log_read_serves_a_real_log_over_the_wire() {
    let root = temp_root("real");
    seed_execution_log(&root, SESSION, 5);

    let mcp_path = McpTestClient::resolve_mcp_path();
    let mut env = HashMap::new();
    env.insert(
        "CHRONOS_EXECUTION_LOG_DIR".to_string(),
        root.to_string_lossy().to_string(),
    );
    let mut client = McpTestClient::start_with_env(&mcp_path, &env)
        .await
        .expect("MCP server must start against the seeded log root");

    // ----------------------------------------------------------------- poll
    // `poll` must return the seeded events. Asserting on the count (not just
    // "no error") is what makes this a delivery test rather than a smoke test.
    let first = client
        .call_tool(
            "execution_log_read",
            json!({ "session_id": SESSION, "mode": "poll", "limit": 5 }),
        )
        .await
        .unwrap_or_else(|e| panic!("poll against a readable log must succeed, got: {e}"));
    let batch = &first["events"]
        .as_array()
        .unwrap_or_else(|| panic!("poll must return an events array, got: {first}"));
    assert_eq!(
        batch.len(),
        5,
        "poll must return every seeded event on the first read: {first}"
    );

    // The cursor must advance. A poll loop that re-seats at seq 0 forever can
    // never terminate and would replay the same events to an agent. After five
    // events the cursor must sit past the last one, not at the start.
    let cursor_after_first = first["next_cursor"]
        .as_str()
        .unwrap_or_else(|| panic!("poll must return a next_cursor, got: {first}"))
        .to_string();
    assert!(
        cursor_after_first.ends_with(":5"),
        "the resume cursor must sit past the five seeded events, \
         otherwise poll would replay them forever: {first}"
    );

    // Resuming from the returned cursor must NOT replay: the log is exhausted.
    let second = client
        .call_tool(
            "execution_log_read",
            json!({
                "session_id": SESSION,
                "mode": "poll",
                "limit": 5,
                "cursor": cursor_after_first,
            }),
        )
        .await
        .unwrap_or_else(|e| panic!("resumed poll must succeed, got: {e}"));
    assert_eq!(
        second["events"].as_array().map(|a| a.len()),
        Some(0),
        "a resumed poll past the end must return no events, not a replay: {second}"
    );

    // ------------------------------------------------------------ summarize
    // `summarize` must bucket the same evidence. It must agree with the raw
    // count rather than inventing a total of its own: the five seeded events
    // sit 1000ns apart, so a 1000ns bucket must yield five buckets of one.
    let summary = client
        .call_tool(
            "execution_log_read",
            json!({
                "session_id": SESSION,
                "mode": "summarize",
                "bucket_size_ns": 1_000,
            }),
        )
        .await
        .unwrap_or_else(|e| panic!("summarize against a readable log must succeed, got: {e}"));
    let summarized = summary["total_events"]
        .as_u64()
        .unwrap_or_else(|| panic!("summarize must report total_events, got: {summary}"));
    assert_eq!(
        summarized, 5,
        "summarize must count every seeded event: {summary}"
    );
    assert_eq!(
        summary["bucket_count"].as_u64(),
        Some(5),
        "five events 1000ns apart under a 1000ns bucket must be five buckets: {summary}"
    );

    // ---------------------------------------------------------------- rollup
    // `rollup` is per-invocation (per-thread), a different axis from the time
    // buckets above. It must still cover the same evidence.
    let rollup = client
        .call_tool(
            "execution_log_read",
            json!({ "session_id": SESSION, "mode": "rollup" }),
        )
        .await
        .unwrap_or_else(|e| panic!("rollup against a readable log must succeed, got: {e}"));
    let rolled = rollup["total_events"]
        .as_u64()
        .unwrap_or_else(|| panic!("rollup must report total_events, got: {rollup}"));
    assert_eq!(
        rolled, 5,
        "rollup must cover the same evidence as poll and summarize: {rollup}"
    );
    // All five events were recorded on thread 1, so there is exactly one
    // invocation. This distinguishes rollup from summarize, which buckets by
    // time and would have counted five.
    assert_eq!(
        rollup["invocation_count"].as_u64(),
        Some(1),
        "five events on one thread must roll up to one invocation: {rollup}"
    );

    // ------------------------------------------------------------- causality
    // Causality is the one mode that reads engine state, so it is the one most
    // likely to differ from the other three. It must still answer rather than
    // fail closed now that a readable log exists.
    match client
        .call_tool(
            "execution_log_read",
            json!({ "session_id": SESSION, "mode": "causality" }),
        )
        .await
    {
        Ok(v) => assert!(
            v.is_object(),
            "causality must return an object envelope when a log is readable, got: {v}"
        ),
        Err(e) => panic!("causality must not fail closed for a session with a readable log: {e}"),
    }

    // ------------------------------------------- a foreign cursor is still refused
    // The seeded session now exists, so this is the strongest form of the
    // binding check: the log IS readable, and the cursor STILL must be refused.
    // Earlier, an unregistered session made this pass trivially.
    assert!(
        client
            .call_tool(
                "execution_log_read",
                json!({
                    "session_id": SESSION,
                    "mode": "poll",
                    "cursor": "ecv1:1:9:session-a:0",
                }),
            )
            .await
            .is_err(),
        "a foreign cursor must be refused even when the target session is readable"
    );

    let _ = std::fs::remove_dir_all(&root);
}
