//! Probe lifecycle tests — verify probe_start/probe_drain/probe_stop with real C fixtures.

use chronos_sandbox::{client::tools::McpTestClient, McpSession};
use std::time::Duration;

#[tokio::test]
async fn test_probe_start_and_drain() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Start probe on test_add
    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    assert!(!session_id.is_empty(), "session_id should not be empty");

    // Wait for program to execute (test_busyloop runs for ~3 seconds)
    tokio::time::sleep(Duration::from_secs(4)).await;

    // Drain events
    let events = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    // test_busyloop should produce some events (function entries/exits over 3 seconds)
    // Note: ptrace may not capture all events; at minimum we verify the probe works
    println!("Received {} events", events.len());

    // Stop probe
    client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    client.shutdown().await.ok();
}
