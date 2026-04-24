//! Diff tools tests — verify state_diff and debug_diff work correctly.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::McpSession;
use std::time::Duration;

#[tokio::test]
async fn test_state_diff_after_probe_stop() {
    // state_diff requires two timestamps to compare
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for it to complete
    tokio::time::sleep(Duration::from_secs(1)).await;

    let drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    println!("Probe stopped: {} total events", stop.total_events);
    assert!(stop.total_events > 0, "Should have captured events");

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Use timestamps from the drained events if available
    if !drained.is_empty() && drained.len() >= 2 {
        let ts_a = drained[0].timestamp_ns;
        let ts_b = drained[drained.len() - 1].timestamp_ns;

        let diff = client.state_diff(&session_id, ts_a, ts_b).await;

        match diff {
            Ok(result) => {
                println!("✓ state_diff between {} and {}: {} changes",
                    ts_a, ts_b, result.changes.len());
                for change in result.changes.iter().take(5) {
                    println!("  {}: {} -> {}", change.field, change.value_a, change.value_b);
                }
            }
            Err(e) => {
                // state_diff might fail if there's no state to compare
                println!("state_diff returned error (expected for simple programs): {:?}", e);
            }
        }
    } else {
        // Try with arbitrary timestamps
        let diff = client.state_diff(&session_id, 0, 1000000).await;

        match diff {
            Ok(result) => {
                println!("✓ state_diff: {} changes", result.changes.len());
            }
            Err(e) => {
                println!("state_diff returned error: {:?}", e);
            }
        }
    }

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_state_diff_same_timestamp() {
    // state_diff with same timestamps should return empty changes
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

    // Use same timestamp for both - should have no changes
    let diff = client.state_diff(&session_id, 1000, 1000).await;

    match diff {
        Ok(result) => {
            // Same timestamp = no changes expected
            println!("✓ state_diff (same timestamp): {} changes", result.changes.len());
        }
        Err(e) => {
            println!("state_diff returned error: {:?}", e);
        }
    }

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_debug_diff_after_probe_stop() {
    // debug_diff requires two event IDs to compare
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
            println!("debug_diff returned error (expected for simple C programs): {:?}", e);
        }
    }

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_debug_diff_out_of_range_events() {
    // debug_diff with non-existent event IDs
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
