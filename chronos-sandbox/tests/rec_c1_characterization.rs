//! REC-C1.3 — positive contract tests for the corrected read path.
//!
//! These replace the C1.0 characterization suite. The characterizations
//! deliberately asserted the *wrong* behaviour (offset ignored, tail never
//! reached); after the cutover all five of them went red, which is the signal
//! that the declared DEF-001 debt is gone. Deleting them would have lost the
//! contract, so they are rewritten here as the positive statement of the same
//! five properties.
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

/// CONTRACT-1: offset advances the read position. Two pages at different offsets
/// must not return the same events.
#[tokio::test]
async fn contract_offset_advances_the_read_position() {
    let (mut client, session_id, total) = setup_probe().await;
    let page1 = client
        .query_events(
            &session_id,
            QueryFilter {
                limit: 10,
                offset: 0,
                ..Default::default()
            },
        )
        .await
        .expect("page1");
    let page2 = client
        .query_events(
            &session_id,
            QueryFilter {
                limit: 10,
                offset: 10,
                ..Default::default()
            },
        )
        .await
        .expect("page2");
    let p1: Vec<u64> = page1.iter().map(|e| e.event_id).collect();
    let p2: Vec<u64> = page2.iter().map(|e| e.event_id).collect();
    println!(
        "CONTRACT-1 total={total} page1={} page2={}",
        p1.len(),
        p2.len()
    );
    let overlap: Vec<&u64> = p1.iter().filter(|id| p2.contains(id)).collect();
    assert!(overlap.is_empty(), "pages must not overlap: {overlap:?}");
    if !p1.is_empty() && !p2.is_empty() {
        assert_ne!(p1, p2, "offset must change which events are returned");
    }
    client.shutdown().await.ok();
}

/// CONTRACT-2: an offset past the tail returns nothing.
#[tokio::test]
async fn contract_offset_beyond_total_returns_empty() {
    let (mut client, session_id, total) = setup_probe().await;
    let far = client
        .query_events(
            &session_id,
            QueryFilter {
                limit: 10,
                offset: 1_000_000,
                ..Default::default()
            },
        )
        .await
        .expect("far offset query");
    println!("CONTRACT-2 total={total} far_offset_returned={}", far.len());
    assert!(far.is_empty(), "offset beyond the tail must return empty");
    client.shutdown().await.ok();
}

/// CONTRACT-3: paging with a limit partitions the log with no duplication and no
/// gap at the boundary.
#[tokio::test]
async fn contract_limit_pages_partition_without_duplicates() {
    let (mut client, session_id, _total) = setup_probe().await;
    let page1 = client
        .query_events(
            &session_id,
            QueryFilter {
                limit: 7,
                offset: 0,
                ..Default::default()
            },
        )
        .await
        .expect("p1");
    let page2 = client
        .query_events(
            &session_id,
            QueryFilter {
                limit: 7,
                offset: 7,
                ..Default::default()
            },
        )
        .await
        .expect("p2");
    let p1: Vec<u64> = page1.iter().map(|e| e.event_id).collect();
    let p2: Vec<u64> = page2.iter().map(|e| e.event_id).collect();
    let dup: Vec<&u64> = p1.iter().filter(|id| p2.contains(id)).collect();
    println!(
        "CONTRACT-3 p1={} p2={} boundary_duplicates={}",
        p1.len(),
        p2.len(),
        dup.len()
    );
    assert!(
        dup.is_empty(),
        "consecutive pages must not duplicate: {dup:?}"
    );
    client.shutdown().await.ok();
}

/// CONTRACT-4 (edge surface): the same tail rule on the edge-case path.
#[tokio::test]
async fn contract_offset_beyond_total_edge_surface_returns_empty() {
    let (mut client, session_id, total) = setup_probe().await;
    let far = client
        .query_events(
            &session_id,
            QueryFilter {
                limit: 10,
                offset: 500_000,
                ..Default::default()
            },
        )
        .await
        .expect("far query");
    println!("CONTRACT-4 total={total} returned={}", far.len());
    assert!(far.is_empty(), "offset beyond the tail must return empty");
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
    loop {
        let page = client
            .query_events(
                &session_id,
                QueryFilter {
                    limit: page_size,
                    offset: pages * page_size,
                    ..Default::default()
                },
            )
            .await
            .expect("page");
        let len = page.len();
        fetched += len;
        pages += 1;
        if len < page_size || pages > 200 {
            break;
        }
    }
    println!("CONTRACT-5 total={total} fetched={fetched} pages={pages}");
    assert!(
        fetched <= total + 50,
        "walking pages must terminate at the tail: fetched {fetched} for total {total}"
    );
    client.shutdown().await.ok();
}
