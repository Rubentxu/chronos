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
/// Starts a probe on test_exit_immediate, waits 300ms, then stops it. The
/// traced program has already exited by then, but the session stays queryable:
/// probe_stop must return Ok with status "stopped" (not a "not found" error)
/// and query_events must return the events captured before the exit.
#[tokio::test]
async fn test_probe_start_program_exits_immediately() {
    let fixture = McpSession::fixture_path("test_exit_immediate")
        .expect("test_exit_immediate fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Start probe on program that exits immediately
    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    // Wait 300ms for the program to exit
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Stop the probe. An already-exited program is not a server error: the
    // session is finalized normally, so probe_stop must succeed.
    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop should succeed even though the traced program already exited");

    assert_eq!(
        stop.status, "stopped",
        "probe_stop on an already-exited program should report status 'stopped'"
    );
    assert!(
        stop.total_events > 0,
        "probe_stop should report the events captured before exit, got 0"
    );

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // query_events should return a valid response (array, maybe empty)
    let filter = QueryFilter::default();
    let events = client
        .query_events(&session_id, filter)
        .await
        .expect("query_events should return valid response, not crash");

    assert!(
        !events.is_empty(),
        "query_events should return the events captured before the program exited, got 0"
    );

    client.shutdown().await.ok();
}

/// B2: Verify probe on SIGSEGV program detects crash correctly.
///
/// Starts a probe on test_segfault, waits 500ms for the crash to happen,
/// stops the probe, then verifies debug_find_crash detects the SIGSEGV.
#[tokio::test]
async fn test_probe_start_program_crashes_sigsegv() {
    let fixture = McpSession::fixture_path("test_segfault")
        .expect("test_segfault fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Start probe on program that will crash with SIGSEGV
    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    // Wait 500ms for the crash to happen
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Stop the probe. The crash is recorded in the trace, so the session is
    // still finalized normally; probe_stop must succeed.
    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop should succeed after the traced program crashed");

    assert_eq!(
        stop.status, "stopped",
        "probe_stop after a crash should report status 'stopped'"
    );
    assert!(
        stop.total_events > 0,
        "probe_stop should report the events captured before the crash, got 0"
    );

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // query_events should return a valid response
    let filter = QueryFilter::default();
    let events = client
        .query_events(&session_id, filter)
        .await
        .expect("query_events should return valid response");

    assert!(
        !events.is_empty(),
        "query_events should return the events captured before the crash, got 0"
    );

    // debug_find_crash must detect the SIGSEGV. A missing crash is a failure,
    // not an acceptable outcome: detecting it is the purpose of this test.
    let crash = client
        .debug_find_crash(&session_id)
        .await
        .expect("debug_find_crash should not crash")
        .expect("debug_find_crash should report the SIGSEGV of the traced program, got None");

    assert!(crash.crash_found, "crash_found should be true");
    assert_eq!(
        crash.signal.as_deref(),
        Some("SIGSEGV"),
        "expected the reported crash signal to be SIGSEGV, got: {:?}",
        crash.signal
    );

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
    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    // Wait 2s for threads to spawn
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Stop the probe
    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");

    assert_eq!(
        stop.status, "stopped",
        "probe_stop should report status 'stopped', got: {}",
        stop.status
    );

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // list_threads should show multiple threads (main + workers)
    let threads = client
        .list_threads(&session_id)
        .await
        .expect("list_threads failed");

    // We expect at least 3 threads (main + at least 2 worker threads)
    assert!(
        threads.len() >= 3,
        "Expected at least 3 threads (main + workers), got {}",
        threads.len()
    );

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
    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    // Wait for events
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Stop the probe
    let _stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Query with limit=0
    let filter = QueryFilter {
        limit: 0,
        ..Default::default()
    };

    let events = client
        .query_events(&session_id, filter)
        .await
        .expect("query_events with limit=0 should return valid response, not error");

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
    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    // Wait for events
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Stop the probe
    let _stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Query with limit=1
    let filter = QueryFilter {
        limit: 1,
        ..Default::default()
    };

    let events = client
        .query_events(&session_id, filter)
        .await
        .expect("query_events with limit=1 should succeed");

    // Should return exactly 1 event
    assert_eq!(events.len(), 1, "limit=1 should return exactly 1 event");

    client.shutdown().await.ok();
}

/// B6: Verify query_events with timestamp_start > timestamp_end returns empty.
///
/// Starts a probe on test_busyloop, runs for 2s, stops, then queries with
/// inverted timestamp range (start > end). Such a range matches nothing, so the
/// server answers with an empty event list rather than an error.
#[tokio::test]
async fn test_query_events_timestamp_start_greater_than_end() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Start probe
    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    // Wait for events
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Stop the probe
    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");

    assert!(
        stop.total_events > 0,
        "the probe should have captured events before the inverted-range query, got 0"
    );

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Query with inverted timestamp range
    let filter = QueryFilter {
        timestamp_start: Some(9999999999999u64),
        timestamp_end: Some(1u64),
        ..Default::default()
    };

    // An inverted range is a valid filter that matches nothing; it is not an
    // error, so the response must be Ok with an empty event list.
    let events = client
        .query_events(&session_id, filter)
        .await
        .expect("query_events with an inverted timestamp range should not error");

    assert!(
        events.is_empty(),
        "an inverted timestamp range should match no events, got {}",
        events.len()
    );

    client.shutdown().await.ok();
}

/// B7: Verify state_diff with same timestamp twice returns no changes.
///
/// Starts a probe on test_busyloop, runs for 2s, stops, gets execution summary
/// to find a valid timestamp, then calls state_diff with same timestamp for both.
#[tokio::test]
async fn test_state_diff_same_timestamp_twice() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Start probe
    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    // Wait for events
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Stop the probe
    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");

    assert!(
        stop.total_events > 0,
        "the probe should have captured events before the state diff, got 0"
    );

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Get execution summary to find a valid timestamp
    let summary = client
        .get_execution_summary(&session_id)
        .await
        .expect("get_execution_summary failed");

    // Use the duration_ns as a timestamp for the diff
    let timestamp = summary.duration_ns;

    // Identical timestamps describe an empty interval, so the diff is
    // computed over no events and must report no changes.
    let diff = client
        .state_diff(&session_id, timestamp, timestamp)
        .await
        .expect("state_diff with the same timestamp on both ends should not error");

    assert!(
        diff.changes.is_empty(),
        "state_diff between identical timestamps should report no changes, got {}",
        diff.changes.len()
    );

    client.shutdown().await.ok();
}

/// B10: Verify debug_analyze_memory with start == end returns an empty analysis.
///
/// Starts a probe on test_busyloop, runs for 2s, stops, then calls
/// debug_analyze_memory with the same address for start and end. The address
/// range is empty, so the analysis succeeds and reports no writes.
#[tokio::test]
async fn test_debug_analyze_memory_start_equals_end() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Start probe
    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    // Wait for events
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Stop the probe
    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");

    assert!(
        stop.total_events > 0,
        "the probe should have captured events before the memory analysis, got 0"
    );

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Call debug_analyze_memory with same address for start and end
    let resp = client
        .debug_analyze_memory(
            &session_id,
            0x1000,   // start_address
            0x1000,   // end_address (same as start)
            0,        // start_ts
            u64::MAX, // end_ts
        )
        .await
        .expect("debug_analyze_memory with start == end should not error");

    assert_eq!(
        resp.start_address, "0x1000",
        "the analysis should echo the requested start address"
    );
    assert_eq!(
        resp.end_address, "0x1000",
        "the analysis should echo the requested end address"
    );
    assert_eq!(
        resp.total_writes, 0,
        "an empty address range should report 0 total writes, got {}",
        resp.total_writes
    );
    assert!(
        resp.accesses.is_empty(),
        "an empty address range should report no accesses, got {}",
        resp.accesses.len()
    );

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
    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    // Wait for events
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Stop the probe
    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");

    assert!(
        stop.total_events > 0,
        "the probe should have captured events before the memory audit, got 0"
    );

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Call forensic_memory_audit with limit=0
    let resp = client
        .forensic_memory_audit(&session_id, 0x1000, 0)
        .await
        .expect("forensic_memory_audit with limit=0 should not error");

    assert_eq!(
        resp.write_count, 0,
        "limit=0 should report 0 writes, got {}",
        resp.write_count
    );
    assert!(
        resp.writes.is_empty(),
        "limit=0 should return an empty writes array"
    );

    client.shutdown().await.ok();
}
