//! Hotspot tools tests — verify debug_expand_hotspot and debug_get_saliency_scores work correctly.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::McpSession;
use std::time::Duration;

#[tokio::test]
async fn test_debug_expand_hotspot_after_probe_stop() {
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for it to complete
    tokio::time::sleep(Duration::from_secs(1)).await;

    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    println!("Probe stopped: {} total events", stop.total_events);
    assert!(stop.total_events > 0, "Should have captured events");

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Get hotspots
    let hotspot = client.debug_expand_hotspot(&session_id, 5).await
        .expect("debug_expand_hotspot failed");

    // Verify the response structure
    println!("✓ debug_expand_hotspot: {} functions, {} total calls",
        hotspot.function, hotspot.call_count);

    if let Some(cycles) = hotspot.total_cycles {
        println!("  Total cycles: {}", cycles);
    }

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_debug_expand_hotspot_busyloop() {
    // Use test_busyloop which runs longer and generates more events
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for it to run a bit
    tokio::time::sleep(Duration::from_secs(4)).await;

    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    println!("Probe stopped: {} total events", stop.total_events);
    assert!(stop.total_events > 0, "Should have captured events");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Get hotspots with top_n=10
    let hotspot = client.debug_expand_hotspot(&session_id, 10).await
        .expect("debug_expand_hotspot failed");

    println!("✓ debug_expand_hotspot (busyloop): {} functions, {} total calls",
        hotspot.function, hotspot.call_count);

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_debug_get_saliency_scores_after_probe_stop() {
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for it to complete
    tokio::time::sleep(Duration::from_secs(1)).await;

    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    println!("Probe stopped: {} total events", stop.total_events);
    assert!(stop.total_events > 0, "Should have captured events");

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Get saliency scores
    let scores = client.debug_get_saliency_scores(&session_id, 10).await
        .expect("debug_get_saliency_scores failed");

    // Verify the response structure
    println!("✓ debug_get_saliency_scores: {} functions scored", scores.len());

    for score in scores.iter().take(5) {
        println!("  {}: {:.4} ({} calls)",
            score.function, score.saliency_score, score.call_count);
    }

    // Each score should have saliency between 0 and 1
    for score in &scores {
        assert!(score.saliency_score >= 0.0 && score.saliency_score <= 1.0,
            "Saliency score should be between 0 and 1, got {}", score.saliency_score);
    }

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_debug_get_saliency_scores_busyloop() {
    // Use test_busyloop which runs longer and generates more events
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for it to run a bit
    tokio::time::sleep(Duration::from_secs(4)).await;

    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    println!("Probe stopped: {} total events", stop.total_events);
    assert!(stop.total_events > 0, "Should have captured events");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Get saliency scores
    let scores = client.debug_get_saliency_scores(&session_id, 20).await
        .expect("debug_get_saliency_scores failed");

    println!("✓ debug_get_saliency_scores (busyloop): {} functions scored", scores.len());

    // Scores should be sorted by saliency (highest first)
    for (i, score) in scores.iter().enumerate().take(10) {
        println!("  [{}] {}: {:.4}", i, score.function, score.saliency_score);
    }

    // Verify ordering (each score should be >= the next)
    for i in 0..scores.len().saturating_sub(1) {
        assert!(scores[i].saliency_score >= scores[i + 1].saliency_score,
            "Scores should be sorted descending");
    }

    client.shutdown().await.ok();
}
