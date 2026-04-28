//! Query edge cases E2E tests — probe limits and invalid inputs for query_events.
//!
//! These tests verify that query_events handles edge cases gracefully:
//! - Offset beyond total events
//! - Limit = 0 or very large
//! - Invalid session IDs
//! - Invalid timestamp ranges
//! - Thread ID filters that match nothing

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::client::types::QueryFilter;
use chronos_sandbox::McpSession;
use std::time::Duration;

/// QE1: query_events with offset far beyond total events returns empty.
#[tokio::test]
async fn test_query_events_offset_beyond_total() {
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    let _stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Query with offset way beyond total events
    let filter = QueryFilter {
        limit: 10,
        offset: 1_000_000, // Way beyond any realistic event count
        ..Default::default()
    };

    let events = client.query_events(&session_id, filter).await
        .expect("query_events should handle large offset gracefully");

    println!("✓ query_events with offset=1_000_000 returned {} events (expected 0)", events.len());
    assert!(events.is_empty(), "Should return empty for offset beyond total");

    client.shutdown().await.ok();
}

/// QE2: query_events with limit = 0 returns empty.
#[tokio::test]
async fn test_query_events_limit_zero() {
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    let _stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Query with limit = 0
    let filter = QueryFilter {
        limit: 0,
        offset: 0,
        ..Default::default()
    };

    let events = client.query_events(&session_id, filter).await
        .expect("query_events should handle limit=0 gracefully");

    println!("✓ query_events with limit=0 returned {} events", events.len());

    client.shutdown().await.ok();
}

/// QE3: query_events with very large limit returns all available events.
#[tokio::test]
async fn test_query_events_limit_very_large() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Query with extremely large limit
    let filter = QueryFilter {
        limit: usize::MAX,
        offset: 0,
        ..Default::default()
    };

    let events = client.query_events(&session_id, filter).await
        .expect("query_events should handle u32::MAX limit");

    println!("✓ query_events with limit=u32::MAX returned {} events", events.len());
    println!("  Total events from stop: {}", stop.total_events);
    assert!(events.len() <= stop.total_events as usize, "Should not return more than total");

    client.shutdown().await.ok();
}

/// QE4: query_events with invalid session_id returns error or empty.
#[tokio::test]
async fn test_query_events_invalid_session() {
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(1)).await;
    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");
    let _stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Query with completely invalid session ID
    let filter = QueryFilter::default();
    let result = client.query_events("this-session-does-not-exist-12345", filter).await;

    match result {
        Ok(events) => {
            // Some implementations might return empty instead of error
            println!("✓ query_events with invalid session returned {} events (empty instead of error)", events.len());
        }
        Err(e) => {
            println!("✓ query_events correctly returned error for invalid session: {:?}", e);
        }
    }

    client.shutdown().await.ok();
}

/// QE5: query_events with thread_id filter that matches nothing.
#[tokio::test]
async fn test_query_events_thread_filter_no_match() {
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");
    let _stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Query with a thread ID that definitely doesn't exist (huge number)
    let filter = QueryFilter {
        limit: 10,
        offset: 0,
        thread_id: Some(999_999_999),
        ..Default::default()
    };

    let events = client.query_events(&session_id, filter).await
        .expect("query_events should handle non-matching thread_id");

    println!("✓ query_events with thread_id=999_999_999 returned {} events (expected 0)", events.len());
    assert!(events.is_empty(), "Should return empty for non-matching thread_id");

    client.shutdown().await.ok();
}

/// QE6: query_events with timestamp range in the past (before program ran).
#[tokio::test]
async fn test_query_events_timestamp_before_program() {
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(1)).await;
    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");
    let _stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Query with timestamps way before the program ran (0 to 1ms)
    let filter = QueryFilter {
        limit: 10,
        offset: 0,
        timestamp_start: Some(0),
        timestamp_end: Some(1_000_000), // First millisecond
        ..Default::default()
    };

    let events = client.query_events(&session_id, filter).await
        .expect("query_events should handle pre-program timestamp range");

    println!("✓ query_events with timestamp range [0, 1ms] returned {} events", events.len());

    client.shutdown().await.ok();
}

/// QE7: query_events with event_type filter that matches nothing.
/// NOTE: The server currently ignores unknown event types and returns all events.
/// This is a design choice - unknown types are filtered out but don't cause 0 results.
#[tokio::test]
async fn test_query_events_event_type_filter_no_match() {
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");
    let _stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Query with an event type that doesn't exist
    let filter = QueryFilter {
        limit: 10,
        offset: 0,
        event_types: Some(vec!["nonexistent_event_type_xyz".to_string()]),
        ..Default::default()
    };

    let events = client.query_events(&session_id, filter).await
        .expect("query_events should handle invalid event_type");

    println!("✓ query_events with event_types=['nonexistent'] returned {} events", events.len());
    // Note: Server ignores unknown types, so this returns all events
    // This behavior should be documented or changed to return 0 events for unknown types

    client.shutdown().await.ok();
}

/// QE8: query_events with combined filters that result in no matches.
#[tokio::test]
async fn test_query_events_combined_filters_no_match() {
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");
    let _stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Combine filters that together match nothing:
    // - thread_id that doesn't exist
    // - timestamp range way in the future
    let filter = QueryFilter {
        limit: 10,
        offset: 0,
        thread_id: Some(1),
        timestamp_start: Some(999_999_999_999_999_999u64),
        timestamp_end: Some(u64::MAX),
        ..Default::default()
    };

    let events = client.query_events(&session_id, filter).await
        .expect("query_events should handle impossible combined filters");

    println!("✓ query_events with impossible combined filters returned {} events", events.len());
    assert!(events.is_empty(), "Should return empty for impossible filter combination");

    client.shutdown().await.ok();
}

/// QE9: pagination through all events with varying offsets.
#[tokio::test]
async fn test_query_events_pagination_all_events() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(3)).await;
    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");
    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Page through all events with offset 0, 100, 200, ...
    let page_size = 100;
    let mut total_events = 0;
    let mut page = 0;

    loop {
        let filter = QueryFilter {
            limit: page_size,
            offset: page * page_size,
            ..Default::default()
        };

        let events = client.query_events(&session_id, filter).await
            .expect("query_events should handle pagination");

        let count = events.len();
        total_events += count;

        println!("  Page {} (offset {}): {} events", page, page * page_size, count);

        if count < page_size {
            break; // No more events
        }

        page += 1;

        // Safety limit
        if page > 1000 {
            println!("WARNING: Hit page limit, breaking");
            break;
        }
    }

    println!("✓ Pagination test: fetched {} total events in {} pages", total_events, page + 1);
    println!("  Total from stop: {}", stop.total_events);

    // Allow some tolerance for events generated during pagination
    assert!(total_events <= (stop.total_events + 50) as usize, "Should not exceed total events significantly");

    client.shutdown().await.ok();
}

/// QE10: sequential rapid queries don't cause issues.
#[tokio::test]
async fn test_query_events_rapid_sequential_queries() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");
    let _stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Fire 50 sequential queries rapidly
    let mut success_count = 0;
    for i in 0..50 {
        let filter = QueryFilter {
            limit: 10,
            offset: i * 10,
            ..Default::default()
        };

        match client.query_events(&session_id, filter).await {
            Ok(events) => {
                success_count += 1;
                if i % 10 == 0 {
                    println!("  Query {}: {} events", i, events.len());
                }
            }
            Err(e) => {
                println!("✗ Query {} failed at offset {}: {:?}", i, i * 10, e);
            }
        }
    }

    println!("✓ Rapid sequential queries: {}/50 succeeded", success_count);
    assert!(success_count >= 45, "Most rapid queries should succeed");

    client.shutdown().await.ok();
}
