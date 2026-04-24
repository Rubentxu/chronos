//! Probe lifecycle edge case tests for the Chronos MCP server.
//!
//! These tests verify edge cases in the probe lifecycle including immediate stop,
//! drain after stop, stopping twice, and programs that exit immediately.
//!
//! Category E tests cover probe lifecycle edge cases.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::McpSession;
use std::time::Duration;

/// E1: Verify immediate stop of probe works correctly.
///
/// Starts a probe on test_busyloop, waits only 100ms, then stops immediately.
/// Asserts that probe_stop succeeds and query_events returns an array (even if empty).
#[tokio::test]
async fn test_probe_start_immediate_stop() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Start probe
    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait a very short time (100ms)
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Stop immediately
    let stop_result = client.probe_stop(&session_id).await;
    assert!(stop_result.is_ok(), "probe_stop should succeed even with immediate stop");

    let stop = stop_result.unwrap();
    println!("Immediate stop: {} events, {}ms", stop.total_events, stop.duration_ms);

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // query_events should return an array (possibly empty)
    let filter = chronos_sandbox::client::types::QueryFilter::default();
    let events = client.query_events(&session_id, filter).await
        .expect("query_events should succeed after probe_stop");

    println!("query_events returned {} events", events.len());

    client.shutdown().await.ok();
}

/// E2: Verify drain then stop then drain again workflow.
///
/// Starts a probe, waits 1.5s, drains events, stops the probe,
/// then tries to drain again - which should return an error or empty result.
#[tokio::test]
async fn test_probe_drain_then_stop_then_drain_again() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Start probe
    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for some events to accumulate
    tokio::time::sleep(Duration::from_millis(1500)).await;

    // First drain - should return some events
    let drain1 = client.probe_drain_raw(&session_id).await
        .expect("first probe_drain failed");

    println!("First drain: {} events", drain1.total_buffered);
    assert!(drain1.total_buffered > 0, "Should have captured some events after 1.5s");

    // Stop the probe
    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    println!("Probe stopped: {} total events", stop.total_events);

    // Second drain - session is now stopped, should return error or empty
    let drain2 = client.probe_drain_raw(&session_id).await;

    match drain2 {
        Ok(response) => {
            // If it returns OK, should have 0 events or error status
            println!("Second drain after stop: status={}, events={}",
                response.status, response.total_buffered);
            // After stop, the session should be gone
            assert!(
                response.status.to_lowercase().contains("not found") ||
                response.total_buffered == 0,
                "Expected 'not found' or 0 events after stop, got status={}",
                response.status
            );
        }
        Err(e) => {
            // Error is also acceptable - session was stopped
            println!("Second drain correctly failed after stop: {}", e);
        }
    }

    client.shutdown().await.ok();
}

/// E3: Verify probe_start returns error for nonexistent binary.
///
/// Calls probe_start with a path to a binary that doesn't exist.
/// Asserts that it returns an error response (not a server crash).
#[tokio::test]
async fn test_probe_start_nonexistent_binary() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Call probe_start with a path to a nonexistent binary
    let result = client.probe_start_raw("/tmp/this_binary_does_not_exist_chronos_test").await;

    match result {
        Ok(value) => {
            // Check if the response indicates an error
            let value_str = serde_json::to_string(&value).unwrap_or_default();
            println!("probe_start for nonexistent binary returned: {}", value_str);
            assert!(
                value_str.to_lowercase().contains("error") ||
                value_str.to_lowercase().contains("failed") ||
                value_str.to_lowercase().contains("not found") ||
                value_str.to_lowercase().contains("invalid"),
                "Expected error for nonexistent binary, got: {}",
                value_str
            );
        }
        Err(e) => {
            // Error at RPC level is also acceptable
            println!("probe_start correctly returned error: {}", e);
        }
    }

    client.shutdown().await.ok();
}

/// E5: Verify stopping a probe twice returns an error.
///
/// Starts a probe, stops it, then tries to stop it again.
/// The second stop should return an error.
#[tokio::test]
async fn test_probe_stop_twice() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Start probe
    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for some events
    tokio::time::sleep(Duration::from_secs(1)).await;

    // First stop - should succeed
    let stop1 = client.probe_stop(&session_id).await
        .expect("first probe_stop failed");

    println!("First stop succeeded: {} events", stop1.total_events);

    // Second stop - should fail with "session not found" or similar
    let stop2 = client.probe_stop(&session_id).await;

    match stop2 {
        Ok(response) => {
            // If it returns OK, check the status
            println!("Second stop returned status: {}", response.status);
            assert!(
                response.status.to_lowercase().contains("not found") ||
                response.status.to_lowercase().contains("error") ||
                response.status.to_lowercase().contains("already stopped"),
                "Expected error status for stopping twice, got: {}",
                response.status
            );
        }
        Err(e) => {
            // Error is the expected behavior
            println!("Second stop correctly returned error: {}", e);
        }
    }

    client.shutdown().await.ok();
}

/// E6: Verify probe on immediately-exiting program works correctly.
///
/// Starts a probe on test_exit_immediate (exits right away),
/// waits 200ms, stops, and verifies query_events works.
#[tokio::test]
async fn test_probe_start_exits_immediately() {
    let fixture = McpSession::fixture_path("test_exit_immediate")
        .expect("test_exit_immediate fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Start probe on program that exits immediately
    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait a bit for the probe to handle the exit
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Stop the probe
    let stop_result = client.probe_stop(&session_id).await;

    // May succeed or fail depending on implementation
    match stop_result {
        Ok(stop) => {
            println!("probe_stop on immediately-exiting program: {} events", stop.total_events);
        }
        Err(e) => {
            println!("probe_stop on immediately-exiting program returned error: {}", e);
        }
    }

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // query_events should return an array (may be empty but no crash)
    let filter = chronos_sandbox::client::types::QueryFilter::default();
    let events = client.query_events(&session_id, filter).await
        .expect("query_events should not crash for valid session");

    println!("query_events returned {} events", events.len());

    client.shutdown().await.ok();
}

/// E7: Verify drain immediately after start works (empty buffer OK).
///
/// Starts a probe on test_busyloop and immediately drains (no sleep).
/// Should return events array (possibly empty, no error).
#[tokio::test]
async fn test_probe_drain_empty_buffer_immediately() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Start probe
    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Drain immediately without waiting
    let drain_result = client.probe_drain_raw(&session_id).await;

    match drain_result {
        Ok(response) => {
            println!("Immediate drain: {} events", response.total_buffered);
            // Should succeed with 0 or more events
            // (0 events is OK - nothing captured yet)
            assert!(
                response.status.to_lowercase().contains("running") ||
                response.status.to_lowercase().contains("stopped"),
                "Expected 'running' or 'stopped' status, got: {}",
                response.status
            );
        }
        Err(e) => {
            // Error is not expected for immediate drain
            panic!("probe_drain immediately should not error: {}", e);
        }
    }

    // Now stop the probe
    client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    client.shutdown().await.ok();
}

/// E8: Verify mid-flight drain captures events and final drain after stop.
///
/// Starts a probe on test_busyloop, waits 1.5s, drains (should get events),
/// waits 1s more, stops, and verifies final query_events has events.
#[tokio::test]
async fn test_probe_busyloop_midflight_drain() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Start probe
    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for events to accumulate
    tokio::time::sleep(Duration::from_millis(1500)).await;

    // Mid-flight drain - should get some events
    let mid_events = client.probe_drain(&session_id).await
        .expect("mid-flight probe_drain failed");

    println!("Mid-flight drain: {} events", mid_events.len());
    assert!(mid_events.len() > 0, "Should capture events during execution");

    // Wait a bit more
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Stop the probe
    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    println!("Probe stopped: {} total events", stop.total_events);

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Final query_events should have events (from both drains and stop)
    let filter = chronos_sandbox::client::types::QueryFilter::default();
    let final_events = client.query_events(&session_id, filter).await
        .expect("query_events failed after stop");

    println!("Final query_events: {} events", final_events.len());
    assert!(final_events.len() > 0, "Should have events after stop");

    client.shutdown().await.ok();
}
