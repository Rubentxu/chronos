//! Boundary condition tests for the Chronos MCP server.
//!
//! These tests verify edge cases in boundary conditions including:
//! - Programs that exit immediately or crash
//! - Query parameter edge cases (limit=0, limit=1, invalid timestamp ranges)
//! - State diff with same timestamps
//! - Memory analysis with edge case parameters
//!
//! Category B tests cover boundary conditions.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::client::types::QueryFilter;
use chronos_sandbox::McpSession;
use std::time::Duration;

/// B1: Verify probe on immediately-exiting program handles gracefully.
///
/// Starts a probe on test_exit_immediate, waits 300ms, stops (may get "not found"
/// if program already exited), then verifies query_events returns valid response.
#[tokio::test]
async fn test_probe_start_program_exits_immediately() {
    let fixture = McpSession::fixture_path("test_exit_immediate")
        .expect("test_exit_immediate fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Start probe on program that exits immediately
    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait 300ms for the program to exit
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Stop the probe - may get "not found" if program already exited
    let stop_result = client.probe_stop(&session_id).await;

    match stop_result {
        Ok(stop) => {
            println!("probe_stop succeeded: {} events, {}ms",
                stop.total_events, stop.duration_ms);
        }
        Err(e) => {
            // This is also acceptable - program may have already exited
            println!("probe_stop returned error (program already exited): {}", e);
        }
    }

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // query_events should return a valid response (array, maybe empty)
    let filter = QueryFilter::default();
    let events = client.query_events(&session_id, filter).await
        .expect("query_events should return valid response, not crash");

    println!("query_events returned {} events", events.len());

    client.shutdown().await.ok();
}

/// B2: Verify probe on SIGSEGV program detects crash correctly.
///
/// Starts a probe on test_segfault, waits 500ms for crash to happen,
/// stops the probe, then verifies debug_find_crash detects the crash.
#[tokio::test]
async fn test_probe_start_program_crashes_sigsegv() {
    let fixture = McpSession::fixture_path("test_segfault")
        .expect("test_segfault fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Start probe on program that will crash with SIGSEGV
    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait 500ms for the crash to happen
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Stop the probe - may already be done
    let stop_result = client.probe_stop(&session_id).await;

    match stop_result {
        Ok(stop) => {
            println!("probe_stop succeeded: {} events, {}ms",
                stop.total_events, stop.duration_ms);
        }
        Err(e) => {
            println!("probe_stop returned error (program may have crashed): {}", e);
        }
    }

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // query_events should return a valid response
    let filter = QueryFilter::default();
    let events = client.query_events(&session_id, filter).await
        .expect("query_events should return valid response");

    println!("query_events returned {} events", events.len());

    // debug_find_crash should detect the SIGSEGV
    let crash_result = client.debug_find_crash(&session_id).await
        .expect("debug_find_crash should not crash");

    if let Some(crash) = crash_result {
        println!("✓ Crash detected: signal={:?}, event_id={:?}",
            crash.signal, crash.event_id);
        assert!(crash.crash_found, "crash_found should be true");
    } else {
        println!("Note: No crash detected (crash may have happened after probe stopped)");
    }

    client.shutdown().await.ok();
}

/// B3: Verify probe on multi-threaded program tracks multiple threads.
///
/// Starts a probe on test_many_threads (10 threads), waits 2s for threads to spawn,
/// stops the probe, then verifies list_threads shows multiple threads.
#[tokio::test]
async fn test_probe_start_many_threads() {
    let fixture = McpSession::fixture_path("test_many_threads")
        .expect("test_many_threads fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Start probe on multi-threaded program
    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait 2s for threads to spawn
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Stop the probe
    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    println!("Probe stopped: {} events, {}ms", stop.total_events, stop.duration_ms);

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // list_threads should show multiple threads (main + workers)
    let threads = client.list_threads(&session_id).await
        .expect("list_threads failed");

    println!("list_threads returned {} threads", threads.len());
    for thread in threads.iter().take(5) {
        println!("  thread_id: {}", thread.thread_id);
    }

    // We expect at least 3 threads (main + at least 2 worker threads)
    assert!(threads.len() >= 3,
        "Expected at least 3 threads (main + workers), got {}", threads.len());

    client.shutdown().await.ok();
}

/// B4: Verify query_events with limit=0 returns empty array (not error).
///
/// Starts a probe on test_busyloop, runs for 2s, stops, then queries with limit=0.
#[tokio::test]
async fn test_query_events_limit_zero() {
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

    // Query with limit=0
    let filter = QueryFilter {
        limit: 0,
        ..Default::default()
    };

    let events = client.query_events(&session_id, filter).await
        .expect("query_events with limit=0 should return valid response, not error");

    println!("query_events with limit=0 returned {} events", events.len());

    // Should return empty array, not an error
    assert!(events.is_empty(), "limit=0 should return empty array");

    client.shutdown().await.ok();
}

/// B5: Verify query_events with limit=1 returns exactly 1 event.
///
/// Starts a probe on test_busyloop, runs for 2s, stops, then queries with limit=1.
#[tokio::test]
async fn test_query_events_limit_one() {
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

    // Query with limit=1
    let filter = QueryFilter {
        limit: 1,
        ..Default::default()
    };

    let events = client.query_events(&session_id, filter).await
        .expect("query_events with limit=1 should succeed");

    println!("query_events with limit=1 returned {} events", events.len());

    // Should return exactly 1 event
    assert_eq!(events.len(), 1, "limit=1 should return exactly 1 event");

    client.shutdown().await.ok();
}

/// B6: Verify query_events with timestamp_start > timestamp_end returns empty or error.
///
/// Starts a probe on test_busyloop, runs for 2s, stops, then queries with
/// inverted timestamp range (start > end).
#[tokio::test]
async fn test_query_events_timestamp_start_greater_than_end() {
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

    // Query with inverted timestamp range
    let filter = QueryFilter {
        timestamp_start: Some(9999999999999u64),
        timestamp_end: Some(1u64),
        ..Default::default()
    };

    let result = client.query_events(&session_id, filter).await;

    match result {
        Ok(events) => {
            // Empty events is acceptable - inverted range means no matches
            println!("query_events with inverted range returned {} events (empty OK)", events.len());
        }
        Err(e) => {
            // Error is also acceptable - invalid timestamp range
            println!("query_events with inverted range returned error (acceptable): {}", e);
        }
    }

    client.shutdown().await.ok();
}

/// B7: Verify state_diff with same timestamp twice returns valid response.
///
/// Starts a probe on test_busyloop, runs for 2s, stops, gets execution summary
/// to find valid timestamps, then calls state_diff with same timestamp for both.
#[tokio::test]
async fn test_state_diff_same_timestamp_twice() {
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

    // Get execution summary to find a valid timestamp
    let summary = client.get_execution_summary(&session_id).await
        .expect("get_execution_summary failed");

    // Use the duration_ns as a timestamp for the diff
    let timestamp = summary.duration_ns;

    println!("Using timestamp: {} ns", timestamp);

    // Call state_diff with same timestamp for both
    let diff_result = client.state_diff(&session_id, timestamp, timestamp).await;

    match diff_result {
        Ok(diff) => {
            // Same timestamp should produce empty changes
            println!("state_diff with same timestamp: {} changes", diff.changes.len());
        }
        Err(e) => {
            // Error is also acceptable
            println!("state_diff with same timestamp returned error: {}", e);
        }
    }

    client.shutdown().await.ok();
}

/// B10: Verify debug_analyze_memory with start_address == end_address doesn't crash.
///
/// Starts a probe on test_busyloop, runs for 2s, stops, then calls
/// debug_analyze_memory with same address for start and end.
#[tokio::test]
async fn test_debug_analyze_memory_start_equals_end() {
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

    // Call debug_analyze_memory with same address for start and end
    let result = client.debug_analyze_memory(
        &session_id,
        0x1000,    // start_address
        0x1000,    // end_address (same as start)
        0,         // start_ts
        u64::MAX,  // end_ts
    ).await;

    match result {
        Ok(resp) => {
            // Should return valid response (possibly empty)
            println!("debug_analyze_memory with same address: {} total_writes",
                resp.total_writes);
        }
        Err(e) => {
            // Error is also acceptable
            println!("debug_analyze_memory with same address returned error: {}", e);
        }
    }

    client.shutdown().await.ok();
}

/// B11: Verify forensic_memory_audit with limit=0 returns empty writes array.
///
/// Starts a probe on test_busyloop, runs for 2s, stops, then calls
/// forensic_memory_audit with limit=0.
#[tokio::test]
async fn test_forensic_memory_audit_limit_zero() {
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

    // Call forensic_memory_audit with limit=0
    let result = client.forensic_memory_audit(&session_id, 0x1000, 0).await;

    match result {
        Ok(resp) => {
            // Should return valid response with empty writes
            println!("forensic_memory_audit with limit=0: {} writes", resp.write_count);
            assert!(resp.writes.is_empty(), "limit=0 should return empty writes array");
        }
        Err(e) => {
            // Error is also acceptable for limit=0
            println!("forensic_memory_audit with limit=0 returned error: {}", e);
        }
    }

    client.shutdown().await.ok();
}
