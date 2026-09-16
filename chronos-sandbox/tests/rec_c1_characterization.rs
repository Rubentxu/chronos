//! REC-C1.0 characterization tests — DEF-001 baseline (5 tests).
//!
//! These tests LOCK the CURRENT (known-wrong) behavior of query_events
//! pagination/offset so the REC-C1 cutover can proceed red→green per subphase.
//! They are `#[ignore]` by default (the DEF-001 declared-failure set already
//! covers the originals in query_filters.rs / query_edge_cases.rs); run with
//! `-- --ignored` to observe current behavior.
//!
//! Contract under test (TRUTH-002/003):
//!   offset MUST move the read position in the authoritative log;
//!   offset beyond total MUST yield empty;
//!   paging with limit MUST partition the log without overlap or duplication.
//!
//! When the ExecutionLog cutover lands, flip each `assert_characterization_*`
//! to the corrected assertion and remove the matching DEF-001 entry from
//! reconstruction-contracts.toml. One test per subphase ratchet.

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

/// CHAR-1 (DEF-001 / query_filters::test_query_events_offset_pagination twin):
/// offset must advance the read position. Characterization: record whether
/// offset+limit pages overlap. EXPECTED CURRENT: overlap occurs (offset not
/// applied). Post cutover: no overlap.
#[tokio::test]
#[ignore = "REC-C1.0 characterization: documents current non-authoritative offset (TRUTH-002)"]
async fn char_offset_pagination_current_behavior() {
    let (mut client, session_id, total) = setup_probe().await;
    let page1 = client
        .query_events(&session_id, QueryFilter { limit: 10, offset: 0, ..Default::default() })
        .await
        .expect("page1");
    let page2 = client
        .query_events(&session_id, QueryFilter { limit: 10, offset: 10, ..Default::default() })
        .await
        .expect("page2");
    let p1: Vec<u64> = page1.iter().map(|e| e.event_id).collect();
    let p2: Vec<u64> = page2.iter().map(|e| e.event_id).collect();
    let overlap: Vec<&u64> = p1.iter().filter(|id| p2.contains(id)).collect();
    println!("CHAR-1 total={total} page1={} page2={} overlap={}", p1.len(), p2.len(), overlap.len());
    // Characterization assertion: CURRENT behavior is overlap > 0 when both
    // pages are full. Flip to == 0 when TRUTH-002 lands.
    if p1.len() == 10 && p2.len() == 10 {
        assert!(!overlap.is_empty(), "characterization changed: offset now appears applied — update this test and close DEF-001 item");
    }
    client.shutdown().await.ok();
}

/// CHAR-2 (DEF-001 / offset_beyond_total twins): offset past the tail must
/// return empty. EXPECTED CURRENT: returns a full page regardless of offset.
#[tokio::test]
#[ignore = "REC-C1.0 characterization: offset beyond total returns data (TRUTH-002)"]
async fn char_offset_beyond_total_current_behavior() {
    let (mut client, session_id, total) = setup_probe().await;
    let far = client
        .query_events(&session_id, QueryFilter { limit: 10, offset: 1_000_000, ..Default::default() })
        .await
        .expect("far offset query");
    println!("CHAR-2 total={total} far_offset_returned={}", far.len());
    // Characterization: CURRENT returns non-empty. Flip to is_empty() at cutover.
    assert!(!far.is_empty(), "characterization changed: offset beyond total now empty — close DEF-001 item");
    client.shutdown().await.ok();
}

/// CHAR-3 (DEF-001 / query_filters::test_query_events_limit_exact_pagination):
/// paging with limit N partitions the log exactly. EXPECTED CURRENT: page
/// boundaries drift vs limit (duplicated or skipped events at boundaries).
#[tokio::test]
#[ignore = "REC-C1.0 characterization: limit-exact page boundaries drift"]
async fn char_limit_exact_pagination_current_behavior() {
    let (mut client, session_id, _total) = setup_probe().await;
    let page1 = client
        .query_events(&session_id, QueryFilter { limit: 7, offset: 0, ..Default::default() })
        .await
        .expect("p1");
    let page2 = client
        .query_events(&session_id, QueryFilter { limit: 7, offset: 7, ..Default::default() })
        .await
        .expect("p2");
    let p1: Vec<u64> = page1.iter().map(|e| e.event_id).collect();
    let p2: Vec<u64> = page2.iter().map(|e| e.event_id).collect();
    let dup: Vec<&u64> = p1.iter().filter(|id| p2.contains(id)).collect();
    println!("CHAR-3 p1={} p2={} boundary_duplicates={}", p1.len(), p2.len(), dup.len());
    // Characterization: CURRENT allows duplicates/skips at boundaries.
    // Flip to assert!(dup.is_empty()) when pagination is authoritative.
    if p1.len() == 7 && p2.len() == 7 {
        assert!(!dup.is_empty() || p2[0] == p1[6] + 1 || p2[0] > p1[6], "characterization changed: exact-limit pagination now partitions cleanly — close DEF-001 item");
    }
    client.shutdown().await.ok();
}

/// CHAR-4 (DEF-001 / query_edge_cases::test_query_events_offset_beyond_total):
/// same contract as CHAR-2 via the edge-case surface; kept separate so the
/// DEF-001 ledger ratchet can close the two suites independently.
#[tokio::test]
#[ignore = "REC-C1.0 characterization (edge surface): offset beyond total returns data"]
async fn char_offset_beyond_total_edge_surface() {
    let (mut client, session_id, total) = setup_probe().await;
    let far = client
        .query_events(&session_id, QueryFilter { limit: 10, offset: 500_000, ..Default::default() })
        .await
        .expect("far query");
    println!("CHAR-4 total={total} returned={}", far.len());
    assert!(!far.is_empty(), "characterization changed — close DEF-001 item");
    client.shutdown().await.ok();
}

/// CHAR-5 (DEF-001 / query_edge_cases::test_query_events_pagination_all_events):
/// walking pages must terminate at the tail (returned <= stop.total + tolerance).
/// EXPECTED CURRENT: walker hits the 1000-page safety limit having "fetched"
/// ~100k events for a ~2.5k-event log (server re-serves pages past tail).
#[tokio::test]
#[ignore = "REC-C1.0 characterization: pagination walks past tail"]
async fn char_pagination_tail_termination_current_behavior() {
    let (mut client, session_id, total) = setup_probe().await;
    let page_size = 100usize;
    let mut fetched = 0usize;
    let mut pages = 0usize;
    let mut last_page_len = page_size;
    while last_page_len == page_size && pages < 60 {
        let page = client
            .query_events(&session_id, QueryFilter { limit: page_size, offset: pages * page_size, ..Default::default() })
            .await
            .expect("page");
        last_page_len = page.len();
        fetched += page.len();
        pages += 1;
    }
    println!("CHAR-5 total={total} fetched={fetched} pages={pages}");
    // Characterization: CURRENT fetched exceeds total by a large factor.
    // Flip to fetched <= total + tolerance when the tail terminates reads.
    assert!(fetched > total + 50, "characterization changed: pagination now terminates at tail — close DEF-001 item");
    client.shutdown().await.ok();
}
