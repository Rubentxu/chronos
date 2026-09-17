//! REC-C2.2.5 — pre-close characterization: UAT-C2-01/02/03.
//!
//! ## Provenance note (read this before judging the names)
//!
//! `proposal.md` and `tasks.md` name "UAT-C2-01/02/03 pre-close
//! characterization" as this cycle's final step, but nothing in the repository
//! defines what those three UATs are. Rather than back-fill three identifiers
//! that were never specified and then "pass" them, they are DEFINED here once,
//! from the laws this cycle closes on, and then executed against the real MCP
//! server. The definitions are the deliverable; the assertions are the check.
//!
//! ```text
//! UAT-C2-01  probe_drain is not an authority and does not create evidence
//! UAT-C2-02  probe_stop / session_snapshot report the log and its completeness
//! UAT-C2-03  durable evidence exceeds the ring, so the ring is not the source
//! ```
//!
//! Each UAT asserts a property that would FAIL under the pre-cycle
//! architecture, so these are falsifiable characterizations and not a summary
//! of green tests.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::client::types::{TripwireConditionType, TripwireCreateParams};
use chronos_sandbox::McpSession;
use std::collections::HashSet;
use std::time::Duration;

/// Far too small to hold any real capture, so "the ring" and "the evidence"
/// cannot be confused for one another.
const TINY_RING: usize = 4;

async fn start_probe_with_ring(client: &mut McpTestClient, ring: usize) -> Option<String> {
    let path = McpSession::fixture_path("test_busyloop")?;
    let session = client
        .probe_start_with_params(path.to_str()?, true, ring)
        .await
        .ok()?;
    tokio::time::sleep(Duration::from_secs(2)).await;
    Some(session)
}

async fn start_probe(client: &mut McpTestClient) -> Option<String> {
    start_probe_with_ring(client, TINY_RING).await
}

/// UAT-C2-01 — `probe_drain` is not an authority and does not create evidence.
///
/// The pre-cycle implementation drained a ring buffer and recomputed the
/// firing count with a live matcher on every call, so:
///
///   * a second drain saw a different (usually smaller) event set,
///   * the reported firing count moved with subscription state,
///   * `probe_drain` was the mechanism through which "firings" appeared.
///
/// Under the canonical route the firing count is persisted evidence, so
/// repeated reads of the same durable range MUST agree.
#[tokio::test]
async fn uat_c2_01_probe_drain_is_not_an_authority() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");
    // The subscription must exist BEFORE the capture starts: firings are
    // derived at the accepted-Raw seam with the subscriptions of that moment,
    // so a subscription created later cannot retroactively match accepted
    // evidence. (That ordering is itself the property, not a test artifact.)
    client
        .tripwire_create(TripwireCreateParams {
            condition: TripwireConditionType::EventType {
                event_types: vec!["syscall_enter".to_string()],
            },
            label: Some("uat-c2-01".to_string()),
        })
        .await
        .expect("UAT-C2-01: the matching subscription must exist before the capture starts");

    let Some(session) = start_probe_with_ring(&mut client, 50_000).await else {
        eprintln!("uat_c2: fixture unavailable, skipping");
        let _ = client.shutdown().await;
        return;
    };

    let first = client
        .probe_drain_with_evidence_cursor(&session, None)
        .await
        .expect("first drain");
    let cursor = first.evidence_cursor.clone().expect("cursor");
    let first_ids: HashSet<u64> = first.events.iter().map(|e| e.event_id).collect();
    let first_firings = first.tripwires_fired.unwrap_or(0);

    // Continue from the cursor: strictly forward, never a re-read.
    let second = client
        .probe_drain_with_evidence_cursor(&session, Some(cursor.as_str()))
        .await
        .expect("continuation drain");
    for ev in &second.events {
        assert!(
            !first_ids.contains(&ev.event_id),
            "UAT-C2-01: continuing from a cursor re-delivered event {}",
            ev.event_id
        );
    }
    assert!(
        !first.events.is_empty(),
        "UAT-C2-01: the fixture must produce events or this UAT proves nothing"
    );
    assert_ne!(
        second.evidence_cursor.as_deref(),
        Some(cursor.as_str()),
        "UAT-C2-01: an examined page must advance the cursor"
    );

    if first_firings == 0 {
        let raw = client
            .probe_drain_wire(&session, None)
            .await
            .expect("diagnostic re-read");
        let listed = client.tripwire_list().await;
        panic!(
            "UAT-C2-01: the EventType(SyscallEnter) tripwire must match the stream, otherwise the \
             invariance below is vacuous (raw event count {}, tripwire_list {:?}, drain {:?})",
            first.events.len(),
            listed,
            raw
        );
    }

    // Read the SAME durable range again. A drain is a READ: under the retired
    // destructive ring drain the first read had consumed the buffer, so this
    // would have come back missing most of what `first` saw.
    let reread = client
        .probe_drain_with_evidence_cursor(&session, None)
        .await
        .expect("re-read drain");
    let reread_ids: HashSet<u64> = reread.events.iter().map(|e| e.event_id).collect();
    assert!(
        first_ids.is_subset(&reread_ids),
        "UAT-C2-01: re-reading lost events the earlier read had already seen — a drain is not a \
         read if it consumes"
    );

    // Now the falsification of "probe_drain computes firings". Add a SECOND
    // subscription matching the same stream. Durable evidence can only gain
    // firings from events that arrive from now on; a live recomputation over
    // the returned range would double the historical prefix instead.
    client
        .tripwire_create(TripwireCreateParams {
            condition: TripwireConditionType::EventType {
                event_types: vec!["syscall_enter".to_string()],
            },
            label: Some("uat-c2-01-perturbation".to_string()),
        })
        .await
        .ok();

    let after = client
        .probe_drain_with_evidence_cursor(&session, None)
        .await
        .expect("post-perturbation drain");
    let after_firings = after.tripwires_fired.unwrap_or(0);
    let new_raw = (after.events.len() as i64) - (first.events.len() as i64);
    assert!(
        after_firings as i64 - first_firings as i64 <= 2 * new_raw.max(0),
        "UAT-C2-01: {} firings after {} for the same prefix, with a second subscription added and \
         only {new_raw} new source events. The historical prefix gained firings, so the count is \
         being recomputed from subscription state instead of read from the log",
        after_firings,
        first_firings
    );

    let _ = client.probe_stop(&session).await;
    let _ = client.shutdown().await;
}

/// UAT-C2-02 — `probe_stop` and `session_snapshot` report the log and its
/// completeness.
///
/// Pre-cycle, both read a bounded ring destructively: the total was capped by
/// the buffer, and a second snapshot found nothing left.
#[tokio::test]
async fn uat_c2_02_consumers_report_the_log_and_its_completeness() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");
    let Some(session) = start_probe(&mut client).await else {
        eprintln!("uat_c2: fixture unavailable, skipping");
        let _ = client.shutdown().await;
        return;
    };

    // Snapshot twice while the probe runs: a read must not consume.
    let snap1 = client
        .call_tool(
            "session_snapshot",
            serde_json::json!({ "session_id": session }),
        )
        .await
        .expect("snapshot 1");
    let snap2 = client
        .call_tool(
            "session_snapshot",
            serde_json::json!({ "session_id": session }),
        )
        .await
        .expect("snapshot 2");
    let n1 = snap1
        .get("events_indexed")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let n2 = snap2
        .get("events_indexed")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    assert!(n1 > 0, "UAT-C2-02: snapshot indexed nothing (raw: {snap1})");
    assert!(
        n2 >= n1,
        "UAT-C2-02: the second snapshot indexed {n2} after {n1}; a snapshot must not consume"
    );
    assert_eq!(
        snap2
            .get("completeness")
            .and_then(|c| c.get("scope"))
            .and_then(|v| v.as_str()),
        Some("examined_range"),
        "UAT-C2-02: completeness must describe the examined range"
    );

    // Stop: the total is a fact about the log, so a tiny ring cannot cap it.
    let stopped = client
        .call_tool("probe_stop", serde_json::json!({ "session_id": session }))
        .await
        .expect("probe_stop");
    let total = stopped
        .get("total_events")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let examined = stopped
        .get("examined_records")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    assert!(
        total > TINY_RING as u64,
        "UAT-C2-02: probe_stop reported {total} with a ring of {TINY_RING}: the ring is still the \
         source (raw: {stopped})"
    );
    assert!(
        examined >= total,
        "UAT-C2-02: examining {examined} records cannot yield {total} Raw events"
    );

    let _ = client.shutdown().await;
}

/// UAT-C2-03 — durable evidence exceeds the ring.
///
/// This is the premise the whole cycle rests on: if the ring held everything,
/// reading the log would just be a different spelling of the same thing. With
/// `bus_capacity = 4` and a real capture, the log must hold vastly more.
#[tokio::test]
async fn uat_c2_03_durable_evidence_exceeds_the_ring() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");
    let Some(session) = start_probe(&mut client).await else {
        eprintln!("uat_c2: fixture unavailable, skipping");
        let _ = client.shutdown().await;
        return;
    };

    let page = client
        .call_tool(
            "probe_drain_log",
            serde_json::json!({ "session_id": session, "limit": 512 }),
        )
        .await
        .expect("probe_drain_log");
    let durable = page
        .get("events")
        .and_then(|v| v.as_array())
        .map(|a| a.len() as u64)
        .unwrap_or(0);
    assert!(
        durable > TINY_RING as u64,
        "UAT-C2-03: the durable log returned {durable} records with a ring of {TINY_RING}; the \
         log must hold what the ring dropped (raw: {page})"
    );
    // The log path must not be silently skipping records it cannot decode.
    assert_eq!(
        page.get("unparseable_payload_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        0,
        "UAT-C2-03: an undecodable record means another producer or schema drift wrote this log"
    );

    let _ = client.probe_stop(&session).await;
    let _ = client.shutdown().await;
}
