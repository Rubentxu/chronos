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

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Query only function_entry events
    let filter = QueryFilter {
        event_types: Some(vec!["function_entry".to_string()]),
        limit: 50,
        ..Default::default()
    };

    let events = client.query_events(&session_id, filter).await
        .expect("query_events failed");

    println!("query_events returned {} function_entry events", events.len());

    // Assert: all returned events have event_type == "function_entry"
    for (i, event) in events.iter().enumerate() {
        assert_eq!(
            event.event_type, "function_entry",
            "Event {} has event_type '{}', expected 'function_entry'",
            i, event.event_type
        );
    }

    println!("✓ All {} events have event_type == 'function_entry'", events.len());
    client.shutdown().await.ok();
}

/// QF2: test_query_events_filter_by_event_type_syscall
/// Probe test_busyloop, query with event_types=["syscall_enter"], verify events match or empty.
#[tokio::test]
async fn test_query_events_filter_by_event_type_syscall() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Query only syscall_enter events
    let filter = QueryFilter {
        event_types: Some(vec!["syscall_enter".to_string()]),
        limit: 50,
        ..Default::default()
    };

    let events = client.query_events(&session_id, filter).await
        .expect("query_events failed");

    println!("query_events returned {} syscall_enter events", events.len());

    // Assert: all events have event_type == "syscall_enter" OR events is empty
    for (i, event) in events.iter().enumerate() {
        assert_eq!(
            event.event_type, "syscall_enter",
            "Event {} has event_type '{}', expected 'syscall_enter'",
            i, event.event_type
        );
    }

    println!("✓ All {} events have event_type == 'syscall_enter' (or empty)", events.len());
    client.shutdown().await.ok();
}

/// QF3: test_query_events_filter_by_thread_id
/// Probe test_threads, get thread_id, query with thread_id filter, verify all match.
#[tokio::test]
async fn test_query_events_filter_by_thread_id() {
    let fixture = McpSession::fixture_path("test_threads")
        .expect("test_threads fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(3)).await;
    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Get threads
    let threads = client.list_threads(&session_id).await
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

    let events = client.query_events(&session_id, filter).await
        .expect("query_events failed");

    println!("query_events returned {} events for thread_id={}", events.len(), target_tid);

    // Assert: all events have thread_id == target_tid
    for (i, event) in events.iter().enumerate() {
        assert_eq!(
            event.thread_id, target_tid,
            "Event {} has thread_id {}, expected {}",
            i, event.thread_id, target_tid
        );
    }

    println!("✓ All {} events have thread_id == {}", events.len(), target_tid);
    client.shutdown().await.ok();
}

/// QF4: test_query_events_function_pattern_glob
/// Probe test_busyloop, query with function_pattern="*loop*", verify response is valid.
#[tokio::test]
async fn test_query_events_function_pattern_glob() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Query with function pattern
    let filter = QueryFilter {
        function_pattern: Some("*loop*".to_string()),
        limit: 50,
        ..Default::default()
    };

    let events = client.query_events(&session_id, filter).await
        .expect("query_events failed");

    // Assert: response is valid (array, no crash) - events may be empty if no matching functions
    println!("query_events with function_pattern='*loop*' returned {} events", events.len());
    println!("✓ Response is valid (no crash, valid array)");

    client.shutdown().await.ok();
}

/// QF5: test_query_events_offset_pagination
/// Probe test_busyloop, query page1 (limit=10, offset=0) and page2 (limit=10, offset=10).
/// Assert: no overlapping event_ids between pages.
#[tokio::test]
async fn test_query_events_offset_pagination() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Page 1
    let filter_page1 = QueryFilter {
        limit: 10,
        offset: 0,
        ..Default::default()
    };
    let page1 = client.query_events(&session_id, filter_page1).await
        .expect("query_events page1 failed");

    // Page 2
    let filter_page2 = QueryFilter {
        limit: 10,
        offset: 10,
        ..Default::default()
    };
    let page2 = client.query_events(&session_id, filter_page2).await
        .expect("query_events page2 failed");

    println!("Page 1: {} events, Page 2: {} events", page1.len(), page2.len());

    // Collect event_ids from each page
    let page1_ids: Vec<u64> = page1.iter().map(|e| e.event_id).collect();
    let page2_ids: Vec<u64> = page2.iter().map(|e| e.event_id).collect();

    // Assert: no overlapping event_ids
    for (i, id) in page1_ids.iter().enumerate() {
        assert!(
            !page2_ids.contains(id),
            "Page1 event_id {} at index {} found in page2 (overlap)",
            id, i
        );
    }

    // Assert: if page2 non-empty, page2[0].event_id != page1[0].event_id
    if !page2.is_empty() && !page1.is_empty() {
        assert_ne!(
            page1[0].event_id, page2[0].event_id,
            "First event of page2 should differ from first event of page1"
        );
    }

    println!("✓ No overlapping event_ids between pages");
    client.shutdown().await.ok();
}

/// QF6: test_query_events_offset_beyond_total
/// Probe test_add (exits quickly), wait 2s, get total count N.
/// Query with offset=N+1000, assert returns empty array (not error).
#[tokio::test]
async fn test_query_events_offset_beyond_total() {
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Get total count
    let all_filter = QueryFilter {
        limit: usize::MAX,
        offset: 0,
        ..Default::default()
    };
    let all_events = client.query_events(&session_id, all_filter).await
        .expect("query_events all failed");
    let total_count = all_events.len();
    println!("Total events: {}", total_count);

    // Query with offset beyond total
    let beyond_filter = QueryFilter {
        limit: 100,
        offset: total_count + 1000,
        ..Default::default()
    };
    let beyond_events = client.query_events(&session_id, beyond_filter).await
        .expect("query_events beyond total failed");

    // Assert: returns empty array
    assert!(
        beyond_events.is_empty(),
        "Expected empty array for offset beyond total, got {} events",
        beyond_events.len()
    );

    println!("✓ Offset beyond total returns empty array (not error)");
    client.shutdown().await.ok();
}

/// QF7: test_query_events_combined_filters
/// Probe test_threads, get thread_id, query with thread_id AND event_types filters.
/// Assert: all events match BOTH filters (or empty array).
#[tokio::test]
async fn test_query_events_combined_filters() {
    let fixture = McpSession::fixture_path("test_threads")
        .expect("test_threads fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(3)).await;
    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Get a thread_id
    let threads = client.list_threads(&session_id).await
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

    let events = client.query_events(&session_id, filter).await
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

    println!("✓ All {} events match both thread_id={} and event_type='function_entry'",
        events.len(), target_tid);
    client.shutdown().await.ok();
}

/// QF8: test_query_events_limit_exact_pagination
/// Probe test_busyloop, query limit=5, offset=0 and limit=5, offset=5.
/// Assert: no overlap.
#[tokio::test]
async fn test_query_events_limit_exact_pagination() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Page 1: offset=0, limit=5
    let page1_filter = QueryFilter {
        limit: 5,
        offset: 0,
        ..Default::default()
    };
    let page1 = client.query_events(&session_id, page1_filter).await
        .expect("query_events page1 failed");

    // Page 2: offset=5, limit=5
    let page2_filter = QueryFilter {
        limit: 5,
        offset: 5,
        ..Default::default()
    };
    let page2 = client.query_events(&session_id, page2_filter).await
        .expect("query_events page2 failed");

    println!("Page 1: {} events, Page 2: {} events", page1.len(), page2.len());

    // Assert: page1 has exactly 5 events (or fewer if total < 5)
    assert!(
        page1.len() <= 5,
        "Page 1 should have at most 5 events, got {}",
        page1.len()
    );

    // Assert: no overlap
    let page1_ids: Vec<u64> = page1.iter().map(|e| e.event_id).collect();
    let page2_ids: Vec<u64> = page2.iter().map(|e| e.event_id).collect();

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

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Get total count
    let all_filter = QueryFilter {
        limit: usize::MAX,
        offset: 0,
        ..Default::default()
    };
    let all_events = client.query_events(&session_id, all_filter).await
        .expect("query_events all failed");
    let total = all_events.len();

    if total == 0 {
        println!("No events captured, skipping test");
        client.shutdown().await.ok();
        return;
    }

    // First event
    let first_event_id = all_events[0].event_id;
    let first_detail = client.get_event(&session_id, first_event_id).await
        .expect("get_event for first event failed");

    assert!(first_detail.get("event_id").is_some(), "First event should have event_id");
    println!("✓ get_event first event_id={} works", first_event_id);

    // Last event (offset = total - 1)
    let last_event_id = all_events[total - 1].event_id;
    let last_detail = client.get_event(&session_id, last_event_id).await
        .expect("get_event for last event failed");

    assert!(last_detail.get("event_id").is_some(), "Last event should have event_id");
    println!("✓ get_event last event_id={} works", last_event_id);

    println!("✓ get_event works for first (id={}) and last (id={}) events",
        first_event_id, last_event_id);

    client.shutdown().await.ok();
}
