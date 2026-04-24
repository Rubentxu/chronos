//! Integration tests for the session_snapshot MCP tool.
//!
//! These tests exercise the session_snapshot tool which creates an indexed
//! QueryEngine snapshot from a running probe without stopping it.
//!
//! The session_snapshot tool allows mid-flight queries on a live probe session.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::client::types::QueryFilter;
use chronos_sandbox::McpSession;
use std::time::Duration;

// ============================================================================
// S1: test_session_snapshot_while_probe_running
// ============================================================================

/// Test S1: session_snapshot while probe is running.
/// - probe_start on test_busyloop
/// - Sleep 1.5s (let events accumulate)
/// - session_snapshot(session_id) — snapshot mid-flight
/// - Assert: response is valid (success, event count > 0)
/// - query_events(session_id) → assert events accessible
/// - probe_stop
#[tokio::test]
async fn test_session_snapshot_while_probe_running() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Start probe on test_busyloop
    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for events to accumulate
    tokio::time::sleep(Duration::from_millis(1500)).await;

    // Take snapshot mid-flight
    let snapshot = client.session_snapshot(&session_id).await
        .expect("session_snapshot failed");

    // Assert: response is valid (success, event count > 0)
    assert_eq!(snapshot.status, "running", "session_snapshot should return status 'running'");
    assert!(snapshot.events_indexed > 0, "Expected some events to be indexed, got {}", snapshot.events_indexed);

    // query_events should work on the snapshot
    let events = client.query_events(&session_id, QueryFilter::default()).await
        .expect("query_events failed on snapshot");

    assert!(!events.is_empty(), "query_events should return events from the snapshot");

    // Stop the probe
    client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    client.shutdown().await.ok();
}

// ============================================================================
// S2: test_session_snapshot_enables_query_while_running
// ============================================================================

/// Test S2: session_snapshot enables queries while probe is still running.
/// - probe_start on test_busyloop
/// - Sleep 1s
/// - session_snapshot
/// - get_execution_summary(session_id) → assert valid response
/// - probe_stop
#[tokio::test]
async fn test_session_snapshot_enables_query_while_running() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Start probe
    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for events
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Take snapshot
    let snapshot = client.session_snapshot(&session_id).await
        .expect("session_snapshot failed");

    assert_eq!(snapshot.status, "running");

    // get_execution_summary should work on the snapshot
    let summary = client.get_execution_summary(&session_id).await
        .expect("get_execution_summary failed");

    // Verify we got a valid summary
    assert!(summary.total_events > 0, "Summary should have events after snapshot");

    // Stop probe
    client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    client.shutdown().await.ok();
}

// ============================================================================
// S3: test_session_snapshot_nonexistent_session
// ============================================================================

/// Test S3: session_snapshot on nonexistent session returns graceful error.
#[tokio::test]
async fn test_session_snapshot_nonexistent_session() {
    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Call session_snapshot on nonexistent session - should return error
    let result = client.session_snapshot("nonexistent-session-xyz").await;

    // Should be an error (not crash)
    assert!(result.is_err(), "session_snapshot on nonexistent session should return error");

    client.shutdown().await.ok();
}

// ============================================================================
// S4: test_session_snapshot_then_stop_then_query
// ============================================================================

/// Test S4: session_snapshot followed by probe_stop still allows querying.
/// - probe_start on test_busyloop
/// - Sleep 1.5s
/// - session_snapshot → snapshot taken
/// - probe_stop → final drain
/// - query_events → assert events accessible (from final stop, not snapshot)
#[tokio::test]
async fn test_session_snapshot_then_stop_then_query() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Start probe
    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for events to accumulate
    tokio::time::sleep(Duration::from_millis(1500)).await;

    // Take snapshot
    let snapshot = client.session_snapshot(&session_id).await
        .expect("session_snapshot failed");
    println!("Snapshot indexed {} events", snapshot.events_indexed);

    // Stop probe (should also drain final events)
    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");
    println!("Probe stopped with {} total events", stop.total_events);

    // query_events should still work (from the final stop's engine)
    let events = client.query_events(&session_id, QueryFilter::default()).await
        .expect("query_events failed after stop");

    assert!(!events.is_empty(), "query_events should return events after stop");

    client.shutdown().await.ok();
}

// ============================================================================
// S5: test_session_snapshot_multiple_times
// ============================================================================

/// Test S5: calling session_snapshot multiple times succeeds.
/// - probe_start on test_busyloop
/// - Sleep 1s → session_snapshot (snapshot 1)
/// - Sleep 1s → session_snapshot again (snapshot 2, more events)
/// - probe_stop
/// - Assert: both calls succeed, no crash
#[tokio::test]
async fn test_session_snapshot_multiple_times() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Start probe
    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // First snapshot after 1s
    tokio::time::sleep(Duration::from_secs(1)).await;
    let snapshot1 = client.session_snapshot(&session_id).await
        .expect("First session_snapshot failed");
    println!("First snapshot: {} events", snapshot1.events_indexed);

    // Second snapshot after another 1s (more events)
    tokio::time::sleep(Duration::from_secs(1)).await;
    let snapshot2 = client.session_snapshot(&session_id).await
        .expect("Second session_snapshot failed");
    println!("Second snapshot: {} events", snapshot2.events_indexed);

    // Both should succeed and have events
    assert!(snapshot1.events_indexed > 0, "First snapshot should have events");
    assert!(snapshot2.events_indexed > 0, "Second snapshot should have events");

    // Stop probe
    client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    client.shutdown().await.ok();
}
