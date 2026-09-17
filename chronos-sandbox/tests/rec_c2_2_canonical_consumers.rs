//! REC-C2.2.3 — `probe_stop` and `session_snapshot` read durable evidence.
//!
//! Both used to call `ProbeBackend::drain_raw_events()`, a **destructive**
//! `EventBus` ring snapshot with three consequences that this suite makes
//! falsifiable through the real MCP server:
//!
//! * the ring is bounded, so `total_events` described whatever survived in a
//!   buffer, not what was captured,
//! * the read consumed, so a second consumer saw nothing,
//! * with the buffer full, `total_events` silently capped.
//!
//! With the log as the authority, a tiny ring must not be able to shrink the
//! reported capture, and a repeat must not come back empty.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::McpSession;
use std::time::Duration;

/// `probe_stop` must still report the real total, because the total is a
/// fact about the log (REC-C2.3: with no live ring, "ring capacity" is gone).
#[tokio::test]
async fn probe_stop_total_is_the_log_not_the_ring() {
    const RING_CAPACITY: usize = 4;

    let Some(fixture) = McpSession::fixture_path("test_busyloop") else {
        eprintln!("canonical_consumers: fixture unavailable, skipping");
        return;
    };
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start_with_params(fixture.to_str().unwrap(), true)
        .await
        .expect("probe_start failed");

    // Captures far more than RING_CAPACITY syscall pairs.
    tokio::time::sleep(Duration::from_secs(2)).await;

    let stopped = client
        .call_tool(
            "probe_stop",
            serde_json::json!({ "session_id": session_id }),
        )
        .await
        .expect("probe_stop failed");

    let total = stopped
        .get("total_events")
        .and_then(|v| v.as_u64())
        .expect("probe_stop must report total_events");
    assert!(
        total > RING_CAPACITY as u64,
        "probe_stop reported {total} events; expected the log-backed total to be larger than {RING_CAPACITY} \
         (REC-C2.3: with no live ring there is no smaller authority the total could collapse to). Raw: {stopped}"
    );

    // A gap-bearing capture must not be presented as a clean total.
    let completeness = stopped
        .get("completeness")
        .expect("probe_stop must report completeness of the evidence it read");
    assert_eq!(
        completeness.get("scope").and_then(|v| v.as_str()),
        Some("examined_range"),
        "completeness must describe the examined range"
    );
    assert!(
        stopped.get("examined_records").is_some(),
        "probe_stop must report the cost of the read"
    );

    let _ = client.shutdown().await;
}

/// `session_snapshot` is a read, so repeating it must not come back empty.
/// The retired path drained the ring, so the second snapshot indexed nothing.
#[tokio::test]
async fn session_snapshot_is_repeatable_and_non_destructive() {
    let Some(fixture) = McpSession::fixture_path("test_busyloop") else {
        eprintln!("canonical_consumers: fixture unavailable, skipping");
        return;
    };
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");
    tokio::time::sleep(Duration::from_secs(2)).await;

    let first = client
        .call_tool(
            "session_snapshot",
            serde_json::json!({ "session_id": session_id }),
        )
        .await
        .expect("first session_snapshot failed");
    let first_indexed = first
        .get("events_indexed")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    assert!(
        first_indexed > 0,
        "first snapshot indexed nothing (raw: {first})"
    );

    let second = client
        .call_tool(
            "session_snapshot",
            serde_json::json!({ "session_id": session_id }),
        )
        .await
        .expect("second session_snapshot failed");
    let second_indexed = second
        .get("events_indexed")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    assert!(
        second_indexed >= first_indexed,
        "second snapshot indexed {second_indexed} after {first_indexed}: a snapshot must not \
         consume the evidence it reads (raw: {second})"
    );
    assert!(
        second.get("completeness").is_some(),
        "session_snapshot must report completeness"
    );

    let _ = client.probe_stop(&session_id).await;
    let _ = client.shutdown().await;
}
