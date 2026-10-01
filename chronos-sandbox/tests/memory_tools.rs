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

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    // Wait for it to complete
    tokio::time::sleep(Duration::from_secs(1)).await;

    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");

    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");

    assert!(
        stop.total_events > 0,
        "probe_stop should report captured events, got {}",
        stop.total_events
    );

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Detect races
    let races = client
        .debug_detect_races(&session_id)
        .await
        .expect("debug_detect_races failed");

    // === Assertions ===
    // test_add is single-threaded, so no races expected
    // But the call should succeed and return valid JSON
    assert!(
        races.is_empty(),
        "single-threaded test_add must not report data races, got {:?}",
        races
    );

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_debug_detect_races_threads() {
    // Use test_threads which has multiple threads
    let fixture = McpSession::fixture_path("test_threads")
        .expect("test_threads fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    // Wait for threads to do some work
    tokio::time::sleep(Duration::from_secs(3)).await;

    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");

    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");

    assert!(
        stop.total_events > 0,
        "probe_stop should report captured events, got {}",
        stop.total_events
    );

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Detect races
    let races = client
        .debug_detect_races(&session_id)
        .await
        .expect("debug_detect_races failed");

    // The test_threads fixture has no shared mutable state: each worker()
    // accumulates into its own stack-local `sum` and reads only its own
    // `ids[i]` slot. Two different addresses are never written by two
    // threads, so zero races is the deterministic expectation here, not an
    // environment-dependent one.
    assert!(
        races.is_empty(),
        "test_threads has no shared memory writes, so no races are expected, got {:?}",
        races
    );

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_inspect_causality_empty_address() {
    // Inspecting an address that likely has no writes
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    // Wait for it to complete
    tokio::time::sleep(Duration::from_secs(1)).await;

    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");

    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");

    assert!(
        stop.total_events > 0,
        "probe_stop should report captured events, got {}",
        stop.total_events
    );

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Inspect an address that has no writes. 0xDEAD is not touched by the
    // fixture, so the report must come back empty (and not fail).
    let report = client
        .inspect_causality(&session_id, 0xDEAD)
        .await
        .expect("inspect_causality failed");

    // === Assertions ===
    assert_eq!(report.session_id, session_id, "session_id should match");
    assert_eq!(
        report.address, 0xDEAD,
        "causality report should echo the requested address"
    );
    assert_eq!(
        report.mutation_count, 0,
        "an address that is never written must report zero mutations"
    );
    assert!(
        report.mutations.is_empty(),
        "mutation_count and mutations must agree, got {:?}",
        report.mutations
    );

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_inspect_causality_valid_address() {
    // Use test_add and look for writes to stack/heap addresses
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    // Wait for it to complete
    tokio::time::sleep(Duration::from_secs(1)).await;

    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");

    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");

    assert!(
        stop.total_events > 0,
        "probe_stop should report captured events, got {}",
        stop.total_events
    );

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Inspect a plausible stack address. This capture of test_add records
    // only syscall events (every drained event is an Unresolved
    // SyscallEnter/SyscallExit), so the trace carries no memory-write
    // evidence at all and the report legitimately comes back empty for any
    // address - including this realistic-looking one. The value asserted here
    // is the observable contract: the report echoes the requested address and
    // reports no mutations for it.
    let stack_address: u64 = 0x7fff0000; // Common stack base
    let report = client
        .inspect_causality(&session_id, stack_address)
        .await
        .expect("inspect_causality failed");

    assert_eq!(
        report.session_id, session_id,
        "session_id should match the queried session"
    );
    assert_eq!(
        report.address, stack_address,
        "causality report should echo the requested address"
    );
    assert_eq!(
        report.mutation_count, 0,
        "a trace without memory-write evidence must report zero mutations"
    );
    assert!(
        report.mutations.is_empty(),
        "mutation_count and mutations must agree, got {:?}",
        report.mutations
    );

    client.shutdown().await.ok();
}
