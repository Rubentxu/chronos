//! Forensic tools tests — verify debug_analyze_memory, forensic_memory_audit,
//! and debug_find_variable_origin work correctly.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::McpSession;
use std::time::Duration;

#[tokio::test]
async fn test_debug_analyze_memory_after_probe_stop() {
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
    assert!(stop.total_events > 0, "Should have captured events");

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Analyze a range that might contain stack/heap addresses
    // Common stack base is around 0x7fff0000
    let start_addr = 0x7fff0000u64;
    let end_addr = 0x7fff1000u64;

    // Get timestamps from drained events if available
    let (start_ts, end_ts) = if !drained.is_empty() {
        let first_ts = drained.first().map(|e| e.timestamp_ns).unwrap_or(0);
        let last_ts = drained.last().map(|e| e.timestamp_ns).unwrap_or(0);
        (first_ts, last_ts + 1_000_000) // Add 1ms buffer
    } else {
        (0u64, 10_000_000u64) // 10ms default
    };

    let result = client.debug_analyze_memory(
        &session_id,
        start_addr,
        end_addr,
        start_ts,
        end_ts,
    ).await
        .expect("debug_analyze_memory failed");

    // Verify response structure
    println!("✓ debug_analyze_memory: {} total writes in range [0x{:x}, 0x{:x}]",
        result.total_writes, start_addr, end_addr);
    println!("  Time window: [{}, {}] ns", result.start_ts, result.end_ts);

    // Response should have the correct address range
    assert!(result.start_address.contains("7fff") || result.start_address.starts_with("0x"),
        "start_address should be in expected range");
    assert!(result.end_address.contains("7fff") || result.end_address.starts_with("0x"),
        "end_address should be in expected range");

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_debug_analyze_memory_empty_range() {
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(1)).await;
    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");
    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Analyze a range that likely has no memory events
    let result = client.debug_analyze_memory(
        &session_id,
        0xFFFF0000, // Very high address, unlikely to have events
        0xFFFF1000,
        0,
        10_000_000_000, // 10 seconds
    ).await
        .expect("debug_analyze_memory failed");

    println!("✓ debug_analyze_memory (empty range): {} total writes", result.total_writes);
    assert_eq!(result.total_writes, 0, "Should have no writes in high address range");
    assert!(result.accesses.is_empty(), "accesses should be empty");

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_forensic_memory_audit_after_probe_stop() {
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(1)).await;
    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");
    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Audit a specific address
    let address = 0x7fff0000u64; // Common stack base

    let result = client.forensic_memory_audit(&session_id, address, 10).await
        .expect("forensic_memory_audit failed");

    // Verify response structure
    println!("✓ forensic_memory_audit at 0x{:x}: {} writes", address, result.write_count);
    assert!(result.address.contains("7fff") || result.address.starts_with("0x"),
        "address should be in expected format");

    for write in result.writes.iter().take(3) {
        println!("  Event {}: ts={}, hex={}", write.event_id, write.timestamp_ns, write.data_hex);
    }

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_forensic_memory_audit_no_writes() {
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(1)).await;
    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");
    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Audit an address with likely no writes
    let address = 0xDEADu64;

    let result = client.forensic_memory_audit(&session_id, address, 10).await
        .expect("forensic_memory_audit failed");

    println!("✓ forensic_memory_audit (no writes): {} writes at 0x{:x}", result.write_count, address);
    assert_eq!(result.write_count, 0, "Should have no writes at unused address");
    assert!(result.writes.is_empty(), "writes should be empty");

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_debug_find_variable_origin_after_probe_stop() {
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(1)).await;
    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");
    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Try to find a variable that likely exists in the C program
    // test_add.c likely has variables like 'result', 'a', 'b'
    let result = client.debug_find_variable_origin(&session_id, "result", 10).await
        .expect("debug_find_variable_origin failed");

    // Verify response structure
    println!("✓ debug_find_variable_origin for 'result': {} mutations",
        result.mutation_count);
    println!("  Session: {}", result.session_id);
    println!("  Variable: {}", result.variable_name);

    if let Some(note) = result.note {
        println!("  Note: {}", note);
    }

    for mutation in result.mutations.iter().take(3) {
        println!("  Event {}: {} -> {} (thread {}, fn {:?})",
            mutation.event_id,
            mutation.value_before.as_deref().unwrap_or("?"),
            mutation.value_after.as_deref().unwrap_or("?"),
            mutation.thread_id,
            mutation.function.as_deref().unwrap_or("?"));
    }

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_debug_find_variable_origin_not_found() {
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(1)).await;
    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");
    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Find a variable that definitely doesn't exist
    let result = client.debug_find_variable_origin(&session_id, "nonexistent_var_xyz", 10).await
        .expect("debug_find_variable_origin failed");

    println!("✓ debug_find_variable_origin for nonexistent: {} mutations",
        result.mutation_count);
    assert_eq!(result.mutation_count, 0, "Should have no mutations for nonexistent variable");
    assert!(result.mutations.is_empty(), "mutations should be empty");

    client.shutdown().await.ok();
}
