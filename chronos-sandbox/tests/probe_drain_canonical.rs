//! REC-C2.2.2 — `probe_drain` canonical wire contract, exercised through the
//! real MCP server.
//!
//! These tests cross the actual MCP boundary on purpose. The properties under
//! test are not "the service compiled"; they are:
//!
//! 1. the coordinate space on the wire is the canonical `ecv1:` cursor,
//! 2. reusing that cursor does not re-deliver what the previous page examined,
//! 3. a cursor minted for another session is refused, not silently re-anchored,
//! 4. completeness is the `events_read` model scoped to the examined range,
//! 5. the firing count is evidence, not subscription state: creating a tripwire
//!    *after* events were accepted must not retroactively change the count.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::McpSession;
use std::collections::HashSet;
use std::time::Duration;

/// Start a probe on the `test_busyloop` fixture and return the session id.
///
/// Both dependencies are supposed to be present on every host that builds this
/// workspace, so a failure here is a defect of the tree, not a missing host
/// facility:
///
///   * the fixture is compiled by `chronos-sandbox/build.rs` into `OUT_DIR` and
///     published as the compile-time `CHRONOS_FIXTURE_DIR`, so it exists
///     whenever the test binary itself was built;
///   * `probe_start` execs it under ptrace, which needs no extra capability.
///
/// This helper therefore returns the session id directly and panics on failure.
/// It used to return `Option<String>` and let every caller `return` early,
/// which turned a broken build into four green tests that asserted nothing.
async fn start_probed_fixture(client: &mut McpTestClient) -> String {
    let fixture = McpSession::fixture_path("test_busyloop").unwrap_or_else(|| {
        panic!(
            "probe_drain_canonical: required fixture `test_busyloop` is missing from {} — \
             it is compiled by chronos-sandbox/build.rs, so a missing fixture is a build \
             defect, not an absent host facility",
            chronos_sandbox::FixtureResolver::root().display()
        )
    });
    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .unwrap_or_else(|e| {
            panic!(
                "probe_drain_canonical: probe_start({}) failed: {e} — the fixture is present \
                 and ptrace needs no capability, so this is a real failure",
                fixture.display()
            )
        });
    tokio::time::sleep(Duration::from_secs(2)).await;
    session_id
}

/// The wire shape is canonical: an `ecv1:` cursor, an examined-range
/// completeness report, `exhausted`, and a null compatibility field. The
/// retired ring-cursor fields must be gone.
#[tokio::test]
async fn probe_drain_wire_exposes_canonical_cursor_and_completeness() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = start_probed_fixture(&mut client).await;

    let first = client
        .probe_drain_with_evidence_cursor(&session_id, None)
        .await
        .expect("probe_drain failed");

    let cursor = first
        .evidence_cursor
        .clone()
        .expect("probe_drain must return an evidence_cursor");
    assert!(
        cursor.starts_with("ecv1:"),
        "wire cursor must be an EventsCursorV1 token, got {cursor:?}"
    );

    let completeness = first
        .completeness
        .as_ref()
        .expect("probe_drain must report completeness");
    assert_eq!(
        completeness.scope, "examined_range",
        "completeness describes the examined range, not pagination"
    );
    assert!(
        completeness.to_seq_exclusive >= completeness.from_seq,
        "examined range must be well-formed"
    );
    assert!(
        first.legacy_cursor.is_none(),
        "legacy_cursor must be null: the ring cursor carries no evidence"
    );

    // The retired fields must not reappear on the wire.
    let raw = client
        .probe_drain_wire(&session_id, None)
        .await
        .expect("raw probe_drain failed");
    for gone in ["cursor_stale", "total_pushed", "snapshot_len"] {
        assert!(
            raw.get(gone).is_none(),
            "retired field {gone:?} is still on the probe_drain wire: {raw}"
        );
    }
    assert!(
        raw.get("cursor").is_none(),
        "the legacy ring cursor must not be exposed as 'cursor'"
    );

    let _ = client.probe_stop(&session_id).await;
    let _ = client.shutdown().await;
}

/// Reusing the returned cursor must continue the read, never replay it.
#[tokio::test]
async fn probe_drain_cursor_continuation_does_not_reread() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = start_probed_fixture(&mut client).await;

    let first = client
        .probe_drain_with_evidence_cursor(&session_id, None)
        .await
        .expect("first drain failed");
    let cursor = first
        .evidence_cursor
        .clone()
        .expect("first drain must return a cursor");
    assert!(
        !first.events.is_empty(),
        "fixture should have produced events to make continuity meaningful"
    );
    let first_ids: HashSet<u64> = first.events.iter().map(|e| e.event_id).collect();

    let second = client
        .probe_drain_with_evidence_cursor(&session_id, Some(cursor.as_str()))
        .await
        .expect("continuation drain failed");

    for ev in &second.events {
        assert!(
            !first_ids.contains(&ev.event_id),
            "cursor reuse re-delivered event {}; a cursor advances past everything examined",
            ev.event_id
        );
    }

    // Continuation is strictly forward: the new cursor is beyond the old one.
    let second_cursor = second
        .evidence_cursor
        .clone()
        .expect("continuation must return a cursor");
    assert!(second_cursor.starts_with("ecv1:"));
    assert_ne!(
        second_cursor, cursor,
        "an examined page must advance the cursor"
    );

    let _ = client.probe_stop(&session_id).await;
    let _ = client.shutdown().await;
}

/// A cursor minted for a different session is a typed refusal, never a silent
/// re-anchor onto this session's log.
#[tokio::test]
async fn probe_drain_rejects_foreign_session_cursor() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let owner_session = start_probed_fixture(&mut client).await;
    let other_session = start_probed_fixture(&mut client).await;

    let owned = client
        .probe_drain_with_evidence_cursor(&owner_session, None)
        .await
        .expect("owner drain failed")
        .evidence_cursor
        .expect("owner drain must return a cursor");

    // Present the owner's token while asking about the other session.
    let err = client
        .probe_drain_wire(&other_session, Some(owned.as_str()))
        .await
        .expect_err("a foreign-session cursor must be refused");
    let text = format!("{err}");
    assert!(
        text.contains("cursor"),
        "refusal should name the cursor as the problem, got: {text}"
    );

    let _ = client.probe_stop(&owner_session).await;
    let _ = client.probe_stop(&other_session).await;
    let _ = client.shutdown().await;
}

/// Productive falsification, at the MCP boundary: the firing count is
/// persisted evidence, not subscription state.
///
/// `tripwire_create` after the events were accepted must not retroactively
/// manufacture firings for them. Re-reading the same durable range from the
/// start must report the same count.
#[tokio::test]
async fn probe_drain_firing_count_is_evidence_not_subscription_state() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = start_probed_fixture(&mut client).await;

    let before = client
        .probe_drain_with_evidence_cursor(&session_id, None)
        .await
        .expect("baseline drain failed");
    let baseline_firings = before.tripwires_fired.expect(
        "probe_drain_log always emits tripwires_fired; absence means the wire \
         contract changed, and defaulting to 0 would make the comparison below vacuous",
    );
    let baseline_events = before.events.len();

    // Add a subscription that would match essentially everything already
    // captured. If decision-at-read-time were still in force, the next read of
    // the same range would report more firings than were ever persisted.
    // CIH-E: scope the tripwire to the same session that owns the capture
    // so the canonical-evidence observe pipeline resolves the canonical
    // session from the explicit scope.
    // REC-C5-C5.2: migrated to the v2 `observe` dispatcher
    // (verb=create, condition.kind=tripwire, scope=session).
    let created = client
        .call_tool(
            "observe",
            serde_json::json!({
                "verb": "create",
                "condition": {
                    "kind": "tripwire",
                    "condition": { "type": "event_type", "event_types": ["function_entry"] },
                    "label": "post-hoc subscription (C2.2 falsification)"
                },
                "scope": {"scope": "session", "session_id": session_id.clone()}
            }),
        )
        .await;
    assert!(
        created.is_ok(),
        "the post-hoc subscription must exist, otherwise this falsification is vacuous: {created:?}"
    );

    let after = client
        .probe_drain_with_evidence_cursor(&session_id, None)
        .await
        .expect("post-subscription drain failed");
    let after_firings = after.tripwires_fired.expect(
        "probe_drain_log always emits tripwires_fired; absence means the wire \
         contract changed, and defaulting to 0 would make the comparison below vacuous",
    );

    assert!(
        after.events.len() >= baseline_events,
        "re-reading from the start must cover at least the same durable range"
    );
    assert_eq!(
        after_firings, baseline_firings,
        "creating a tripwire after the fact changed the reported firing count for the \
         same durable range ({baseline_firings} -> {after_firings}): subscription state is \
         still being consulted as authority"
    );

    let _ = client.probe_stop(&session_id).await;
    let _ = client.shutdown().await;
}
