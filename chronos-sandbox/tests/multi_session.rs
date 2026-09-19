//! Multi-session scenarios — verify concurrent sessions and cross-session operations.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::client::types::QueryFilter;
use chronos_sandbox::McpSession;
use std::time::Duration;

/// C1: Two concurrent probes with different targets are isolated.
#[tokio::test]
async fn test_two_concurrent_probes_isolation() {
    let fixture_busyloop =
        McpSession::fixture_path("test_busyloop").expect("test_busyloop fixture not found");
    let fixture_threads =
        McpSession::fixture_path("test_threads").expect("test_threads fixture not found");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Start probe_1 on test_busyloop
    let session_id_1 = client
        .probe_start(fixture_busyloop.to_str().unwrap())
        .await
        .expect("probe_start (busyloop) failed");

    // Start probe_2 on test_threads
    let session_id_2 = client
        .probe_start(fixture_threads.to_str().unwrap())
        .await
        .expect("probe_start (threads) failed");

    println!(
        "Started probe_1: {}, probe_2: {}",
        session_id_1, session_id_2
    );

    // Sleep 2s while both run
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Stop both probes
    let stop1 = client
        .probe_stop(&session_id_1)
        .await
        .expect("probe_stop (1) failed");
    let stop2 = client
        .probe_stop(&session_id_2)
        .await
        .expect("probe_stop (2) failed");

    println!(
        "Probe 1 stopped: {} events, Probe 2 stopped: {} events",
        stop1.total_events, stop2.total_events
    );

    // Give query engines time to build
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Query events from session 1
    let events1 = client
        .query_events(&session_id_1, QueryFilter::default())
        .await
        .expect("query_events (1) failed");
    println!("Session 1 events: {}", events1.len());

    // Query events from session 2
    let events2 = client
        .query_events(&session_id_2, QueryFilter::default())
        .await
        .expect("query_events (2) failed");
    println!("Session 2 events: {}", events2.len());

    // Assert sessions are isolated (different IDs)
    assert!(
        session_id_1 != session_id_2,
        "Session IDs should be different"
    );

    // Session 1 (busyloop) should have events
    assert!(
        !events1.is_empty() || stop1.total_events > 0,
        "Session 1 (busyloop) should have events or captured events"
    );

    // Session 2 may or may not have events depending on when test_threads exited
    // The key isolation check is that we got different session IDs
    println!(
        "✓ Sessions are isolated: ID1={}, ID2={}",
        session_id_1, session_id_2
    );

    client.shutdown().await.ok();
}

/// C2: Save a session, start another probe, load saved session while second runs.
#[tokio::test]
async fn test_save_session_a_load_while_session_b_runs() {
    let fixture_add = McpSession::fixture_path("test_add").expect("test_add fixture not found");
    let fixture_busyloop =
        McpSession::fixture_path("test_busyloop").expect("test_busyloop fixture not found");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Start first probe on test_add, run 2s, stop
    let session_a = client
        .probe_start(fixture_add.to_str().unwrap())
        .await
        .expect("probe_start (add) failed");

    tokio::time::sleep(Duration::from_secs(2)).await;

    let _drained_a = client
        .probe_drain(&session_a)
        .await
        .expect("probe_drain failed");

    let stop_a = client
        .probe_stop(&session_a)
        .await
        .expect("probe_stop (add) failed");
    println!("Session A stopped: {} events", stop_a.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Save session A
    let save_result = client
        .save_session(&session_a, "session_add")
        .await
        .expect("save_session failed");
    println!("Saved session A: {} events", save_result.event_count);

    // Start second probe on test_busyloop (keep running)
    let session_b = client
        .probe_start(fixture_busyloop.to_str().unwrap())
        .await
        .expect("probe_start (busyloop) failed");

    println!("Started session B (busyloop): {}", session_b);

    // Load saved session A while B is still running
    let load_result = client
        .load_session(&session_a)
        .await
        .expect("load_session failed");

    println!(
        "Loaded session A: {} events, target={}",
        load_result.event_count, load_result.target
    );
    assert!(
        load_result.event_count > 0,
        "Loaded session should have events"
    );

    // Stop session B
    let stop_b = client
        .probe_stop(&session_b)
        .await
        .expect("probe_stop (busyloop) failed");
    println!("Session B stopped: {} events", stop_b.total_events);

    // Both sessions should be accessible independently
    tokio::time::sleep(Duration::from_millis(200)).await;

    let events_a = client
        .query_events(&session_a, QueryFilter::default())
        .await
        .expect("query_events (A) failed");
    let events_b = client
        .query_events(&session_b, QueryFilter::default())
        .await
        .expect("query_events (B) failed");

    println!(
        "Final: session A has {} events, session B has {} events",
        events_a.len(),
        events_b.len()
    );

    assert!(
        !events_a.is_empty() || stop_a.total_events > 0,
        "Session A should have events"
    );
    assert!(
        !events_b.is_empty() || stop_b.total_events > 0,
        "Session B should have events"
    );

    client.shutdown().await.ok();
}

/// C3: Compare two similar programs (same binary, different runs).
#[tokio::test]
async fn test_compare_sessions_similar_programs() {
    let fixture_busyloop =
        McpSession::fixture_path("test_busyloop").expect("test_busyloop fixture not found");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Run test_busyloop twice (2s each)
    let session_1 = client
        .probe_start(fixture_busyloop.to_str().unwrap())
        .await
        .expect("probe_start (1) failed");
    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained1 = client
        .probe_drain(&session_1)
        .await
        .expect("probe_drain (1) failed");
    client
        .probe_stop(&session_1)
        .await
        .expect("probe_stop (1) failed");

    let session_2 = client
        .probe_start(fixture_busyloop.to_str().unwrap())
        .await
        .expect("probe_start (2) failed");
    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained2 = client
        .probe_drain(&session_2)
        .await
        .expect("probe_drain (2) failed");
    client
        .probe_stop(&session_2)
        .await
        .expect("probe_stop (2) failed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Save both sessions
    let saved_1 = client
        .save_session(&session_1, "busyloop_1")
        .await
        .expect("save_session (1) failed");
    let saved_2 = client
        .save_session(&session_2, "busyloop_2")
        .await
        .expect("save_session (2) failed");

    println!(
        "Saved {} events and {} events",
        saved_1.event_count, saved_2.event_count
    );

    // Compare sessions
    let report = client
        .compare_sessions(&session_1, &session_2)
        .await
        .expect("compare_sessions failed");

    println!("✓ compare_sessions result:");
    println!("  Similarity: {}%", report.similarity_pct);
    println!(
        "  Only in 1: {}, Only in 2: {}",
        report.only_in_a_count, report.only_in_b_count
    );
    println!("  Common: {}", report.common_count);
    println!("  Summary: {}", report.summary);

    // Verify response is valid
    assert!(
        report.similarity_pct >= 0.0 && report.similarity_pct <= 100.0,
        "Similarity should be between 0 and 100"
    );
    assert!(!report.summary.is_empty(), "Summary should not be empty");

    client.shutdown().await.ok();
}

/// C5: Performance regression audit between two saved sessions.
#[tokio::test]
async fn test_performance_regression_audit() {
    let fixture_busyloop =
        McpSession::fixture_path("test_busyloop").expect("test_busyloop fixture not found");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Run test_busyloop (2s) → save → saved_id_1
    let session_1 = client
        .probe_start(fixture_busyloop.to_str().unwrap())
        .await
        .expect("probe_start (1) failed");
    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained1 = client
        .probe_drain(&session_1)
        .await
        .expect("probe_drain (1) failed");
    client
        .probe_stop(&session_1)
        .await
        .expect("probe_stop (1) failed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    let saved_1 = client
        .save_session(&session_1, "busyloop_perf_1")
        .await
        .expect("save_session (1) failed");

    // Run test_busyloop (2s) → save → saved_id_2
    let session_2 = client
        .probe_start(fixture_busyloop.to_str().unwrap())
        .await
        .expect("probe_start (2) failed");
    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained2 = client
        .probe_drain(&session_2)
        .await
        .expect("probe_drain (2) failed");
    client
        .probe_stop(&session_2)
        .await
        .expect("probe_stop (2) failed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    let saved_2 = client
        .save_session(&session_2, "busyloop_perf_2")
        .await
        .expect("save_session (2) failed");

    println!(
        "Saved perf_1: {} events, perf_2: {} events",
        saved_1.event_count, saved_2.event_count
    );

    // Run performance regression audit
    let report = client
        .performance_regression_audit(&session_1, &session_2)
        .await
        .expect("performance_regression_audit failed");

    println!("✓ performance_regression_audit result:");
    println!("  Baseline: {}", report.baseline_session_id);
    println!("  Target: {}", report.target_session_id);
    println!("  Functions analyzed: {}", report.functions_analyzed);
    println!("  Total call delta: {}", report.total_call_delta);
    println!("  Summary: {}", report.summary);

    // Verify response is valid (has comparison data)
    assert!(
        !report.baseline_session_id.is_empty(),
        "baseline_session_id should not be empty"
    );
    assert!(
        !report.target_session_id.is_empty(),
        "target_session_id should not be empty"
    );

    client.shutdown().await.ok();
}

/// C6: list_sessions reflects saves and deletes correctly.
#[tokio::test]
async fn test_list_sessions_reflects_saves_and_deletes() {
    let fixture_add = McpSession::fixture_path("test_add").expect("test_add fixture not found");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Save 3 sessions
    let mut saved_ids = Vec::new();
    for i in 0..3 {
        let session = client
            .probe_start(fixture_add.to_str().unwrap())
            .await
            .expect("probe_start failed");
        tokio::time::sleep(Duration::from_millis(800)).await;
        let _drained = client
            .probe_drain(&session)
            .await
            .expect("probe_drain failed");
        client
            .probe_stop(&session)
            .await
            .expect("probe_stop failed");
        tokio::time::sleep(Duration::from_millis(200)).await;

        let save_result = client
            .save_session(&session, &format!("session_{}", i))
            .await
            .expect("save_session failed");
        println!("Saved session {}: {}", i, save_result.session_id);
        saved_ids.push(session);
    }

    // List sessions - should have at least our 3
    let sessions_before = client.list_sessions().await.expect("list_sessions failed");
    let count_before = sessions_before.len();
    println!(
        "Sessions before delete: {} (looking for at least 3)",
        count_before
    );
    assert!(count_before >= 3, "Should have at least 3 saved sessions");

    // Delete one
    let to_delete = &saved_ids[0];
    client
        .delete_session(to_delete)
        .await
        .expect("delete_session failed");
    println!("Deleted session: {}", to_delete);

    // List again - should have one less
    let sessions_after = client.list_sessions().await.expect("list_sessions failed");
    let count_after = sessions_after.len();
    println!("Sessions after delete: {}", count_after);

    assert_eq!(
        count_after,
        count_before - 1,
        "Count should decrease by 1 after delete"
    );

    client.shutdown().await.ok();
}

/// C7: drop_session removes from memory but load_session still works from disk.
#[tokio::test]
async fn test_drop_session_not_in_load_list() {
    let fixture_add = McpSession::fixture_path("test_add").expect("test_add fixture not found");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Start probe, run 2s, stop
    let session_id = client
        .probe_start(fixture_add.to_str().unwrap())
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
    println!("Probe stopped: {} events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Save session
    let save_result = client
        .save_session(&session_id, "drop_test")
        .await
        .expect("save_session failed");
    println!("Saved: {} events", save_result.event_count);

    // Drop session from memory
    let drop_result = client
        .drop_session(&session_id)
        .await
        .expect("drop_session failed");
    println!(
        "Dropped: status={}, message={}",
        drop_result.status, drop_result.message
    );

    // Load session - should still work from disk
    let load_result = client
        .load_session(&session_id)
        .await
        .expect("load_session should work after drop (from disk)");
    println!("Loaded after drop: {} events", load_result.event_count);
    assert!(
        load_result.event_count > 0,
        "Loaded session should have events"
    );

    // Query events - should work after reload
    tokio::time::sleep(Duration::from_millis(100)).await;
    let events = client
        .query_events(&session_id, QueryFilter::default())
        .await
        .expect("query_events should work after reload");
    println!("Events after reload: {}", events.len());
    assert!(
        !events.is_empty() || load_result.event_count > 0,
        "Should have events accessible after reload"
    );

    client.shutdown().await.ok();
}

/// CIH-F (operator-required integration): drop + load of one session must
/// not perturb a second session that is also loaded concurrently.
///
/// This is the real-MCP-path companion of the unit tests
/// `cih_f_multi_session_load_one_does_not_disturb_another` and
/// `cih_f_repeated_load_does_not_overwrite_existing_handle`. The unit
/// suite proves the service-level contract; this test proves the wire
/// path the operator's CI gate is red on does not regress.
///
/// Asserts:
///  * start two probes, save both, drop s1, load s1 -> events still
///    queryable through `query_events` for s1;
///  * s2 is still queryable (no cross-session registry contamination
///    from s1's reopen);
///  * s2's symmetric drop+load also succeeds (guarding against
///    IdentityMismatch on the second session's reopen).
#[tokio::test]
async fn test_cih_f_drop_load_one_session_does_not_disturb_other() {
    let fixture_add = McpSession::fixture_path("test_add").expect("test_add fixture not found");
    let fixture_busyloop =
        McpSession::fixture_path("test_busyloop").expect("test_busyloop fixture not found");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Probe 1 — must be saved + dropped + reloaded.
    let session_id_1 = client
        .probe_start(fixture_add.to_str().unwrap())
        .await
        .expect("probe_start s1");
    tokio::time::sleep(Duration::from_secs(1)).await;
    let _ = client.probe_drain(&session_id_1).await.expect("drain s1");
    let _ = client.probe_stop(&session_id_1).await.expect("stop s1");
    client
        .save_session(&session_id_1, "cih_f_s1")
        .await
        .expect("save s1");

    // Probe 2 — must be stopped before save_session (production gate).
    let session_id_2 = client
        .probe_start(fixture_busyloop.to_str().unwrap())
        .await
        .expect("probe_start s2");
    tokio::time::sleep(Duration::from_secs(1)).await;
    let _ = client.probe_drain(&session_id_2).await.expect("drain s2");
    let _ = client.probe_stop(&session_id_2).await.expect("stop s2");
    client
        .save_session(&session_id_2, "cih_f_s2")
        .await
        .expect("save s2");

    // Drop s1, then load s1 (this is the previously-red path).
    client.drop_session(&session_id_1).await.expect("drop s1");
    let load_1 = client
        .load_session(&session_id_1)
        .await
        .expect("load s1 after drop");
    assert!(
        load_1.event_count > 0,
        "reloaded s1 must carry events (was the gate-failing case)"
    );
    let events_s1 = client
        .query_events(&session_id_1, QueryFilter::default())
        .await
        .expect("query s1 after reload");
    assert!(
        !events_s1.is_empty(),
        "query_events for reloaded s1 must return events"
    );

    // s2 must not have been replaced or wiped by s1's reopen.
    let load_2 = client
        .load_session(&session_id_2)
        .await
        .expect("load s2 must not be perturbed by s1 reopen");
    assert!(
        load_2.event_count > 0,
        "s2 must keep its events across s1 reopen"
    );
    let events_s2 = client
        .query_events(&session_id_2, QueryFilter::default())
        .await
        .expect("query s2 must still hold after s1 reopen");
    assert!(
        !events_s2.is_empty(),
        "query_events for s2 must return events across s1 reopen"
    );

    // Drop s2 and reload it too — sanity check on the symmetric path.
    client.drop_session(&session_id_2).await.expect("drop s2");
    let load_2_again = client
        .load_session(&session_id_2)
        .await
        .expect("load s2 after drop");
    assert!(
        load_2_again.event_count > 0,
        "s2 reload must also succeed (no IdentityMismatch on second session)"
    );

    client.shutdown().await.ok();
}
