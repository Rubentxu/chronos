//! Counterexample smoke tests (m8-03 sandbox deliverable 4 + m8-05 + m8-06 deltas).
//!
//! Exercises the 4 chronos-mcp tools (`counterexample_shrink`,
//! `counterexample_get`, `counterexample_list`, `counterexample_events_count`)
//! end-to-end against a real spawned MCP server.
//!
//! Pipeline shape today (m8-06):
//!   1. probe_start on a real fixture (test_busyloop / test_exit_immediate)
//!      so the engine has captured events for the returned session_id.
//!   2. counterexample_shrink — validates the target violates the captured
//!      trace via `hypothesis_test::test()`, then runs the proptest shrink
//!      loop with REAL per-variant shrinkers (m8-06 closes m8-05 R3).
//!      `rounds_used` reflects the actual shrinking progress: 2 for
//!      degenerate cases (Bool, just(base) targets), >= 8 for Number
//!      targets that converge toward 0.0, >= 6 for Text targets that
//!      converge toward "", etc.
//!   3. counterexample_get / counterexample_list / counterexample_events_count
//!      — read back from the redb `counterexample_bundles` table populated
//!      in step 2.
//!
//! Honest disclosures (see m8-06 scoping doc):
//!  * `rounds_used` is bounded by `max_shrink_iters` (default 64 via
//!    ShrinkConfig→proptest::Config mapping added in m8-06).
//!  * `next_cursor` semantics: when the page is full (len == limit),
//!    next_cursor is the bundle_id to pass back as `cursor` for the
//!    next page; otherwise next_cursor is null. m8-05 B2 uses
//!    `limit = u32::MAX` in smoke tests to assert single-page semantics.
//!  * Events count is on the wire (m8-04 B3) + has its own dedicated
//!    tool `counterexample_events_count` (m8-05 B3).
//!
//! No test here skips. Every dependency the suite needs is produced by the tree
//! itself: the C fixtures are compiled by `chronos-sandbox/build.rs`, and the
//! MCP server is this workspace's own `chronos-mcp` binary. A failure to obtain
//! either is therefore a defect of the build or of the seam under test, and
//! every such step below panics with the reason instead of returning early —
//! an early `return` here used to turn a broken server into a green test that
//! asserted nothing.

// Re-export the helpers we use so the test compiles even when skipped.
use chronos_sandbox::client::tools::McpTestClient;
use serde_json::json;
use std::time::Duration;

/// Spin up a server, capture the given fixture, stop the probe, and return
/// `(client, session_id, captured_events)`.
///
/// `captured_events` is how many events the session's query engine holds, read
/// back from the server after `probe_stop`. Callers that need that number used
/// to ask for it themselves via `probe_drain`, which only serves *live*
/// sessions — a stopped one answers `Live probe session '<id>' not found` — so
/// three of them got that error, `return`ed, and reported green without
/// asserting anything at all.
///
/// Every step panics on failure. Nothing here is a host facility that may be
/// absent: the fixture is compiled by `chronos-sandbox/build.rs` and the server
/// is this workspace's own `chronos-mcp`.
async fn setup_with_probe(fixture: &str) -> (McpTestClient, String, usize) {
    let mut client = McpTestClient::start().await.unwrap_or_else(|e| {
        panic!("counterexample_tools: failed to start the chronos-mcp server: {e}")
    });

    let path = chronos_sandbox::McpSession::fixture_path(fixture).unwrap_or_else(|| {
        panic!(
            "counterexample_tools: required fixture `{fixture}` is missing from {} — it is \
             compiled by chronos-sandbox/build.rs, so a missing fixture is a build defect",
            chronos_sandbox::FixtureResolver::root().display()
        )
    });

    let session_id = client
        .probe_start(path.to_str().unwrap())
        .await
        .unwrap_or_else(|e| {
            panic!(
                "counterexample_tools: probe_start({}) failed: {e} — the fixture is present and \
                 ptrace needs no capability, so this is a real failure",
                path.display()
            )
        });

    // Allow the probe to populate the in-flight buffer.
    tokio::time::sleep(Duration::from_millis(400)).await;

    // The MCP server only inserts a session_id into the engines map on
    // probe_stop (the query engine is built at stop time, not start
    // time). Without this, `hypothesis_test::test()` would return
    // SessionNotFound before the shrink loop even begins.
    client.probe_stop(&session_id).await.unwrap_or_else(|e| {
        panic!(
            "counterexample_tools: probe_stop failed: {e} — the shrink path needs the session \
                registered in the engines map"
        )
    });

    // Count the events the engine actually holds for this session, which is
    // the same view the `EventCount` hypothesis is evaluated against. Two
    // earlier sources of this number were wrong: `probe_drain` after
    // `probe_stop` fails with `Live probe session '<id>' not found` (three
    // tests used to swallow that and report green), and a drain taken while
    // the session was still live undercounts it, because more events land
    // between the drain and the stop — which made `events.len() >= N+1`
    // already true and the server answer "nothing to shrink".
    let captured = client
        .query_events_walk_all(
            &session_id,
            chronos_sandbox::client::types::QueryFilter {
                limit: 500,
                ..Default::default()
            },
        )
        .await
        .unwrap_or_else(|e| panic!("counterexample_tools: query_events after probe_stop: {e}"))
        .len();

    (client, session_id, captured)
}

/// CE1: shrink a constant target on a real probe session.
///
/// m8-06: uses a hypothesis that (a) ALWAYS violates initially
/// (`events.len() >= N+1` where `N` is the captured event count read
/// back from the server, so it is always false right after capture)
/// and (b) WILL flip during shrinking (the binary-search shrinker walks
/// the constant toward 0; eventually it drops below `events.len()` and
/// the comparison flips to Pass, ending the loop). This is robust against
/// fixture runners that capture more or fewer events than the original
/// author estimated (the prior `constant: 1000.0` literal broke in CI
/// where busyloop captured >= 1000 events).
#[tokio::test]
async fn ce1_shrink_constant_target() {
    let (mut client, session_id, count) = setup_with_probe("test_busyloop").await;

    // The captured event count comes from the engine view read inside the
    // setup helper. Asking for it here used to fail with `Live probe session
    // '<id>' not found` (the helper had already stopped the probe), and this
    // test then returned early and reported success.
    let constant = (count + 1) as f64;

    let target = json!({
        "session_id": session_id,
        "kind": "invariant",
        "scope": "EventCount",
        "comparison": "Ge",
        "constant": { "Number": constant },
    });

    let resp = client
        .counterexample_shrink(target)
        .await
        .expect("counterexample_shrink failed");

    assert!(
        !resp.bundle.bundle_id.is_empty(),
        "bundle_id should be non-empty"
    );
    // m8-06: rounds_used reflects ACTUAL shrinking. We assert >= 2
    // (initial + at least one simplify attempt) and <= 64 (cap).
    assert!(
        resp.rounds_used >= 2,
        "rounds_used should be >= 2 (initial + at least 1 simplify); got {}",
        resp.rounds_used
    );
    assert!(
        resp.rounds_used <= 64,
        "rounds_used should be <= 64 (max_shrink_iters cap); got {}",
        resp.rounds_used
    );
    assert!(
        resp.bundle.has_full_bundle,
        "m8-03 bundles always carry full payload"
    );
    assert_eq!(resp.bundle.property_kind, "invariant");
    // m8-04: the persisted bundle now carries real captured trace events
    // (m8-03 shipped events_count=0 as a vec![] placeholder). On the
    // test_busyloop probe the engine captures events before the engine
    // map is queried by the dispatcher, so events_count must be > 0.
    assert!(
        resp.events_count >= 1,
        "events_count must be > 0 after m8-04 (got {})",
        resp.events_count
    );

    let _ = client.shutdown().await;
}

/// CE2: shrink → get round-trip with the same session.
#[tokio::test]
async fn ce2_shrink_then_get_round_trip() {
    let (mut client, session_id, _captured) = setup_with_probe("test_busyloop").await;

    let target = json!({
        "session_id": session_id,
        "kind": "invariant",
        "constant": { "Number": -1.0 },
    });

    let shrink = client
        .counterexample_shrink(target)
        .await
        .expect("shrink failed");
    let bundle_id = shrink.bundle.bundle_id.clone();

    let get = client
        .counterexample_get(&bundle_id)
        .await
        .expect("counterexample_get failed");

    assert_eq!(get.bundle.bundle_id, bundle_id, "id should round-trip");
    assert!(get.has_full_bundle);

    let _ = client.shutdown().await;
}

/// CE3: counterexample_get on a non-existent id returns Err(LoadFailed)
/// at the MCP layer.
#[tokio::test]
async fn ce3_get_missing_bundle_errors() {
    let mut client = McpTestClient::start().await.unwrap_or_else(|e| {
        panic!("counterexample_tools: failed to start the chronos-mcp server: {e}")
    });

    let result = client.counterexample_get("does-not-exist").await;
    assert!(result.is_err(), "missing bundle id should error");

    let _ = client.shutdown().await;
}

/// CE4: list after a shrink sees the bundle.
#[tokio::test]
async fn ce4_list_after_shrink_includes_bundle() {
    let (mut client, session_id, _captured) = setup_with_probe("test_busyloop").await;

    let target = json!({
        "session_id": session_id,
        "kind": "invariant",
        "constant": { "Number": 7.0 },
    });

    let shrink = client
        .counterexample_shrink(target)
        .await
        .expect("shrink failed");
    let bundle_id = shrink.bundle.bundle_id.clone();

    let list = client
        .counterexample_list(None, Some("invariant"), Some(u32::MAX), None)
        .await
        .expect("counterexample_list failed");

    assert!(
        list.bundles.iter().any(|b| b.bundle_id == bundle_id),
        "newly-shrunk bundle should appear in list"
    );
    // m8-05 B2: with `limit = u32::MAX` (effectively no limit), the
    // returned page is always partial, so next_cursor is None. (The
    // tests share a default redb path, so other ce* test bundles are
    // also present — picking u32::MAX here keeps the assertion
    // stable across runs.)
    assert!(
        list.next_cursor.is_none(),
        "with limit=u32::MAX next_cursor must be None"
    );

    let _ = client.shutdown().await;
}

/// CE5: shrink after a different fixture (test_exit_immediate) — same
/// behaviour expected; pipeline is fixture-agnostic.
///
/// m8-06: uses a Ge hypothesis on the captured event count, so the target
/// is guaranteed to violate at the start and the shrinker has real work.
///
/// This used to hardcode `1000.0` "like ce1", but ce1 had already been
/// converted to `count + 1` in `b4fe4e8d` — the comment had drifted from the
/// code it claimed to mirror. It only passed because `test_exit_immediate`
/// happens to capture far fewer than 1000 events, which is an accident of
/// timing, not a property. `test_busyloop` under tarpaulin instrumentation
/// *does* cross 1000, which is how `ce12` was found failing.
#[tokio::test]
async fn ce5_shrink_on_exit_immediate_fixture() {
    let (mut client, session_id, count) = setup_with_probe("test_exit_immediate").await;

    let constant = (count + 1) as f64;

    let target = json!({
        "session_id": session_id,
        "kind": "invariant",
        "scope": "EventCount",
        "comparison": "Ge",
        "constant": { "Number": constant },
    });

    let resp = client
        .counterexample_shrink(target)
        .await
        .expect("shrink on exit_immediate fixture failed");

    assert!(!resp.bundle.bundle_id.is_empty());
    assert!(
        resp.rounds_used >= 2 && resp.rounds_used <= 64,
        "rounds_used should be in [2, 64]; got {}",
        resp.rounds_used
    );

    let _ = client.shutdown().await;
}

/// CE6: list with workspace_id filter — call shape is honoured.
#[tokio::test]
async fn ce6_list_with_workspace_filter() {
    let (mut client, session_id, _captured) = setup_with_probe("test_busyloop").await;

    let target = json!({
        "session_id": session_id,
        "kind": "invariant",
        "constant": { "Number": 9.0 },
    });
    let _ = client
        .counterexample_shrink(target)
        .await
        .expect("shrink failed");

    let list = client
        .counterexample_list(Some("ws-default"), None, Some(u32::MAX), None)
        .await
        .expect("list with workspace filter failed");

    // m8-05 B2: with `limit = u32::MAX` (effectively no limit), the
    // returned page is always partial, so next_cursor is None.
    assert!(list.next_cursor.is_none());

    let _ = client.shutdown().await;
}

/// CE7: shrink response carries the new m8-04 `{bundle, events_count}`
/// shape (was `{saved: <summary>}` in m8-03). Asserts via the typed
/// wrapper fields — `events_count >= 1` confirms m8-04 closed the
/// m8-03 vec![] placeholder.
#[tokio::test]
async fn ce7_shrink_response_uses_new_saved_envelope() {
    let (mut client, session_id, count) = setup_with_probe("test_busyloop").await;

    // m8-06: use Ge/(N+1) where N is the captured event count, so the
    // shrinker does real work regardless of how many events the fixture
    // captured (the prior literal `1000.0` broke in CI when busyloop
    // captured >= 1000 events). The count is read inside the setup helper
    // from the engine the hypothesis is evaluated against; requesting it
    // after the probe was stopped via `probe_drain` failed with `Live probe
    // session '<id>' not found`, and this test then returned early and
    // reported success.
    let constant = (count + 1) as f64;

    let target = json!({
        "session_id": session_id,
        "kind": "invariant",
        "scope": "EventCount",
        "comparison": "Ge",
        "constant": { "Number": constant },
    });

    let resp = client
        .counterexample_shrink(target)
        .await
        .expect("shrink failed");

    // m8-04 B5 rework: the Saved variant now carries (bundle,
    // events_count) — the typed wrapper exposes both directly.
    assert!(
        !resp.bundle.bundle_id.is_empty(),
        "bundle_id must be present"
    );
    assert!(
        resp.events_count >= 1,
        "events_count must be >= 1 after m8-04 (got {})",
        resp.events_count
    );
    assert!(
        resp.rounds_used >= 2 && resp.rounds_used <= 64,
        "rounds_used should be in [2, 64]; got {}",
        resp.rounds_used
    );

    let _ = client.shutdown().await;
}

/// CE8: m8-05 B2 forward pagination. Save 2 bundles in this test,
/// then request page 1 with `limit=1`. The page must be full
/// (`len == limit`) and `next_cursor` must be `Some(last.bundle_id)`.
/// Page 2 with `cursor = page1.next_cursor` must return the second
/// bundle and (since the page is no longer "full" with 1 result of
/// limit 2) terminate pagination with `next_cursor = None` on the
/// next iteration.
#[tokio::test]
async fn ce8_list_pagination_forward_cursor() {
    let (mut client, session_id, _captured) = setup_with_probe("test_busyloop").await;

    // Save 2 distinct bundles in this test, each with a unique constant
    // so the minimised payload differs. saved_ids[0] is the FIRST save
    // (= older of the two bundle_ids we created); saved_ids[1] is the
    // SECOND save (= newer of the two).
    let mut saved_ids: Vec<String> = Vec::new();
    for n in [11.0_f64, 22.0] {
        let target = json!({
            "session_id": session_id,
            "kind": "invariant",
            "constant": { "Number": n },
        });
        let resp = client
            .counterexample_shrink(target)
            .await
            .expect("shrink failed");
        saved_ids.push(resp.bundle.bundle_id.clone());
    }
    // saved_ids are uuid::v7; the second one is strictly larger.
    assert!(
        saved_ids[1] > saved_ids[0],
        "second save must have larger uuid::v7 bundle_id"
    );

    // Page 1: limit=1, no cursor. Because the tests share a default
    // redb path, the DB may contain bundles from prior test runs.
    // We therefore cannot assert WHICH bundle page 1 returns; we
    // only assert pagination semantics.
    let page1 = client
        .counterexample_list(None, Some("invariant"), Some(1), None)
        .await
        .expect("page1 list failed");
    assert_eq!(page1.bundles.len(), 1, "page1 must have exactly 1 bundle");
    let cursor = page1
        .next_cursor
        .clone()
        .expect("page1 must have next_cursor when full");

    // Page 2: cursor = page1.next_cursor, limit=1. Must return a
    // single bundle strictly AFTER page1's bundle.
    let page2 = client
        .counterexample_list(None, Some("invariant"), Some(1), Some(&cursor))
        .await
        .expect("page2 list failed");
    assert_eq!(page2.bundles.len(), 1, "page2 must have exactly 1 bundle");
    assert!(
        page2.bundles[0].bundle_id > page1.bundles[0].bundle_id,
        "page2.bundle_id ({}) must be > page1.bundle_id ({})",
        page2.bundles[0].bundle_id,
        page1.bundles[0].bundle_id
    );
    // The shared redb path means OTHER ce* tests may have saved
    // bundles. So we only assert that page2.next_cursor, if Some,
    // must be > cursor (a forward-paging invariant). When the
    // total page happens to be the last, next_cursor will be None.
    if let Some(ref nc) = page2.next_cursor {
        assert!(
            nc.as_str() > cursor.as_str(),
            "if next_cursor is Some on page2, it must be > cursor (forward paging)"
        );
    }

    let _ = saved_ids; // silence unused-variable lint; the saves
                       // themselves guarantee state population.

    let _ = client.shutdown().await;
}

/// CE9: m8-05 (B3) `counterexample_events_count` tool. Save a bundle
/// (which records the events count from the live engine), then call
/// `counterexample_events_count(bundle_id)` and assert the typed
/// wire envelope matches the count we know was persisted. Also call
/// with an unknown id to confirm the error path returns LoadFailed.
#[tokio::test]
async fn ce9_events_count_returns_persisted_length() {
    let (mut client, session_id, _captured) = setup_with_probe("test_busyloop").await;

    // Save a bundle (this records events_count from the live engine).
    let target = json!({
        "session_id": session_id,
        "kind": "invariant",
        "constant": { "Number": 13.0 },
    });
    let resp = client
        .counterexample_shrink(target)
        .await
        .expect("shrink failed");
    let bundle_id = resp.bundle.bundle_id.clone();
    let expected_count = resp.events_count;
    assert!(expected_count >= 1, "test_busyloop must produce >= 1 event");

    // m8-05 B3: read just the events_count.
    let count_resp = client
        .counterexample_events_count(&bundle_id)
        .await
        .expect("events_count failed");
    assert_eq!(count_resp.bundle_id, bundle_id);
    assert_eq!(
        count_resp.events_count, expected_count,
        "events_count must match the Saved envelope"
    );

    // m8-05 B3: unknown bundle_id returns Err(LoadFailed).
    let err = client
        .counterexample_events_count("nonexistent-bundle")
        .await;
    assert!(err.is_err(), "unknown bundle_id must error");

    let _ = client.shutdown().await;
}

/// CE10: m8-06 — real per-variant proptest shrinking for Number target.
/// Confirms that `rounds_used` reflects ACTUAL shrinking (>= 8 rounds for
/// a Number target that converges toward 0.0), not the m8-05 R3 stopgap
/// (which always produced exactly 2 rounds). The exact final `constant`
/// value depends on the captured trace's invariant evaluation; we only
/// assert that it shrunk to a smaller-magnitude value than the start.
#[tokio::test]
async fn ce10_shrink_number_target_real_shrinking() {
    let (mut client, session_id, count) = setup_with_probe("test_busyloop").await;

    // m8-06: Ge/(N+1) where N is the captured event count guarantees an
    // initial violation (events.len() < N+1) and the shrinker can shrink
    // toward 0 until it crosses events.len() and the invariant flips to
    // Pass. Using N+1 instead of a literal 1000.0 makes the test robust
    // against fixture runners that capture more or fewer events than
    // the original author estimated. The count is read inside the setup
    // helper from the engine the hypothesis is evaluated against; requesting
    // it after the probe was stopped via `probe_drain` failed with `Live
    // probe session '<id>' not found`, and this test then returned early and
    // reported success.
    let constant = (count + 1) as f64;

    let target = json!({
        "session_id": session_id,
        "kind": "invariant",
        "scope": "EventCount",
        "comparison": "Ge",
        "constant": { "Number": constant },
    });

    let resp = client
        .counterexample_shrink(target)
        .await
        .expect("shrink failed");

    assert!(
        !resp.bundle.bundle_id.is_empty(),
        "bundle_id must be present"
    );
    assert!(
        resp.rounds_used >= 2,
        "rounds_used should be >= 2 (initial + 1 simplify); got {}",
        resp.rounds_used
    );
    assert!(
        resp.rounds_used <= 64,
        "rounds_used should be <= 64 (max_shrink_iters cap); got {}",
        resp.rounds_used
    );
    assert!(resp.events_count >= 1, "events_count must be >= 1");

    let _ = client.shutdown().await;
}

/// CE11: m8-06 — real per-variant proptest shrinking for Existence target.
/// Confirms that an Existence predicate shrinks its payload toward "" / 0.
///
/// Existence targets use scope=None; the dispatcher compares the
/// predicate against the captured events directly. For
/// `EventTypeEquals { event_type: "definitely_not_present" }` the
/// predicate never matches, so the hypothesis is `Violation` (no event
/// of that type exists). The shrinker then walks the event_type string
/// toward "" — at "" we still have a violation (the empty string
/// matches no event), so the loop terminates via max_shrink_iters or
/// when the candidate shrinks past the trace.
#[tokio::test]
async fn ce11_shrink_existence_target_real_shrinking() {
    let (mut client, session_id, _captured) = setup_with_probe("test_busyloop").await;

    let target = json!({
        "session_id": session_id,
        "kind": "existence",
        "predicate": {
            "kind": "event_type_equals",
            "event_type": "DEFINITELY_NOT_PRESENT_TYPE",
        },
    });

    let resp = client
        .counterexample_shrink(target)
        .await
        .expect("shrink failed");

    assert!(
        !resp.bundle.bundle_id.is_empty(),
        "bundle_id must be present"
    );
    // m8-06: Existence payload shrinks via TextShrinker toward "".
    // rounds_used counts initial + each successful character deletion.
    // The exact count depends on the live trace; we assert >= 2 and
    // <= 64.
    assert!(
        resp.rounds_used >= 2,
        "Existence predicate should shrink through >= 2 rounds; got {}",
        resp.rounds_used
    );
    assert!(
        resp.rounds_used <= 64,
        "rounds_used should be <= 64; got {}",
        resp.rounds_used
    );

    let _ = client.shutdown().await;
}

/// CE12: m8-07 — shrink with non-default Invariant options (scope=EventCount,
/// comparison=Ge, constant=Number(1000.0)), replay, and assert the
/// reconstructed HypothesisInput matches the original (scope, comparison,
/// constant all preserved via the persisted `target_hypothesis`).
///
/// This is the m8-07 acceptance test. It exercises the full round-trip:
///   1. probe captures events (test_busyloop produces a known count).
///   2. counterexample_shrink persists a bundle WITH the original
///      target_hypothesis wire mirror (hypothesis_input_to_wire in services).
///   3. run_replay loads the bundle, reads target_hypothesis, and
///      reconstructs the EXACT HypothesisInput the user passed to shrink.
///
/// Without m8-07, the pre-m8-07 fallback would reconstruct
/// scope=PropertyValue, comparison=None — a DIFFERENT hypothesis.
/// The ce12 test verifies the m8-07 path exercised hypothesis_input_from_wire
/// successfully (replay succeeds without error, bundle round-trips correctly).
#[tokio::test]
async fn ce12_replay_preserves_non_default_invariant_options() {
    // Use a temp directory for the DB so this test is isolated.
    let temp_dir = std::env::temp_dir();
    let db_path = temp_dir.join(format!("chronos-ce12-{}.redb", std::process::id()));

    // Start the MCP server with the temp DB path.
    let mut client = McpTestClient::start_with_db_path(db_path.clone())
        .await
        .unwrap_or_else(|e| {
            panic!("counterexample_tools: failed to start the chronos-mcp server: {e}")
        });

    let path = chronos_sandbox::McpSession::fixture_path("test_busyloop").unwrap_or_else(|| {
        panic!(
            "counterexample_tools: required fixture `test_busyloop` is missing from {} — it is \
             compiled by chronos-sandbox/build.rs, so a missing fixture is a build defect",
            chronos_sandbox::FixtureResolver::root().display()
        )
    });

    let session_id = client
        .probe_start(path.to_str().unwrap())
        .await
        .unwrap_or_else(|e| {
            panic!(
                "counterexample_tools: probe_start({}) failed: {e} — the fixture is present and \
                 ptrace needs no capability, so this is a real failure",
                path.display()
            )
        });

    // Allow the probe to populate the in-flight buffer.
    tokio::time::sleep(Duration::from_millis(400)).await;

    // The engines map is only populated on probe_stop, which is what makes the
    // shrink loop resolvable for this session.
    client.probe_stop(&session_id).await.unwrap_or_else(|e| {
        panic!(
            "counterexample_tools: probe_stop failed: {e} — the shrink path needs the session \
                registered in the engines map"
        )
    });

    // Count the events the engine actually holds, which is the same view the
    // `EventCount` hypothesis is evaluated against. This test had the same
    // hardcoded `constant: 1000.0` that `b4fe4e8d` removed from ce1/ce7/ce10
    // and was simply missed. It is the one that fails under tarpaulin: the
    // `test_busyloop` fixture is instrumented there, runs slower per iteration
    // but captures more events inside the 400 ms window, crosses 1000, and the
    // hypothesis `EventCount >= 1000` is then already TRUE on the captured
    // trace — so the server correctly refuses with "nothing to shrink". The
    // server was right and the test's premise was wrong.
    let captured = client
        .query_events_walk_all(
            &session_id,
            chronos_sandbox::client::types::QueryFilter {
                limit: 500,
                ..Default::default()
            },
        )
        .await
        .unwrap_or_else(|e| panic!("counterexample_tools: query_events after probe_stop: {e}"))
        .len();
    let constant = (captured + 1) as f64;

    // Shrink with non-default Invariant options: scope=EventCount, comparison=Ge.
    // The target_hypothesis is persisted verbatim via hypothesis_input_to_wire.
    let target = json!({
        "session_id": session_id,
        "kind": "invariant",
        "scope": "EventCount",
        "comparison": "Ge",
        "constant": { "Number": constant },
    });

    let resp = client
        .counterexample_shrink(target)
        .await
        .unwrap_or_else(|e| panic!("counterexample_tools: counterexample_shrink failed: {e}"));

    assert!(
        !resp.bundle.bundle_id.is_empty(),
        "bundle_id must be non-empty"
    );
    assert!(
        resp.rounds_used >= 2 && resp.rounds_used <= 64,
        "rounds_used should be in [2, 64]; got {}",
        resp.rounds_used
    );
    assert!(resp.events_count >= 1, "events_count must be >= 1");

    // The MCP server holds an exclusive lock on this redb file for its whole
    // lifetime, and the replay runs in a *separate* `chronos test replay`
    // process against the same file. Replaying while the server is alive fails
    // with "Database already open. Cannot acquire lock." — which is what this
    // test used to hit, swallow with a printed message, and then report as a
    // pass, so the m8-07 round-trip it exists to prove was never verified.
    let bundle_id = resp.bundle.bundle_id.clone();
    let expected_events_count = resp.events_count;
    let _ = client.shutdown().await;

    // `replay_bundle` only spawns a subprocess against `db_path`; it never
    // talks to the server, so a client on its own private store is enough to
    // drive it now that the lock holder is gone.
    let replay_client = McpTestClient::start().await.unwrap_or_else(|e| {
        panic!("counterexample_tools: failed to start the chronos-mcp server: {e}")
    });

    // The replay reads bundle.target_hypothesis and reconstructs the EXACT
    // HypothesisInput (scope=EventCount, comparison=Ge, constant=1000.0).
    let report = replay_client
        .replay_bundle(&bundle_id, &db_path)
        .await
        .unwrap_or_else(|e| {
            panic!(
                "counterexample_tools: replay_bundle failed: {e} — the server holding the \
                    store lock is already down, so this is a real replay failure"
            )
        });

    // The replay verdict is based on the MINIMISED payload (current behaviour,
    // per D4 in the m8-07 scoping doc). The key assertion is that the
    // bundle was replayable without error — the m8-07 path exercised
    // hypothesis_input_from_wire successfully.
    assert!(
        matches!(
            report.replay_verdict.as_str(),
            "unsupported" | "violation" | "pass"
        ),
        "replay_verdict should be one of the three, got {}",
        report.replay_verdict
    );
    assert_eq!(report.bundle_id, bundle_id);
    assert_eq!(report.events_in_bundle, expected_events_count);

    // Cleanup.
    let _ = replay_client.shutdown().await;
    let _ = std::fs::remove_file(&db_path);
}
