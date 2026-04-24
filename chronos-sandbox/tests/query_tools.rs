//! Query tools tests — verify event querying functionality after probe_stop.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::client::types::QueryFilter;
use chronos_sandbox::McpSession;
use std::time::Duration;

#[tokio::test]
async fn test_query_events_after_probe_stop() {
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Start probe on test_add
    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for it to complete (test_add exits quickly)
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Drain any pending events
    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    // Stop probe - this builds the query engine
    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    println!("Probe stopped: {} total events", stop.total_events);
    assert!(stop.total_events > 0, "Should have captured events");

    // Query events from the completed session
    let filter = QueryFilter {
        limit: 100,
        ..Default::default()
    };

    let events = client.query_events(&session_id, filter).await
        .expect("query_events failed");

    // === Assertions ===
    assert!(!events.is_empty(), "query_events should return events after probe_stop");

    // Debug: print first few events
    println!("First 3 events:");
    for (i, e) in events.iter().take(3).enumerate() {
        println!("  [{}] event_id={}, type={}, function={:?}", i, e.event_id, e.event_type, e.function);
    }

    // Verify event structure
    let first = &events[0];
    assert!(first.event_id >= 0, "event_id should be non-negative");
    assert!(first.timestamp_ns > 0, "timestamp should be non-zero");
    assert!(!first.event_type.is_empty(), "event_type should be non-empty");
    assert!(first.timestamp_ns > 0, "timestamp should be non-zero");
    assert!(first.event_type.len() > 0, "event_type should be non-empty");

    println!("✓ query_events returned {} events", events.len());
    println!("  First event: id={}, type={}, function={:?}",
        first.event_id, first.event_type, first.function);

    // Query with limit
    let filter_small = QueryFilter {
        limit: 5,
        ..Default::default()
    };
    let events_limited = client.query_events(&session_id, filter_small).await
        .expect("query_events with limit failed");

    assert!(events_limited.len() <= 5, "limit should be respected");

    println!("✓ Limited query returned {} events (limit: 5)", events_limited.len());

    client.shutdown().await.ok();
}
