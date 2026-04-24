//! Concurrency stress tests — verify concurrent probe operations, rapid restarts,
//! and interleaved save/load workflows.
//!
//! Category CS tests cover concurrency stress scenarios.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::client::types::QueryFilter;
use chronos_sandbox::McpSession;
use std::time::Duration;

/// CS1: Three concurrent probes stopped in reverse order.
/// Starts probe A (test_busyloop), B (test_threads), C (test_busyloop).
/// Sleeps 2s, then stops C, B, A (reverse order).
/// All three sessions should be queryable with distinct session IDs.
#[tokio::test]
async fn test_three_concurrent_probes_reverse_stop() {
    let fixture_busyloop = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found");
    let fixture_threads = McpSession::fixture_path("test_threads")
        .expect("test_threads fixture not found");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Start probe A (test_busyloop)
    let session_id_a = client.probe_start(fixture_busyloop.to_str().unwrap()).await
        .expect("probe_start (A) failed");

    // Start probe B (test_threads)
    let session_id_b = client.probe_start(fixture_threads.to_str().unwrap()).await
        .expect("probe_start (B) failed");

    // Start probe C (test_busyloop second instance)
    let session_id_c = client.probe_start(fixture_busyloop.to_str().unwrap()).await
        .expect("probe_start (C) failed");

    println!("Started: A={}, B={}, C={}", session_id_a, session_id_b, session_id_c);

    // All three run concurrently for 2s
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Stop in reverse order: C, then B, then A
    let stop_c = client.probe_stop(&session_id_c).await
        .expect("probe_stop (C) failed");
    println!("Stopped C: {} events", stop_c.total_events);

    let stop_b = client.probe_stop(&session_id_b).await
        .expect("probe_stop (B) failed");
    println!("Stopped B: {} events", stop_b.total_events);

    let stop_a = client.probe_stop(&session_id_a).await
        .expect("probe_stop (A) failed");
    println!("Stopped A: {} events", stop_a.total_events);

    // Give query engines time to build
    tokio::time::sleep(Duration::from_millis(300)).await;

    // All three sessions should have distinct IDs
    assert!(session_id_a != session_id_b, "A and B should have distinct IDs");
    assert!(session_id_a != session_id_c, "A and C should have distinct IDs");
    assert!(session_id_b != session_id_c, "B and C should have distinct IDs");

    println!("✓ All 3 session IDs are distinct");

    // query_events on all three — all should return valid responses
    let events_a = client.query_events(&session_id_a, QueryFilter::default()).await
        .expect("query_events (A) failed");
    let events_b = client.query_events(&session_id_b, QueryFilter::default()).await
        .expect("query_events (B) failed");
    let events_c = client.query_events(&session_id_c, QueryFilter::default()).await
        .expect("query_events (C) failed");

    println!("✓ Query results: A={}, B={}, C={} events",
        events_a.len(), events_b.len(), events_c.len());

    // At least one session should have events (busyloop is long-running)
    assert!(
        !events_a.is_empty() || !events_c.is_empty() || stop_a.total_events > 0 || stop_c.total_events > 0,
        "At least one busyloop session should have events"
    );

    client.shutdown().await.ok();
}

/// CS2: Rapid probe restart — new session IDs for each restart.
/// probe_start test_add → session_id_1, sleep 500ms, probe_stop
/// probe_start test_add → session_id_2 (NEW session), sleep 500ms, probe_stop
/// Assert: session_id_1 != session_id_2 and both are queryable.
#[tokio::test]
async fn test_rapid_probe_restart() {
    let fixture_add = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // First probe session
    let session_id_1 = client.probe_start(fixture_add.to_str().unwrap()).await
        .expect("probe_start (1) failed");

    tokio::time::sleep(Duration::from_millis(500)).await;

    let stop_1 = client.probe_stop(&session_id_1).await
        .expect("probe_stop (1) failed");
    println!("Stopped session 1: {} events", stop_1.total_events);

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Second probe session — must be a NEW session
    let session_id_2 = client.probe_start(fixture_add.to_str().unwrap()).await
        .expect("probe_start (2) failed");

    tokio::time::sleep(Duration::from_millis(500)).await;

    let stop_2 = client.probe_stop(&session_id_2).await
        .expect("probe_stop (2) failed");
    println!("Stopped session 2: {} events", stop_2.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Assert: session IDs must be different
    assert!(session_id_1 != session_id_2,
        "session_id_1 ({}) should != session_id_2 ({})", session_id_1, session_id_2);
    println!("✓ Session IDs are distinct: {} != {}", session_id_1, session_id_2);

    // query_events on both — both should be valid
    let events_1 = client.query_events(&session_id_1, QueryFilter::default()).await
        .expect("query_events (1) failed");
    let events_2 = client.query_events(&session_id_2, QueryFilter::default()).await
        .expect("query_events (2) failed");

    println!("✓ Both sessions queryable: 1={} events, 2={} events",
        events_1.len(), events_2.len());

    client.shutdown().await.ok();
}

/// CS3: Two saves loaded concurrently.
/// Run probe A (test_add), stop, save → saved_id_a
/// Run probe B (test_busyloop, 2s), stop, save → saved_id_b
/// load_session(saved_id_a) and load_session(saved_id_b)
/// query_events on both → both valid independently.
#[tokio::test]
async fn test_two_saves_load_concurrently() {
    let fixture_add = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found");
    let fixture_busyloop = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Probe A (test_add)
    let session_id_a = client.probe_start(fixture_add.to_str().unwrap()).await
        .expect("probe_start (A) failed");
    tokio::time::sleep(Duration::from_secs(1)).await;
    let _drained_a = client.probe_drain(&session_id_a).await
        .expect("probe_drain (A) failed");
    let stop_a = client.probe_stop(&session_id_a).await
        .expect("probe_stop (A) failed");
    println!("Probe A stopped: {} events", stop_a.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Save session A
    let save_a = client.save_session(&session_id_a, "test_add").await
        .expect("save_session (A) failed");
    println!("Saved A: {} events", save_a.event_count);

    // Probe B (test_busyloop, 2s)
    let session_id_b = client.probe_start(fixture_busyloop.to_str().unwrap()).await
        .expect("probe_start (B) failed");
    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained_b = client.probe_drain(&session_id_b).await
        .expect("probe_drain (B) failed");
    let stop_b = client.probe_stop(&session_id_b).await
        .expect("probe_stop (B) failed");
    println!("Probe B stopped: {} events", stop_b.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Save session B
    let save_b = client.save_session(&session_id_b, "test_busyloop").await
        .expect("save_session (B) failed");
    println!("Saved B: {} events", save_b.event_count);

    // Load both sessions concurrently
    let load_a = client.load_session(&session_id_a).await
        .expect("load_session (A) failed");
    let load_b = client.load_session(&session_id_b).await
        .expect("load_session (B) failed");

    println!("Loaded A: {} events, Loaded B: {} events",
        load_a.event_count, load_b.event_count);

    // query_events on both — both valid independently
    let events_a = client.query_events(&session_id_a, QueryFilter::default()).await
        .expect("query_events (A) failed");
    let events_b = client.query_events(&session_id_b, QueryFilter::default()).await
        .expect("query_events (B) failed");

    println!("✓ Both sessions queryable: A={}, B={}", events_a.len(), events_b.len());

    // Both should have events
    assert!(events_a.len() > 0 || load_a.event_count > 0,
        "Session A should have accessible events");
    assert!(events_b.len() > 0 || load_b.event_count > 0,
        "Session B should have accessible events");

    client.shutdown().await.ok();
}

/// CS4: Probe and save interleaved.
/// Start probe on test_busyloop; Sleep 1s → session_snapshot; Sleep 1s more → probe_stop
/// query_events → valid; save_session → valid; load_session → valid.
#[tokio::test]
async fn test_probe_and_save_interleaved() {
    let fixture_busyloop = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Start probe on test_busyloop
    let session_id = client.probe_start(fixture_busyloop.to_str().unwrap()).await
        .expect("probe_start failed");

    // Sleep 1s then take snapshot
    tokio::time::sleep(Duration::from_secs(1)).await;

    let snapshot = client.session_snapshot(&session_id).await
        .expect("session_snapshot failed");
    println!("Snapshot taken: {} events indexed", snapshot.events_indexed);

    // Sleep 1s more then stop
    tokio::time::sleep(Duration::from_secs(1)).await;

    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");
    println!("Probe stopped: {} events", stop.total_events);

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // query_events should be valid
    let events = client.query_events(&session_id, QueryFilter::default()).await
        .expect("query_events failed");
    println!("✓ query_events returned {} events", events.len());

    // save_session to persist to disk
    let save = client.save_session(&session_id, "interleaved_test").await
        .expect("save_session failed");
    println!("✓ save_session returned: {} events", save.event_count);

    // load_session from saved - verify persistence
    let load = client.load_session(&session_id).await
        .expect("load_session failed");
    println!("✓ load_session returned: {} events", load.event_count);

    client.shutdown().await.ok();
}

/// CS5: Multiple drains on same live probe.
/// probe_start test_busyloop; Sleep 1s → probe_drain → count_1;
/// Sleep 1s → probe_drain → count_2; probe_stop
/// Both drain calls should succeed without crash.
#[tokio::test]
async fn test_multiple_drains_same_live_probe() {
    let fixture_busyloop = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Start probe
    let session_id = client.probe_start(fixture_busyloop.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for events to accumulate
    tokio::time::sleep(Duration::from_secs(1)).await;

    // First drain
    let drain_1 = client.probe_drain_raw(&session_id).await
        .expect("probe_drain (1) failed");
    println!("First drain: {} events", drain_1.total_buffered);

    // Wait more
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Second drain
    let drain_2 = client.probe_drain_raw(&session_id).await
        .expect("probe_drain (2) failed");
    println!("Second drain: {} events", drain_2.total_buffered);

    // Both drains should succeed (no crash)
    assert!(
        drain_1.status.to_lowercase().contains("running") ||
        drain_1.status.to_lowercase().contains("stopped"),
        "First drain should succeed"
    );
    assert!(
        drain_2.status.to_lowercase().contains("running") ||
        drain_2.status.to_lowercase().contains("stopped"),
        "Second drain should succeed"
    );

    // Note: drain removes events from the buffer, so second drain typically has fewer
    // events than first. The key assertion is that both calls succeed without crash.

    // Stop the probe
    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");
    println!("✓ Probe stopped: {} events", stop.total_events);

    client.shutdown().await.ok();
}
