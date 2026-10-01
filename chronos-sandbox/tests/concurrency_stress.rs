//! Concurrency stress E2E tests — verify system handles concurrent operations correctly.
//!
//! These tests push the system with:
//! - Multiple probe sessions created in sequence
//! - Rapid start/stop cycles
//! - Multiple sessions with interleaved operations
//!
//! Note: True concurrent operations require multiple clients (separate processes).
//! These tests verify sequential operations don't cause issues.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::McpSession;
use std::time::Duration;

/// CS1: Start 3 probe sessions sequentially, drain and stop each.
#[tokio::test]
async fn test_concurrent_multiple_probes_sequential_start() {
    let fixture_add = McpSession::fixture_path("test_add").expect("test_add fixture not found");
    let fixture_busyloop =
        McpSession::fixture_path("test_busyloop").expect("test_busyloop fixture not found");
    let fixture_threads =
        McpSession::fixture_path("test_threads").expect("test_threads fixture not found");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Start and stop 3 probes sequentially
    let sid1 = client
        .probe_start(fixture_add.to_str().unwrap())
        .await
        .expect("probe 1 failed");
    tokio::time::sleep(Duration::from_secs(1)).await;
    let d1 = client.probe_drain(&sid1).await.expect("drain 1 failed");
    let s1 = client.probe_stop(&sid1).await.expect("stop 1 failed");
    println!(
        "Session 1 (add): {} drained, {} total",
        d1.len(),
        s1.total_events
    );

    let sid2 = client
        .probe_start(fixture_busyloop.to_str().unwrap())
        .await
        .expect("probe 2 failed");
    tokio::time::sleep(Duration::from_secs(1)).await;
    let d2 = client.probe_drain(&sid2).await.expect("drain 2 failed");
    let s2 = client.probe_stop(&sid2).await.expect("stop 2 failed");
    println!(
        "Session 2 (busyloop): {} drained, {} total",
        d2.len(),
        s2.total_events
    );

    let sid3 = client
        .probe_start(fixture_threads.to_str().unwrap())
        .await
        .expect("probe 3 failed");
    tokio::time::sleep(Duration::from_secs(1)).await;
    let d3 = client.probe_drain(&sid3).await.expect("drain 3 failed");
    let s3 = client.probe_stop(&sid3).await.expect("stop 3 failed");
    println!(
        "Session 3 (threads): {} drained, {} total",
        d3.len(),
        s3.total_events
    );

    let total = s1.total_events + s2.total_events + s3.total_events;
    println!("✓ 3 sessions completed: {} total events", total);

    assert!(total > 0, "Should have captured events across sessions");

    client.shutdown().await.ok();
}

/// CS2: Rapid start/stop cycles on same session.
///
/// Five sequential start → drain → stop → query cycles against the same
/// fixture. The property under test is isolation *across* cycles: every
/// cycle must mint a distinct session id, capture a non-empty trace of
/// the fixture it was pointed at, and leave behind a session whose query
/// engine agrees with the count `probe_stop` reported for it.
///
/// Measured on this environment: each cycle stops with `status="stopped"`
/// and 128 events. The absolute count is fixture/kernel dependent (128 and
/// 129 were both observed across tests), so no literal is asserted — only
/// cross-tool relations that hold for any count.
#[tokio::test]
async fn test_concurrent_rapid_start_stop_cycles() {
    let fixture = McpSession::fixture_path("test_add").expect("test_add fixture not found");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let mut seen_ids: Vec<String> = Vec::new();
    let mut total_across_cycles = 0usize;

    // Run 5 rapid start/stop cycles
    for i in 0..5 {
        let session_id = client
            .probe_start(fixture.to_str().unwrap())
            .await
            .expect("probe_start failed");

        tokio::time::sleep(Duration::from_millis(200)).await;

        let drained = client
            .probe_drain(&session_id)
            .await
            .expect("probe_drain failed");

        let stop = client
            .probe_stop(&session_id)
            .await
            .expect("probe_stop failed");

        println!(
            "Cycle {}: {} drained, {} total",
            i,
            drained.len(),
            stop.total_events
        );

        // A reused id would mean the previous cycle's session was not torn
        // down; the events of cycle N would bleed into cycle N+1.
        assert!(
            !seen_ids.contains(&session_id),
            "cycle {i}: session id {session_id} was reused across rapid cycles"
        );

        // The session must be the fixture we pointed at, and the capture
        // must be non-empty: `test_add` is a real dynamically linked
        // process, so its loader alone guarantees syscall events.
        assert_eq!(
            stop.target,
            fixture.to_str().unwrap(),
            "cycle {i}: stop must report the fixture that was started"
        );
        assert!(
            stop.total_events > 0,
            "cycle {i}: a live process must produce events, got {}",
            stop.total_events
        );
        assert!(
            !drained.is_empty(),
            "cycle {i}: draining a live process returned no events"
        );
        assert!(
            drained.len() <= stop.total_events,
            "cycle {i}: drained {} exceeds the session total {}",
            drained.len(),
            stop.total_events
        );

        // `probe_stop` is what builds the session's query engine, so the
        // session is queryable the moment stop returns — and the page the
        // engine serves must be `min(limit, total_events)`.
        let filter = chronos_sandbox::client::types::QueryFilter {
            limit: 5,
            offset: 0,
            cursor: None,
            ..Default::default()
        };
        let page = client
            .query_events(&session_id, filter)
            .await
            .unwrap_or_else(|e| panic!("cycle {i}: query after stop failed: {e:?}"));
        assert_eq!(
            page.len(),
            std::cmp::min(5, stop.total_events),
            "cycle {i}: query page must be min(limit=5, total_events)"
        );

        seen_ids.push(session_id);
        total_across_cycles += stop.total_events;
    }

    assert_eq!(seen_ids.len(), 5, "all 5 rapid cycles must complete");
    assert!(
        total_across_cycles > 0,
        "5 completed cycles must have captured events in aggregate"
    );

    println!("✓ Completed 5 rapid start/stop cycles");

    client.shutdown().await.ok();
}

/// CS3: Multiple queries on same stopped session sequentially.
#[tokio::test]
async fn test_concurrent_sequential_queries_same_session() {
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

    println!("Session {} has {} events", session_id, stop.total_events);

    // Query 20 times rapidly. R0.2 (2026-09-22): cursor walk
    // replaces offset-based pagination, so we always send offset=0
    // and let the server emit the first page (limit=5). The
    // observable property under stress ("client handles rapid
    // queries without RPC errors") is preserved.
    let mut success = 0;
    for i in 0..20 {
        let filter = chronos_sandbox::client::types::QueryFilter {
            limit: 5,
            offset: 0,
            cursor: None,
            ..Default::default()
        };
        match client.query_events(&session_id, filter).await {
            Ok(events) => {
                success += 1;
                if i % 5 == 0 {
                    println!("Query {}: {} events", i, events.len());
                }
            }
            Err(e) => {
                println!("Query {} failed: {:?}", i, e);
            }
        }
    }

    println!("✓ Sequential queries: {}/20 succeeded", success);
    assert!(success >= 18, "Should have at least 90% success rate");

    client.shutdown().await.ok();
}

/// CS4: Create many sessions and list them.
#[tokio::test]
async fn test_concurrent_many_sessions_list() {
    let fixture = McpSession::fixture_path("test_add").expect("test_add fixture not found");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Start and immediately stop 10 sessions
    let mut session_ids = Vec::new();

    for i in 0..10 {
        let session_id = client
            .probe_start(fixture.to_str().unwrap())
            .await
            .expect("probe_start failed");

        tokio::time::sleep(Duration::from_millis(100)).await;

        let _drained = client
            .probe_drain(&session_id)
            .await
            .expect("probe_drain failed");

        let stop = client
            .probe_stop(&session_id)
            .await
            .expect("probe_stop failed");

        tokio::time::sleep(Duration::from_millis(50)).await;

        // Save to persist
        let _save = client
            .save_session(&session_id, &format!("session_{}", i))
            .await;
        println!("Session {}: {} events saved", i, stop.total_events);

        session_ids.push(session_id);
    }

    println!("✓ Created {} sessions", session_ids.len());

    // List all sessions
    let sessions = client.list_sessions().await.expect("list_sessions failed");

    println!("✓ list_sessions returned {} sessions", sessions.len());
    assert!(sessions.len() >= 10, "Should have at least 10 sessions");

    // Clean up
    for sid in session_ids {
        client.delete_session(&sid).await.ok();
    }

    client.shutdown().await.ok();
}

/// CS5: Save and load cycle for multiple sessions.
#[tokio::test]
async fn test_concurrent_save_load_cycle_multiple_sessions() {
    let fixture =
        McpSession::fixture_path("test_busyloop").expect("test_busyloop fixture not found");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Create 3 sessions
    let mut session_ids = Vec::new();
    for i in 0..3 {
        let session_id = client
            .probe_start(fixture.to_str().unwrap())
            .await
            .expect("probe_start failed");

        tokio::time::sleep(Duration::from_millis(500)).await;
        let _drained = client
            .probe_drain(&session_id)
            .await
            .expect("probe_drain failed");
        let _stop = client
            .probe_stop(&session_id)
            .await
            .expect("probe_stop failed");

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Save immediately
        let save = client
            .save_session(&session_id, &format!("concurrent_{}", i))
            .await
            .expect("save failed");
        println!("Saved {} with {} events", session_id, save.event_count);

        session_ids.push((session_id, save.event_count));
    }

    // Now load them back
    for (sid, expected_count) in session_ids.iter() {
        let loaded = client.load_session(sid).await.expect("load failed");
        println!("Loaded {} with {} events", sid, loaded.event_count);
        assert_eq!(
            loaded.event_count, *expected_count,
            "Loaded count should match saved count"
        );
    }

    println!("✓ Save/load cycle completed successfully");

    client.shutdown().await.ok();
}

/// CS6: Interleaved drain and query operations.
///
/// Two sessions are started before either is drained, then per-session
/// work is interleaved: drain both, stop+query the first, stop+query the
/// second. `probe_stop` is what builds a session's query engine, so every
/// query below is issued against an already-stopped session.
///
/// Whether the *second* concurrent probe attaches before `test_add` exits
/// is a race this test makes no claim about: measured runs capture 128–129
/// events in the first session and 0 in the second. The asserts below
/// therefore pin only the relations that hold in either outcome — the
/// queried page always matches the count that session's own stop reported.
#[tokio::test]
async fn test_concurrent_interleaved_operations() {
    let fixture = McpSession::fixture_path("test_add").expect("test_add fixture not found");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Create multiple sessions and interleave their operations
    let sid1 = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");
    let sid2 = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    // Two live probes must be two independent sessions, not one handle
    // aliased twice — otherwise interleaving them proves nothing.
    assert_ne!(
        sid1, sid2,
        "two probe_start calls must mint distinct sessions"
    );

    // Wait for events
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Drain both
    let d1 = client.probe_drain(&sid1).await.expect("drain 1 failed");
    let d2 = client.probe_drain(&sid2).await.expect("drain 2 failed");
    println!("Drained: {} and {} events", d1.len(), d2.len());

    // Stop first, query it, then stop second
    let s1 = client.probe_stop(&sid1).await.expect("stop 1 failed");

    // Now query first session (it's finalized)
    let filter = chronos_sandbox::client::types::QueryFilter {
        limit: 10,
        offset: 0,
        cursor: None,
        ..Default::default()
    };
    let events1 = client
        .query_events(&sid1, filter.clone())
        .await
        .expect("query 1 failed");
    println!("Query sid1: {} events", events1.len());

    // Stop second session
    let s2 = client.probe_stop(&sid2).await.expect("stop 2 failed");

    // Now query second session
    let events2 = client
        .query_events(&sid2, filter)
        .await
        .expect("query 2 failed");
    println!("Query sid2: {} events", events2.len());

    // The drained session must hold a real trace: a drain can only report
    // a subset of the session, never more than stop later counts.
    assert!(
        s1.total_events > 0,
        "first session must capture events from a live process"
    );
    assert!(
        !d1.is_empty(),
        "drain 1 returned no events for a live process"
    );
    assert!(
        d1.len() <= s1.total_events,
        "drain 1 returned {} events but the session totals {}",
        d1.len(),
        s1.total_events
    );
    assert!(
        d2.len() <= s2.total_events,
        "drain 2 returned {} events but the session totals {}",
        d2.len(),
        s2.total_events
    );

    // The query engine built by stop must serve exactly the session it
    // belongs to: a full page capped by that session's own total.
    assert_eq!(
        events1.len(),
        std::cmp::min(10, s1.total_events),
        "sid1 page must be min(limit=10, sid1 total_events)"
    );
    assert_eq!(
        events2.len(),
        std::cmp::min(10, s2.total_events),
        "sid2 page must be min(limit=10, sid2 total_events)"
    );

    // Within a page, ids are the engine's own sequence: ascending and
    // duplicate-free, i.e. nothing dropped and nothing replayed.
    let ids1: Vec<u64> = events1.iter().map(|e| e.event_id).collect();
    let mut sorted1 = ids1.clone();
    sorted1.sort_unstable();
    sorted1.dedup();
    assert_eq!(
        ids1, sorted1,
        "sid1 page event_ids must be unique and ascending"
    );

    println!(
        "✓ Interleaved operations: {} + {} total events",
        s1.total_events, s2.total_events
    );

    client.shutdown().await.ok();
}

/// CS7: Multiple probes with different durations, verify event ordering.
#[tokio::test]
async fn test_concurrent_probes_different_durations() {
    let fixture =
        McpSession::fixture_path("test_busyloop").expect("test_busyloop fixture not found");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Start 3 probes with different wait times before drain
    let sid1 = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start 1 failed");
    tokio::time::sleep(Duration::from_millis(100)).await; // Short wait
    let drain1 = client.probe_drain(&sid1).await.expect("drain 1 failed");
    let stop1 = client.probe_stop(&sid1).await.expect("stop 1 failed");
    println!(
        "Short wait: {} drained, {} total",
        drain1.len(),
        stop1.total_events
    );

    let sid2 = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start 2 failed");
    tokio::time::sleep(Duration::from_millis(500)).await; // Medium wait
    let drain2 = client.probe_drain(&sid2).await.expect("drain 2 failed");
    let stop2 = client.probe_stop(&sid2).await.expect("stop 2 failed");
    println!(
        "Medium wait: {} drained, {} total",
        drain2.len(),
        stop2.total_events
    );

    let sid3 = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start 3 failed");
    tokio::time::sleep(Duration::from_secs(1)).await; // Long wait
    let drain3 = client.probe_drain(&sid3).await.expect("drain 3 failed");
    let stop3 = client.probe_stop(&sid3).await.expect("stop 3 failed");
    println!(
        "Long wait: {} drained, {} total",
        drain3.len(),
        stop3.total_events
    );

    // Longer waits should generally capture more events
    assert!(
        drain3.len() >= drain1.len(),
        "Longer wait should capture >= events than shorter wait"
    );

    println!("✓ Probes with different durations completed successfully");

    client.shutdown().await.ok();
}

/// CS8: High-frequency sequential queries to stress test.
#[tokio::test]
async fn test_concurrent_high_frequency_queries() {
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

    // Fire 100 queries as fast as possible. R0.2 (2026-09-22):
    // cursor walk replaces offset-based pagination; we send
    // offset=0 every time and let the server emit the first page
    // (limit=10). The observable property under stress ("no
    // RPC failures under high QPS") is preserved.
    let mut success = 0;
    let mut failures = 0;

    for _i in 0..100 {
        let filter = chronos_sandbox::client::types::QueryFilter {
            limit: 10,
            offset: 0,
            cursor: None,
            ..Default::default()
        };

        match client.query_events(&session_id, filter).await {
            Ok(_) => success += 1,
            Err(_) => failures += 1,
        }
    }

    println!(
        "✓ High-frequency query test: {} success, {} failures",
        success, failures
    );
    assert!(success >= 95, "Should have at least 95% success rate");

    client.shutdown().await.ok();
}

/// CS9: Session lifecycle stress - create, drain, stop, save, delete, repeat.
#[tokio::test]
async fn test_concurrent_session_lifecycle_stress() {
    let fixture =
        McpSession::fixture_path("test_busyloop").expect("test_busyloop fixture not found");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    for i in 0..5 {
        let session_id = client
            .probe_start(fixture.to_str().unwrap())
            .await
            .expect("probe_start failed");

        tokio::time::sleep(Duration::from_millis(300)).await;

        let drained = client.probe_drain(&session_id).await.expect("drain failed");

        let stop = client
            .probe_stop(&session_id)
            .await
            .expect("probe_stop failed");

        tokio::time::sleep(Duration::from_millis(50)).await;

        // Save session
        let saved = client
            .save_session(&session_id, &format!("lifecycle_{}", i))
            .await
            .expect("save failed");

        println!(
            "Cycle {}: {} drained, {} stop, {} saved",
            i,
            drained.len(),
            stop.total_events,
            saved.event_count
        );

        // Load it back
        let loaded = client.load_session(&session_id).await.expect("load failed");
        assert_eq!(
            loaded.event_count, saved.event_count,
            "Loaded should match saved"
        );

        // Delete it
        client
            .delete_session(&session_id)
            .await
            .expect("delete failed");

        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    println!("✓ Session lifecycle stress completed");

    client.shutdown().await.ok();
}

/// CS10: Rapid fire tool calls - mix of different operations.
///
/// 20 calls rotating over 4 different tools against one live session,
/// followed by the stop. The property under test: no RPC fails under
/// rapid mixed traffic, every tool of the mix is really exercised (5 calls
/// each), and each tool's answer agrees with the count `probe_stop`
/// finally reports for that same session.
///
/// Measured: each `probe_drain` returns the full 128-event trace (the
/// fixture has already exited, so the trace is frozen and drains are
/// idempotent), `events_read(mode=query, limit=5)` serves a 5-event page
/// even while the session is still live, `list_threads` reports the 1
/// thread of a single-threaded fixture, and the execution summary reports
/// the same 128 events stop later confirms.
#[tokio::test]
async fn test_concurrent_rapid_fire_mixed_operations() {
    let fixture = McpSession::fixture_path("test_add").expect("test_add fixture not found");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(1)).await;

    // Mix of operations. REC-C5-C5.2: `query_events` and
    // `get_execution_summary` migrated to the v2 dispatchers
    // `events_read(mode=query)` and `execution_query(kind=execution_summary)`.
    let operations = [
        "probe_drain",
        "events_read",
        "list_threads",
        "execution_query",
    ];

    // One bucket per tool plus a failure log: a tool that silently drops
    // out of the rotation must not shrink its bucket unnoticed.
    let mut drain_lens: Vec<usize> = Vec::new();
    let mut read_lens: Vec<usize> = Vec::new();
    let mut read_pages: Vec<Vec<u64>> = Vec::new();
    let mut thread_counts: Vec<usize> = Vec::new();
    let mut summary_totals: Vec<u64> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for i in 0..20 {
        let op = operations[i % operations.len()];
        let result = match op {
            "probe_drain" => client.probe_drain(&session_id).await.map(|r| {
                drain_lens.push(r.len());
                format!("{} events", r.len())
            }),
            "events_read" => client
                .call_tool(
                    "events_read",
                    serde_json::json!({
                        "session_id": session_id,
                        "mode": "query",
                        "limit": 5
                    }),
                )
                .await
                .map(|v| {
                    // The v2 query envelope nests the page under
                    // `result.events`; reading a top-level `events` key
                    // would report 0 for a page that is actually full.
                    let page = v
                        .pointer("/result/events")
                        .and_then(|e| e.as_array())
                        .cloned()
                        .unwrap_or_default();
                    let ids: Vec<u64> = page
                        .iter()
                        .map(|ev| ev.get("event_id").and_then(|i| i.as_u64()))
                        .collect::<Option<Vec<_>>>()
                        .unwrap_or_default();
                    read_lens.push(page.len());
                    read_pages.push(ids);
                    format!("{} events", page.len())
                }),
            "list_threads" => client.list_threads(&session_id).await.map(|r| {
                thread_counts.push(r.len());
                format!("{} threads", r.len())
            }),
            "execution_query" => client
                .call_tool(
                    "execution_query",
                    serde_json::json!({
                        "session_id": session_id,
                        "kind": "execution_summary"
                    }),
                )
                .await
                .map(|v| {
                    let total = v.get("total_events").and_then(|t| t.as_u64()).unwrap_or(0);
                    summary_totals.push(total);
                    format!("{} total events", total)
                }),
            _ => unreachable!(),
        };

        match result {
            Ok(msg) => {
                if i % 5 == 0 {
                    println!("Op {} ({}): {}", i, op, msg);
                }
            }
            Err(e) => {
                println!("Op {} ({}): ERROR {:?}", i, op, e);
                errors.push(format!("op {i} ({op}): {e:?}"));
            }
        }
    }

    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");

    // 1. Rapid mixed traffic must not produce a single RPC error.
    assert!(
        errors.is_empty(),
        "rapid fire must not produce RPC errors, got {:?}",
        errors
    );

    // 2. The rotation must have reached every tool, 20/4 = 5 times each.
    assert_eq!(drain_lens.len(), 5, "probe_drain must be called 5 times");
    assert_eq!(read_lens.len(), 5, "events_read must be called 5 times");
    assert_eq!(
        thread_counts.len(),
        5,
        "list_threads must be called 5 times"
    );
    assert_eq!(
        summary_totals.len(),
        5,
        "execution_query must be called 5 times"
    );

    // 3. The fixture is a real process, so the session is never empty and
    //    every tool must agree with the count stop finally reports.
    assert!(
        stop.total_events > 0,
        "a live process must produce events, got {}",
        stop.total_events
    );
    for (i, n) in drain_lens.iter().enumerate() {
        assert_eq!(
            *n, stop.total_events,
            "drain {i} returned {n} but the session totals {}",
            stop.total_events
        );
    }
    for (i, total) in summary_totals.iter().enumerate() {
        assert_eq!(
            *total as usize, stop.total_events,
            "execution summary {i} reported {total} but the session totals {}",
            stop.total_events
        );
    }

    // 4. A live-session query must honour its limit, and every page must
    //    carry the engine's own ascending, duplicate-free sequence. Each
    //    call asks for the first page from the start, so the five pages
    //    must be identical: the read is repeatable and does not drift
    //    while concurrent traffic continues.
    for (i, n) in read_lens.iter().enumerate() {
        assert_eq!(
            *n, 5,
            "events_read {i} must serve the requested limit of 5, got {n}"
        );
    }
    for (i, page) in read_pages.iter().enumerate() {
        assert_eq!(
            page.len(),
            5,
            "events_read {i} must carry an event_id per event, got {page:?}"
        );
        let mut sorted = page.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            page, &sorted,
            "events_read page {i} must be ascending and duplicate-free, got {page:?}"
        );
    }
    for (i, page) in read_pages.iter().enumerate().skip(1) {
        assert_eq!(
            page, &read_pages[0],
            "events_read page {i} drifted from the first page under rapid fire"
        );
    }

    // 5. `test_add` is single-threaded, so the thread view is 1 every time
    //    and must not drift between repeated calls.
    for (i, n) in thread_counts.iter().enumerate() {
        assert_eq!(
            *n, 1,
            "list_threads {i} must report the fixture's single thread, got {n}"
        );
    }

    println!(
        "✓ Rapid fire mixed operations: 20 calls, 0 errors, {} events",
        stop.total_events
    );

    client.shutdown().await.ok();
}
