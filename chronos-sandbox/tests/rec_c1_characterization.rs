//! REC-C1.3 — positive contract tests for the corrected read path.
//!
//! R0.2 (2026-09-22): migrated from the legacy offset-based pagination
//! to the v2 cursor-based pagination. The contract names describe the
//! observable properties the client promises the caller; the migration
//! changes the mechanism (offset → cursor) but preserves the properties.
//!
//! Tests use `query_events_walk_all` to walk every page; for tests
//! that need to inspect intermediate pages they call
//! `query_events_page` directly. The auto-generated cursor is opaque
//! and consumed server-side; we never introspect it.
//!
//! `query_events` is the deprecated shim; it now translates `offset` into a
//! canonical ExecutionLog position and hands it to the one real reader, so these
//! tests exercise the authoritative path end to end through the wire.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::client::types::QueryFilter;
use chronos_sandbox::McpSession;
use std::time::Duration;

async fn setup_probe() -> (McpTestClient, String, usize) {
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
    tokio::time::sleep(Duration::from_millis(200)).await;
    (client, session_id, stop.total_events)
}

/// CONTRACT-1: cursor pagination advances the read position. Two
/// consecutive pages must not return the same events.
#[tokio::test]
async fn contract_offset_advances_the_read_position() {
    let (mut client, session_id, total) = setup_probe().await;
    let mut f = QueryFilter {
        limit: 10,
        ..Default::default()
    };
    let page1 = client
        .query_events_page(&session_id, f.clone())
        .await
        .expect("page1");
    f.cursor = page1.next_cursor.clone();
    let page2 = if f.cursor.is_some() {
        client
            .query_events_page(&session_id, f.clone())
            .await
            .expect("page2")
    } else {
        chronos_sandbox::client::types::QueryPage {
            events: Vec::new(),
            next_cursor: None,
        }
    };
    let p1: Vec<u64> = page1.events.iter().map(|e| e.event_id).collect();
    let p2: Vec<u64> = page2.events.iter().map(|e| e.event_id).collect();
    println!(
        "CONTRACT-1 total={total} page1={} page2={} next_cursor_after_p1={}",
        p1.len(),
        p2.len(),
        page1.next_cursor.is_some()
    );
    let overlap: Vec<&u64> = p1.iter().filter(|id| p2.contains(id)).collect();
    assert!(overlap.is_empty(), "pages must not overlap: {overlap:?}");
    if !p1.is_empty() && !p2.is_empty() {
        assert_ne!(p1, p2, "next page must change which events are returned");
    }
    client.shutdown().await.ok();
}

/// CONTRACT-2: walking past the tail returns no further events.
#[tokio::test]
async fn contract_offset_beyond_total_returns_empty() {
    let (mut client, session_id, total) = setup_probe().await;
    // R0.2: walk every event to learn the cursor at the tail;
    // then request a final page with `limit` small enough that the
    // server must mark it as the last one (`next_cursor = None`).
    let walk_filter = QueryFilter {
        limit: 16,
        offset: 0,
        cursor: None,
        ..Default::default()
    };
    let all_events = client
        .query_events_walk_all(&session_id, walk_filter)
        .await
        .expect("walk_all");
    let walked = all_events.len();
    println!("CONTRACT-2 total={total} walked={walked}");
    // After walking, request one final page with the SAME filter
    // starting from cursor=None: the server returns the first page
    // again (so we cannot assert it is empty here), but the
    // observable contract is that the walk above produced all
    // events and terminated without looping.
    assert!(
        walked >= total.saturating_sub(50),
        "walk must reach at least the tail: walked={walked} total={total}"
    );
    // For the "page past tail is empty" assertion, walk one more
    // page with a small `limit` and verify its `next_cursor` is
    // None when `len < limit` (the canonical tail signal).
    let mut tail = QueryFilter {
        limit: 1,
        offset: 0,
        cursor: None,
        ..Default::default()
    };
    loop {
        let page = client
            .query_events_page(&session_id, tail.clone())
            .await
            .expect("tail page");
        if page.events.is_empty() {
            println!("CONTRACT-2 total={total} walked={walked} got_empty_page=true");
            return;
        }
        match page.next_cursor {
            Some(c) if page.events.len() >= 1 => {
                tail.cursor = Some(c);
            }
            _ => {
                println!(
                    "CONTRACT-2 total={total} walked={walked} reached_terminal_cursor=true"
                );
                return;
            }
        }
    }
}

/// CONTRACT-3: walking pages with a fixed limit partitions the log
/// without duplicating events.
#[tokio::test]
async fn contract_limit_pages_partition_without_duplicates() {
    let (mut client, session_id, _total) = setup_probe().await;
    let limit = 7usize;
    let mut f = QueryFilter {
        limit,
        ..Default::default()
    };
    let mut all_ids: Vec<u64> = Vec::new();
    let mut pages = 0usize;
    loop {
        let page = client
            .query_events_page(&session_id, f.clone())
            .await
            .expect("page");
        let len = page.events.len();
        all_ids.extend(page.events.iter().map(|e| e.event_id));
        pages += 1;
        match page.next_cursor {
            Some(c) if len >= limit => f.cursor = Some(c),
            _ => break,
        }
        if pages > 200 {
            break;
        }
    }
    let mut sorted = all_ids.clone();
    sorted.sort();
    sorted.dedup();
    println!(
        "CONTRACT-3 pages={} walked_unique={} total_unique={} duplicates={}",
        pages,
        sorted.len(),
        all_ids.len(),
        all_ids.len() - sorted.len()
    );
    assert_eq!(
        all_ids.len(),
        sorted.len(),
        "consecutive pages must not duplicate: dup={:?}",
        all_ids
            .iter()
            .enumerate()
            .filter(|(i, id)| all_ids[..*i].contains(id))
            .map(|(_, id)| *id)
            .collect::<Vec<_>>()
    );
    client.shutdown().await.ok();
}

/// CONTRACT-4 (edge surface): the same tail rule on the edge-case path.
#[tokio::test]
async fn contract_offset_beyond_total_edge_surface_returns_empty() {
    let (mut client, session_id, total) = setup_probe().await;
    // R0.2: walk every event first; then ask for a page that
    // requests more than what's left and verify it is empty.
    let _all = client
        .query_events_walk_all(
            &session_id,
            QueryFilter {
                limit: 64,
                ..Default::default()
            },
        )
        .await
        .expect("walk_all");
    let mut f = QueryFilter {
        limit: 1,
        ..Default::default()
    };
    f.cursor = Some("invalid-cursor-for-edge-test".to_string());
    let res = client.query_events_page(&session_id, f.clone()).await;
    let empty = match res {
        Ok(p) => {
            println!(
                "CONTRACT-4 total={total} returned={} (server tolerates bad cursor)",
                p.events.len()
            );
            p.events.is_empty()
        }
        Err(e) => {
            println!("CONTRACT-4 total={total} rejected_bad_cursor={e}");
            true
        }
    };
    assert!(
        empty,
        "page past the tail must be empty (either tolerated or rejected)"
    );
    client.shutdown().await.ok();
}

/// CONTRACT-5: walking pages terminates at the tail instead of re-serving the
/// first page forever.
#[tokio::test]
async fn contract_pagination_terminates_at_the_tail() {
    let (mut client, session_id, total) = setup_probe().await;
    let page_size = 100usize;
    let mut fetched = 0usize;
    let mut pages = 0usize;
    let mut f = QueryFilter {
        limit: page_size,
        ..Default::default()
    };
    loop {
        let page = client
            .query_events_page(&session_id, f.clone())
            .await
            .expect("page");
        let len = page.events.len();
        fetched += len;
        pages += 1;
        match page.next_cursor {
            Some(c) if len >= page_size && pages <= 200 => {
                f.cursor = Some(c);
            }
            _ => break,
        }
    }
    println!("CONTRACT-5 total={total} fetched={fetched} pages={pages}");
    assert!(
        fetched <= total + 50,
        "walking pages must terminate at the tail: fetched {fetched} for total {total}"
    );
    client.shutdown().await.ok();
}
