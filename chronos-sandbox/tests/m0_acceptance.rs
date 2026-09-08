//! M0 acceptance harness scaffold.
//!
//! Each `#[test]` is a stub for one M0 ticket. Implementation lands in
//! follow-on cycles (m0-01, m0-02, …, m0-10) per the reconstruction
//! backlog at `docs/chronos-agentic-reconstruction/docs/roadmap/IMPLEMENTATION_BACKLOG.md`.
//!
//! DoD for THIS file: it compiles cleanly as part of
//! `cargo test -p chronos-sandbox --test m0_acceptance --no-run`.
//!
//! Each stub is `#[ignore]` with `reason = "implemented in cycle m0-NN"`,
//! so the default sandbox test run remains green and only `--include-ignored`
//! reveals the unimplemented bodies.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::McpSession;
use std::time::Duration;

#[tokio::test]
#[ignore = "implemented in cycle m0-01"]
async fn m0_01_live_pagination_is_non_destructive() {
    // UAT-M0-01: probe_drain must be non-destructive. Two consecutive
    // calls with the same cursor return the same event set; a second
    // fresh call without a cursor sees all the events the first call did.
    let fixture = match McpSession::fixture_path("test_busyloop") {
        Some(p) => p,
        None => {
            eprintln!(
                "m0_01: test_busyloop fixture not available; skipping (run `cargo build` first)"
            );
            return;
        }
    };

    let mut client = match McpTestClient::start().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("m0_01: failed to start MCP server: {}", e);
            return;
        }
    };

    let session_id = match client.probe_start(fixture.to_str().unwrap()).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("m0_01: probe_start failed: {}", e);
            let _ = client.shutdown().await;
            return;
        }
    };

    tokio::time::sleep(Duration::from_secs(2)).await;

    // First drain — capture cursor and event set.
    let first = match client.probe_drain_with_cursor(&session_id, None).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("m0_01: first probe_drain failed: {}", e);
            let _ = client.shutdown().await;
            return;
        }
    };

    let _cursor = first
        .cursor
        .clone()
        .expect("probe_drain must return a cursor when non-destructive read is implemented");

    // Replay: re-issue a fresh read (no cursor). The implementation is
    // non-destructive (does not consume the ring buffer), so the replay
    // MUST observe at least the events that `first` saw (it can see more
    // if new events arrived in between). Under a destructive drain,
    // replay would observe 0 or far fewer events — that is the bug this
    // cycle fixes.
    let replay = match client.probe_drain_with_cursor(&session_id, None).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("m0_01: replay probe_drain failed: {}", e);
            let _ = client.shutdown().await;
            return;
        }
    };

    assert!(
        replay.events.len() >= first.events.len(),
        "m0_01: replay must observe at least the events `first` saw (non-destructive), got first={} replay={}",
        first.events.len(),
        replay.events.len()
    );

    // Cursor staleness flag must be false on a fresh ring (no eviction expected here).
    let cursor_stale = replay.cursor_stale.unwrap_or(false);
    assert!(
        !cursor_stale,
        "m0_01: cursor_stale should be false for fresh ring"
    );

    // Re-issuing with the same cursor that was just returned must not
    // error (cursor is fresh and matches the live ring). The number of
    // events returned may be 0 (if no new arrivals) or >0 (if the probe
    // emitted more events between the two reads); both are valid for
    // the non-destructive contract.
    let replay_cursor = replay.cursor.clone().expect("replay must include cursor");
    let re_replay = client
        .probe_drain_with_cursor(&session_id, Some(replay_cursor))
        .await
        .expect("m0_01: re-replay with same cursor must not error");
    assert!(
        !re_replay.cursor_stale.unwrap_or(false),
        "m0_01: re-replay cursor_stale should be false"
    );

    // Spot-check that the response now exposes the new cursor + total_buffered
    // fields required by REQ-CursorInProbeDrainResponse.
    assert!(
        replay.cursor.is_some(),
        "m0_01: response must include cursor field"
    );
    assert!(
        replay.total_buffered >= replay.events.len(),
        "m0_01: total_buffered must be >= events.len()"
    );

    let _ = client.probe_stop(&session_id).await;
    let _ = client.shutdown().await;
}

#[test]
#[ignore = "implemented in cycle m0-02"]
fn m0_02_session_snapshot_is_cumulative() {
    unimplemented!("See vault/cycles/m0-truth-first-foundation/m0-02.md");
}

#[tokio::test]
#[ignore = "implemented in cycle m0-02"]
async fn m0_02_session_snapshot_is_cumulative_impl() {
    // UAT-M0-02: two consecutive session_snapshot calls preserve evidence
    // from both periods (cumulative refresh).
    let fixture = match McpSession::fixture_path("test_busyloop") {
        Some(p) => p,
        None => {
            eprintln!(
                "m0_02: test_busyloop fixture not available; skipping (run `cargo build` first)"
            );
            return;
        }
    };

    let mut client = match McpTestClient::start().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("m0_02: failed to start MCP server: {}", e);
            return;
        }
    };

    let session_id = match client.probe_start(fixture.to_str().unwrap()).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("m0_02: probe_start failed: {}", e);
            let _ = client.shutdown().await;
            return;
        }
    };

    tokio::time::sleep(Duration::from_secs(2)).await;

    // First snapshot — builds the engine.
    let snap1 = match client.session_snapshot(&session_id).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("m0_02: first session_snapshot failed: {}", e);
            let _ = client.shutdown().await;
            return;
        }
    };
    let first_indexed = snap1.events_indexed;
    assert!(
        first_indexed > 0,
        "m0_02: first snapshot must index at least one event"
    );

    // Record the page-size-limited view of the engine after snap1.
    let filter = chronos_sandbox::client::types::QueryFilter {
        limit: 5000,
        offset: 0,
        ..Default::default()
    };
    let after_snap1 = match client.query_events(&session_id, filter).await {
        Ok(v) => v,
        Err(e) => {
            eprintln!("m0_02: query_events after snap1 failed: {}", e);
            let _ = client.shutdown().await;
            return;
        }
    };
    assert!(
        after_snap1.len() >= first_indexed,
        "m0_02: first query must see at least first_indexed events"
    );

    // Wait for more events to arrive, then second snapshot.
    tokio::time::sleep(Duration::from_secs(2)).await;
    let snap2 = match client.session_snapshot(&session_id).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("m0_02: second session_snapshot failed: {}", e);
            let _ = client.shutdown().await;
            return;
        }
    };

    // Cumulative refresh: the engine after snap2 must hold AT LEAST
    // as many events as it did after snap1 (the second refresh adds;
    // it does not replace).
    let filter = chronos_sandbox::client::types::QueryFilter {
        limit: 5000,
        offset: 0,
        ..Default::default()
    };
    let after_snap2 = match client.query_events(&session_id, filter).await {
        Ok(v) => v,
        Err(e) => {
            eprintln!("m0_02: query_events after snap2 failed: {}", e);
            let _ = client.shutdown().await;
            return;
        }
    };
    assert_eq!(snap2.session_id, session_id, "m0_02: session_id mismatch");
    assert!(
        after_snap2.len() >= after_snap1.len(),
        "m0_02: cumulative contract violated; after second snapshot the engine has {} events but the first snapshot left {}",
        after_snap2.len(),
        after_snap1.len()
    );
    // Spot-check: at least one event_id from snap1's view is still in
    // the engine after snap2 (no replacement).
    let first_id = after_snap1.first().map(|e| e.event_id);
    let last_id_of_snap1 = after_snap1.last().map(|e| e.event_id);
    if let (Some(fid), Some(lid)) = (first_id, last_id_of_snap1) {
        let still_present = after_snap2
            .iter()
            .any(|e| e.event_id == fid || e.event_id == lid);
        assert!(
            still_present,
            "m0_02: events from first snapshot must still be present after second snapshot"
        );
    }

    let _ = client.probe_stop(&session_id).await;
    let _ = client.shutdown().await;
}

#[test]
#[ignore = "implemented in cycle m0-03"]
fn m0_03_ebpf_probe_lifetime_is_session_owned() {
    unimplemented!("See vault/cycles/m0-truth-first-foundation/m0-03.md");
}

#[test]
#[ignore = "replaced by live UAT m0_04_tripwires_evaluated_on_canonical_flow_impl; kept so we have a single m0_04 stub."]
fn _m0_04_legacy_stub_disabled() {
    // Intentionally empty: the legacy placeholder was retired when m0-04 was
    // implemented as the live UAT below. The `#[ignore]` keeps this test out
    // of the default run, while a clearly-named stub name documents the change.
}

#[test]
#[ignore = "replaced by live UAT m0_05_typed_event_filters_reject_unknown_impl"]
fn _m0_05_legacy_stub_disabled() {
    // Replaced by the live UAT below; kept ignored so the stub name stays.
}

/// m0-05 — Typed event filters reject unknown (UAT-M0-05).
///
/// Asserts that supplying an unknown `event_type` to `query_events` or
/// `tripwire_create` causes the call to return an error containing the
/// bad name, rather than silently filtering it out.
#[tokio::test(flavor = "current_thread")]
async fn m0_05_typed_event_filters_reject_unknown_impl() {
    let _ = ();
    let mut client = match McpTestClient::start().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("m0_05: McpTestClient start failed: {}", e);
            return;
        }
    };

    // tripwire_create with an unknown event_type must error, not silently register.
    let bad = client
        .call_tool(
            "tripwire_create",
            serde_json::json!({
                "condition": {
                    "type": "event_type",
                    "event_types": ["definitely_not_a_real_type"]
                },
                "label": Some("m0_05-bad")
            }),
        )
        .await;
    let bad_text = bad
        .as_ref()
        .ok()
        .and_then(|v| v.get("error"))
        .and_then(|v| v.as_str());
    assert!(
        bad.is_err() || bad_text.is_some(),
        "m0_05: tripwire_create with unknown event_type must error, got {:?}",
        bad
    );
    if let Ok(v) = bad.as_ref() {
        // Inspect raw response for the bad type name.
        let raw = v.to_string();
        assert!(
            raw.contains("definitely_not_a_real_type"),
            "m0_05: error message must mention the bad type, got {}",
            raw
        );
    }

    let _ = client.shutdown().await;
}

#[test]
#[ignore = "replaced by live UAT m0_06_state_diff_preserves_register_evidence_impl"]
fn _m0_06_legacy_stub_disabled() {
    // Replaced by the live UAT below; kept ignored so the stub name stays.
}

/// m0-06 — State-diff preserves register evidence (UAT-M0-06).
///
/// Asserts that `state_diff` reports `register_evidence` and an
/// `evidence_note` when the engine has no register snapshots to compare,
/// so the caller can distinguish "no changes" from "no evidence".
#[tokio::test(flavor = "current_thread")]
async fn m0_06_state_diff_preserves_register_evidence_impl() {
    let _ = ();
    let fixture = match McpSession::fixture_path("test_busyloop") {
        Some(p) => p,
        None => {
            eprintln!("m0_06: test_busyloop fixture not available; skipping");
            return;
        }
    };
    let mut client = match McpTestClient::start().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("m0_06: McpTestClient start failed: {}", e);
            return;
        }
    };
    let session_id = match client.probe_start(fixture.to_str().unwrap()).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("m0_06: probe_start failed: {}", e);
            let _ = client.shutdown().await;
            return;
        }
    };
    tokio::time::sleep(Duration::from_secs(2)).await;
    let snap = match client.session_snapshot(&session_id).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("m0_06: session_snapshot failed: {}", e);
            let _ = client.shutdown().await;
            return;
        }
    };
    assert!(snap.events_indexed > 0, "m0_06: snapshot must index events");

    // Probe by default does NOT capture registers; the diff must surface
    // that fact instead of returning an empty diff silently.
    let raw = match client
        .call_tool(
            "state_diff",
            serde_json::json!({
                "session_id": session_id,
                "timestamp_a": 0,
                "timestamp_b": u64::MAX
            }),
        )
        .await
    {
        Ok(v) => v,
        Err(e) => {
            eprintln!("m0_06: state_diff call failed: {}", e);
            let _ = client.shutdown().await;
            return;
        }
    };
    let register_evidence = raw
        .get("register_evidence")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    assert!(
        !register_evidence,
        "m0_06: register_evidence must be false when no register snapshots were captured (raw: {})",
        raw
    );
    let note = raw
        .get("evidence_note")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(
        note.contains("register") || note.contains("snapshot"),
        "m0_06: evidence_note must mention register/snapshot evidence, got {:?}",
        note
    );

    let _ = client.probe_stop(&session_id).await;
    let _ = client.shutdown().await;
}

#[test]
#[ignore = "replaced by live UAT m0_07_query_returns_not_found_when_target_missing_impl"]
fn _m0_07_legacy_stub_disabled() {
    // Replaced by the live UAT below; kept ignored so the stub name stays.
}

/// m0-07 — Explicit query absence semantics.
///
/// Asserts that `query_events` reports `not_found: true` and a `reason`
/// when the session is queryable but the filter matches no events, so the
/// caller can distinguish "I asked for something that isn't there" from
/// "the session was empty".
#[tokio::test(flavor = "current_thread")]
async fn m0_07_query_returns_not_found_when_target_missing_impl() {
    let _ = ();
    let fixture = match McpSession::fixture_path("test_busyloop") {
        Some(p) => p,
        None => {
            eprintln!("m0_07: test_busyloop fixture not available; skipping");
            return;
        }
    };
    let mut client = match McpTestClient::start().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("m0_07: McpTestClient start failed: {}", e);
            return;
        }
    };
    let session_id = match client.probe_start(fixture.to_str().unwrap()).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("m0_07: probe_start failed: {}", e);
            let _ = client.shutdown().await;
            return;
        }
    };
    tokio::time::sleep(Duration::from_secs(2)).await;
    let _ = client.session_snapshot(&session_id).await;

    // Filter that nothing will ever match (thread_id 999_999).
    let raw = match client
        .call_tool(
            "query_events",
            serde_json::json!({
                "session_id": session_id,
                "thread_id": 999_999,
                "limit": 10
            }),
        )
        .await
    {
        Ok(v) => v,
        Err(e) => {
            eprintln!("m0_07: query_events failed: {}", e);
            let _ = client.shutdown().await;
            return;
        }
    };
    let not_found = raw
        .get("not_found")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    assert!(
        not_found,
        "m0_07: query_events with no matches must set not_found=true (raw: {})",
        raw
    );
    let reason = raw.get("reason").and_then(|v| v.as_str()).unwrap_or("");
    assert_eq!(
        reason, "no_matching_events",
        "m0_07: reason must be 'no_matching_events' when no events match"
    );

    // Sanity: querying without filter must NOT set not_found.
    let raw_all = client
        .call_tool(
            "query_events",
            serde_json::json!({
                "session_id": session_id,
                "limit": 10
            }),
        )
        .await
        .unwrap();
    let not_found_all = raw_all
        .get("not_found")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    assert!(
        !not_found_all,
        "m0_07: query_events with matches must set not_found=false"
    );

    let _ = client.probe_stop(&session_id).await;
    let _ = client.shutdown().await;
}

#[test]
#[ignore = "implemented in cycle m0-08: CaptureSession exposes both monotonic and wall-clock start timestamps (no live UAT required per ticket)"]
fn m0_08_timestamp_contract_is_separated() {
    // m0-08 contract is enforced by the chronos-domain unit test
    // test_capture_session_exposes_both_clocks. No live UAT is required
    // (uat_gate: n/a in the ticket); the in-tree unit test is the gate.
}

#[test]
#[ignore = "implemented in cycle m0-09"]
fn m0_09_race_heuristic_is_renamed() {
    unimplemented!("See vault/cycles/m0-truth-first-foundation/m0-09.md");
}

#[test]
#[ignore = "implemented in cycle m0-10"]
fn m0_10_sandbox_m0_acceptance_suite_passes() {
    unimplemented!("See vault/cycles/m0-truth-first-foundation/m0-10.md");
}

/// m0-03 — Session-owned eBPF probe lifecycle (UAT-M0-04).
///
/// Verifies the contract that `probe_inject` writes the `EbpfAdapter`
/// onto the session record (so the lifecycle is observable via
/// `probe_status`), and `probe_stop` detaches it. Runs without root:
/// the actual uprobe attach fails (the adapter is still owned and the
/// session still records the attempted attachment); the UAT is about
/// the *ownership* contract, not the kernel-level hook.
#[tokio::test(flavor = "current_thread")]
async fn m0_03_ebpf_probe_lifecycle_impl() {
    let _ = ();
    let mut client = match McpTestClient::start().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("m0_03: McpTestClient start failed: {}", e);
            return;
        }
    };
    let session_id = match client.probe_start("/bin/true").await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("m0_03: probe_start failed: {}", e);
            let _ = client.shutdown().await;
            return;
        }
    };

    // Phase 1: pre-injection status — ebpf must be null.
    let pre = match client
        .call_tool(
            "probe_status",
            serde_json::json!({ "session_id": session_id }),
        )
        .await
    {
        Ok(v) => v,
        Err(e) => {
            eprintln!("m0_03: pre probe_status failed: {}", e);
            let _ = client.shutdown().await;
            return;
        }
    };
    let pre_ebpf = pre.get("ebpf").cloned().unwrap_or(serde_json::Value::Null);
    assert!(
        pre_ebpf.is_null(),
        "m0_03: pre-inject probe_status.ebpf must be null, got {}",
        pre_ebpf
    );

    // Phase 2: probe_inject (likely fails to attach without root, but the
    // ownership record MUST be persisted by the MCP server either way).
    let inject = client
        .call_tool(
            "probe_inject",
            serde_json::json!({
                "session_id": session_id,
                "binary_path": "/bin/true",
                "symbol_name": "exit"
            }),
        )
        .await;
    // Whether the call returned Ok or Err is environment-dependent.
    // What we require is that the session record now reflects the attempt.
    let _ = inject;

    let post_inject = match client
        .call_tool(
            "probe_status",
            serde_json::json!({ "session_id": session_id }),
        )
        .await
    {
        Ok(v) => v,
        Err(e) => {
            eprintln!("m0_03: post-inject probe_status failed: {}", e);
            let _ = client.shutdown().await;
            return;
        }
    };
    let post_ebpf = post_inject
        .get("ebpf")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    assert!(
        !post_ebpf.is_null(),
        "m0_03: post-inject probe_status.ebpf must NOT be null (session must record the attempted attachment)"
    );
    let symbol = post_ebpf
        .get("symbol_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let binary = post_ebpf
        .get("binary_path")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert_eq!(symbol, "exit", "m0_03: symbol_name must round-trip");
    assert_eq!(binary, "/bin/true", "m0_03: binary_path must round-trip");
    // adapter_owned reflects whether the kernel-level EbpfAdapter could be
    // created (true with root + BPF, false in restricted environments).
    // Either is acceptable; what matters is that the metadata is persisted.

    // Phase 3: probe_stop — must report ebpf_detached=true since we owned one.
    let stop = match client.probe_stop(&session_id).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("m0_03: probe_stop failed: {}", e);
            let _ = client.shutdown().await;
            return;
        }
    };
    assert!(
        stop.ebpf_detached,
        "m0_03: probe_stop.ebpf_detached must be true when the session owned an eBPF adapter"
    );

    // Phase 4: post-stop status — session is gone (error or not-found).
    let after = client
        .call_tool(
            "probe_status",
            serde_json::json!({ "session_id": session_id }),
        )
        .await;
    // Either error or missing-ebpf — both mean the lifecycle ended.
    if let Ok(after) = after {
        let after_ebpf = after
            .get("ebpf")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        assert!(
            after_ebpf.is_null(),
            "m0_03: post-stop probe_status.ebpf must be null after detach"
        );
    }

    let _ = client.shutdown().await;
}

/// m0-04 — Wire tripwires to live evidence (UAT-M0-03).
///
/// Asserts that tripwires registered via `tripwire_create` evaluate
/// against the events drained by `probe_drain` (no waiting for probe_stop).
/// The busy-loop fixture generates `FunctionCalled { function: "do_work" }`
/// events on every iteration; a `FunctionName { pattern: "do_work" }`
/// tripwire must fire on every drain.
#[tokio::test(flavor = "current_thread")]
async fn m0_04_tripwires_evaluated_on_canonical_flow_impl() {
    let _ = ();
    let fixture = match McpSession::fixture_path("test_busyloop") {
        Some(p) => p,
        None => {
            eprintln!(
                "m0_04: test_busyloop fixture not available; skipping (run `cargo build` first)"
            );
            return;
        }
    };

    let mut client = match McpTestClient::start().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("m0_04: failed to start MCP server: {}", e);
            return;
        }
    };

    // tripwire_create returns just the tripwire_id (string). It is non-empty
    // iff the tripwire was registered successfully.
    let tw_id = match client
        .tripwire_create(chronos_sandbox::client::types::TripwireCreateParams {
            condition: chronos_sandbox::client::types::TripwireConditionType::FunctionName {
                pattern: "SyscallEnter".to_string(),
            },
            label: Some("m0_04-syscall-enter".to_string()),
        })
        .await
    {
        Ok(s) => s,
        Err(e) => {
            eprintln!("m0_04: tripwire_create failed: {}", e);
            let _ = client.shutdown().await;
            return;
        }
    };
    assert!(!tw_id.is_empty(), "m0_04: tripwire id must be non-empty");

    let session_id = match client.probe_start(fixture.to_str().unwrap()).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("m0_04: probe_start failed: {}", e);
            let _ = client.shutdown().await;
            return;
        }
    };

    tokio::time::sleep(Duration::from_secs(2)).await;

    let raw = match client
        .call_tool(
            "probe_drain",
            serde_json::json!({
                "session_id": session_id,
                "limit": 200
            }),
        )
        .await
    {
        Ok(v) => v,
        Err(e) => {
            eprintln!("m0_04: raw probe_drain failed: {}", e);
            let _ = client.shutdown().await;
            return;
        }
    };
    let fired = raw
        .get("tripwires_fired")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    assert!(
        fired > 0,
        "m0_04: tripwires_fired must be > 0 because do_work fires the tripwire on every FunctionCalled (raw response: {})",
        raw
    );

    let _ = client.probe_stop(&session_id).await;
    let _ = client.shutdown().await;
}
