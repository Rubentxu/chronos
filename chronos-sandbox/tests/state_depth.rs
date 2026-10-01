//! State and Diff Depth tests — verify state/diff tools work correctly at various depths.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::client::types::QueryFilter;
use chronos_sandbox::McpSession;
use std::time::Duration;

/// SD1: test_debug_get_registers_at_first_event
/// Probe test_busyloop, query the first events, and pin the *measured*
/// contract of `state_query(kind=register_snapshot)`:
///
/// - a real event id is answered with an error naming the event and saying
///   there is no register state for it (`no register state at event <id>`),
///   because a C fixture capture carries no register evidence;
/// - an event id that does not exist produces a *different* error
///   (`event <id> not found`).
///
/// The second assertion is what keeps this test alive: if the tool answered
/// every id with the same blanket error, the first assertion alone would
/// still pass on a tool that never resolves the event at all.
#[tokio::test]
async fn test_debug_get_registers_at_first_event() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

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

    // Query the first events of the session.
    let filter = QueryFilter {
        limit: 5,
        offset: 0,
        ..Default::default()
    };
    let events = client
        .query_events(&session_id, filter)
        .await
        .expect("query_events failed");

    // The previous body returned early here, printing "No events found,
    // skipping test" and exiting green. A busy loop traced for two seconds
    // always yields a full page; if it ever did not, that is a failure.
    assert_eq!(
        events.len(),
        5,
        "test_busyloop must yield a full page of 5 events, got {}",
        events.len()
    );

    let first_event_id = events[0].event_id;
    println!("First event_id: {}", first_event_id);

    // Get registers at first event
    let registers = client
        .debug_get_registers(&session_id, first_event_id)
        .await;

    let err = registers.expect_err(
        "debug_get_registers must report 'no register state' for a real event of a \
         capture that carries no register evidence, not answer with data",
    );
    let err_msg = err.to_string();
    assert!(
        err_msg.contains(&format!("no register state at event {first_event_id}")),
        "the error must name the queried event ({first_event_id}), got: {err_msg}"
    );

    // Every sampled real event behaves the same way, and the error always
    // names the event that was asked for.
    for event in &events {
        let sampled = client
            .debug_get_registers(&session_id, event.event_id)
            .await;
        let sampled_err = sampled.expect_err(&format!(
            "event {} is a real event of the capture and must answer \
             'no register state at event {}'",
            event.event_id, event.event_id
        ));
        assert!(
            sampled_err
                .to_string()
                .contains(&format!("no register state at event {}", event.event_id)),
            "error for event {} did not name that event: {}",
            event.event_id,
            sampled_err
        );
    }

    // Discriminator: the tool *does* resolve the event id, because an unknown
    // id fails differently ("not found") instead of with a blanket refusal.
    let bogus = client
        .debug_get_registers(&session_id, 999_999)
        .await
        .expect_err("an unknown event id must be rejected");
    let bogus_msg = bogus.to_string();
    assert!(
        bogus_msg.contains("999999") && bogus_msg.contains("not found"),
        "an unknown event id must report 'event 999999 not found', got: {bogus_msg}"
    );

    client.shutdown().await.ok();
}

/// SD2: test_debug_diff_consecutive_events
/// Probe test_busyloop, get 3 events, debug_diff between event 0 and 2, assert valid response.
#[tokio::test]
async fn test_debug_diff_consecutive_events() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

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
    assert!(
        stop.total_events > 0,
        "test_busyloop should have been captured, got 0 events"
    );

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Query 3 events
    let filter = QueryFilter {
        limit: 3,
        offset: 0,
        ..Default::default()
    };
    let events = client
        .query_events(&session_id, filter)
        .await
        .expect("query_events failed");

    // The previous body returned early here, printing "Not enough events" and
    // exiting green. A busy loop traced for two seconds always yields three
    // events; if it ever did not, that is the failure this test exists to catch.
    assert!(
        events.len() >= 3,
        "test_busyloop should yield at least 3 events, got {}",
        events.len()
    );

    // Diff between the first and the third event.
    let (id_a, id_b) = (events[0].event_id, events[2].event_id);
    let diff = client
        .debug_diff(&session_id, id_a, id_b)
        .await
        .expect("debug_diff should succeed for two real events of a busy loop");

    assert_eq!(
        diff.event_id_a, id_a,
        "debug_diff echoed a different event_id_a"
    );
    assert_eq!(
        diff.event_id_b, id_b,
        "debug_diff echoed a different event_id_b"
    );

    // Same fixture limitation as diff_tools: the C capture yields no register
    // or variable evidence, so an empty diff is the honest expectation.
    assert!(
        diff.variables_changed.is_empty() && diff.registers_changed.is_empty(),
        "expected no changes without register or variable evidence, got {:?}",
        diff
    );

    // Two distinct events of a spinning loop are far apart in time. This is
    // the assertion that would have caught the tool being broken.
    assert!(
        diff.timestamp_delta_ns > 0,
        "the first and third event of a two-second busy loop must be more than zero nanoseconds apart, got 0"
    );

    client.shutdown().await.ok();
}

/// SD3: test_state_diff_first_and_last_timestamps
/// Probe test_busyloop, resolve the real first and last event timestamps by
/// walking the session, and pin the measured contract of
/// `state_query(kind=register_diff)`:
///
/// - the cursor walk reaches the true tail of the session (the last event it
///   returns carries the maximum timestamp of the whole session);
/// - the first and last events of a ~2 s busy loop are far apart in time;
/// - `state_diff` answers `Ok` with zero changes, because a C fixture
///   capture carries no register or variable evidence.
///
/// The previous body accepted `Ok` *and* `Err`, and when the walk came back
/// empty it replaced `ts_last` with a fabricated `ts_first + duration_ms` —
/// a timestamp that never existed, which would have poisoned any assertion
/// made with it. Both are gone.
#[tokio::test]
async fn test_state_diff_first_and_last_timestamps() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

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

    // Get first event
    let filter_first = QueryFilter {
        limit: 1,
        offset: 0,
        ..Default::default()
    };
    let first_events = client
        .query_events(&session_id, filter_first)
        .await
        .expect("query_events for first failed");

    // No early return: a two-second busy loop always yields events, and a
    // silent skip would hide the exact regression this test exists to catch.
    assert_eq!(
        first_events.len(),
        1,
        "the first page of a two-second busy loop must contain one event"
    );

    let ts_first = first_events[0].timestamp_ns;
    println!("First timestamp: {}", ts_first);

    // Get last event.
    //
    // R9.8 (drift #13): the pre-C5.3.1 v1 server accepted `offset: 10000`
    // as "skip the first 10k events" and returned whatever was next.
    // The v2 server (`client/tools.rs:689`) explicitly rejects with
    //   `query_events: offset=N is no longer supported by v2 events_read;
    //    use cursor-based pagination (set QueryFilter::cursor from the
    //    previous page's QueryPage::next_cursor)`
    // so we walk the whole session cursor-style and take the last event.
    let last_events = client
        .query_events_walk_all(
            &session_id,
            QueryFilter {
                limit: 1,
                ..Default::default()
            },
        )
        .await
        .expect("query_events_walk_all for last failed");

    // Measured: the walk returns the whole session (~1650 events for a 2 s
    // busy loop). An empty walk is a failure, not a reason to skip, and the
    // walk must never invent events either.
    assert!(
        !last_events.is_empty(),
        "the cursor walk over a two-second busy loop must return events, got 0"
    );
    assert!(
        last_events.len() <= stop.total_events as usize,
        "the walk must not invent events: {} walked vs {} reported by stop",
        last_events.len(),
        stop.total_events
    );

    let ts_last = last_events.last().expect("walk is non-empty").timestamp_ns;

    // The walk must end at the *true* tail: the last event it returned has to
    // carry the maximum timestamp of the whole session. This is the assertion
    // that would catch a pagination loop that stops one page early.
    let max_ts = last_events
        .iter()
        .map(|e| e.timestamp_ns)
        .max()
        .expect("walk is non-empty");
    assert_eq!(
        ts_last, max_ts,
        "the last event of the walk must be the highest-timestamped event of the session"
    );

    // A two-second busy loop cannot have identical first and last timestamps.
    assert!(
        ts_last > ts_first,
        "first ({ts_first}) and last ({ts_last}) timestamps of a two-second busy loop must differ"
    );
    println!("Last timestamp: {}", ts_last);

    // State diff
    let diff = client.state_diff(&session_id, ts_first, ts_last).await;

    let result =
        diff.expect("state_diff over two real timestamps of a real session must answer Ok");
    println!(
        "✓ state_diff between {} and {}: {} changes",
        ts_first,
        ts_last,
        result.changes.len()
    );

    // `StateDiffResponse.timestamp_a/b` are echoed from the request when the
    // server omits them (`client/tools.rs`: `v2.timestamp_a.unwrap_or(timestamp_a)`),
    // so these assertions pin "the client passes the two timestamps through
    // untouched", not "the server re-derived them". The real evidence in this
    // test is the walk reaching the tail plus the non-zero temporal span.
    assert_eq!(
        result.timestamp_a, ts_first,
        "state_diff must not alter the first timestamp it was given"
    );
    assert_eq!(
        result.timestamp_b, ts_last,
        "state_diff must not alter the second timestamp it was given"
    );
    assert!(
        result.timestamp_b > result.timestamp_a,
        "the diff window must stay ordered"
    );

    // Empty by construction: a C fixture emits `kind=Unresolved` events whose
    // descriptors are `SyscallEnter`/`SyscallExit`, i.e. no register and no
    // variable evidence, so there is nothing for a state diff to report.
    // Measured: `changes == []`. Asserted as a tripwire for wire drift — a
    // non-empty diff would mean the decoding or the fixture changed.
    assert!(
        result.changes.is_empty(),
        "expected no changes without register or variable evidence, got {:?}",
        result.changes
    );

    client.shutdown().await.ok();
}

/// SD4: test_debug_get_variables_at_valid_event
/// Probe test_add, query the first 5 events, and ask `debug_get_variables`
/// about each one, pinning the *measured* contract of
/// `state_query(kind=variable_snapshot)`: it answers `Ok` with an empty
/// variable list for every event of this fixture.
///
/// Emptiness is the honest expectation, not a weak one, because a C fixture
/// capture emits `kind=Unresolved` events with `SyscallEnter`/`SyscallExit`
/// descriptors — there is no frame with in-scope variables to report. The
/// previous body accepted `Ok` *and* `Err` per event and then printed
/// "All debug_get_variables calls returned valid responses (or expected
/// errors)", which asserted nothing at all.
///
/// Known weakness (measured, deliberately not asserted here): an event id
/// that does not exist also answers `Ok([])`, so this tool cannot currently
/// distinguish "no variables at this event" from "no such event". Reported
/// as a production defect rather than pinned as expected behaviour.
#[tokio::test]
async fn test_debug_get_variables_at_valid_event() {
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

    // Query some events
    let filter = QueryFilter {
        limit: 5,
        offset: 0,
        ..Default::default()
    };
    let events = client
        .query_events(&session_id, filter)
        .await
        .expect("query_events failed");

    println!("Got {} events", events.len());

    // The queried page must be a full, non-empty page of distinct events;
    // otherwise the loop below would assert nothing at all.
    assert_eq!(
        events.len(),
        5,
        "test_add must yield a full page of 5 events, got {}",
        events.len()
    );
    let mut ids: Vec<u64> = events.iter().map(|e| e.event_id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(
        ids.len(),
        events.len(),
        "the queried page must hold 5 distinct event ids, got {ids:?}"
    );

    // Ask for the variables of each queried event: every call must answer Ok,
    // and the list must be empty.
    for event in &events {
        let variables = client
            .debug_get_variables(&session_id, event.event_id)
            .await
            .unwrap_or_else(|e| {
                panic!(
                    "debug_get_variables failed for the real event {} ({}): {e}",
                    event.event_id, event.event_type
                )
            });

        println!(
            "✓ debug_get_variables at event {} ({}): {} vars",
            event.event_id,
            event.event_type,
            variables.len()
        );
        assert!(
            variables.is_empty(),
            "a C fixture capture carries no in-scope variables, so event {} must report none, got {:?}",
            event.event_id,
            variables
        );
    }

    client.shutdown().await.ok();
}

/// SD5: test_evaluate_expression_simple_arithmetic
/// Probe test_add, take the first event id, and evaluate `1 + 2 * 3`
/// against it through the v2 `state_query` dispatcher
/// (`kind=expression_eval`), asserting the real numeric result 7.0.
///
/// The previous body claimed to do this but could not: it invoked
/// `call_with_timeout("state_query", ...)`, which sends a *bare JSON-RPC
/// method* named `state_query`. The MCP server only implements `tools/call`,
/// so the call always failed with `-32601 {"message":"state_query"}` — and
/// because the body accepted that `Err` as "acceptable", the test stayed
/// green while proving nothing. Tools must be reached through `call_tool` (or
/// the `evaluate_expression` helper, which wraps it).
///
/// Measured: `evaluate_expression("1 + 2 * 3")` returns `Ok(7.0)`, and the
/// raw `tools/call` envelope is `{"kind":"expression_eval","result":7.0}`.
#[tokio::test]
async fn test_evaluate_expression_simple_arithmetic() {
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

    // Get an event_id to use
    let filter = QueryFilter {
        limit: 1,
        offset: 0,
        ..Default::default()
    };
    let events = client
        .query_events(&session_id, filter)
        .await
        .expect("query_events failed");

    // No early return: without a real event to scope the expression to, this
    // test cannot evaluate anything and must fail rather than skip.
    assert_eq!(
        events.len(),
        1,
        "the first page of a traced test_add run must contain one event"
    );

    let event_id = events[0].event_id;
    println!("Using event_id: {}", event_id);

    // The v2 envelope, asserted so the dispatcher shape is pinned and not
    // only the helper's convenience return value.
    let params = serde_json::json!({
        "session_id": session_id,
        "kind": "expression_eval",
        "event_id": event_id,
        "expression": "1 + 2 * 3"
    });

    let envelope = client
        .call_tool("state_query", params)
        .await
        .expect("state_query(kind=expression_eval) must be callable through tools/call");
    assert_eq!(
        envelope,
        serde_json::json!({"kind": "expression_eval", "result": 7.0}),
        "state_query(kind=expression_eval) must evaluate 1 + 2 * 3 as 7.0 and report the kind"
    );

    // Same evaluation through the dedicated helper, which unwraps `result`.
    let evaluated = client
        .evaluate_expression(&session_id, "1 + 2 * 3")
        .await
        .expect("evaluate_expression must answer for a real session and event");
    assert_eq!(
        evaluated,
        serde_json::json!(7.0),
        "1 + 2 * 3 must evaluate to 7.0, got {evaluated}"
    );

    println!("✓ state_query(kind=expression_eval): 1 + 2 * 3 = {evaluated}");

    client.shutdown().await.ok();
}
