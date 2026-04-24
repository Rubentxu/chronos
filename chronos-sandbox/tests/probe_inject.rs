//! Integration tests for the probe_inject MCP tool.
//!
//! probe_inject uses eBPF uprobe injection to inject tracing into a running
//! process. It requires root privileges and a loaded eBPF program. In a normal
//! sandbox environment without root, it WILL fail — but the failure should be
//! graceful (not a crash or panic).

use chronos_sandbox::{client::tools::McpTestClient, McpSession};
use std::time::Duration;

// ============================================================================
// I1: test_probe_inject_without_root_returns_error
// ============================================================================

/// Test I1: probe_inject without root returns graceful error.
/// - probe_start on test_busyloop
/// - Sleep 500ms (let it run, get a PID)
/// - probe_inject(session_id, symbol="main", library=None)
/// - Assert: response contains error (either "permission denied", "eBPF not available",
///   "not supported", or similar — NOT a server crash or panic)
/// - probe_stop
#[tokio::test]
async fn test_probe_inject_without_root_returns_error() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Start probe
    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for process to start and get a PID
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Attempt probe_inject (should fail without root/eBPF)
    let result = client.probe_inject_raw(
        &session_id,
        "",  // empty binary_path
        "main",  // symbol
    ).await;

    // Should not crash - either returns Ok with error in response, or Err
    match result {
        Ok(response) => {
            // If Ok, the response should indicate an error in its content
            let response_text = response.to_string();
            let is_graceful_error = response_text.contains("error")
                || response_text.contains("eBPF")
                || response_text.contains("permission")
                || response_text.contains("denied")
                || response_text.contains("not available")
                || response_text.contains("not supported")
                || response_text.contains("CAP_BPF")
                || response_text.contains("EPERM");
            assert!(is_graceful_error,
                "probe_inject should return graceful error without root/eBPF, got: {}",
                response_text);
        }
        Err(_) => {
            // Err is also acceptable - means the MCP layer returned an error
        }
    }

    // Stop probe
    client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    client.shutdown().await.ok();
}

// ============================================================================
// I2: test_probe_inject_nonexistent_session
// ============================================================================

/// Test I2: probe_inject on nonexistent session returns graceful error.
#[tokio::test]
async fn test_probe_inject_nonexistent_session() {
    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Call probe_inject on nonexistent session
    let result = client.probe_inject_raw(
        "nonexistent-session-xyz",
        "/some/path.so",
        "foo",
    ).await;

    // Should not crash - should return an error
    match result {
        Ok(response) => {
            let response_text = response.to_string();
            let is_graceful_error = response_text.contains("not found")
                || response_text.contains("not found")
                || response_text.contains("Start a probe");
            assert!(is_graceful_error,
                "probe_inject on nonexistent session should return graceful error, got: {}",
                response_text);
        }
        Err(_) => {
            // Err is also acceptable
        }
    }

    client.shutdown().await.ok();
}

// ============================================================================
// I3: test_probe_inject_invalid_symbol
// ============================================================================

/// Test I3: probe_inject with empty symbol returns error.
/// - probe_start on test_busyloop
/// - Sleep 500ms
/// - probe_inject(session_id, symbol="", library=None)
/// - Assert: error (empty symbol is invalid)
/// - probe_stop
#[tokio::test]
async fn test_probe_inject_invalid_symbol() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Start probe
    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for process to start
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Attempt probe_inject with empty symbol
    let result = client.probe_inject_raw(
        &session_id,
        fixture.to_str().unwrap(),
        "",  // empty symbol - invalid
    ).await;

    // Should not crash
    match result {
        Ok(response) => {
            let response_text = response.to_string();
            // Should get some kind of error
            let is_error = response_text.contains("error")
                || response_text.contains("eBPF")
                || response_text.contains("failed")
                || response_text.contains("Failed")
                || response_text.contains("not found");
            assert!(is_error,
                "probe_inject with empty symbol should return error, got: {}",
                response_text);
        }
        Err(_) => {
            // Err is also acceptable
        }
    }

    // Stop probe
    client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    client.shutdown().await.ok();
}

// ============================================================================
// I4: test_probe_inject_before_pid_known
// ============================================================================

/// Test I4: probe_inject immediately after probe_start may get "PID not yet known" error.
/// - probe_start on test_busyloop
/// - Immediately (no sleep) probe_inject
/// - Assert: either "PID not yet known, retry" error OR normal eBPF error — NOT a crash
#[tokio::test]
async fn test_probe_inject_before_pid_known() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Start probe
    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Immediately try probe_inject without waiting (PID may not be known yet)
    let result = client.probe_inject_raw(
        &session_id,
        fixture.to_str().unwrap(),
        "main",
    ).await;

    // Should not crash - should get either PID retry message or eBPF error
    match result {
        Ok(response) => {
            let response_text = response.to_string();
            let is_graceful = response_text.contains("PID not yet known")
                || response_text.contains("retry")
                || response_text.contains("error")
                || response_text.contains("eBPF")
                || response_text.contains("permission")
                || response_text.contains("denied")
                || response_text.contains("not available")
                || response_text.contains("Failed")
                || response_text.contains("failed");
            assert!(is_graceful,
                "probe_inject immediately should return graceful error or PID retry message, got: {}",
                response_text);
        }
        Err(_) => {
            // Err is also acceptable
        }
    }

    // Stop probe
    client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    client.shutdown().await.ok();
}
