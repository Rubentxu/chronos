//! REC-C1.6 wire envelope UATs.
//!
//! Verifies the agent-visible shape of:
//! - `events_read` success: `retention` + `tail` keys appear flat at the top level.
//! - `events_read` CursorStale: BOTH the existing text content AND a second
//!   structured json content item carrying the two numbers (`error`,
//!   `requested_next_seq`, `retained_from_seq`) are surfaced.
//!
//! These complement the chronos-services unit tests in `rec_c1_6_wire_facts_tests`,
//! which pin the on-the-wire shape of the structs; this file pins what the MCP
//! server actually emits to the agent.

use chronos_sandbox::client::tools::McpTestClient;
use std::path::PathBuf;
use std::time::Duration;

fn unique_root() -> PathBuf {
    std::env::temp_dir().join(format!("chronos-wire-retention-{}", std::process::id()))
}

/// Drive a raw `tools/call` against the MCP server and return the full
/// `result` object. Unlike `McpTestClient::call_tool`, this preserves every
/// content item (we need both text + json content for the CursorStale arm).
async fn raw_tools_call(
    client: &mut McpTestClient,
    tool_name: &str,
    arguments: serde_json::Value,
) -> serde_json::Value {
    let params = serde_json::json!({
        "name": tool_name,
        "arguments": arguments,
    });
    client
        .call_with_timeout("tools/call", params, Duration::from_secs(30))
        .await
        .expect("tools/call transport")
        .get("result")
        .cloned()
        .expect("tool response has `result`")
}

/// Pre-seed a session directory + manifest with `retained_from` advanced,
/// then bootstrap the MCP server. Returns the client and the fixed session id.
async fn bootstrap_with_retained(
    root: &PathBuf,
    session_id: &str,
    retained_from: u64,
) -> McpTestClient {
    let db = root.join("sessions.redb");
    let _ = std::fs::remove_dir_all(root);
    std::fs::create_dir_all(root).expect("create root");

    let session_log_dir = root.join(session_id);
    std::fs::create_dir_all(&session_log_dir).expect("create session dir");
    let manifest = serde_json::json!({
        "schema_version": 2,
        "session_id": session_id,
        "retained_from": retained_from,
        "created_at_unix_ms": 0u64,
        // Unknown tail state — the bootstrap reopen path is satisfied by the
        // manifest alone (no replay required); tests that exercise the sealed
        // scenario live in restart_uat and don't need this harness.
        "tail_state": {
            "state": "unknown",
            "reason": "synthetic fixture for wire_retention_facts UATs"
        }
    });
    std::fs::write(
        session_log_dir.join("execution-log.manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .expect("write manifest");

    McpTestClient::start_with_db_and_exec_log_root(db, root.clone())
        .await
        .expect("start MCP server")
}

/// REC-C1.6 UAT-WIRE-1: success envelope flattens retention + tail.
///
/// The agent must see `retention` and `tail` as top-level keys (not buried
/// inside another field). The structs are derived Serialize, so any
/// `#[serde(flatten)]` failure would show up here as a missing key.
#[tokio::test]
async fn wire_1_success_envelope_flattens_retention_and_tail() {
    let root = unique_root().join("wire-1");
    let session_id = "11111111-1111-1111-1111-aaaaaaaaaaaa".to_string();
    let mut client = bootstrap_with_retained(&root, &session_id, 0).await;

    // Fresh cursor (seq#0): the read returns no records (empty log), but the
    // envelope MUST still carry retention/tail.
    let cursor_encoded = format!("ecv1:1:{}:{}:0", session_id.len(), session_id);
    let result = raw_tools_call(
        &mut client,
        "events_read",
        serde_json::json!({
            "mode": "query",
            "session_id": session_id,
            "cursor": cursor_encoded,
            "limit": 16,
        }),
    )
    .await;

    // The success path returns a single json content item whose text is the
    // serialized `EventsReadOutput::Query`.
    let content_array = result
        .get("content")
        .and_then(|c| c.as_array())
        .expect("content array");
    assert_eq!(content_array.len(), 1, "success envelope has one content");
    let inner: serde_json::Value = serde_json::from_str(
        content_array[0]
            .get("text")
            .and_then(|t| t.as_str())
            .expect("text field"),
    )
    .expect("inner JSON");

    assert_eq!(inner["mode"], "query");
    assert!(
        inner.get("retention").is_some(),
        "retention facts must be present at the top level: {inner}"
    );
    assert!(
        inner.get("tail").is_some(),
        "tail facts must be present at the top level: {inner}"
    );

    let retention = &inner["retention"];
    assert_eq!(retention["retained_from_seq"], 0);
    assert_eq!(retention["history_truncated"], false);

    let tail = &inner["tail"];
    // Bootstrap fixture uses tail_state=Unknown (no replay), so the wire form
    // presents state="unknown" and tail_seq=null — the honest answer.
    assert_eq!(tail["state"], "unknown");
    assert_eq!(tail["tail_seq"], serde_json::Value::Null);

    client.shutdown().await.ok();
    let _ = std::fs::remove_dir_all(&root);
}

/// REC-C1.6 UAT-WIRE-2: success envelope reflects advanced retained_from.
///
/// After bootstrap with retained_from=5, a fresh read's `retention` must carry
/// `retained_from_seq=5, history_truncated=true`. This pins the wire form of
/// the "facts" claim — these are NOT a policy, they are the boundary.
#[tokio::test]
async fn wire_2_success_envelope_reports_advanced_retained_from() {
    let root = unique_root().join("wire-2");
    let session_id = "22222222-2222-2222-2222-bbbbbbbbbbbb".to_string();
    const RETAINED_FROM: u64 = 5;
    let mut client = bootstrap_with_retained(&root, &session_id, RETAINED_FROM).await;

    let cursor_encoded = format!("ecv1:1:{}:{}:0", session_id.len(), session_id);
    // A fresh cursor at seq#0 would be CursorStale; advance it past the
    // boundary so the success path runs.
    let advanced = format!(
        "ecv1:1:{}:{}:{}",
        session_id.len(),
        session_id,
        RETAINED_FROM
    );
    let result = raw_tools_call(
        &mut client,
        "events_read",
        serde_json::json!({
            "mode": "query",
            "session_id": session_id,
            "cursor": advanced,
            "limit": 16,
        }),
    )
    .await;

    let inner = parse_first_content(&result).expect("first content parseable");
    let _ = cursor_encoded; // keep the variable to make the cursor shape explicit
    assert_eq!(inner["retention"]["retained_from_seq"], RETAINED_FROM);
    assert_eq!(inner["retention"]["history_truncated"], true);
    // Tail state was Unknown in the manifest; we expect it on the wire.
    assert_eq!(inner["tail"]["state"], "unknown");

    client.shutdown().await.ok();
    let _ = std::fs::remove_dir_all(&root);
}

/// REC-C1.6 UAT-WIRE-CURSOR-1: CursorStale envelope carries structured JSON.
///
/// When the agent supplies a cursor below `retained_from`, `events_read`
/// returns:
/// - a text content item (preserved for back-compat with restart_uat R2), and
/// - a SECOND json content item with `{error, requested_next_seq,
///   retained_from_seq}`.
///
/// Both items must be present in `result.content`; the json content's payload
/// must carry the EXACT numbers the agent asked about.
#[tokio::test]
async fn wire_cursor_1_stale_envelope_carries_structured_numbers() {
    let root = unique_root().join("wire-cursor-1");
    let session_id = "33333333-3333-3333-3333-cccccccccccc".to_string();
    const RETAINED_FROM: u64 = 7;
    let mut client = bootstrap_with_retained(&root, &session_id, RETAINED_FROM).await;

    // Cursor at seq#0, well below retained_from=7.
    let cursor_encoded = format!("ecv1:1:{}:{}:0", session_id.len(), session_id);
    let result = raw_tools_call(
        &mut client,
        "events_read",
        serde_json::json!({
            "mode": "query",
            "session_id": session_id,
            "cursor": cursor_encoded,
            "limit": 16,
        }),
    )
    .await;

    let content_array = result
        .get("content")
        .and_then(|c| c.as_array())
        .expect("content array");
    assert_eq!(
        content_array.len(),
        2,
        "CursorStale envelope MUST have both text + json content items: {result}"
    );

    // Item 0: text (preserved verbatim — restart_uat R2 parses this).
    let text = content_array[0]
        .get("text")
        .and_then(|t| t.as_str())
        .expect("text content[0]");
    assert!(
        text.contains("is stale"),
        "text content must still carry the stale message: {text}"
    );
    assert!(
        text.contains(&format!("{RETAINED_FROM}")),
        "text content must carry retained_from={RETAINED_FROM}: {text}"
    );

    // Item 1: json. The MCP text wrapper around json content is just the
    // pretty-printed JSON value as a string; parse it.
    let json_text = content_array[1]
        .get("text")
        .and_then(|t| t.as_str())
        .expect("json content[1] has text");
    let json_payload: serde_json::Value =
        serde_json::from_str(json_text).expect("json content is valid JSON");
    assert_eq!(json_payload["error"], "cursor_stale");
    assert_eq!(json_payload["requested_next_seq"], 0);
    assert_eq!(json_payload["retained_from_seq"], RETAINED_FROM);

    // The top-level isError flag should also be set so the agent's transport
    // sees this as an error (we explicitly chose CallToolResult::error, not
    // success).
    assert_eq!(
        result.get("isError").and_then(|v| v.as_bool()),
        Some(true),
        "CursorStale must surface as isError=true: {result}"
    );

    client.shutdown().await.ok();
    let _ = std::fs::remove_dir_all(&root);
}

/// Parse the first text content item as JSON (success-envelope helper).
fn parse_first_content(result: &serde_json::Value) -> Option<serde_json::Value> {
    let content = result.get("content")?.as_array()?;
    let first = content.first()?;
    let text = first.get("text")?.as_str()?;
    serde_json::from_str(text).ok()
}
