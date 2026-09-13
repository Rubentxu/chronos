//! The MCP server must not start against a store it cannot open.
//!
//! Regression coverage for
//! `FIND-M9-73-SILENT-IN-MEMORY-FALLBACK-MASKS-STORE-OPEN-FAILURE`, closed by
//! m9-75. Before that cycle `ChronosServer::open_default_store` logged a warning
//! and used an in-memory store whenever the configured store could not be opened
//! (locked by another process, corrupt, permission denied). A server asked to
//! open a locked store therefore started successfully with an empty one:
//! `session_save` reported success, `session_list` returned nothing, and the
//! health check stayed green — silent data loss.
//!
//! These tests exercise the *policy* through the real binary, because that is the
//! only place it is observable: a fail-closed server exits before the transport
//! starts, so a client can tell "your store is there" from "the server quietly
//! started with an empty one" by the exit status alone.
//!
//! The unopenable fixture is a **directory** at the store path: it exists (so the
//! server cannot claim the path was merely new) and can never be opened as a redb
//! database.

use chronos_sandbox::client::tools::{McpSession, McpTestClient};
use std::collections::HashMap;
use std::path::PathBuf;

/// Create a directory that will be used *as* the store path.
///
/// A directory can never be opened as a redb database, and it exists, which is
/// exactly the "store is there but unusable" case the policy is about.
fn unopenable_store_path(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "chronos-sandbox-store-open-{}-{}-{}",
        tag,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("create unopenable store fixture");
    dir
}

/// The overrides a scenario wants the server started with.
fn env_with(overrides: &[(&str, String)]) -> HashMap<String, String> {
    overrides
        .iter()
        .map(|(key, value)| ((*key).to_string(), value.clone()))
        .collect()
}

/// Fail closed: exit before serving, name the path and the opt-in, and do not
/// pretend to have a working store.
#[tokio::test]
async fn test_server_refuses_to_start_when_the_configured_store_cannot_be_opened() {
    let store = unopenable_store_path("fail-closed");
    let mcp = McpTestClient::resolve_mcp_path();

    let output = tokio::process::Command::new(&mcp)
        .env("RUST_LOG", "info")
        .env("CHRONOS_DB_PATH", &store)
        .env_remove("CHRONOS_ALLOW_IN_MEMORY_FALLBACK")
        // No client: the server must decide before it ever reads a request.
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .expect("failed to spawn chronos-mcp");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(2),
        "a server that cannot open its store must exit non-zero before serving; \
         got {:?} with stderr:\n{stderr}",
        output.status
    );
    assert!(
        stderr.contains("cannot open the session store"),
        "the failure must be reported as a store failure, stderr:\n{stderr}"
    );
    assert!(
        stderr.contains(&store.to_string_lossy().to_string()),
        "the failure must name the store path it could not open, stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("CHRONOS_ALLOW_IN_MEMORY_FALLBACK"),
        "the failure must name the explicit opt-in, stderr:\n{stderr}"
    );

    let _ = std::fs::remove_dir_all(&store);
}

/// The opt-in is honoured: the server serves, from a store that is genuinely
/// in-memory, i.e. the unopenable path never became a database file.
#[tokio::test]
async fn test_server_serves_in_degraded_mode_only_with_the_explicit_opt_in() {
    let store = unopenable_store_path("degraded");
    let mcp = McpTestClient::resolve_mcp_path();

    let env = env_with(&[
        ("CHRONOS_DB_PATH", store.to_string_lossy().to_string()),
        ("CHRONOS_ALLOW_IN_MEMORY_FALLBACK", "1".to_string()),
    ]);

    let (process, stdin, reader) =
        chronos_sandbox::client::process::factory::start_with_env(&mcp, env)
            .await
            .expect("the opt-in must let the server start");

    // `McpSession::new` performs the MCP initialize handshake, so reaching this
    // point proves the transport is live rather than merely that the process
    // survived.
    let mut session = McpSession::new(stdin, reader)
        .await
        .expect("degraded server must complete the MCP handshake");

    // A tool call round-trip over a store that does not exist on disk.
    let listed = session
        .call_tool("list_sessions", serde_json::json!({}))
        .await
        .expect("session_list must answer in degraded mode");
    let rendered = listed.to_string();
    assert!(
        !listed.is_null() && (rendered.contains("content") || rendered.contains("session")),
        "session_list must answer with a tool result, got {listed:?}"
    );

    let _ = session.shutdown().await;
    let _ = process.shutdown().await;

    // Degraded means ephemeral: the path the server was pointed at is still the
    // directory the test created, not a database it made behind the caller's back.
    assert!(
        store.is_dir(),
        "the degraded store must not have replaced the unopenable path"
    );

    let _ = std::fs::remove_dir_all(&store);
}
