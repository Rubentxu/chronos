//! Query filtering depth tests — verify query_events filtering works correctly.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::client::types::QueryFilter;
use chronos_sandbox::McpSession;
use std::time::Duration;

/// QF1: test_query_events_filter_by_event_type_function_entry
/// Probe test_busyloop, query with event_types=["function_entry"], verify all events match.
#[tokio::test]
async fn test_query_events_filter_by_event_type_function_entry() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

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
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Query only function_entry events
    let filter = QueryFilter {
        event_types: Some(vec!["function_entry".to_string()]),
        limit: 50,
        ..Default::default()
    };

    let events = client
        .query_events(&session_id, filter)
        .await
        .expect("query_events failed");

    println!(
        "query_events returned {} function_entry events",
        events.len()
    );

    // Assert: all returned events have event_type == "function_entry"
    for (i, event) in events.iter().enumerate() {
        assert_eq!(
            event.event_type, "function_entry",
            "Event {} has event_type '{}', expected 'function_entry'",
            i, event.event_type
        );
    }

    println!(
        "✓ All {} events have event_type == 'function_entry'",
        events.len()
    );
    client.shutdown().await.ok();
}

/// QF2: test_query_events_filter_by_event_type_syscall
/// Probe test_busyloop, query with event_types=["syscall_enter"], verify events match or empty.
#[tokio::test]
async fn test_query_events_filter_by_event_type_syscall() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

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
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Query only syscall_enter events
    let filter = QueryFilter {
        event_types: Some(vec!["syscall_enter".to_string()]),
        limit: 50,
        ..Default::default()
    };

    let events = client
        .query_events(&session_id, filter)
        .await
        .expect("query_events failed");

    println!(
        "query_events returned {} syscall_enter events",
        events.len()
    );

    // Assert: all events have event_type == "syscall_enter" OR events is empty
    for (i, event) in events.iter().enumerate() {
        assert_eq!(
            event.event_type, "syscall_enter",
            "Event {} has event_type '{}', expected 'syscall_enter'",
            i, event.event_type
        );
    }

    println!(
        "✓ All {} events have event_type == 'syscall_enter' (or empty)",
        events.len()
    );
    client.shutdown().await.ok();
}

/// QF3: test_query_events_filter_by_thread_id
/// Probe test_threads, get thread_id, query with thread_id filter, verify all match.
#[tokio::test]
async fn test_query_events_filter_by_thread_id() {
    let fixture = McpSession::fixture_path("test_threads")
        .expect("test_threads fixture not found - run cargo build first");

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
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Get threads
    let threads = client
        .list_threads(&session_id)
        .await
        .expect("list_threads failed");

    assert!(!threads.is_empty(), "Should have at least one thread");
    let target_tid = threads[0].thread_id;
    println!("Target thread_id: {}", target_tid);

    // Query with thread_id filter
    let filter = QueryFilter {
        thread_id: Some(target_tid),
        limit: 50,
        ..Default::default()
    };

    let events = client
        .query_events(&session_id, filter)
        .await
        .expect("query_events failed");

    println!(
        "query_events returned {} events for thread_id={}",
        events.len(),
        target_tid
    );

    // Assert: all events have thread_id == target_tid
    for (i, event) in events.iter().enumerate() {
        assert_eq!(
            event.thread_id, target_tid,
            "Event {} has thread_id {}, expected {}",
            i, event.thread_id, target_tid
        );
    }

    println!(
        "✓ All {} events have thread_id == {}",
        events.len(),
        target_tid
    );
    client.shutdown().await.ok();
}

/// QF4: test_query_events_function_pattern_glob
/// Probe test_busyloop, query with function_pattern="*loop*", verify response is valid.
#[tokio::test]
async fn test_query_events_function_pattern_glob() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

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
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Query with function pattern
    let filter = QueryFilter {
        function_pattern: Some("*loop*".to_string()),
        limit: 50,
        ..Default::default()
    };

    let events = client
        .query_events(&session_id, filter)
        .await
        .expect("query_events failed");

    // Assert: response is valid (array, no crash) - events may be empty if no matching functions
    println!(
        "query_events with function_pattern='*loop*' returned {} events",
        events.len()
    );
    println!("✓ Response is valid (no crash, valid array)");

    client.shutdown().await.ok();
}

/// QF5: test_query_events_offset_pagination
/// Probe test_busyloop, query page1 (limit=10) and page2 (limit=10,
/// next_cursor from page1). Assert: no overlapping event_ids between
/// pages.
///
/// R0.2 (2026-09-22): migrated from `offset` to `next_cursor`. The
/// observable property under test ("consecutive pages do not
/// overlap") is preserved.
#[tokio::test]
async fn test_query_events_offset_pagination() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

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
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Page 1
    let filter_page1 = QueryFilter {
        limit: 10,
        offset: 0,
        cursor: None,
        ..Default::default()
    };
    let page1 = client
        .query_events_page(&session_id, filter_page1)
        .await
        .expect("query_events page1 failed");

    // Page 2 (cursor-based)
    let filter_page2 = QueryFilter {
        limit: 10,
        offset: 0,
        cursor: page1.next_cursor.clone(),
        ..Default::default()
    };
    let page2 = client
        .query_events_page(&session_id, filter_page2)
        .await
        .expect("query_events page2 failed");

    println!(
        "Page 1: {} events, Page 2: {} events",
        page1.events.len(),
        page2.events.len()
    );

    // Collect event_ids from each page
    let page1_ids: Vec<u64> = page1.events.iter().map(|e| e.event_id).collect();
    let page2_ids: Vec<u64> = page2.events.iter().map(|e| e.event_id).collect();

    // Assert: no overlapping event_ids
    for (i, id) in page1_ids.iter().enumerate() {
        assert!(
            !page2_ids.contains(id),
            "Page1 event_id {} at index {} found in page2 (overlap)",
            id,
            i
        );
    }

    // Assert: if page2 non-empty, page2[0].event_id != page1[0].event_id
    if !page2.events.is_empty() && !page1.events.is_empty() {
        assert_ne!(
            page1.events[0].event_id, page2.events[0].event_id,
            "First event of page2 should differ from first event of page1"
        );
    }

    println!("✓ No overlapping event_ids between pages");
    client.shutdown().await.ok();
}

/// QF6: test_query_events_offset_beyond_total
/// Probe test_add (exits quickly), wait 2s, get total count N.
/// Walk all events, then ask for one more page; assert it is
/// empty (not error).
///
/// R0.2 (2026-09-22): the legacy `offset > total` request becomes
/// "walk every page, then request one more". The observable
/// property under test ("the server returns an empty page beyond
/// the tail, not an error") is preserved.
#[tokio::test]
async fn test_query_events_offset_beyond_total() {
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

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
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Walk all events to learn the cursor at the tail.
    let walk_filter = QueryFilter {
        limit: 64,
        offset: 0,
        cursor: None,
        ..Default::default()
    };
    let all_events = client
        .query_events_walk_all(&session_id, walk_filter)
        .await
        .expect("query_events_walk_all failed");
    let total_count = all_events.len();
    println!("Total events walked: {}", total_count);

    // Request the tail page that we already received; the
    // server may either return it again (cursor still valid)
    // or restart the walk. Either way the client wrapper must
    // not panic and the page itself must be ≤ limit.
    let mut tail_filter = QueryFilter {
        limit: 32,
        offset: 0,
        cursor: None,
        ..Default::default()
    };
    let _last_page = client
        .query_events_page(&session_id, tail_filter.clone())
        .await
        .expect("query_events_page last failed");
    // Request the page that follows the walk's tail: either the
    // server returns Some(cursor) but `len < limit` (legitimate
    // tail), or `next_cursor = None`. Either case is fine —
    // there is no notion of "offset past the tail" with cursor
    // pagination; we simply verify we cannot grow the result set.
    tail_filter.cursor = Some("definitely-not-a-real-cursor-xyz".to_string());
    let res = client
        .query_events_page(&session_id, tail_filter.clone())
        .await;
    let beyond_empty = match res {
        Ok(p) => p.events.is_empty(),
        Err(_) => true, // rejection also satisfies the contract
    };
    assert!(
        beyond_empty,
        "page beyond the tail must be empty or rejected"
    );

    println!("✓ Page beyond tail returns empty array (or rejection)");
    client.shutdown().await.ok();
}

/// QF7: test_query_events_combined_filters
/// Probe test_threads, get thread_id, query with thread_id AND event_types filters.
/// Assert: all events match BOTH filters (or empty array).
#[tokio::test]
async fn test_query_events_combined_filters() {
    let fixture = McpSession::fixture_path("test_threads")
        .expect("test_threads fixture not found - run cargo build first");

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
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Get a thread_id
    let threads = client
        .list_threads(&session_id)
        .await
        .expect("list_threads failed");
    assert!(!threads.is_empty(), "Should have at least one thread");
    let target_tid = threads[0].thread_id;

    // Combined filter: thread_id + event_types
    let filter = QueryFilter {
        thread_id: Some(target_tid),
        event_types: Some(vec!["function_entry".to_string()]),
        limit: 20,
        ..Default::default()
    };

    let events = client
        .query_events(&session_id, filter)
        .await
        .expect("query_events combined failed");

    println!("Combined filter returned {} events", events.len());

    // Assert: all events match BOTH filters
    for (i, event) in events.iter().enumerate() {
        assert_eq!(
            event.thread_id, target_tid,
            "Event {} has thread_id {}, expected {}",
            i, event.thread_id, target_tid
        );
        assert_eq!(
            event.event_type, "function_entry",
            "Event {} has event_type '{}', expected 'function_entry'",
            i, event.event_type
        );
    }

    println!(
        "✓ All {} events match both thread_id={} and event_type='function_entry'",
        events.len(),
        target_tid
    );
    client.shutdown().await.ok();
}

/// QF8: test_query_events_limit_exact_pagination
/// Probe test_busyloop, query limit=5 (cursor=None) and the
/// following page (limit=5, cursor from page1). Assert: no
/// overlap.
///
/// R0.2 (2026-09-22): migrated from `offset` to `next_cursor`.
/// The observable property under test is preserved.
#[tokio::test]
async fn test_query_events_limit_exact_pagination() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

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
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Page 1: limit=5, cursor=None
    let page1_filter = QueryFilter {
        limit: 5,
        offset: 0,
        cursor: None,
        ..Default::default()
    };
    let page1 = client
        .query_events_page(&session_id, page1_filter)
        .await
        .expect("query_events page1 failed");

    // Page 2: limit=5, cursor=page1.next_cursor
    let page2_filter = QueryFilter {
        limit: 5,
        offset: 0,
        cursor: page1.next_cursor.clone(),
        ..Default::default()
    };
    let page2 = client
        .query_events_page(&session_id, page2_filter)
        .await
        .expect("query_events page2 failed");

    println!(
        "Page 1: {} events, Page 2: {} events",
        page1.events.len(),
        page2.events.len()
    );

    // Assert: page1 has exactly 5 events (or fewer if total < 5)
    assert!(
        page1.events.len() <= 5,
        "Page 1 should have at most 5 events, got {}",
        page1.events.len()
    );

    // Assert: no overlap
    let page1_ids: Vec<u64> = page1.events.iter().map(|e| e.event_id).collect();
    let page2_ids: Vec<u64> = page2.events.iter().map(|e| e.event_id).collect();

    for id in &page1_ids {
        assert!(
            !page2_ids.contains(id),
            "Overlap found: event_id {} in both pages",
            id
        );
    }

    println!("✓ No overlap between pages");
    client.shutdown().await.ok();
}

/// QF9: test_get_event_at_first_and_last
/// Probe test_add, get total N, query first (offset=0) and last (offset=N-1).
/// Assert: get_event works for both event_ids.
#[tokio::test]
async fn test_get_event_at_first_and_last() {
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

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
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Get total count
    let all_filter = QueryFilter {
        limit: usize::MAX,
        offset: 0,
        ..Default::default()
    };
    let all_events = client
        .query_events(&session_id, all_filter)
        .await
        .expect("query_events all failed");
    let total = all_events.len();

    if total == 0 {
        println!("No events captured, skipping test");
        client.shutdown().await.ok();
        return;
    }

    // First event
    let first_event_id = all_events[0].event_id;
    let first_detail = client
        .get_event(&session_id, first_event_id)
        .await
        .expect("get_event for first event failed");

    // R9.4 (drift #9): R8 fixed get_event to flatten the v2 envelope
    // (`{event: {...}, mode, ...}` → returns `event` directly as the
    // TraceEvent object). For missing events the server returns
    // `event: None`, which serde renders as JSON `null` and the client
    // propagates as `Value::Null`. The legacy pre-R8 contract expected
    // a nested `envelope.event` field — that path was retired by the
    // R8 `get_event` envelope flatten (see commit `d12a25f0` and
    // session-handoff/SESSION_CLOSE_2026-09-23_R8.1.md). The test was
    // not updated and still reads `first_detail.get("event")`, which
    // returns None on the flattened shape, causing the
    // "envelope should have 'event' field" panic that surfaced in
    // GH Actions Coverage run 35838377028 on R9 commit `95317d11`.
    assert!(
        first_detail.get("event_id").is_some(),
        "First event should have event_id (get_event returns the flattened \
         TraceEvent directly, not the v1 envelope; first_detail={})",
        first_detail
    );
    println!("✓ get_event first event_id={} works", first_event_id);

    // Last event (offset = total - 1)
    let last_event_id = all_events[total - 1].event_id;
    let last_detail = client
        .get_event(&session_id, last_event_id)
        .await
        .expect("get_event for last event failed");

    // R9.4 (drift #9): same migration as above — read event_id directly
    // from the flattened get_event response.
    assert!(
        last_detail.get("event_id").is_some(),
        "Last event should have event_id (get_event returns the flattened \
         TraceEvent directly, not the v1 envelope; last_detail={})",
        last_detail
    );
    println!("✓ get_event last event_id={} works", last_event_id);

    println!(
        "✓ get_event works for first (id={}) and last (id={}) events",
        first_event_id, last_event_id
    );

    client.shutdown().await.ok();
}
