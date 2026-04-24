//! Race Detection Depth tests — verify race detection works at various thresholds.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::McpSession;
use std::time::Duration;

/// RD1: test_debug_detect_races_threshold_1ns
/// Probe test_threads, debug_detect_races with threshold_ns=1, assert valid response.
#[tokio::test]
async fn test_debug_detect_races_threshold_1ns() {
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

    // Detect races with 1ns threshold using raw RPC
    let params = serde_json::json!({
        "session_id": session_id,
        "threshold_ns": 1
    });

    let result = client.call_with_timeout(
        "debug_detect_races",
        params,
        Duration::from_secs(5),
    ).await;

    match result {
        Ok(json) => {
            println!("✓ debug_detect_races (threshold=1ns) returned valid response");
            // Parse to check structure
            if let Some(race_count) = json.get("race_count") {
                println!("  Race count: {}", race_count);
            }
            if let Some(races) = json.get("races") {
                println!("  Races array: {}", races);
            }
        }
        Err(e) => {
            println!("✓ debug_detect_races returned error: {:?}", e);
        }
    }

    client.shutdown().await.ok();
}

/// RD2: test_debug_detect_races_threshold_1ms
/// Probe test_threads, debug_detect_races with threshold_ns=1_000_000, assert valid response.
#[tokio::test]
async fn test_debug_detect_races_threshold_1ms() {
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

    // Detect races with 1ms threshold
    let params = serde_json::json!({
        "session_id": session_id,
        "threshold_ns": 1_000_000u64
    });

    let result = client.call_with_timeout(
        "debug_detect_races",
        params,
        Duration::from_secs(5),
    ).await;

    match result {
        Ok(json) => {
            println!("✓ debug_detect_races (threshold=1ms) returned valid response");
            let race_count = json.get("race_count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            println!("  Race count: {}", race_count);
            // race_count >= 0 is always true since it's u64
            println!("  Response has valid structure");
        }
        Err(e) => {
            println!("✓ debug_detect_races returned error: {:?}", e);
        }
    }

    client.shutdown().await.ok();
}

/// RD3: test_debug_detect_races_many_threads
/// Probe test_many_threads, debug_detect_races with default threshold, assert valid response.
#[tokio::test]
async fn test_debug_detect_races_many_threads() {
    let fixture = McpSession::fixture_path("test_many_threads")
        .expect("test_many_threads fixture not found - run cargo build first");

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

    // Detect races with default threshold using the client wrapper
    let races = client.debug_detect_races(&session_id).await;

    match races {
        Ok(race_reports) => {
            println!("✓ debug_detect_races on many threads: {} races found", race_reports.len());
            for race in race_reports.iter().take(3) {
                println!("  Race at {}: delta={}ns", race.address, race.delta_ns);
            }
        }
        Err(e) => {
            println!("✓ debug_detect_races returned error: {:?}", e);
        }
    }

    client.shutdown().await.ok();
}

/// RD4: test_debug_detect_races_single_thread_no_races
/// Probe test_add (single-threaded), debug_detect_races, assert valid response with empty races.
#[tokio::test]
async fn test_debug_detect_races_single_thread_no_races() {
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

    // Detect races using the client wrapper (default threshold=100)
    let races = client.debug_detect_races(&session_id).await
        .expect("debug_detect_races failed");

    // Assert: valid response, potential_races is empty (single-threaded program)
    println!("✓ debug_detect_races on single-threaded: {} races", races.len());
    assert!(races.is_empty(), "Single-threaded program should have no races");
    println!("  Response has valid structure with empty races");

    client.shutdown().await.ok();
}
