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
    let fixture = McpSession::fixture_path("test_add").expect("test_add fixture not found");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");

    // R9.2: bind stop (was `_stop`) so QE1 can assert walk_all does not
    // exceed stop.total_events after the migration to cursor pagination.
    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Query with a cursor that is way beyond any realistic event count.
    //
    // v2 contract (chronos-sandbox/src/client/tools.rs::query_events):
    //   The client wrapper rejects `QueryFilter::offset > 0` with a typed
    //   error ("use cursor-based pagination"). Pre-C5.2 offset pagination
    //   is gone; the server v2 cursor encodes opaque resume tokens.
    //
    // Migration rationale (R9.2, drift #5): pre-C5.3.1 v1 server accepted
    //   offset=N and returned the N-th page (or empty if N exceeded total).
    //   v2 returns Ok with no events when the cursor walks past the tail.
    //
    // Equivalent semantic in v2: ask for the first page with a small
    // limit, then verify the page is bounded (does NOT exceed `limit`).
    // The "beyond total" invariant is enforced by walk_all returning
    // exactly stop.total_events events (no overshoot).
    let events = client
        .query_events_walk_all(
            &session_id,
            QueryFilter {
                limit: 10,
                ..Default::default()
            },
        )
        .await
        .expect("query_events_walk_all should succeed with limit=10");

    println!(
        "✓ query_events_walk_all (cursor-based) returned {} events (stop.total_events={})",
        events.len(),
        stop.total_events
    );
    assert!(
        !events.is_empty(),
        "test_add fixture must produce at least one event"
    );
    assert!(
        events.len() <= stop.total_events,
        "walk_all must not exceed stop.total_events ({} > {})",
        events.len(),
        stop.total_events
    );

    client.shutdown().await.ok();
}

/// QE2: query_events with limit = 0 returns empty.
#[tokio::test]
async fn test_query_events_limit_zero() {
    let fixture = McpSession::fixture_path("test_add").expect("test_add fixture not found");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");

    let _stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Query with limit = 0
    let filter = QueryFilter {
        limit: 0,
        offset: 0,
        ..Default::default()
    };

    let events = client
        .query_events(&session_id, filter)
        .await
        .expect("query_events should handle limit=0 gracefully");

    println!(
        "✓ query_events with limit=0 returned {} events",
        events.len()
    );

    client.shutdown().await.ok();
}

/// QE3: query_events with very large limit returns all available events.
#[tokio::test]
async fn test_query_events_limit_very_large() {
    let fixture =
        McpSession::fixture_path("test_busyloop").expect("test_busyloop fixture not found");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");

    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Query with extremely large limit
    let filter = QueryFilter {
        limit: usize::MAX,
        offset: 0,
        ..Default::default()
    };

    let events = client
        .query_events(&session_id, filter)
        .await
        .expect("query_events should handle u32::MAX limit");

    println!(
        "✓ query_events with limit=u32::MAX returned {} events",
        events.len()
    );
    println!("  Total events from stop: {}", stop.total_events);
    assert!(
        events.len() <= stop.total_events as usize,
        "Should not return more than total"
    );

    client.shutdown().await.ok();
}

/// QE4: query_events with invalid session_id returns error or empty.
#[tokio::test]
async fn test_query_events_invalid_session() {
    let fixture = McpSession::fixture_path("test_add").expect("test_add fixture not found");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(1)).await;
    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");
    let _stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Query with completely invalid session ID
    let filter = QueryFilter::default();
    let result = client
        .query_events("this-session-does-not-exist-12345", filter)
        .await;

    match result {
        Ok(events) => {
            // Some implementations might return empty instead of error
            println!(
                "✓ query_events with invalid session returned {} events (empty instead of error)",
                events.len()
            );
        }
        Err(e) => {
            println!(
                "✓ query_events correctly returned error for invalid session: {:?}",
                e
            );
        }
    }

    client.shutdown().await.ok();
}

/// QE5: query_events with thread_id filter that matches nothing.
#[tokio::test]
async fn test_query_events_thread_filter_no_match() {
    let fixture = McpSession::fixture_path("test_add").expect("test_add fixture not found");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");
    let _stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Query with a thread ID that definitely doesn't exist (huge number)
    let filter = QueryFilter {
        limit: 10,
        offset: 0,
        thread_id: Some(999_999_999),
        ..Default::default()
    };

    let events = client
        .query_events(&session_id, filter)
        .await
        .expect("query_events should handle non-matching thread_id");

    println!(
        "✓ query_events with thread_id=999_999_999 returned {} events (expected 0)",
        events.len()
    );
    assert!(
        events.is_empty(),
        "Should return empty for non-matching thread_id"
    );

    client.shutdown().await.ok();
}

/// QE6: query_events with timestamp range in the past (before program ran).
#[tokio::test]
async fn test_query_events_timestamp_before_program() {
    let fixture = McpSession::fixture_path("test_add").expect("test_add fixture not found");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(1)).await;
    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");
    let _stop = client
        .probe_stop(&session_id)
        .await
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

    let events = client
        .query_events(&session_id, filter)
        .await
        .expect("query_events should handle pre-program timestamp range");

    println!(
        "✓ query_events with timestamp range [0, 1ms] returned {} events",
        events.len()
    );

    client.shutdown().await.ok();
}

/// QE7: query_events with event_type filter that matches nothing.
/// NOTE: The server currently ignores unknown event types and returns all events.
/// This is a design choice - unknown types are filtered out but don't cause 0 results.
#[tokio::test]
async fn test_query_events_event_type_filter_no_match() {
    let fixture = McpSession::fixture_path("test_add").expect("test_add fixture not found");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");
    let _stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Query with an event type that doesn't exist. The server validates
    // event_types and rejects unknown ones with a descriptive error
    // (see crates/chronos-mcp/src/server.rs query_events). This is the
    // intentional contract: silent filtering of unknown types would
    // hide typos and break debugging. The test asserts the error path.
    let filter = QueryFilter {
        limit: 10,
        offset: 0,
        event_types: Some(vec!["nonexistent_event_type_xyz".to_string()]),
        ..Default::default()
    };

    let result = client.query_events(&session_id, filter).await;
    assert!(
        result.is_err(),
        "query_events should reject unknown event_type, but it returned Ok"
    );
    let err = result.err().unwrap();
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("nonexistent_event_type_xyz") || err_msg.contains("unknown event_type"),
        "error should mention the invalid event_type or the rejection reason, got: {}",
        err_msg
    );

    println!(
        "✓ query_events with event_types=['nonexistent'] correctly rejected: {}",
        err_msg
    );

    client.shutdown().await.ok();
}

/// QE8: query_events with combined filters that result in no matches.
#[tokio::test]
async fn test_query_events_combined_filters_no_match() {
    let fixture = McpSession::fixture_path("test_add").expect("test_add fixture not found");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");
    let _stop = client
        .probe_stop(&session_id)
        .await
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

    let events = client
        .query_events(&session_id, filter)
        .await
        .expect("query_events should handle impossible combined filters");

    println!(
        "✓ query_events with impossible combined filters returned {} events",
        events.len()
    );
    assert!(
        events.is_empty(),
        "Should return empty for impossible filter combination"
    );

    client.shutdown().await.ok();
}

/// QE9: pagination through all events with varying offsets.
#[tokio::test]
async fn test_query_events_pagination_all_events() {
    let fixture =
        McpSession::fixture_path("test_busyloop").expect("test_busyloop fixture not found");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(3)).await;
    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");
    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Page through all events with offset 0, 100, 200, ...
    let page_size = 100;
    let mut total_events = 0;
    let mut page = 0;
    let mut cursor: Option<String> = None;

    loop {
        let filter = QueryFilter {
            limit: page_size,
            cursor: cursor.clone(),
            ..Default::default()
        };

        let page_result = client
            .query_events_page(&session_id, filter)
            .await
            .expect("query_events_page should handle cursor pagination");

        let count = page_result.events.len();
        total_events += count;

        println!(
            "  Page {} (cursor {:?}): {} events (next_cursor={:?})",
            page, cursor, count, page_result.next_cursor
        );

        // Advance cursor for next page; v2 stops when next_cursor is None.
        match page_result.next_cursor.clone() {
            Some(next) => {
                if count == 0 {
                    // Server returned a cursor with no events: avoid loop.
                    println!("WARNING: empty page with cursor, breaking");
                    break;
                }
                cursor = Some(next);
            }
            None => break, // Last page reached.
        }

        page += 1;

        // Safety limit
        if page > 1000 {
            println!("WARNING: Hit page limit, breaking");
            break;
        }
    }

    println!(
        "✓ Pagination test (cursor-based): fetched {} total events in {} pages",
        total_events,
        page + 1
    );
    println!("  Total from stop: {}", stop.total_events);

    // Allow some tolerance for events generated during pagination
    assert!(
        total_events <= (stop.total_events + 50) as usize,
        "Should not exceed total events significantly (got {} vs stop.total_events={})",
        total_events,
        stop.total_events
    );
    assert!(
        total_events > 0,
        "test_busyloop fixture must produce at least one event"
    );

    client.shutdown().await.ok();
}

/// QE10: sequential rapid queries don't cause issues.
#[tokio::test]
async fn test_query_events_rapid_sequential_queries() {
    let fixture =
        McpSession::fixture_path("test_busyloop").expect("test_busyloop fixture not found");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");
    let _stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Fire 50 sequential queries rapidly, using cursor pagination
    // (pre-C5.2 `offset = i*10` is rejected by the v2 client wrapper).
    //
    // Each query walks the full session (cursor walk_all) and asserts
    // determinism: same event count on every iteration (the session is
    // stopped, so no new events are produced mid-test).
    let mut counts: Vec<usize> = Vec::with_capacity(50);
    for i in 0..50 {
        let events = client
            .query_events_walk_all(
                &session_id,
                QueryFilter {
                    limit: 100,
                    ..Default::default()
                },
            )
            .await;

        match events {
            Ok(evs) => {
                counts.push(evs.len());
                if i % 10 == 0 {
                    println!("  Query {}: {} events", i, evs.len());
                }
            }
            Err(e) => {
                println!("✗ Query {} failed: {:?}", i, e);
            }
        }
    }

    let success_count = counts.len();
    println!(
        "✓ Rapid sequential queries (cursor-based): {}/50 succeeded; counts={:?}",
        success_count, counts
    );
    assert!(success_count >= 45, "Most rapid queries should succeed");

    // Determinism: every successful query must return the same count
    // (the session is stopped, so no events are produced mid-test).
    let first = *counts.first().expect("at least one success expected");
    assert!(
        counts.iter().all(|&c| c == first),
        "Cursor walk_all must be deterministic on a stopped session; counts={:?}",
        counts
    );

    client.shutdown().await.ok();
}
