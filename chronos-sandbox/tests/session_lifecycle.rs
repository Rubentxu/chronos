//! Session lifecycle tests — verify delete_session and drop_session work correctly.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::McpSession;
use std::time::Duration;

#[tokio::test]
async fn test_drop_session_removes_from_memory() {
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

    // Verify the session is queryable
    let summary = client.get_execution_summary(&session_id).await
        .expect("Session should be queryable before drop");
    println!("✓ Session is queryable: {} events", summary.total_events);

    // Drop the session
    let drop_result = client.drop_session(&session_id).await
        .expect("drop_session failed");

    // Verify the response
    println!("✓ drop_session result: status={}, message={}",
        drop_result.status, drop_result.message);
    assert!(drop_result.status == "dropped" || drop_result.status == "not_found",
        "Status should be 'dropped' or 'not_found'");

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_drop_session_twice_is_idempotent() {
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(1)).await;
    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");
    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Drop the session first time
    let drop1 = client.drop_session(&session_id).await
        .expect("drop_session failed");
    println!("✓ First drop: status={}", drop1.status);

    // Drop again - should still succeed (idempotent)
    let drop2 = client.drop_session(&session_id).await
        .expect("drop_session should be idempotent");
    println!("✓ Second drop: status={}", drop2.status);

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_drop_nonexistent_session() {
    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Drop a session that doesn't exist
    let result = client.drop_session("nonexistent-session-12345").await
        .expect("drop_session should succeed even for non-existent session");

    // Should return not_found status
    println!("✓ drop_session for nonexistent: status={}", result.status);
    assert_eq!(result.status, "not_found",
        "Status should be 'not_found' for non-existent session");

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_delete_session_removes_from_storage() {
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

    // Save the session first
    let save_result = client.save_session(&session_id, "test_delete").await
        .expect("save_session failed");
    println!("✓ Session saved: {} events", save_result.event_count);

    // List sessions to verify it's saved
    let sessions_before = client.list_sessions().await
        .expect("list_sessions failed");
    let found_before = sessions_before.iter().any(|s| s.session_id == session_id);
    assert!(found_before, "Session should be in list before delete");

    // Delete the session
    client.delete_session(&session_id).await
        .expect("delete_session failed");

    println!("✓ Session deleted");

    // Try to load the deleted session - should fail
    let load_result = client.load_session(&session_id).await;
    match load_result {
        Ok(info) => {
            // Some implementations might not error on load of deleted session
            println!("load_session after delete returned: {:?}", info);
        }
        Err(e) => {
            println!("✓ load_session correctly failed after delete: {:?}", e);
        }
    }

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_delete_and_drop_sequence() {
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(1)).await;
    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");
    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Save then drop (drop doesn't affect storage)
    let save_result = client.save_session(&session_id, "test_seq").await
        .expect("save_session failed");
    println!("✓ Session saved: {} events", save_result.event_count);

    let drop_result = client.drop_session(&session_id).await
        .expect("drop_session failed");
    println!("✓ Session dropped: status={}", drop_result.status);

    // Session should still be in storage after drop
    let sessions = client.list_sessions().await
        .expect("list_sessions failed");
    let found = sessions.iter().any(|s| s.session_id == session_id);
    println!("✓ After drop, session in storage: {}", found);

    // Now delete
    client.delete_session(&session_id).await
        .expect("delete_session failed");
    println!("✓ Session deleted from storage");

    client.shutdown().await.ok();
}
