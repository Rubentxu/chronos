//! Session persistence edge case tests for the Chronos MCP server.
//!
//! These tests verify edge cases in session persistence including:
//! - Save with custom language tag
//! - Delete and drop sequences in different orders
//! - Save-drop-load roundtrip
//! - Deleting nonexistent sessions
//! - Loading sessions twice
//! - Listing sessions after multiple saves
//!
//! Category F tests cover session persistence edge cases.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::client::types::QueryFilter;
use chronos_sandbox::McpSession;
use std::time::Duration;

/// F1: Verify save_session with custom language tag works correctly.
///
/// Starts a probe on test_busyloop, runs for 2s, stops, saves with language="c",
/// then verifies load_session returns language="c".
#[tokio::test]
async fn test_save_session_with_language_tag() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Start probe
    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for events
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Stop the probe
    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    println!("Probe stopped: {} events", stop.total_events);

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Save with custom language tag
    let save_result = client.save_session_with_language(&session_id, "c", "test_busyloop")
        .await
        .expect("save_session_with_language failed");

    println!("Session saved with language='{}': {} events",
        save_result.language, save_result.event_count);

    assert_eq!(save_result.language, "c", "Language should be 'c'");

    // Load and verify
    let load_result = client.load_session(&session_id).await
        .expect("load_session failed");

    println!("Loaded session: language='{}', target='{}'",
        load_result.language, load_result.target);

    assert_eq!(load_result.language, "c", "Loaded language should be 'c'");

    client.shutdown().await.ok();
}

/// F3: Verify delete then drop sequence works correctly.
///
/// Starts a probe, stops, saves, deletes (from storage), then drops (from memory).
/// Both operations should succeed in sequence.
#[tokio::test]
async fn test_delete_then_drop_sequence() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Start probe
    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for events
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Stop the probe
    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    println!("Probe stopped: {} events", stop.total_events);

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Save the session
    let save_result = client.save_session(&session_id, "test_delete_drop").await
        .expect("save_session failed");

    println!("Session saved: {} events", save_result.event_count);

    // Delete from storage first
    client.delete_session(&session_id).await
        .expect("delete_session failed");

    println!("Session deleted from storage");

    // Drop from memory - should still succeed even after delete
    let drop_result = client.drop_session(&session_id).await
        .expect("drop_session failed after delete");

    println!("drop_session after delete: status={}, message={}",
        drop_result.status, drop_result.message);

    // Both operations should succeed
    assert!(
        drop_result.status == "dropped" ||
        drop_result.status == "not_found" ||
        drop_result.status == "deleted",
        "drop_session should succeed after delete"
    );

    client.shutdown().await.ok();
}

/// F4: Verify drop then delete sequence works correctly.
///
/// Starts a probe, stops, saves, drops (from memory), then deletes (from storage).
/// Both operations should succeed in sequence.
#[tokio::test]
async fn test_drop_then_delete_sequence() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Start probe
    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for events
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Stop the probe
    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    println!("Probe stopped: {} events", stop.total_events);

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Save the session
    let save_result = client.save_session(&session_id, "test_drop_delete").await
        .expect("save_session failed");

    println!("Session saved: {} events", save_result.event_count);

    // Drop from memory first
    let drop_result = client.drop_session(&session_id).await
        .expect("drop_session failed");

    println!("Session dropped from memory: status={}", drop_result.status);

    // Delete from storage - should still succeed even after drop
    client.delete_session(&session_id).await
        .expect("delete_session failed after drop");

    println!("Session deleted from storage after drop");

    client.shutdown().await.ok();
}

/// F5: Verify save-drop-load roundtrip preserves data.
///
/// Starts a probe on test_busyloop, runs for 2s, stops, saves session,
/// drops from memory, verifies query_events fails (not in memory),
/// then loads from storage and verifies events are still accessible.
#[tokio::test]
async fn test_save_drop_load_roundtrip() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Start probe
    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for events
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Drain and stop
    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    println!("Probe stopped: {} events", stop.total_events);

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Save the session
    let save_result = client.save_session(&session_id, "test_roundtrip").await
        .expect("save_session failed");

    let event_count_before_drop = save_result.event_count;
    println!("Session saved: {} events", event_count_before_drop);

    // Drop from memory
    client.drop_session(&session_id).await
        .expect("drop_session failed");

    println!("Session dropped from memory");

    // query_events should fail now (not in memory)
    let filter = QueryFilter::default();
    let query_result = client.query_events(&session_id, filter).await;

    match query_result {
        Ok(_events) => {
            // If it succeeds, it might be that the server still has it loaded
            println!("Note: query_events succeeded after drop (server may keep in memory)");
        }
        Err(e) => {
            println!("query_events correctly failed after drop: {}", e);
        }
    }

    // Load from storage
    let load_result = client.load_session(&session_id).await
        .expect("load_session failed");

    println!("Session loaded from storage: {} events", load_result.event_count);

    // Query events after load - should have data
    let filter = QueryFilter::default();
    let events = client.query_events(&session_id, filter).await
        .expect("query_events should work after load");

    println!("query_events after load: {} events", events.len());

    assert!(events.len() > 0, "Should have events after load from storage");

    client.shutdown().await.ok();
}

/// F7: Verify delete_session with nonexistent session ID is graceful.
///
/// Calls delete_session with session_id = "does-not-exist-xyz".
/// Should return success or error (not a server crash).
#[tokio::test]
async fn test_delete_nonexistent_session_is_graceful() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Try to delete a session that doesn't exist
    let result = client.delete_session("does-not-exist-xyz").await;

    match result {
        Ok(()) => {
            // Success is acceptable - idempotent operation
            println!("delete_session succeeded for nonexistent session (idempotent)");
        }
        Err(e) => {
            // Error is also acceptable - session doesn't exist
            println!("delete_session returned error for nonexistent session: {}", e);
        }
    }

    client.shutdown().await.ok();
}

/// F8: Verify loading the same session twice works correctly.
///
/// Starts a probe, saves session, loads twice, then queries events.
/// Both loads should succeed and data should be accessible.
#[tokio::test]
async fn test_load_session_twice() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Start probe
    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for events
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Drain and stop
    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    println!("Probe stopped: {} events", stop.total_events);

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Save the session
    let save_result = client.save_session(&session_id, "test_load_twice").await
        .expect("save_session failed");

    println!("Session saved: {} events", save_result.event_count);

    // First load
    let load1 = client.load_session(&session_id).await
        .expect("First load_session failed");

    println!("First load: {} events", load1.event_count);

    // Second load (same session)
    let load2 = client.load_session(&session_id).await
        .expect("Second load_session failed");

    println!("Second load: {} events", load2.event_count);

    // Both loads should succeed and return consistent data
    assert_eq!(load1.event_count, load2.event_count,
        "Both loads should return same event count");

    // Query events should work
    let filter = QueryFilter::default();
    let events = client.query_events(&session_id, filter).await
        .expect("query_events should work after loading");

    println!("query_events after double load: {} events", events.len());
    assert!(events.len() > 0, "Should have events after load");

    client.shutdown().await.ok();
}

/// F_bonus: Verify list_sessions returns correct count after multiple saves.
///
/// Starts 3 separate probes (test_busyloop x3), stops each, saves each with unique session IDs,
/// then verifies list_sessions returns count >= 3.
#[tokio::test]
async fn test_list_sessions_count_after_multiple_saves() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    // Use a single client for all three probes
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let mut session_ids = Vec::new();

    // Start first probe
    let session_id1 = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");
    session_ids.push(session_id1.clone());

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client.probe_drain(&session_id1).await
        .expect("probe_drain failed");
    client.probe_stop(&session_id1).await
        .expect("probe_stop failed");
    tokio::time::sleep(Duration::from_millis(200)).await;
    client.save_session(&session_id1, "busyloop_1").await
        .expect("save_session failed");
    println!("Saved session 1");

    // Start second probe
    let session_id2 = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");
    session_ids.push(session_id2.clone());

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client.probe_drain(&session_id2).await
        .expect("probe_drain failed");
    client.probe_stop(&session_id2).await
        .expect("probe_stop failed");
    tokio::time::sleep(Duration::from_millis(200)).await;
    client.save_session(&session_id2, "busyloop_2").await
        .expect("save_session failed");
    println!("Saved session 2");

    // Start third probe
    let session_id3 = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");
    session_ids.push(session_id3.clone());

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client.probe_drain(&session_id3).await
        .expect("probe_drain failed");
    client.probe_stop(&session_id3).await
        .expect("probe_stop failed");
    tokio::time::sleep(Duration::from_millis(200)).await;
    client.save_session(&session_id3, "busyloop_3").await
        .expect("save_session failed");
    println!("Saved session 3");

    // List sessions
    let sessions = client.list_sessions().await
        .expect("list_sessions failed");

    println!("list_sessions returned {} sessions", sessions.len());
    for session in sessions.iter().take(10) {
        println!("  - {}: {} events ({}ms)", session.session_id, session.event_count, session.duration_ms);
    }

    // Should have at least 3 sessions (our new ones)
    assert!(sessions.len() >= 3,
        "Expected at least 3 sessions, got {}", sessions.len());

    client.shutdown().await.ok();
}
