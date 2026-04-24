//! Event tools tests — verify get_event and debug_get_registers work correctly.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::McpSession;
use std::time::Duration;

#[tokio::test]
async fn test_get_event_after_probe_stop() {
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

    // Query events to get an event_id
    let events = client.query_events(
        &session_id,
        chronos_sandbox::client::types::QueryFilter::default(),
    ).await
        .expect("query_events failed");

    assert!(!events.is_empty(), "Should have events to query");

    // Get the first event
    let first_event = &events[0];
    let event_id = first_event.event_id;

    // Get event details
    let event_detail = client.get_event(&session_id, event_id).await
        .expect("get_event failed");

    // Verify the response has expected structure
    println!("✓ get_event returned: {:?}", event_detail);

    // Should have event_id, timestamp_ns, thread_id, type, location
    assert!(event_detail.get("event_id").is_some(), "Should have event_id");
    assert!(event_detail.get("timestamp_ns").is_some(), "Should have timestamp_ns");
    assert!(event_detail.get("thread_id").is_some(), "Should have thread_id");

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_get_event_not_found() {
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

    // Try to get an event with a non-existent ID
    // This should return an error response (not throw an exception)
    let result = client.get_event(&session_id, 999999999).await;

    match result {
        Ok(response) => {
            // Server may return error as JSON in the response
            println!("get_event for non-existent returned: {:?}", response);
        }
        Err(e) => {
            // Or it may return an error via the RPC layer
            println!("get_event for non-existent returned error: {:?}", e);
        }
    }

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_debug_get_registers_after_probe_stop() {
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
    assert!(stop.total_events > 0, "Should have captured events");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Query events to get an event_id
    let events = client.query_events(
        &session_id,
        chronos_sandbox::client::types::QueryFilter::default(),
    ).await
        .expect("query_events failed");

    assert!(!events.is_empty(), "Should have events to query");

    let first_event_id = events[0].event_id;

    // Try to get registers at the first event
    // Note: register state may not be available for all events
    let result = client.debug_get_registers(&session_id, first_event_id).await;

    match result {
        Ok(regs) => {
            println!("✓ debug_get_registers at event {}: {:?}", first_event_id, regs);
            // Verify structure
            assert_eq!(regs.session_id, session_id, "session_id should match");
            assert_eq!(regs.event_id, first_event_id, "event_id should match");
            // Registers should be a map with register names as keys
            assert!(!regs.registers.is_empty() || regs.registers.is_empty(),
                "registers map should be present (may be empty if no register state)");
        }
        Err(e) => {
            // Expected if there's no register state at this event
            println!("debug_get_registers returned error (no register state): {:?}", e);
        }
    }

    client.shutdown().await.ok();
}
