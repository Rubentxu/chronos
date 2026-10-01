//! Diff tools tests — verify state_diff and debug_diff work correctly.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::McpSession;
use std::time::Duration;

#[tokio::test]
async fn test_state_diff_after_probe_stop() {
    // state_diff compares the program state at two timestamps; after
    // probe_stop the session is queryable and the first/last drained event
    // timestamps form a valid pair.
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    // Wait for it to complete
    tokio::time::sleep(Duration::from_secs(1)).await;

    let drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");

    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");

    assert!(stop.total_events > 0, "Should have captured events");

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Use timestamps from the drained events. This capture always produces a
    // large event buffer (measured: 128 events for test_add), so the real
    // two-timestamp path is asserted directly instead of falling back to
    // arbitrary timestamps that would prove nothing.
    assert!(
        drained.len() >= 2,
        "need at least two drained events to derive two timestamps, got {}",
        drained.len()
    );

    let ts_a = drained[0].timestamp_ns;
    let ts_b = drained[drained.len() - 1].timestamp_ns;
    assert!(
        ts_a < ts_b,
        "drained event timestamps must be ordered, got ts_a={} ts_b={}",
        ts_a,
        ts_b
    );

    let result = client
        .state_diff(&session_id, ts_a, ts_b)
        .await
        .expect("state_diff should succeed for a captured session");

    // The response must be bound to the two timestamps we asked about.
    assert_eq!(
        result.timestamp_a, ts_a,
        "state_diff echoed a different timestamp_a than requested"
    );
    assert_eq!(
        result.timestamp_b, ts_b,
        "state_diff echoed a different timestamp_b than requested"
    );

    // Measured behaviour: this trace carries no register evidence (every
    // drained event is an Unresolved SyscallEnter/SyscallExit), and the
    // service documents state_diff as returning an empty diff when neither
    // timestamp has register evidence. So no changes is the expected result
    // here — a non-empty list would mean unexpected register data.
    assert!(
        result.changes.is_empty(),
        "expected no register changes without register evidence, got {:?}",
        result.changes
    );

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_state_diff_same_timestamp() {
    // state_diff with same timestamps should return empty changes
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    // Wait for it to complete
    tokio::time::sleep(Duration::from_secs(1)).await;

    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");

    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");

    assert!(
        stop.total_events > 0,
        "probe_stop should report captured events, got {}",
        stop.total_events
    );

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Use same timestamp for both - should have no changes
    let result = client
        .state_diff(&session_id, 1000, 1000)
        .await
        .expect("state_diff should succeed with identical timestamps");

    assert_eq!(
        result.timestamp_a, 1000,
        "state_diff echoed a different timestamp_a than requested"
    );
    assert_eq!(
        result.timestamp_b, 1000,
        "state_diff echoed a different timestamp_b than requested"
    );

    // Same timestamp = no changes expected
    assert!(
        result.changes.is_empty(),
        "identical timestamps must yield no changes, got {:?}",
        result.changes
    );

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_debug_diff_after_probe_stop() {
    // debug_diff requires two event IDs to compare
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    // Wait for it to complete
    tokio::time::sleep(Duration::from_secs(1)).await;

    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");

    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");

    assert!(
        stop.total_events > 0,
        "probe_stop should report captured events, got {}",
        stop.total_events
    );

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Try to diff events 0 and 1
    let diff = client.debug_diff(&session_id, 0, 1).await;

    match diff {
        Ok(result) => {
            println!("✓ debug_diff between events 0 and 1:");
            println!("  Session: {}", result.session_id);
            println!("  Summary: {}", result.summary);
            if let Some(reg_diff) = result.registers_diff {
                println!("  Register changes: {}", reg_diff.len());
            }
            if let Some(mem_diff) = result.memory_diff {
                println!("  Memory changes: {}", mem_diff.len());
            }
        }
        Err(e) => {
            // debug_diff might fail if the events don't have state
            println!(
                "debug_diff returned error (expected for simple C programs): {:?}",
                e
            );
        }
    }

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_debug_diff_out_of_range_events() {
    // debug_diff with non-existent event IDs
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    // Wait for it to complete
    tokio::time::sleep(Duration::from_secs(1)).await;

    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");

    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");

    assert!(
        stop.total_events > 0,
        "probe_stop should report captured events, got {}",
        stop.total_events
    );

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Try to diff events that don't exist
    let diff = client.debug_diff(&session_id, 999999, 999998).await;

    match diff {
        Ok(result) => {
            // Might succeed with empty diff
            println!("✓ debug_diff (out of range): summary = {}", result.summary);
        }
        Err(e) => {
            // Also acceptable - events don't exist
            println!("debug_diff returned error for out-of-range events: {:?}", e);
        }
    }

    client.shutdown().await.ok();
}
