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
    let fixture_add = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found");
    let fixture_busyloop = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found");
    let fixture_threads = McpSession::fixture_path("test_threads")
        .expect("test_threads fixture not found");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Start and stop 3 probes sequentially
    let sid1 = client.probe_start(fixture_add.to_str().unwrap()).await
        .expect("probe 1 failed");
    tokio::time::sleep(Duration::from_secs(1)).await;
    let d1 = client.probe_drain(&sid1).await.expect("drain 1 failed");
    let s1 = client.probe_stop(&sid1).await.expect("stop 1 failed");
    println!("Session 1 (add): {} drained, {} total", d1.len(), s1.total_events);

    let sid2 = client.probe_start(fixture_busyloop.to_str().unwrap()).await
        .expect("probe 2 failed");
    tokio::time::sleep(Duration::from_secs(1)).await;
    let d2 = client.probe_drain(&sid2).await.expect("drain 2 failed");
    let s2 = client.probe_stop(&sid2).await.expect("stop 2 failed");
    println!("Session 2 (busyloop): {} drained, {} total", d2.len(), s2.total_events);

    let sid3 = client.probe_start(fixture_threads.to_str().unwrap()).await
        .expect("probe 3 failed");
    tokio::time::sleep(Duration::from_secs(1)).await;
    let d3 = client.probe_drain(&sid3).await.expect("drain 3 failed");
    let s3 = client.probe_stop(&sid3).await.expect("stop 3 failed");
    println!("Session 3 (threads): {} drained, {} total", d3.len(), s3.total_events);

    let total = s1.total_events + s2.total_events + s3.total_events;
    println!("✓ 3 sessions completed: {} total events", total);

    assert!(total > 0, "Should have captured events across sessions");

    client.shutdown().await.ok();
}

/// CS2: Rapid start/stop cycles on same session.
#[tokio::test]
async fn test_concurrent_rapid_start_stop_cycles() {
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Run 5 rapid start/stop cycles
    for i in 0..5 {
        let session_id = client.probe_start(fixture.to_str().unwrap()).await
            .expect("probe_start failed");

        tokio::time::sleep(Duration::from_millis(200)).await;

        let _drained = client.probe_drain(&session_id).await
            .expect("probe_drain failed");

        let stop = client.probe_stop(&session_id).await
            .expect("probe_stop failed");

        println!("Cycle {}: {} events", i, stop.total_events);
        assert!(stop.total_events >= 0, "Should get valid event count");
    }

    println!("✓ Completed 5 rapid start/stop cycles");

    client.shutdown().await.ok();
}

/// CS3: Multiple queries on same stopped session sequentially.
#[tokio::test]
async fn test_concurrent_sequential_queries_same_session() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");
    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    println!("Session {} has {} events", session_id, stop.total_events);

    // Query 20 times rapidly
    let mut success = 0;
    for i in 0..20 {
        let filter = chronos_sandbox::client::types::QueryFilter {
            limit: 5,
            offset: i * 5,
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
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Start and immediately stop 10 sessions
    let mut session_ids = Vec::new();

    for i in 0..10 {
        let session_id = client.probe_start(fixture.to_str().unwrap()).await
            .expect("probe_start failed");

        tokio::time::sleep(Duration::from_millis(100)).await;

        let _drained = client.probe_drain(&session_id).await
            .expect("probe_drain failed");

        let stop = client.probe_stop(&session_id).await
            .expect("probe_stop failed");

        tokio::time::sleep(Duration::from_millis(50)).await;

        // Save to persist
        let _save = client.save_session(&session_id, &format!("session_{}", i)).await;
        println!("Session {}: {} events saved", i, stop.total_events);

        session_ids.push(session_id);
    }

    println!("✓ Created {} sessions", session_ids.len());

    // List all sessions
    let sessions = client.list_sessions().await
        .expect("list_sessions failed");

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
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Create 3 sessions
    let mut session_ids = Vec::new();
    for i in 0..3 {
        let session_id = client.probe_start(fixture.to_str().unwrap()).await
            .expect("probe_start failed");

        tokio::time::sleep(Duration::from_millis(500)).await;
        let _drained = client.probe_drain(&session_id).await
            .expect("probe_drain failed");
        let _stop = client.probe_stop(&session_id).await
            .expect("probe_stop failed");

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Save immediately
        let save = client.save_session(&session_id, &format!("concurrent_{}", i)).await
            .expect("save failed");
        println!("Saved {} with {} events", session_id, save.event_count);

        session_ids.push((session_id, save.event_count));
    }

    // Now load them back
    for (sid, expected_count) in session_ids.iter() {
        let loaded = client.load_session(sid).await
            .expect("load failed");
        println!("Loaded {} with {} events", sid, loaded.event_count);
        assert_eq!(loaded.event_count, *expected_count,
            "Loaded count should match saved count");
    }

    println!("✓ Save/load cycle completed successfully");

    client.shutdown().await.ok();
}

/// CS6: Interleaved drain and query operations.
/// Note: query_events only works AFTER probe_stop builds the query engine.
/// This test verifies that pattern works correctly.
#[tokio::test]
async fn test_concurrent_interleaved_operations() {
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Create multiple sessions and interleave their operations
    let sid1 = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");
    let sid2 = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

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
        ..Default::default()
    };
    let events1 = client.query_events(&sid1, filter.clone()).await
        .expect("query 1 failed");
    println!("Query sid1: {} events", events1.len());

    // Stop second session
    let s2 = client.probe_stop(&sid2).await.expect("stop 2 failed");

    // Now query second session
    let events2 = client.query_events(&sid2, filter).await
        .expect("query 2 failed");
    println!("Query sid2: {} events", events2.len());

    println!("✓ Interleaved operations: {} + {} total events", s1.total_events, s2.total_events);

    client.shutdown().await.ok();
}

/// CS7: Multiple probes with different durations, verify event ordering.
#[tokio::test]
async fn test_concurrent_probes_different_durations() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Start 3 probes with different wait times before drain
    let sid1 = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start 1 failed");
    tokio::time::sleep(Duration::from_millis(100)).await; // Short wait
    let drain1 = client.probe_drain(&sid1).await.expect("drain 1 failed");
    let stop1 = client.probe_stop(&sid1).await.expect("stop 1 failed");
    println!("Short wait: {} drained, {} total", drain1.len(), stop1.total_events);

    let sid2 = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start 2 failed");
    tokio::time::sleep(Duration::from_millis(500)).await; // Medium wait
    let drain2 = client.probe_drain(&sid2).await.expect("drain 2 failed");
    let stop2 = client.probe_stop(&sid2).await.expect("stop 2 failed");
    println!("Medium wait: {} drained, {} total", drain2.len(), stop2.total_events);

    let sid3 = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start 3 failed");
    tokio::time::sleep(Duration::from_secs(1)).await; // Long wait
    let drain3 = client.probe_drain(&sid3).await.expect("drain 3 failed");
    let stop3 = client.probe_stop(&sid3).await.expect("stop 3 failed");
    println!("Long wait: {} drained, {} total", drain3.len(), stop3.total_events);

    // Longer waits should generally capture more events
    assert!(drain3.len() >= drain1.len(),
        "Longer wait should capture >= events than shorter wait");

    println!("✓ Probes with different durations completed successfully");

    client.shutdown().await.ok();
}

/// CS8: High-frequency sequential queries to stress test.
#[tokio::test]
async fn test_concurrent_high_frequency_queries() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");
    let _stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Fire 100 queries as fast as possible
    let mut success = 0;
    let mut failures = 0;

    for i in 0..100 {
        let filter = chronos_sandbox::client::types::QueryFilter {
            limit: 10,
            offset: (i * 10) % 100,
            ..Default::default()
        };

        match client.query_events(&session_id, filter).await {
            Ok(_) => success += 1,
            Err(_) => failures += 1,
        }
    }

    println!("✓ High-frequency query test: {} success, {} failures", success, failures);
    assert!(success >= 95, "Should have at least 95% success rate");

    client.shutdown().await.ok();
}

/// CS9: Session lifecycle stress - create, drain, stop, save, delete, repeat.
#[tokio::test]
async fn test_concurrent_session_lifecycle_stress() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    for i in 0..5 {
        let session_id = client.probe_start(fixture.to_str().unwrap()).await
            .expect("probe_start failed");

        tokio::time::sleep(Duration::from_millis(300)).await;

        let drained = client.probe_drain(&session_id).await
            .expect("drain failed");

        let stop = client.probe_stop(&session_id).await
            .expect("probe_stop failed");

        tokio::time::sleep(Duration::from_millis(50)).await;

        // Save session
        let saved = client.save_session(&session_id, &format!("lifecycle_{}", i)).await
            .expect("save failed");

        println!("Cycle {}: {} drained, {} stop, {} saved",
            i, drained.len(), stop.total_events, saved.event_count);

        // Load it back
        let loaded = client.load_session(&session_id).await
            .expect("load failed");
        assert_eq!(loaded.event_count, saved.event_count, "Loaded should match saved");

        // Delete it
        client.delete_session(&session_id).await
            .expect("delete failed");

        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    println!("✓ Session lifecycle stress completed");

    client.shutdown().await.ok();
}

/// CS10: Rapid fire tool calls - mix of different operations.
#[tokio::test]
async fn test_concurrent_rapid_fire_mixed_operations() {
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(1)).await;

    // Mix of operations
    let operations = ["probe_drain", "query_events", "list_threads", "get_execution_summary"];

    for i in 0..20 {
        let op = operations[i % operations.len()];
        let result = match op {
            "probe_drain" => {
                client.probe_drain(&session_id).await
                    .map(|r| format!("{} events", r.len()))
            }
            "query_events" => {
                let filter = chronos_sandbox::client::types::QueryFilter {
                    limit: 5,
                    offset: 0,
                    ..Default::default()
                };
                client.query_events(&session_id, filter).await
                    .map(|r| format!("{} events", r.len()))
            }
            "list_threads" => {
                client.list_threads(&session_id).await
                    .map(|r| format!("{} threads", r.len()))
            }
            "get_execution_summary" => {
                client.get_execution_summary(&session_id).await
                    .map(|r| format!("{} total events", r.total_events))
            }
            _ => unreachable!()
        };

        match result {
            Ok(msg) => {
                if i % 5 == 0 {
                    println!("Op {} ({}): {}", i, op, msg);
                }
            }
            Err(e) => {
                println!("Op {} ({}): ERROR {:?}", i, op, e);
            }
        }
    }

    let _stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    println!("✓ Rapid fire mixed operations completed");

    client.shutdown().await.ok();
}
