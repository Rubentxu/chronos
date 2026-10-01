//! Error handling and resilience tests for the Chronos MCP server.
//!
//! These tests verify that the MCP server gracefully handles error conditions
//! such as invalid session IDs, out-of-range event IDs, and operations on
//! non-existent sessions.
//!
//! Category A tests cover error handling and resilience.

use chronos_sandbox::client::error::McpSandboxError;
use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::McpSession;
use std::time::Duration;

/// A2: Verify probe_stop returns an error for nonexistent session.
///
/// Calls probe_stop with session_id = "nonexistent-session-xyz" and asserts
/// that the call fails with an RPC error naming that session, rather than
/// returning a successful response or dropping the connection.
#[tokio::test]
async fn test_probe_stop_nonexistent_session() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Call probe_stop with a session that doesn't exist
    let result = client.probe_stop("nonexistent-session-xyz").await;

    // The unknown session must be reported as an error. Accepting a success
    // response here would make this test unable to detect a regression that
    // silently pretends the session exists.
    let err = result.expect_err("probe_stop should reject a nonexistent session with an error");

    assert!(
        matches!(&err, McpSandboxError::RpcError(_)),
        "expected an RPC error, got: {:?}",
        err
    );
    assert!(
        err.to_string().contains("nonexistent-session-xyz"),
        "the error should name the unknown session, got: {}",
        err
    );

    client.shutdown().await.ok();
}

/// A3: Verify probe_drain returns an error for nonexistent session.
///
/// Calls probe_drain with session_id = "nonexistent-session-xyz" and asserts
/// that the call fails with an RPC error naming that session.
#[tokio::test]
async fn test_probe_drain_nonexistent_session() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Call probe_drain with a session that doesn't exist
    let result = client.probe_drain_raw("nonexistent-session-xyz").await;

    // Draining an unknown session must be an error, never an empty success.
    let err = result.expect_err("probe_drain should reject a nonexistent session with an error");

    assert!(
        matches!(&err, McpSandboxError::RpcError(_)),
        "expected an RPC error, got: {:?}",
        err
    );
    assert!(
        err.to_string().contains("nonexistent-session-xyz"),
        "the error should name the unknown session, got: {}",
        err
    );

    client.shutdown().await.ok();
}

/// A4: Verify query_events returns an error for invalid session.
///
/// Calls query_events with session_id = "invalid-does-not-exist" and asserts
/// that the call fails with an RPC error naming that session. An empty event
/// list is not acceptable: the server has no log for the session and must say so.
#[tokio::test]
async fn test_query_events_invalid_session() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let filter = chronos_sandbox::client::types::QueryFilter::default();
    let result = client.query_events("invalid-does-not-exist", filter).await;

    // An unknown session is an error, not an empty result: reporting zero
    // events would be indistinguishable from a real session with no events.
    let err = result.expect_err("query_events should reject an unknown session with an error");

    assert!(
        matches!(&err, McpSandboxError::RpcError(_)),
        "expected an RPC error, got: {:?}",
        err
    );
    assert!(
        err.to_string().contains("ExecutionLog unavailable"),
        "the error should report the missing execution log, got: {}",
        err
    );
    assert!(
        err.to_string().contains("invalid-does-not-exist"),
        "the error should name the unknown session, got: {}",
        err
    );

    client.shutdown().await.ok();
}

/// A5: Verify get_event returns error for out-of-range event ID.
///
/// Starts a real probe, stops it to get a real session with events,
/// then calls get_event with event_id = 999999 (which shouldn't exist).
#[tokio::test]
async fn test_get_event_out_of_range() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Start probe on a real program
    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    // Wait for some events
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Drain events
    let _events = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");

    // Stop the probe to finalize the session
    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");

    assert!(
        stop.total_events > 0,
        "the probe should have captured events before querying an out-of-range id, got 0"
    );

    // Give the query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Call get_event with an event ID that's definitely out of range.
    //
    // v2 contract (chronos-services/src/events_read.rs::read_event_by_id):
    //   Returns Ok(EventsReadOutput::ById { event: None, ... })
    //   when the event id does not exist. `event: None` is serialized as
    //   JSON `null` by serde (Option<None> → null), NOT omitted.
    //
    // chronos-sandbox/src/client/tools.rs::get_event flattens the envelope:
    //   let flattened = response.get("event").cloned()
    //       .unwrap_or_else(|| serde_json::json!({}));
    // `response.get("event")` returns Some(Null) (the field is present but
    // null), so the fallback does NOT trigger and flattened = Value::Null.
    //
    // Migration rationale (R9.1, drift #6): pre-C5.3.1 v1 server returned
    // an error envelope; v2 server returns Ok with event:null. The test
    // must accept the v2 contract.
    let result = client.get_event(&session_id, 999999).await;

    match result {
        Ok(value) => {
            // v2 contract: out-of-range event id returns Ok(Value::Null).
            // The client flattens the `event` field directly without
            // substituting a fallback object, so missing events surface
            // as JSON null (NOT an empty object).
            assert!(
                value.is_null(),
                "Expected JSON null for missing event id, got: {}",
                value
            );
        }
        Err(e) => {
            // Future-proofing: if a future server variant returns an error,
            // surface the change loudly instead of silently passing.
            panic!(
                "Unexpected Err for out-of-range event id; v2 server must \
                 return Ok with event:null per EventsReadOutput::ById. Error: {}",
                e
            );
        }
    }

    client.shutdown().await.ok();
}

/// A7: Verify save_then_delete_then_load workflow.
///
/// Starts a probe, stops it, saves the session, deletes it, then verifies
/// that load_session returns an error for the deleted session.
#[tokio::test]
async fn test_save_then_delete_then_load() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Start probe
    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    // Wait for some events
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Drain events
    let _events = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");

    // Stop the probe
    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");

    assert!(stop.total_events > 0, "Should have captured some events");

    // Give the query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Save the session
    let save_result = client
        .save_session(&session_id, "test_save_delete_load")
        .await
        .expect("save_session failed");

    assert_eq!(
        save_result.status, "saved",
        "save_session should report status 'saved', got: {}",
        save_result.status
    );
    assert!(
        save_result.event_count > 0,
        "save_session should persist the captured events, got 0"
    );

    // Delete the session
    client
        .delete_session(&session_id)
        .await
        .expect("delete_session failed");

    // Try to load the deleted session - should fail
    let load_result = client.load_session(&session_id).await;

    // A deleted session must not load. Returning session info here would mean
    // the delete did not take effect and the test would pass while broken.
    let err = load_result.expect_err("load_session should fail for a deleted session");

    assert!(
        matches!(&err, McpSandboxError::RpcError(_)),
        "expected an RPC error, got: {:?}",
        err
    );
    assert!(
        err.to_string().contains("session not found"),
        "the error should report the session as not found, got: {}",
        err
    );

    client.shutdown().await.ok();
}

/// A9: Verify list_threads returns graceful error for invalid session.
///
/// Calls list_threads with session_id = "ghost-session" and asserts
/// that it fails with an RPC error naming that session, not a crash.
#[tokio::test]
async fn test_list_threads_invalid_session() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Call list_threads with a session that doesn't exist
    let result = client.list_threads("ghost-session").await;

    // An unknown session has no execution log to summarise, so the server
    // must report an error instead of an empty thread list.
    let err = result.expect_err("list_threads should reject an unknown session with an error");

    assert!(
        matches!(&err, McpSandboxError::RpcError(_)),
        "expected an RPC error, got: {:?}",
        err
    );
    assert!(
        err.to_string().contains("ghost-session"),
        "the error should name the unknown session, got: {}",
        err
    );

    client.shutdown().await.ok();
}

/// A10: Verify probe_start rejects an empty program path.
///
/// Calls probe_start with program = "" and asserts that it fails with an
/// "Invalid program path" error. The empty string is rejected by the
/// non-absolute path validation, which is the check that fires first.
#[tokio::test]
async fn test_probe_start_empty_path() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Call probe_start with empty program path
    let result = client.probe_start_raw("").await;

    // The empty path must be rejected outright; spawning it cannot succeed.
    let err = result.expect_err("probe_start should reject an empty program path with an error");

    assert!(
        matches!(&err, McpSandboxError::RpcError(_)),
        "expected an RPC error, got: {:?}",
        err
    );
    let message = err.to_string();
    assert!(
        message.contains("Invalid program path"),
        "the error should report an invalid program path, got: {}",
        message
    );
    assert!(
        message.contains("Non-absolute path rejected"),
        "an empty path should be rejected as non-absolute, got: {}",
        message
    );

    client.shutdown().await.ok();
}

/// A10b: Verify probe_start returns error for nonexistent binary path.
///
/// Calls probe_start with program = "/nonexistent/path/to/binary" and asserts
/// that it fails with an "Invalid program path" error reporting the missing
/// program.
#[tokio::test]
async fn test_probe_start_nonexistent_path() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Call probe_start with a path that doesn't exist
    let result = client.probe_start_raw("/nonexistent/path/to/binary").await;

    // An absolute but missing binary must be rejected with a not-found error.
    let err =
        result.expect_err("probe_start should reject a nonexistent binary path with an error");

    assert!(
        matches!(&err, McpSandboxError::RpcError(_)),
        "expected an RPC error, got: {:?}",
        err
    );
    let message = err.to_string();
    assert!(
        message.contains("Invalid program path"),
        "the error should report an invalid program path, got: {}",
        message
    );
    assert!(
        message.contains("Program not found"),
        "the error should report the program as not found, got: {}",
        message
    );
    assert!(
        message.contains("/nonexistent/path/to/binary"),
        "the error should name the missing binary, got: {}",
        message
    );

    client.shutdown().await.ok();
}
