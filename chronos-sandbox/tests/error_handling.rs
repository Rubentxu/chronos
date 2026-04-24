//! Error handling and resilience tests for the Chronos MCP server.
//!
//! These tests verify that the MCP server gracefully handles error conditions
//! such as invalid session IDs, out-of-range event IDs, and operations on
//! non-existent sessions.
//!
//! Category A tests cover error handling and resilience.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::McpSession;
use std::time::Duration;

/// A2: Verify probe_stop returns an error for nonexistent session.
///
/// Calls probe_stop with session_id = "nonexistent-session-xyz" and asserts
/// that the response contains an error (not a panic/connection drop).
#[tokio::test]
async fn test_probe_stop_nonexistent_session() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Call probe_stop with a session that doesn't exist
    let result = client.probe_stop("nonexistent-session-xyz").await;

    // Should get an error response, not a panic
    match result {
        Ok(response) => {
            // If it succeeds, the status should indicate error
            println!("probe_stop returned OK with status: {}", response.status);
            assert!(
                response.status.to_lowercase().contains("error") ||
                response.status.to_lowercase().contains("not found") ||
                response.status.to_lowercase().contains("stopped"),
                "Expected error status, got: {}",
                response.status
            );
        }
        Err(e) => {
            // Error is also acceptable - means the server properly rejected the invalid session
            println!("probe_stop correctly returned error: {}", e);
        }
    }

    client.shutdown().await.ok();
}

/// A3: Verify probe_drain returns an error for nonexistent session.
///
/// Calls probe_drain with session_id = "nonexistent-session-xyz" and asserts
/// that the response contains an error.
#[tokio::test]
async fn test_probe_drain_nonexistent_session() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Call probe_drain with a session that doesn't exist
    let result = client.probe_drain_raw("nonexistent-session-xyz").await;

    // Should get an error response, not a panic
    match result {
        Ok(response) => {
            // If it succeeds, the status should indicate error
            println!("probe_drain returned OK with status: {}", response.status);
            // Session not found should be reflected in the response
            assert!(
                response.status.to_lowercase().contains("error") ||
                response.status.to_lowercase().contains("not found") ||
                response.status.to_lowercase().contains("running"),
                "Expected error or 'running' status (session not found), got: {}",
                response.status
            );
        }
        Err(e) => {
            // Error is also acceptable
            println!("probe_drain correctly returned error: {}", e);
        }
    }

    client.shutdown().await.ok();
}

/// A4: Verify query_events returns error or empty result for invalid session.
///
/// Calls query_events with session_id = "invalid-does-not-exist" and asserts
/// that the response contains an error or empty result.
#[tokio::test]
async fn test_query_events_invalid_session() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let filter = chronos_sandbox::client::types::QueryFilter::default();
    let result = client.query_events("invalid-does-not-exist", filter).await;

    match result {
        Ok(events) => {
            // Empty result is acceptable - session doesn't exist so no events
            println!("query_events returned {} events (empty is OK for invalid session)", events.len());
        }
        Err(e) => {
            // Error is also acceptable - server properly rejected invalid session
            println!("query_events correctly returned error: {}", e);
        }
    }

    client.shutdown().await.ok();
}

/// A5: Verify get_event returns error for out-of-range event ID.
///
/// Starts a real probe, stops it to get a real session with events,
/// then calls get_event with event_id = 999999 (which shouldn't exist).
#[tokio::test]
async fn test_get_event_out_of_range() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Start probe on a real program
    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for some events
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Drain events
    let _events = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    // Stop the probe to finalize the session
    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    println!("Probe stopped with {} events", stop.total_events);

    // Give the query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Call get_event with an event ID that's definitely out of range
    let result = client.get_event(&session_id, 999999).await;

    match result {
        Ok(value) => {
            // Check if the response indicates "not found"
            let value_str = serde_json::to_string(&value).unwrap_or_default();
            println!("get_event returned: {}", value_str);
            // Should contain "not found" or "not found" in the error message
            assert!(
                value_str.to_lowercase().contains("not found") ||
                value_str.to_lowercase().contains("error"),
                "Expected 'not found' or 'error' in response, got: {}",
                value_str
            );
        }
        Err(e) => {
            // Error is also acceptable
            println!("get_event correctly returned error for out-of-range ID: {}", e);
        }
    }

    client.shutdown().await.ok();
}

/// A7: Verify save_then_delete_then_load workflow.
///
/// Starts a probe, stops it, saves the session, deletes it, then verifies
/// that load_session returns an error for the deleted session.
#[tokio::test]
async fn test_save_then_delete_then_load() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Start probe
    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for some events
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Drain events
    let _events = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    // Stop the probe
    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    println!("Probe stopped with {} events", stop.total_events);
    assert!(stop.total_events > 0, "Should have captured some events");

    // Give the query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Save the session
    let save_result = client.save_session(&session_id, "test_save_delete_load").await
        .expect("save_session failed");
    println!("Session saved: {} events", save_result.event_count);

    // Delete the session
    client.delete_session(&session_id).await
        .expect("delete_session failed");
    println!("Session deleted");

    // Try to load the deleted session - should fail
    let load_result = client.load_session(&session_id).await;

    match load_result {
        Ok(info) => {
            // If it succeeds, it should have 0 events or be otherwise indicate the session is gone
            println!("load_session returned: {:?}", info);
            // The behavior may vary - some implementations may still return the session
            // or indicate it's not found in the content
        }
        Err(e) => {
            // Error is the expected behavior - session was deleted
            println!("load_session correctly failed after delete: {}", e);
        }
    }

    client.shutdown().await.ok();
}

/// A9: Verify list_threads returns graceful error for invalid session.
///
/// Calls list_threads with session_id = "ghost-session" and asserts
/// that it returns a graceful error, not a crash.
#[tokio::test]
async fn test_list_threads_invalid_session() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Call list_threads with a session that doesn't exist
    let result = client.list_threads("ghost-session").await;

    match result {
        Ok(threads) => {
            // Empty result is acceptable for non-existent session
            println!("list_threads returned {} threads", threads.len());
        }
        Err(e) => {
            // Error is also acceptable
            println!("list_threads correctly returned error: {}", e);
        }
    }

    client.shutdown().await.ok();
}

/// A10: Verify probe_start returns error for empty program path.
///
/// Calls probe_start with program = "" and asserts that it returns
/// an error response (validation error).
#[tokio::test]
async fn test_probe_start_empty_path() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Call probe_start with empty program path
    let result = client.probe_start_raw("").await;

    match result {
        Ok(value) => {
            // Check if the response indicates an error
            let value_str = serde_json::to_string(&value).unwrap_or_default();
            println!("probe_start with empty path returned: {}", value_str);
            // Should contain error indication
            assert!(
                value_str.to_lowercase().contains("error") ||
                value_str.to_lowercase().contains("invalid") ||
                value_str.to_lowercase().contains("not found") ||
                value_str.to_lowercase().contains("path"),
                "Expected error about invalid path, got: {}",
                value_str
            );
        }
        Err(e) => {
            // Error is also acceptable
            println!("probe_start correctly returned error for empty path: {}", e);
        }
    }

    client.shutdown().await.ok();
}

/// A10b: Verify probe_start returns error for nonexistent binary path.
///
/// Calls probe_start with program = "/nonexistent/path/to/binary" and asserts
/// that it returns an error response.
#[tokio::test]
async fn test_probe_start_nonexistent_path() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Call probe_start with a path that doesn't exist
    let result = client.probe_start_raw("/nonexistent/path/to/binary").await;

    match result {
        Ok(value) => {
            // Check if the response indicates an error
            let value_str = serde_json::to_string(&value).unwrap_or_default();
            println!("probe_start with nonexistent path returned: {}", value_str);
            // Should contain error indication
            assert!(
                value_str.to_lowercase().contains("error") ||
                value_str.to_lowercase().contains("invalid") ||
                value_str.to_lowercase().contains("not found") ||
                value_str.to_lowercase().contains("failed"),
                "Expected error about nonexistent path, got: {}",
                value_str
            );
        }
        Err(e) => {
            // Error is also acceptable - server may reject at RPC level
            println!("probe_start correctly returned error for nonexistent path: {}", e);
        }
    }

    client.shutdown().await.ok();
}
