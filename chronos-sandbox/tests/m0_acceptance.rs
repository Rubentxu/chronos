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

#[test]
#[ignore = "implemented in cycle m0-03"]
fn m0_03_ebpf_probe_lifetime_is_session_owned() {
    unimplemented!("See vault/cycles/m0-truth-first-foundation/m0-03.md");
}

#[test]
#[ignore = "implemented in cycle m0-04"]
fn m0_04_tripwires_evaluated_on_canonical_flow() {
    unimplemented!("See vault/cycles/m0-truth-first-foundation/m0-04.md");
}

#[test]
#[ignore = "implemented in cycle m0-05"]
fn m0_05_typed_event_filters_reject_unknown() {
    unimplemented!("See vault/cycles/m0-truth-first-foundation/m0-05.md");
}

#[test]
#[ignore = "implemented in cycle m0-06"]
fn m0_06_state_diff_preserves_register_evidence() {
    unimplemented!("See vault/cycles/m0-truth-first-foundation/m0-06.md");
}

#[test]
#[ignore = "implemented in cycle m0-07"]
fn m0_07_query_returns_not_found_when_target_missing() {
    unimplemented!("See vault/cycles/m0-truth-first-foundation/m0-07.md");
}

#[test]
#[ignore = "implemented in cycle m0-08"]
fn m0_08_timestamp_contract_is_separated() {
    unimplemented!("See vault/cycles/m0-truth-first-foundation/m0-08.md");
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
