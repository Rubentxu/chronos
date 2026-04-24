//! Probe lifecycle tests — verify probe_start/probe_drain/probe_stop with real C fixtures.

use chronos_sandbox::{client::tools::McpTestClient, McpSession};
use std::time::Duration;

#[tokio::test]
async fn test_probe_start_and_drain() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Start probe
    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    assert!(!session_id.is_empty(), "session_id should not be empty");

    // Wait for program to execute (test_busyloop runs for ~3 seconds)
    tokio::time::sleep(Duration::from_secs(4)).await;

    // Drain events
    let events = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    // === Meaningful assertions ===
    // test_busyloop makes getpid() syscalls every 10 iterations + exit syscall
    assert!(!events.is_empty(), "Should capture at least 1 event from test_busyloop");

    // Verify event structure
    let first = &events[0];
    assert!(first.timestamp_ns > 0, "Event timestamp should be non-zero");
    assert!(first.thread_id > 0, "Event thread_id should be non-zero");

    // Collect unique kinds and descriptions
    let kinds: std::collections::HashSet<&str> = events.iter()
        .map(|e| e.kind.as_str())
        .collect();
    let descriptions: std::collections::HashSet<&str> = events.iter()
        .map(|e| e.description.as_str())
        .collect();

    println!("✓ Captured {} events", events.len());
    println!("  Kinds: {:?}", kinds);
    println!("  Descriptions: {:?}", descriptions);

    // We should have at least some events with non-empty kind
    assert!(!kinds.is_empty(), "Should have at least one event kind");

    // Verify kind is Unresolved (no C semantic resolver exists yet - that's expected)
    // The important thing is we capture events, not that they're semantically resolved
    assert!(kinds.contains("Unresolved"), "Events should be marked Unresolved (no C resolver)");

    // Verify descriptions include syscall events
    let has_syscalls = descriptions.iter().any(|d| d.contains("Syscall"));
    assert!(has_syscalls, "Should capture syscall events (got descriptions: {:?})", descriptions);

    // Stop probe
    client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_crash_detection() {
    // test_segfault crashes immediately with SIGSEGV
    let fixture = McpSession::fixture_path("test_segfault")
        .expect("test_segfault fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Start probe on crashing program
    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Give it time to crash
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Drain events (may have some before crash)
    let events = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    println!("Captured {} events before/after crash", events.len());

    // Stop probe - this builds the query engine
    let stop_result = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    println!("Probe stopped: {} total events", stop_result.total_events);
    assert!(stop_result.total_events > 0, "Should have captured events from crash");

    // Give the query engine a moment to index
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Find the crash
    let crash = client.debug_find_crash(&session_id).await
        .expect("debug_find_crash failed");

    println!("debug_find_crash result: crash={:?}", crash);

    // === Assertions ===
    assert!(crash.is_some(), "Should detect a crash in test_segfault (total_events={})", stop_result.total_events);
    let crash = crash.unwrap();
    assert!(crash.crash_found, "crash_found should be true");

    // Verify crash details
    assert!(crash.signal.is_some(), "Should have signal information");
    let signal = crash.signal.unwrap();
    assert!(signal.contains("SEGV") || signal.contains("SIGSEGV") || signal.contains("11"),
        "Signal should be SIGSEGV (11), got: {}", signal);

    println!("✓ Crash detected: signal={}, event_id={:?}", signal, crash.event_id);
    if let Some(cs) = &crash.call_stack {
        println!("  Call stack ({} frames):", cs.len());
        for frame in cs.iter().take(3) {
            println!("    {:?}", frame);
        }
    }

    client.shutdown().await.ok();
}
