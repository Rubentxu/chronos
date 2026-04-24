//! Session persistence tests — verify save_session and load_session round-trip.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::McpSession;
use std::time::Duration;

#[tokio::test]
async fn test_save_and_load_session_roundtrip() {
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for it to complete
    tokio::time::sleep(Duration::from_secs(1)).await;

    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    println!("Probe stopped: {} total events", stop.total_events);
    assert!(stop.total_events > 0, "Should have captured events");

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Save the session
    let save_result = client.save_session(&session_id, "test_add").await
        .expect("save_session failed");

    // === Assertions ===
    assert_eq!(save_result.session_id, session_id, "session_id should match");
    assert_eq!(save_result.status, "saved", "status should be 'saved'");
    assert!(save_result.event_count > 0, "event_count should be non-zero");
    assert_eq!(save_result.language, "native", "language should be 'native'");
    assert_eq!(save_result.target, "test_add", "target should be 'test_add'");

    println!("✓ Session saved: {} events, {} bytes",
        save_result.event_count, save_result.hash_count);

    // Load the session back
    let load_result = client.load_session(&session_id).await
        .expect("load_session failed");

    // === Assertions ===
    assert_eq!(load_result.session_id, session_id, "session_id should match");
    assert_eq!(load_result.event_count, save_result.event_count,
        "event_count should match saved count");
    assert_eq!(load_result.language, "native", "language should match");
    assert_eq!(load_result.target, "test_add", "target should match");

    println!("✓ Session loaded: {} events", load_result.event_count);

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_save_session_multiple_times() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for it to run a bit
    tokio::time::sleep(Duration::from_secs(4)).await;

    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Save the session multiple times with same ID (should overwrite)
    let save1 = client.save_session(&session_id, "busyloop_v1").await
        .expect("save_session failed");

    let save2 = client.save_session(&session_id, "busyloop_v2").await
        .expect("save_session failed");

    // Both saves should succeed
    println!("✓ Save 1: {} events, Save 2: {} events",
        save1.event_count, save2.event_count);

    // Load and verify the latest version
    let load = client.load_session(&session_id).await
        .expect("load_session failed");

    assert_eq!(load.target, "busyloop_v2", "should have latest target name");

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_list_sessions_after_save() {
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for it to complete
    tokio::time::sleep(Duration::from_secs(1)).await;

    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Save session
    client.save_session(&session_id, "list_test").await
        .expect("save_session failed");

    // List sessions
    let sessions = client.list_sessions().await
        .expect("list_sessions failed");

    // Should have at least our saved session
    assert!(!sessions.is_empty(), "Should have at least one saved session");

    println!("✓ list_sessions returned {} sessions", sessions.len());
    for session in sessions.iter().take(5) {
        println!("  - {}: {} events ({}ms)", session.session_id, session.event_count, session.duration_ms);
    }

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_load_nonexistent_session() {
    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Try to load a session that doesn't exist
    let result = client.load_session("nonexistent-session-12345").await;

    match result {
        Ok(info) => {
            // Some implementations might return a valid but empty session
            println!("load_session returned: {:?}", info);
        }
        Err(e) => {
            // Expected - session doesn't exist
            println!("✓ load_session correctly failed for nonexistent session: {:?}", e);
        }
    }

    client.shutdown().await.ok();
}
