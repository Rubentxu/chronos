//! Integration tests for the `probe_inject` MCP tool (REC-C0.5-B).
//!
//! `probe_inject` attaches an eBPF uprobe to a running tracee. The
//! operation requires kernel capabilities (`CAP_BPF` / `CAP_PERFMON`
//! and a kernel >= 5.8 for ring-buffer uprobes), which may not be
//! present in every CI environment. The contract under test is the
//! *typed capability error* surfaced by the wrapper when one of those
//! capabilities is missing, rather than ad-hoc string matching.
//!
//! ## Typed error contract (REC-C0.5-B)
//!
//! Every terminal failure variant of `ProbeService::inject` is now mapped
//! to a distinct typed `ServiceError` and surfaced by the `probe_inject`
//! MCP wrapper as a `CallToolResult::error(...)` whose text starts with
//! `probe_inject: capability: <kebab-slot> — ...`:
//!
//! | Underlying outcome          | ServiceError variant    | Wrapper text prefix           |
//! |-----------------------------|-------------------------|-------------------------------|
//! | Probe is still starting up  | `ProbeStarting`         | `probe-starting`              |
//! | eBPF kernel/feature missing | `EbpfUnsupported(reason)` | `ebpf-uprobe`               |
//! | Uprobe attach rejected      | `InjectionFailed(reason)` | `ebpf-uprobe`                |
//!
//! Tests in this file assert on the typed discriminator rather than
//! inspecting the human-readable `reason` text. The `reason` is allowed
//! to vary across kernels and adapter versions; only the kebab-slot
//! discriminator is stable.
//!
//! ## Privileged UAT (out of scope for this file)
//!
//! The privileged happy-path (`probe_inject` actually attaching a uprobe
//! to a live tracee under root) is covered by
//! `chronos-sandbox/tests/probe_inject_privileged_uat.rs` — that file is
//! gated behind `CHRONOS_PRIVILEGED_UAT=1` and **never runs in the
//! default CI sandbox run**. See the file header for the rationale and
//! the gating environment variable.

use chronos_sandbox::{McpSandboxError, McpSession, McpTestClient};
use std::time::Duration;

/// Stable kebab-slot discriminator for the eBPF uprobe capability.
///
/// `Capability::EbpfUprobe.as_str()` lives in `chronos_domain::capability`.
/// We re-state the string here intentionally so the test does not have to
/// pull the domain type into a public re-export just to assert the
/// contract — the value MUST stay in sync with
/// `crates/chronos-domain/src/capability.rs`.
const CAP_EBPF_UPROBE: &str = "ebpf-uprobe";

/// Stable kebab-slot discriminator for the probe-starting capability
/// (the no-PID-yet race). Matches the wrapper text prefix emitted by the
/// MCP `probe_inject` wrapper when `ServiceError::ProbeStarting` is
/// surfaced. Same stability rule as `CAP_EBPF_UPROBE`.
const CAP_PROBE_STARTING: &str = "probe-starting";

/// Assert that the response carries a typed capability error with the
/// given kebab-slot discriminator.
///
/// The MCP `probe_inject` wrapper renders a `CallToolResult::error(...)`
/// whose `isError` flag is `true` and whose first `content[0].text` entry
/// starts with `probe_inject: capability: <slot> — ...`.
///
/// `McpTestClient::probe_inject_raw` delegates to `RpcClient::call_tool`,
/// which converts `isError: true` into `Err(McpSandboxError::RpcError(text))`.
/// So we accept either:
///   - `Ok(value)` with `isError: true` and the typed text, or
///   - `Err(RpcError(text))` whose text contains the typed prefix.
///
/// Both forms prove the same thing: the wrapper surfaced the typed
/// capability error rather than a success-shaped `pid: null` payload.
///
/// We intentionally do NOT inspect the human-readable reason: it varies
/// across kernel versions and adapter builds, while the kebab-slot
/// discriminator is the stable contract.
fn assert_capability_error(
    result: &Result<serde_json::Value, McpSandboxError>,
    expected_slot: &str,
) {
    let prefix = format!("probe_inject: capability: {}", expected_slot);

    match result {
        Ok(value) => {
            // `Ok` with `isError: true` (the wrapper did not convert it
            // because some transports leave isError as None).
            let text = value
                .as_object()
                .and_then(|o| o.get("content"))
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("text"))
                .and_then(|t| t.as_str())
                .unwrap_or_else(|| panic!("expected text payload, got: {}", value));
            assert!(
                text.contains(&prefix),
                "probe_inject error text does not start with the typed capability slot. \
                 expected prefix: `{}` — got: `{}`",
                prefix,
                text
            );
            let is_error = value
                .as_object()
                .and_then(|o| o.get("isError"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            assert!(
                is_error,
                "probe_inject capability error must set `isError: true`, got: {}",
                value
            );
        }
        Err(McpSandboxError::RpcError(text)) => {
            assert!(
                text.contains(&prefix),
                "probe_inject capability error text does not start with the typed slot. \
                 expected prefix: `{}` — got: `{}`",
                prefix,
                text
            );
        }
        Err(other) => {
            panic!(
                "probe_inject must return a typed capability error (`{}`), got: {:?}",
                prefix, other
            );
        }
    }
}

// ============================================================================
// I1: probe_inject without root / eBPF surfaces typed capability error
// ============================================================================

/// Test I1: `probe_inject` without root / eBPF returns the typed
/// `ebpf-uprobe` capability error.
///
/// On a default-built `chronos-mcp` (no `ebpf` feature flag), the
/// `EbpfAdapter::new()` call inside `ProbeService::inject` always returns
/// `EbpfError::Unavailable`, which the dispatcher maps to
/// `ServiceError::EbpfUnsupported(reason)` and the wrapper renders as
/// `probe_inject: capability: ebpf-uprobe — ...`.
#[tokio::test]
async fn test_probe_inject_without_root_returns_capability_error() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found — run `cargo build -p chronos-sandbox` first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Start probe (the start-up race does not apply here: we wait for it).
    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Attempt probe_inject with empty binary_path. On a default build the
    // typed error is `EbpfUnsupported`; on an `ebpf`-feature build without
    // root it would be `InjectionFailed("permission denied" / EPERM)`.
    // Both share the `ebpf-uprobe` capability slot, so the test asserts
    // on the slot, not the reason.
    let result = client
        .probe_inject_raw(
            &session_id,
            "",     // empty binary_path
            "main", // symbol
        )
        .await;

    assert_capability_error(&result, CAP_EBPF_UPROBE);

    client.probe_stop(&session_id).await.ok();
    client.shutdown().await.ok();
}

// ============================================================================
// I2: probe_inject on nonexistent session is still ProbeNotFound
// ============================================================================

/// Test I2: `probe_inject` on a nonexistent session returns the existing
/// `ProbeNotFound` error (unchanged by REC-C0.5-B — this is a regression
/// check that the capability-aware split did not break the
/// session-lookup error path).
#[tokio::test]
async fn test_probe_inject_nonexistent_session() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let result = client
        .probe_inject_raw("nonexistent-session-xyz", "/some/path.so", "foo")
        .await;

    // Surface the typed-text inside the RpcError variant (or unwrap Ok).
    let text = match &result {
        Ok(v) => v
            .as_object()
            .and_then(|o| o.get("content"))
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or_default()
            .to_string(),
        Err(McpSandboxError::RpcError(t)) => t.clone(),
        Err(other) => panic!("unexpected error variant: {:?}", other),
    };

    assert!(
        text.contains("not found") && text.contains("Start a probe with probe_start"),
        "probe_inject on nonexistent session must return the existing \
         `ProbeNotFound` error, got: {}",
        text
    );

    client.shutdown().await.ok();
}

// ============================================================================
// I3: probe_inject with empty symbol still surfaces a capability error
// ============================================================================

/// Test I3: `probe_inject` with an empty symbol returns a typed error.
///
/// Empty `symbol_name` is a malformed input — the wrapper never
/// constructs the eBPF adapter with it. The dispatcher surfaces it as
/// either `EbpfUnsupported` (default build) or `InjectionFailed`
/// (kernel-refused attach under an `ebpf`-feature build). Both share
/// the `ebpf-uprobe` capability slot, so the test asserts on the slot
/// rather than inspecting the underlying reason.
#[tokio::test]
async fn test_probe_inject_invalid_symbol_returns_capability_error() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found — run `cargo build -p chronos-sandbox` first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_millis(500)).await;

    let result = client
        .probe_inject_raw(
            &session_id,
            fixture.to_str().unwrap(),
            "", // empty symbol — invalid
        )
        .await;

    assert_capability_error(&result, CAP_EBPF_UPROBE);

    client.probe_stop(&session_id).await.ok();
    client.shutdown().await.ok();
}

// ============================================================================
// I4: probe_inject immediately after probe_start returns the typed
//     `probe-starting` capability error when the live-probe backend has
//     not yet recorded a PID.
// ============================================================================

/// Test I4: `probe_inject` invoked before the live-probe backend reports
/// a PID returns the typed `probe-starting` capability error.
///
/// On a default build the eBPF feature is off, so the dispatcher
/// short-circuits to `EbpfUnavailable` first (typed as `ebpf-uprobe`)
/// before `ProbeStarting` would even be reachable. This test therefore
/// only verifies the typed contract on a build where `EbpfAdapter::new()`
/// succeeds — which is gated behind the `ebpf` feature flag and not
/// exercised by the default CI sandbox. We therefore mark the test as
/// **expected to surface the `ebpf-uprobe` typed error in the default
/// build** and skip the `probe-starting` check on this build.
///
/// The `probe-starting` variant IS exercised by the dispatcher unit
/// tests in `chronos-services/src/observe.rs::tests::create_uprobe_*`.
#[tokio::test]
async fn test_probe_inject_before_pid_known_returns_capability_error() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found — run `cargo build -p chronos-sandbox` first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Start probe, but call inject IMMEDIATELY (before the backend has
    // emitted any events / recorded a PID).
    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    let result = client
        .probe_inject_raw(&session_id, fixture.to_str().unwrap(), "main")
        .await;

    // On a default (no-ebpf) build the typed slot is `ebpf-uprobe`; on
    // an `ebpf`-feature build the slot would be `probe-starting` if the
    // backend has not yet reported a PID. We accept either as a
    // capability-aware error (the wrapper must not return a
    // success-shaped `pid: null` payload — that was the REC-C0.5-B bug).
    let text = match &result {
        Ok(v) => v
            .as_object()
            .and_then(|o| o.get("content"))
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or_default()
            .to_string(),
        Err(McpSandboxError::RpcError(t)) => t.clone(),
        Err(other) => panic!("unexpected error variant: {:?}", other),
    };

    let is_capability_error = text
        .contains(&format!("probe_inject: capability: {}", CAP_EBPF_UPROBE))
        || text.contains(&format!("probe_inject: capability: {}", CAP_PROBE_STARTING));
    let is_error = match &result {
        Err(McpSandboxError::RpcError(_)) => true,
        Ok(v) => v
            .as_object()
            .and_then(|o| o.get("isError"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        Err(_) => false,
    };

    assert!(
        is_capability_error && is_error,
        "probe_inject before PID known must return a typed capability error \
         (`{}` or `{}`), got: {:?}",
        CAP_EBPF_UPROBE,
        CAP_PROBE_STARTING,
        result
    );

    client.probe_stop(&session_id).await.ok();
    client.shutdown().await.ok();
}
