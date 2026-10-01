//! Variable tools tests — verify debug_get_variables and evaluate_expression
//! work correctly after probe_stop.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::McpSession;
use std::time::Duration;

#[tokio::test]
async fn test_debug_get_variables_empty_session() {
    // debug_get_variables requires Python frame events with local variables.
    // Native C programs don't produce these, so we expect empty results.
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

    println!("Probe stopped: {} total events", stop.total_events);

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Try to get variables at event 0 - will be empty for C programs
    let variables = client
        .debug_get_variables(&session_id, 0)
        .await
        .expect("debug_get_variables failed");

    // The comment above already said what the result must be: a C program
    // produces no Python-style frame events, so there are no variables in
    // scope. That was printed, not asserted, so the test passed either way.
    assert!(
        variables.is_empty(),
        "a native C fixture produces no frame locals, so event 0 must yield no \
         variables; got {:?}",
        variables
            .iter()
            .map(|v| (&v.name, &v.type_name))
            .collect::<Vec<_>>()
    );

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_debug_get_variables_out_of_range() {
    // An event id that was never captured is not an event without variables.
    // This test used to assert the opposite — an empty list — which made a
    // typo in the id indistinguishable from a real absence. `get_registers`
    // already answered "event N not found" for the same input; this now matches.
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
        "the probe should have captured events, got 0"
    );

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    let err = client
        .debug_get_variables(&session_id, 999999)
        .await
        .expect_err("an event id that was never captured must not answer with an empty list");

    let message = err.to_string();
    assert!(
        message.contains("event 999999 not found"),
        "the error should name the missing event, got: {}",
        message
    );

    // The neighbouring case keeps its answer: event 0 exists, and this C fixture
    // gives it no frame data. That must stay an empty list, not an error —
    // otherwise "no variables" and "no such event" would be swapped rather than
    // separated.
    let variables = client
        .debug_get_variables(&session_id, 0)
        .await
        .expect("an existing event without frame data must answer with an empty list");
    assert!(
        variables.is_empty(),
        "test_add event 0 carries no frame data, so no variables are expected; got {}",
        variables.len()
    );

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_evaluate_expression_empty_session() {
    // evaluate_expression requires Python frame events with local variables.
    // Native C programs don't produce these.
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

    println!("Probe stopped: {} total events", stop.total_events);

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Try to evaluate an expression - will fail because no Python frames with locals
    let result = client.evaluate_expression(&session_id, "x + y").await;

    // Measured, not assumed: a C fixture has no variable named `x`, and the
    // evaluation error is carried as a *value*, not as a transport error --
    // `DebugReadService::evaluate_expression` maps it to
    // `EvalResult::Error`. The old body accepted both branches, so it could
    // not tell a working evaluator from a broken one.
    let rendered = match &result {
        Ok(serde_json::Value::String(s)) => s.clone(),
        other => panic!(
            "a C fixture has no `x`, so evaluate_expression must return the \
             evaluation error as a string value; got {other:?}"
        ),
    };
    assert!(
        rendered.contains("UnknownVariable"),
        "the error must name the unknown variable so the caller can act on it; \
         got {rendered:?}"
    );

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_evaluate_expression_invalid_expression() {
    // Test evaluate_expression with an invalid expression
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

    println!("Probe stopped: {} total events", stop.total_events);

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Try to evaluate a syntactically invalid expression
    let result = client.evaluate_expression(&session_id, "1 +++ 1").await;

    // The old comment read "Might succeed" / "Also acceptable", which is a
    // test that cannot fail. Measured: a malformed expression is reported as
    // the parse error inside the returned string, and that is the only
    // outcome that distinguishes a parser from a stub.
    //
    // Note the expression carries no identifiers. `x +++ y` would not do: the
    // evaluator resolves variables as it goes, so it reports
    // UnknownVariable("x") and never reaches the `+++`. That ordering is
    // pinned separately below, because "the first error wins" is a real part
    // of the contract and it is why the two cases differ.
    let rendered = match &result {
        Ok(serde_json::Value::String(s)) => s.clone(),
        other => panic!("`1 +++ 1` cannot parse, so evaluate_expression must return the parse error as a string value; got {other:?}"),
    };
    assert!(
        rendered.contains("InvalidNumber"),
        "the parse failure must be reported as such; got {rendered:?}"
    );

    // The first error wins: an unknown identifier is reported before the
    // malformed arithmetic is ever reached.
    let mixed = client.evaluate_expression(&session_id, "x +++ y").await;
    let mixed = match &mixed {
        Ok(serde_json::Value::String(s)) => s.clone(),
        other => panic!("`x +++ y` must report its first error as a string; got {other:?}"),
    };
    assert!(
        mixed.contains("UnknownVariable"),
        "identifier resolution precedes arithmetic parsing, so the unknown \
         variable is what gets reported; got {mixed:?}"
    );

    client.shutdown().await.ok();
}

/// The other half the file never had: proof that `evaluate_expression`
/// actually *evaluates*.
///
/// Every other test in this file fed it something that cannot succeed, and
/// each one accepted whatever came back. Nothing ever asked for a number and
/// checked it arrived -- so the evaluator could return an error for every
/// input, forever, and the file stayed green.
///
/// That is not hypothetical. `state_query`'s `expression_eval` payload was
/// flattened from a single-field `#[serde(untagged)]` enum, which serialises
/// to nothing, so the whole payload went out as `{}` and this client's
/// deserialiser failed on it every single time. Fixed in
/// chronos-services::output; this is the test that would have caught it.
#[tokio::test]
async fn test_evaluate_expression_computes_arithmetic() {
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(1)).await;
    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");
    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");
    tokio::time::sleep(Duration::from_millis(200)).await;

    // The capture must have produced something, or "the evaluator works" would
    // be vacuous on a session that never ran.
    assert!(
        stop.total_events > 0,
        "the fixture must have been captured before evaluating anything"
    );

    for (expression, expected) in [("1 + 1", 2.0), ("2 * 3", 6.0), ("10 - 4", 6.0)] {
        let value = client
            .evaluate_expression(&session_id, expression)
            .await
            .unwrap_or_else(|e| panic!("`{expression}` must evaluate: {e:?}"));
        assert_eq!(
            value.as_f64(),
            Some(expected),
            "`{expression}` must evaluate to {expected}; got {value:?}"
        );
    }

    client.shutdown().await.ok();
}
