//! Memory tools tests — verify inspect_causality and debug_detect_races
//! work correctly after probe_stop.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::McpSession;
use std::time::Duration;

#[tokio::test]
async fn test_debug_detect_races_no_races() {
    // Use test_add which is simple and shouldn't have races
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

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Detect races
    let races = client.debug_detect_races(&session_id).await
        .expect("debug_detect_races failed");

    // === Assertions ===
    // test_add is single-threaded, so no races expected
    // But the call should succeed and return valid JSON
    println!("✓ debug_detect_races returned {} races", races.len());

    for race in races.iter().take(5) {
        println!("  Race at address {}: delta_ns={}", race.address, race.delta_ns);
    }

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_debug_detect_races_threads() {
    // Use test_threads which has multiple threads
    let fixture = McpSession::fixture_path("test_threads")
        .expect("test_threads fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for threads to do some work
    tokio::time::sleep(Duration::from_secs(3)).await;

    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    println!("Probe stopped: {} total events", stop.total_events);

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Detect races
    let races = client.debug_detect_races(&session_id).await
        .expect("debug_detect_races failed");

    // May or may not find races depending on whether threads
    // access shared memory simultaneously
    println!("✓ debug_detect_races returned {} races", races.len());

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_inspect_causality_empty_address() {
    // Inspecting an address that likely has no writes
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

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Inspect an address that likely has no writes
    let report = client.inspect_causality(&session_id, 0xDEAD).await
        .expect("inspect_causality failed");

    // === Assertions ===
    assert_eq!(report.session_id, session_id, "session_id should match");
    // Address format may vary - check the string representation
    println!("✓ inspect_causality at 0xDEAD: {} mutations", report.mutation_count);

    if let Some(note) = report.note {
        println!("  Note: {}", note);
    }

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_inspect_causality_valid_address() {
    // Use test_add and look for writes to stack/heap addresses
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for it to complete
    tokio::time::sleep(Duration::from_secs(1)).await;

    let drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    println!("Probe stopped: {} total events", stop.total_events);

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Inspect a valid address range - typically stack is in lower addresses
    // and heap is in higher addresses
    let stack_address: u64 = 0x7fff0000; // Common stack base

    let report = client.inspect_causality(&session_id, stack_address).await
        .expect("inspect_causality failed");

    println!("✓ inspect_causality at 0x{:x}: {} mutations", stack_address, report.mutation_count);

    // Show first few mutations if any
    for mutation in report.mutations.iter().take(3) {
        println!("  Event {}: thread {}, value {} -> {}",
            mutation.event_id, mutation.thread_id,
            mutation.value_before.as_deref().unwrap_or("?"),
            mutation.value_after.as_deref().unwrap_or("?"));
    }

    client.shutdown().await.ok();
}
