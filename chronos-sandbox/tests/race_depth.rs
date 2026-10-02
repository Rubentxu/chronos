//! Race Detection Depth tests — verify race detection works at various thresholds.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::McpSession;
use std::time::Duration;

/// RD1: `race_detect` at `threshold_ns=1` over the 3-thread fixture.
///
/// The previous version of this test called `execution_query` as a raw
/// JSON-RPC *method*; since C5.3.2 the server only exposes it as an MCP
/// tool, so that call always answered `-32601 method not found`. The old
/// body matched `Ok` and `Err` and printed either way, so the test could
/// never fail — it passed while exercising nothing. This version calls the
/// tool the way the protocol requires (`tools/call`) and asserts that the
/// caller-supplied threshold actually reaches the detector and comes back
/// on the wire.
///
/// Emptiness caveat, measured on this fixture: `total_writes == 0` and
/// `access_count == 0` over ~207 captured events. Note the unit of the
/// `total_writes` key despite its name: it counts distinct addresses the
/// detector checked, not write events. The ptrace capture of a C fixture
/// yields no `VariableWrite`/`MemoryWrite` events at all, so the detector has
/// no address to pair and the empty verdict is structural —
/// it is not evidence that the detector examined and cleared the threads.
#[tokio::test]
async fn test_debug_detect_races_threshold_1ns() {
    let fixture = McpSession::fixture_path("test_threads")
        .expect("test_threads fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(3)).await;
    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");

    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Anti-vacuity guard: the session must really have captured traffic,
    // otherwise "no races" would also be what an empty session reports.
    assert!(
        stop.total_events >= 100,
        "expected the 3-thread fixture to produce a real event stream, got {}",
        stop.total_events
    );

    // REC-C5-C5.2: migrated to the v2 `execution_query` dispatcher with
    // `kind=race_detect` (same threshold_ns field).
    let params = serde_json::json!({
        "session_id": session_id,
        "kind": "race_detect",
        "threshold_ns": 1
    });

    let json = client.call_tool("execution_query", params).await.expect(
        "execution_query must be invoked as an MCP tool (tools/call); \
                 a raw JSON-RPC method call answers -32601 method not found",
    );

    // Envelope identity: the discriminator and the echoed session.
    assert_eq!(
        json.get("kind").and_then(|v| v.as_str()),
        Some("race_detect"),
        "response must carry the kind discriminator, got {}",
        json
    );
    assert_eq!(
        json.get("session_id").and_then(|v| v.as_str()),
        Some(session_id.as_str()),
        "response must echo the queried session_id, got {}",
        json
    );

    // The point of this test: the 1ns threshold is plumbed through, not
    // silently replaced by the 100ns default.
    assert_eq!(
        json.get("threshold_ns").and_then(|v| v.as_u64()),
        Some(1),
        "server must apply the requested threshold_ns=1, got {}",
        json
    );

    // Wire self-consistency: the counters agree with the arrays they
    // summarise, so an empty verdict is a real measurement, not a stub.
    let accesses = json
        .get("accesses")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("race_detect response has no `accesses` array: {}", json));
    let access_count = json
        .get("access_count")
        .and_then(|v| v.as_u64())
        .unwrap_or_else(|| panic!("race_detect response has no `access_count`: {}", json));
    let pairs = json
        .get("suspicious_pairs")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("race_detect response has no `suspicious_pairs`: {}", json));

    assert_eq!(
        access_count as usize,
        accesses.len(),
        "access_count must match the accesses array, got {}",
        json
    );
    assert_eq!(
        pairs.len(),
        accesses.len(),
        "suspicious_pairs must have one entry per reported access, got {}",
        json
    );

    // Measured: this capture carries no write events, so the detector had
    // zero addresses to compare. Pinned explicitly so the empty result is
    // never read as "the 3 threads were cleared".
    assert_eq!(
        json.get("total_writes").and_then(|v| v.as_u64()),
        Some(0),
        "C captures carry no VariableWrite/MemoryWrite events, so the detector had 0 addresses to compare; the `total_writes` key counts distinct addresses, not write events (both are 0 here only because the capture has no writes at all), got {}",
        json
    );
    assert_eq!(
        access_count, 0,
        "with zero write events no suspicious pair can be formed, got {}",
        json
    );
    assert!(
        json.get("summary")
            .and_then(|v| v.as_str())
            .map(|s| s.contains("No suspicious concurrent accesses"))
            .unwrap_or(false),
        "summary must state that nothing was found, got {}",
        json
    );

    client.shutdown().await.ok();
}

/// RD2: `race_detect` at `threshold_ns=1_000_000` (1ms) over the same
/// 3-thread fixture.
///
/// Same contract as RD1, with the 1ms threshold: the discriminating
/// assert is that the wire echoes 1_000_000 (the 100ns default is *not*
/// applied). Together RD1 and RD2 fail if the threshold ever stops being
/// forwarded. Emptiness caveat and measurements as in RD1.
#[tokio::test]
async fn test_debug_detect_races_threshold_1ms() {
    let fixture = McpSession::fixture_path("test_threads")
        .expect("test_threads fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(3)).await;
    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");

    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    assert!(
        stop.total_events >= 100,
        "expected the 3-thread fixture to produce a real event stream, got {}",
        stop.total_events
    );

    // REC-C5-C5.2: migrated to the v2 `execution_query` dispatcher with
    // `kind=race_detect` (1ms threshold).
    let params = serde_json::json!({
        "session_id": session_id,
        "kind": "race_detect",
        "threshold_ns": 1_000_000u64
    });

    let json = client.call_tool("execution_query", params).await.expect(
        "execution_query must be invoked as an MCP tool (tools/call); \
                 a raw JSON-RPC method call answers -32601 method not found",
    );

    assert_eq!(
        json.get("kind").and_then(|v| v.as_str()),
        Some("race_detect"),
        "response must carry the kind discriminator, got {}",
        json
    );
    assert_eq!(
        json.get("session_id").and_then(|v| v.as_str()),
        Some(session_id.as_str()),
        "response must echo the queried session_id, got {}",
        json
    );
    assert_eq!(
        json.get("threshold_ns").and_then(|v| v.as_u64()),
        Some(1_000_000),
        "server must apply the requested threshold_ns=1_000_000, got {}",
        json
    );

    let accesses = json
        .get("accesses")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("race_detect response has no `accesses` array: {}", json));
    let access_count = json
        .get("access_count")
        .and_then(|v| v.as_u64())
        .unwrap_or_else(|| panic!("race_detect response has no `access_count`: {}", json));
    let pairs = json
        .get("suspicious_pairs")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("race_detect response has no `suspicious_pairs`: {}", json));

    assert_eq!(
        access_count as usize,
        accesses.len(),
        "access_count must match the accesses array, got {}",
        json
    );
    assert_eq!(
        pairs.len(),
        accesses.len(),
        "suspicious_pairs must have one entry per reported access, got {}",
        json
    );
    assert_eq!(
        json.get("total_writes").and_then(|v| v.as_u64()),
        Some(0),
        "C captures carry no VariableWrite/MemoryWrite events, so the detector had 0 addresses to compare; the `total_writes` key counts distinct addresses, not write events (both are 0 here only because the capture has no writes at all), got {}",
        json
    );
    assert_eq!(
        access_count, 0,
        "a 1ms window over a capture with zero write events cannot form a pair, got {}",
        json
    );

    client.shutdown().await.ok();
}

/// RD3: 10 threads → still zero races, and the emptiness is structural.
///
/// The old body matched `Ok`/`Err` and printed either way, so it could not
/// fail. It now decides the outcome: `race_detect` on a many-threaded
/// session must **succeed** and report **no** suspicious accesses.
///
/// Why "no races" is a safe assert here and not a scheduler coin-flip,
/// verified by reading the fixture (`programs/c/test_many_threads.c`):
/// each worker accumulates into its *own* stack-local `volatile long sum`
/// and only reads `ids[i]`, which `main` wrote before any thread started.
/// No two threads ever write the same address, so the fixture itself has
/// no race. On top of that, measured on this capture: `total_writes == 0`
/// (distinct addresses checked, not write events) and `access_count == 0`
/// over 374-376 events — the ptrace capture of a C
/// fixture emits no `VariableWrite`/`MemoryWrite` events, so the detector
/// has no address to pair. Both reasons are properties of the fixture and
/// the capture, not of thread interleaving: the assert is deterministic.
#[tokio::test]
async fn test_debug_detect_races_many_threads() {
    let fixture = McpSession::fixture_path("test_many_threads")
        .expect("test_many_threads fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(3)).await;
    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");

    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Anti-vacuity guard: without a real event stream, "no races" would
    // also be what an empty or dead session reports. Measured 374-376.
    assert!(
        stop.total_events >= 100,
        "expected the 10-thread fixture to produce a real event stream, got {}",
        stop.total_events
    );

    // Detect races with the wrapper's default threshold. The old test
    // accepted `Err`; the contract is that this query succeeds.
    let races = client
        .debug_detect_races(&session_id)
        .await
        .expect("race_detect must succeed on a many-threaded session");

    println!(
        "✓ debug_detect_races on many threads: {} races",
        races.len()
    );
    assert!(
        races.is_empty(),
        "no worker in the fixture writes a shared address, so no race can be reported (got {:?})",
        races
    );

    // Cross-check the typed wrapper against the raw wire response at the
    // same threshold: both must agree that nothing was found.
    let json = client
        .call_tool(
            "execution_query",
            serde_json::json!({
                "session_id": session_id,
                "kind": "race_detect",
                "threshold_ns": 100u64,
            }),
        )
        .await
        .expect("execution_query must be invoked as an MCP tool (tools/call)");

    assert_eq!(
        json.get("kind").and_then(|v| v.as_str()),
        Some("race_detect"),
        "response must carry the kind discriminator, got {}",
        json
    );
    assert_eq!(
        json.get("threshold_ns").and_then(|v| v.as_u64()),
        Some(100),
        "the wrapper's default threshold is 100ns and must be applied, got {}",
        json
    );
    assert_eq!(
        json.get("access_count").and_then(|v| v.as_u64()),
        Some(0),
        "wrapper and wire must agree that no access was reported, got {}",
        json
    );
    assert_eq!(
        json.get("total_writes").and_then(|v| v.as_u64()),
        Some(0),
        "C captures carry no VariableWrite/MemoryWrite events, so the detector had 0 addresses to compare; the `total_writes` key counts distinct addresses, not write events (both are 0 here only because the capture has no writes at all), got {}",
        json
    );
    assert!(
        json.get("summary")
            .and_then(|v| v.as_str())
            .map(|s| s.contains("No suspicious concurrent accesses"))
            .unwrap_or(false),
        "summary must state that nothing was found, got {}",
        json
    );

    client.shutdown().await.ok();
}

/// RD4: test_add (single-threaded) → `race_detect` succeeds with no races.
///
/// This test already decided its outcome (`expect` on the query plus an
/// emptiness assert), so it is kept as-is apart from one honest caveat
/// added after measuring: the empty result is **not** mainly a fact about
/// single-threadedness. Like RD1-RD3, this capture carries no
/// `VariableWrite`/`MemoryWrite` events (measured `total_writes == 0`,
/// which counts distinct addresses rather than write events, over 128
/// events), so the detector has no address to pair. The green
/// assert therefore proves the query works end to end and reports
/// nothing; it does not prove the detector weighed concurrent writes and
/// found none.
#[tokio::test]
async fn test_debug_detect_races_single_thread_no_races() {
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

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
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Detect races using the client wrapper (default threshold=100)
    let races = client
        .debug_detect_races(&session_id)
        .await
        .expect("debug_detect_races failed");

    // Assert: valid response, potential_races is empty (see the caveat in
    // the doc comment: the emptiness is structural, not a verdict).
    println!(
        "✓ debug_detect_races on single-threaded: {} races",
        races.len()
    );
    assert!(
        races.is_empty(),
        "single-threaded program with no write events should have no races"
    );
    println!("  Response has valid structure with empty races");

    client.shutdown().await.ok();
}
