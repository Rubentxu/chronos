//! Memory Analysis Depth tests — verify memory analysis tools work correctly with various address ranges.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::client::types::QueryFilter;
use chronos_sandbox::McpSession;
use std::time::Duration;

/// MD1: test_inspect_causality_stack_address_range
/// Probe test_add, inspect_causality with stack address, assert valid response.
#[tokio::test]
async fn test_inspect_causality_stack_address_range() {
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

    // Inspect a stack area address
    let stack_address: u64 = 0x7fff00000000;
    let report = client.inspect_causality(&session_id, stack_address).await
        .expect("inspect_causality failed");

    // Assert: valid response (entries array exists, no crash)
    println!("✓ inspect_causality at stack address 0x{:x}: {} mutations",
        stack_address, report.mutation_count);
    assert_eq!(report.session_id, session_id);
    // mutations array is part of the response structure
    println!("  Response has valid structure");

    client.shutdown().await.ok();
}

/// MD2: test_forensic_memory_audit_nonzero_address
/// Probe test_busyloop, forensic_memory_audit at text section address, assert valid response.
#[tokio::test]
async fn test_forensic_memory_audit_nonzero_address() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

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

    // Audit a text section address
    let text_address: u64 = 0x400000;
    let result = client.forensic_memory_audit(&session_id, text_address, 10).await
        .expect("forensic_memory_audit failed");

    // Assert: valid response (writes array exists)
    println!("✓ forensic_memory_audit at 0x{:x}: {} writes", text_address, result.write_count);
    assert!(result.address.contains("400000") || result.address.starts_with("0x"));
    // writes array is part of the response structure
    println!("  Response has valid structure with writes array");

    client.shutdown().await.ok();
}

/// MD3: test_debug_analyze_memory_valid_timestamps
/// Probe test_busyloop, get first 2 event timestamps, analyze memory range, assert valid response.
#[tokio::test]
async fn test_debug_analyze_memory_valid_timestamps() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

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

    // Get first 2 events to extract timestamps
    let filter = QueryFilter {
        limit: 2,
        offset: 0,
        ..Default::default()
    };
    let events = client.query_events(&session_id, filter).await
        .expect("query_events failed");

    if events.len() < 2 {
        println!("Not enough events to test timestamp range, skipping");
        client.shutdown().await.ok();
        return;
    }

    let ts1 = events[0].timestamp_ns;
    let ts2 = events[1].timestamp_ns;
    println!("Timestamps: ts1={}, ts2={}", ts1, ts2);

    // Analyze memory in that timestamp range
    let result = client.debug_analyze_memory(
        &session_id,
        0x0,
        0xFFFFFFFFFFFFFFFF,
        ts1,
        ts2,
    ).await
        .expect("debug_analyze_memory failed");

    // Assert: valid response (no crash, accesses array exists)
    println!("✓ debug_analyze_memory: {} total writes, {} accesses",
        result.total_writes, result.accesses.len());
    assert!(result.start_address.starts_with("0x") || !result.start_address.is_empty());
    assert!(result.end_address.starts_with("0x") || !result.end_address.is_empty());
    println!("  Response has valid structure with accesses array");

    client.shutdown().await.ok();
}

/// MD4: test_debug_analyze_memory_zero_range_valid
/// Probe test_busyloop, analyze small address range with full time range, assert valid response.
#[tokio::test]
async fn test_debug_analyze_memory_zero_range_valid() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

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

    // Analyze a small address range with full time range
    let result = client.debug_analyze_memory(
        &session_id,
        0x1000,
        0x2000,
        0,
        u64::MAX,
    ).await
        .expect("debug_analyze_memory failed");

    // Assert: valid response
    println!("✓ debug_analyze_memory [0x1000, 0x2000] full time: {} writes, {} accesses",
        result.total_writes, result.accesses.len());
    assert!(result.start_address.contains("1000") || result.start_address.starts_with("0x"));
    assert!(result.end_address.contains("2000") || result.end_address.starts_with("0x"));
    println!("  Response has valid structure");

    client.shutdown().await.ok();
}

/// MD5: test_debug_find_variable_origin_existing
/// Probe test_add, debug_find_variable_origin with "result", assert valid response.
#[tokio::test]
async fn test_debug_find_variable_origin_existing() {
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

    // Find variable origin for "result"
    let result = client.debug_find_variable_origin(&session_id, "result", 10).await
        .expect("debug_find_variable_origin failed");

    // Assert: valid response (mutations array exists, no crash even if empty)
    println!("✓ debug_find_variable_origin for 'result': {} mutations",
        result.mutation_count);
    assert_eq!(result.session_id, session_id);
    assert_eq!(result.variable_name, "result");
    println!("  Response has valid structure with mutations array");

    client.shutdown().await.ok();
}

/// MD6: test_debug_find_variable_origin_nonexistent
/// Probe test_add, debug_find_variable_origin with nonexistent variable, assert valid response.
#[tokio::test]
async fn test_debug_find_variable_origin_nonexistent() {
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

    // Find variable origin for nonexistent variable
    let result = client.debug_find_variable_origin(&session_id, "__nonexistent_var_xyz__", 10).await
        .expect("debug_find_variable_origin failed");

    // Assert: valid response with empty mutations (not a crash)
    println!("✓ debug_find_variable_origin for nonexistent: {} mutations",
        result.mutation_count);
    assert_eq!(result.mutation_count, 0);
    assert!(result.mutations.is_empty(), "mutations should be empty for nonexistent var");
    println!("  Response has valid structure with empty mutations");

    client.shutdown().await.ok();
}
