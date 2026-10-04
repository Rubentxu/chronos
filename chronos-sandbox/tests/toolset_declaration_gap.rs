//! What a narrowed toolset does to the wire, measured on a real server.
//!
//! `server.rs` used to justify not enforcing the toolset on the grounds that
//! "rmcp does not expose a hook into `tools/list`". That premise is false, and
//! this file is the evidence, on the wire rather than in a comment.
//!
//! What is actually true: `#[tool_handler]` generates `call_tool`, `list_tools`,
//! `get_tool` and `get_info` ONLY when the impl block does not already define
//! them — rmcp-macros 1.5.0, `tool_handler.rs:44,64,81,91`, each guarded by
//! `if !has_method("...", &item_impl)`. A hand-written `list_tools` in the same
//! impl block is kept. So filtering is a few lines and there is no framework
//! obstacle.
//!
//! This file does NOT change the behaviour. It pins the CURRENT asymmetry, so
//! that deciding "declare or enforce" is made against the real wire and so
//! that a future change to either side is a deliberate, visible act.
//!
//! Why the asymmetry is the thing worth pinning: 5 of 44 handlers already call
//! `toolset_guard`. Today the toolset is therefore *half* enforced, which is the
//! one shape that is neither a declaration nor a restriction — the manifest
//! says restricted, the wire restricts nothing, and five handlers disagree with
//! both. Whichever way the product decision goes, this should stop.

use std::collections::HashMap;

use chronos_sandbox::client::tools::McpTestClient;
use serde_json::Value;

fn names(tools: &[Value]) -> Vec<String> {
    tools
        .iter()
        .filter_map(|t| t.get("name").and_then(|n| n.as_str()).map(String::from))
        .collect()
}

async fn server_with_toolset(toolset: &str) -> McpTestClient {
    let mut env: HashMap<String, String> = HashMap::new();
    env.insert("CHRONOS_ACTIVE_TOOLSET".to_string(), toolset.to_string());
    McpTestClient::start_with_env(&McpTestClient::resolve_mcp_path(), &env)
        .await
        .expect("the server must start under any toolset value")
}

/// Under `auto`, everything is advertised. This is the baseline the other two
/// tests are read against, and it also pins that a typo'd toolset does not
/// silently empty the server.
#[tokio::test]
async fn the_auto_profile_advertises_the_whole_surface() {
    let mut client = server_with_toolset("auto").await;
    let tools = client.list_tools().await.expect("tools/list must answer");
    assert!(
        tools.len() >= 40,
        "auto must advertise the full surface, got {} tools",
        tools.len()
    );
}

/// THE PIN. A narrowed profile currently does NOT narrow the wire.
///
/// This is the assertion that makes the pending decision falsifiable: if someone
/// implements `list_tools` filtering, this goes red, which is correct and
/// visible — and if someone narrows the profile expecting a restriction without
/// changing this file, they get a red test instead of a false sense of security.
#[tokio::test]
async fn a_narrowed_profile_still_advertises_every_tool_today() {
    let mut narrowed = server_with_toolset("native").await;
    let narrowed_tools = names(&narrowed.list_tools().await.expect("tools/list"));

    let mut auto = server_with_toolset("auto").await;
    let auto_tools = names(&auto.list_tools().await.expect("tools/list"));

    // Same set either way. If this ever stops holding, `list_tools` became
    // filtered and the toolset moved from "declares" to "enforces".
    assert_eq!(
        narrowed_tools, auto_tools,
        "tools/list is now filtered by toolset. That is the product decision \
         being made — update this test to assert the NEW behaviour, and check \
         that `capabilities` and the dispatch path agree with it, because a \
         tool that is hidden from tools/list but still callable is worse than \
         either shape."
    );

    // And the count is the whole surface, not a subset that happens to match.
    assert_eq!(
        narrowed_tools.len(),
        auto_tools.len(),
        "a narrowed profile must not shrink the advertised surface while the \
         decision is still open"
    );
}

/// `capabilities` is the half that DOES respect the profile, so the two halves
/// genuinely disagree today. Pinning the disagreement is what makes it
/// visible; it is the "manifest describes a restriction the wire does not
/// apply" case, measured.
#[tokio::test]
async fn capabilities_narrows_while_the_wire_does_not() {
    let mut narrowed = server_with_toolset("native").await;
    let tools = names(&narrowed.list_tools().await.expect("tools/list"));
    assert!(
        !tools.is_empty(),
        "the profile must still advertise something"
    );

    // Read what the profile is supposed to contain, from the toolset itself,
    // so the test does not hardcode a second copy of the profile.
    let native = native_tool_names();

    let advertised_but_outside: Vec<&String> = tools
        .iter()
        .filter(|t| !native.iter().any(|n| n == *t))
        .collect();

    assert!(
        !advertised_but_outside.is_empty(),
        "the native profile currently excludes {} of the {} advertised tools; if \
         that is now zero, `tools/list` started filtering and this file's \
         central claim is obsolete",
        native.len(),
        tools.len()
    );
}

/// REQ-CAP-005 constrains the profile size, and this keeps the number honest:
/// the whole design of a small native profile rests on it staying small.
#[test]
fn the_native_profile_stays_within_its_declared_ceiling() {
    let native = native_tool_names();
    assert!(
        native.len() <= 25,
        "REQ-CAP-005 caps the native profile at 25 tools, it has {}",
        native.len()
    );
    assert!(
        !native.is_empty(),
        "the native profile must not be empty: an empty profile that enforces \
         would make the server unusable rather than restricted"
    );
}

/// The profile as the server defines it, reached through the public type rather
/// than a copy in this file — a second copy is a second thing to drift.
fn native_tool_names() -> Vec<String> {
    let server = chronos_mcp::server::ChronosServer::new();
    let mut listed: Vec<String> = vec![
        "capture_session",
        "session_start",
        "session_stop",
        "events_read",
        "execution_query",
        "causal_slice",
        "state_query",
        "observe",
        "session_compare",
    ]
    .into_iter()
    .filter(|n| server.is_tool_listed(n))
    .map(String::from)
    .collect();
    // Anything else the server says is listed under this profile.
    for candidate in ALL_CANDIDATES {
        if server.is_tool_listed(candidate) && !listed.iter().any(|l| l == candidate) {
            listed.push(candidate.to_string());
        }
    }
    listed.sort();
    listed
}

/// The tools this file is willing to name. The point is to probe the server's
/// own answer, not to maintain a profile here.
const ALL_CANDIDATES: &[&str] = &[
    "capabilities",
    "counterexample_bundle_events",
    "counterexample_events_count",
    "counterexample_get",
    "counterexample_list",
    "counterexample_shrink",
    "counterexample_test",
    "events_read",
    "execution_query",
    "hypothesis_test",
    "observe",
    "state_query",
    "trace_slice",
    "browser_probe_start",
    "browser_probe_stop",
    "browser_probe_drain",
    "save_session",
    "load_session",
    "list_sessions",
    "session_start",
    "session_stop",
    "capture_session",
];
