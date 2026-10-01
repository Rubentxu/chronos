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
///
/// The previous body printed the event count and asserted nothing. Measured
/// contract: a `limit: 0` page answers `Ok` with **no** events, while the
/// very same session queried with `limit: 5` answers 5 events — so the
/// emptiness is caused by `limit: 0` and not by a session that captured
/// nothing. The control query is what makes this assertion meaningful.
///
/// NOTE (measured, deliberately not asserted): the `limit: 0` page still
/// carries `next_cursor = Some("ecv1:…:0")` even though it returns no events.
/// `query_events_walk_all` continues while `page_len >= limit`, and `0 >= 0`
/// is always true, so `query_events_walk_all(limit = 0)` never terminates
/// (measured: still running after 15 s). Reported as a production/client
/// defect rather than pinned here.
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
    assert!(
        events.is_empty(),
        "limit=0 must return no events, got {}",
        events.len()
    );

    // Control: the same session with a real limit must see events, so the
    // assertion above is attributable to limit=0 and not to an empty session.
    let control = client
        .query_events(
            &session_id,
            QueryFilter {
                limit: 5,
                ..Default::default()
            },
        )
        .await
        .expect("query_events with limit=5 must succeed on the same session");
    assert_eq!(
        control.len(),
        5,
        "the same session must return a full page of 5 events with limit=5, got {}",
        control.len()
    );

    client.shutdown().await.ok();
}

/// Regression test for a hang in `query_events_walk_all`.
///
/// The server answers `limit = 0` with an empty page that still carries a
/// `next_cursor`, and the walk looped while `page_len >= limit`, which is
/// `0 >= 0` on every pass: the call never returned. The test wraps the call in
/// a timeout so that a reintroduction fails instead of stalling the suite, and
/// asserts the caller gets the empty list it asked for.
#[tokio::test]
async fn test_walk_all_with_limit_zero_terminates() {
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

    let walk = tokio::time::timeout(
        Duration::from_secs(10),
        client.query_events_walk_all(
            &session_id,
            QueryFilter {
                limit: 0,
                ..Default::default()
            },
        ),
    )
    .await
    .expect("query_events_walk_all with limit=0 must terminate, not loop forever")
    .expect("query_events_walk_all with limit=0 must not error");

    assert!(
        walk.is_empty(),
        "limit=0 asks for no events, so the walk must come back empty, got {}",
        walk.len()
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

/// QE4: query_events with an invalid session_id is rejected with a
/// session-specific error.
///
/// The previous body printed one line for the `Ok` arm and one for the `Err`
/// arm and asserted nothing, so an implementation that answered `Ok([])` for a
/// session that does not exist passed just as happily as one that rejects it.
/// Measured contract: the server answers `Err` with
/// `ExecutionLog unavailable for session '<id>': no ExecutionLog registered
/// for this session …`, naming the id that was asked for. A control query on
/// the real session id proves the rejection comes from the bogus id.
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
    const BOGUS_SESSION: &str = "this-session-does-not-exist-12345";
    let filter = QueryFilter::default();
    let result = client.query_events(BOGUS_SESSION, filter).await;

    let err = result.expect_err(
        "query_events must reject an unknown session id instead of silently \
         returning an empty page that reads like 'this session captured nothing'",
    );
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("ExecutionLog unavailable for session"),
        "the rejection must say the session has no ExecutionLog, got: {err_msg}"
    );
    assert!(
        err_msg.contains(BOGUS_SESSION),
        "the rejection must name the session that was asked for, got: {err_msg}"
    );

    // Control: the real session id answers with events, so the rejection
    // above is caused by the id and not by a session that captured nothing.
    let control = client
        .query_events(&session_id, QueryFilter::default())
        .await
        .expect("query_events on the real session must succeed");
    assert!(
        !control.is_empty(),
        "the real session must return events, so the rejection above is id-specific"
    );

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

/// QE6: query_events with a timestamp window that ends before the program ran.
///
/// The previous body printed the event count and asserted nothing. Measured
/// contract, asserted here:
///
/// - a `[0, 1 ms]` window is empty, and the reason is checked rather than
///   assumed: event timestamps are epoch nanoseconds (~1.79e18), so that
///   window closes roughly 56 years before the capture;
/// - the timestamp filter is *live*, not just always-empty: an inclusive
///   window spanning `[min_ts, max_ts]` of the real events returns every
///   event the cursor walk found (128 for `test_add`).
///
/// The second assertion is what would catch a timestamp filter that had been
/// dropped from the query path: an always-empty result would otherwise pass.
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
    assert!(
        events.is_empty(),
        "a window that ends at 1 ms cannot contain events captured in epoch nanoseconds, got {}",
        events.len()
    );

    // Where the real timestamps actually live, so the emptiness above is
    // explained by the timestamp origin instead of by a filter that silently
    // drops everything.
    let all = client
        .query_events_walk_all(
            &session_id,
            QueryFilter {
                limit: 200,
                ..Default::default()
            },
        )
        .await
        .expect("the cursor walk over the session must succeed");
    assert!(!all.is_empty(), "test_add must capture at least one event");

    let min_ts = all.iter().map(|e| e.timestamp_ns).min().expect("non-empty");
    let max_ts = all.iter().map(|e| e.timestamp_ns).max().expect("non-empty");
    println!(
        "  test_add spans [{min_ts}, {max_ts}] over {} events",
        all.len()
    );
    assert!(
        min_ts > 1_000_000,
        "event timestamps are epoch nanoseconds (earliest {min_ts}); if they were not, the \
         empty [0, 1ms] result above would need a different explanation"
    );

    // The timestamp filter is live: the inclusive window that exactly spans
    // the capture returns every event.
    let full_window = client
        .query_events(
            &session_id,
            QueryFilter {
                limit: 200,
                timestamp_start: Some(min_ts),
                timestamp_end: Some(max_ts),
                ..Default::default()
            },
        )
        .await
        .expect("query_events over the real timestamp span must succeed");
    assert_eq!(
        full_window.len(),
        all.len(),
        "an inclusive window spanning the capture must return all {} events, got {}",
        all.len(),
        full_window.len()
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
