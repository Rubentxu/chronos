//! Diff tools tests — verify state_diff and debug_diff work correctly.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::McpSession;
use std::time::Duration;

#[tokio::test]
async fn test_state_diff_after_probe_stop() {
    // state_diff compares the program state at two timestamps; after
    // probe_stop the session is queryable and the first/last drained event
    // timestamps form a valid pair.
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

    let drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");

    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");

    assert!(stop.total_events > 0, "Should have captured events");

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Use timestamps from the drained events. This capture always produces a
    // large event buffer (measured: 128 events for test_add), so the real
    // two-timestamp path is asserted directly instead of falling back to
    // arbitrary timestamps that would prove nothing.
    assert!(
        drained.len() >= 2,
        "need at least two drained events to derive two timestamps, got {}",
        drained.len()
    );

    let ts_a = drained[0].timestamp_ns;
    let ts_b = drained[drained.len() - 1].timestamp_ns;
    assert!(
        ts_a < ts_b,
        "drained event timestamps must be ordered, got ts_a={} ts_b={}",
        ts_a,
        ts_b
    );

    let result = client
        .state_diff(&session_id, ts_a, ts_b)
        .await
        .expect("state_diff should succeed for a captured session");

    // The response must be bound to the two timestamps we asked about.
    assert_eq!(
        result.timestamp_a, ts_a,
        "state_diff echoed a different timestamp_a than requested"
    );
    assert_eq!(
        result.timestamp_b, ts_b,
        "state_diff echoed a different timestamp_b than requested"
    );

    // Measured behaviour: this trace carries no register evidence (every
    // drained event is an Unresolved SyscallEnter/SyscallExit), and the
    // service documents state_diff as returning an empty diff when neither
    // timestamp has register evidence. So no changes is the expected result
    // here — a non-empty list would mean unexpected register data.
    assert!(
        result.changes.is_empty(),
        "expected no register changes without register evidence, got {:?}",
        result.changes
    );

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_state_diff_same_timestamp() {
    // state_diff with same timestamps should return empty changes
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

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Use same timestamp for both - should have no changes
    let result = client
        .state_diff(&session_id, 1000, 1000)
        .await
        .expect("state_diff should succeed with identical timestamps");

    assert_eq!(
        result.timestamp_a, 1000,
        "state_diff echoed a different timestamp_a than requested"
    );
    assert_eq!(
        result.timestamp_b, 1000,
        "state_diff echoed a different timestamp_b than requested"
    );

    // Same timestamp = no changes expected
    assert!(
        result.changes.is_empty(),
        "identical timestamps must yield no changes, got {:?}",
        result.changes
    );

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_debug_diff_after_probe_stop() {
    // debug_diff requires two event IDs to compare
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

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Diff events 0 and 1. The call must succeed: the tool used to fail on
    // every invocation with `-32602 "unknown variant 'state_diff'"`, and these
    // tests hid it by accepting both branches.
    let result = client
        .debug_diff(&session_id, 0, 1)
        .await
        .expect("debug_diff should succeed for two event ids of a captured session");

    // The snapshot must be anchored to the ids that were asked for.
    assert_eq!(
        result.event_id_a, 0,
        "debug_diff echoed a different event_id_a"
    );
    assert_eq!(
        result.event_id_b, 1,
        "debug_diff echoed a different event_id_b"
    );

    // No variable or register evidence, and the fixture is why: the C
    // capture path yields only Unresolved SyscallEnter/SyscallExit events, with
    // no register snapshots. An empty result is the honest expectation here,
    // not a tolerated failure.
    assert!(
        result.variables_added.is_empty(),
        "no variables can appear out of nowhere without variable evidence, got {:?}",
        result.variables_added
    );
    assert!(
        result.variables_removed.is_empty(),
        "no variables can disappear without variable evidence, got {:?}",
        result.variables_removed
    );
    assert!(
        result.variables_changed.is_empty(),
        "no variables can change value without variable evidence, got {:?}",
        result.variables_changed
    );
    assert!(
        result.registers_changed.is_empty(),
        "expected no register changes without register evidence, got {:?}",
        result.registers_changed
    );

    // This is the assertion that carries the weight. A tool that answered every
    // pair with an empty zero-delta snapshot would pass all of the checks
    // above, and would have passed this test when it accepted both branches.
    // Two distinct real events must be measurably apart in time.
    assert!(
        result.timestamp_delta_ns > 0,
        "two distinct captured events must be more than zero nanoseconds apart, got 0"
    );

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_debug_diff_out_of_range_events() {
    // debug_diff with non-existent event IDs
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

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Event ids that were never captured. The server does not reject them: it
    // answers with a zero-delta snapshot, because an event that does not exist
    // cannot have differed from anything. Whether rejecting would be nicer is a
    // product question, but the behaviour under test is unambiguous, so it gets
    // asserted rather than tolerated.
    let result = client
        .debug_diff(&session_id, 999999, 999998)
        .await
        .expect("debug_diff answers out-of-range event ids with a zero-delta snapshot");

    assert_eq!(
        result.event_id_a, 999999,
        "debug_diff echoed a different event_id_a"
    );
    assert_eq!(
        result.event_id_b, 999998,
        "debug_diff echoed a different event_id_b"
    );

    assert!(
        result.variables_added.is_empty()
            && result.variables_removed.is_empty()
            && result.variables_changed.is_empty()
            && result.registers_changed.is_empty(),
        "events that do not exist cannot yield changes, got {:?}",
        result
    );

    // The discriminating half. Compare with test_debug_diff_after_probe_stop,
    // which asserts a strictly positive delta for two real events: the tool is
    // genuinely reading the event stream, not answering every pair the same way.
    assert_eq!(
        result.timestamp_delta_ns, 0,
        "event ids that were never captured have no time between them"
    );

    client.shutdown().await.ok();
}
