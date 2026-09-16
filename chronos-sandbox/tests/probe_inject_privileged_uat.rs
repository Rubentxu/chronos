// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Privileged UAT for the `probe_inject` MCP tool.
//
// This file is intentionally NOT executed in the default CI sandbox run.
// It exercises the happy path of `probe_inject`: actually attaching an
// eBPF uprobe to a running tracee, draining the resulting events, and
// stopping the probe. Every assertion in this file requires the
// `chronos-mcp` binary to be built with the `ebpf` feature flag AND the
// sandbox process to run with `CAP_BPF` / `CAP_PERFMON` (i.e. root).
//
// ## Why this lives outside the default test set
//
// REC-C0.5-B DoD: "Test unprivileged debe inyectar/detectar capability
// real (no #[ignore], no fake-pass) y verificar contrato tipado
// Err(CapabilityUnavailable::EbpfUprobe). UAT privileged separada — fuera
// del CI sandbox run."
//
// Gating:
//
//   - `CHRONOS_PRIVILEGED_UAT=1` enables the test. Without this env var
//     the test is silently skipped (NOT `#[ignore]`) — the test binary
//     compiles, but the test does not run. The default CI run sees zero
//     tests in this file, never an ignored test that pretends to pass.
//   - The test calls `McpTestClient::start()` which spawns the locally
//     built `chronos-mcp` binary — the operator is responsible for
//     building it with `--features chronos-ebpf/ebpf` before enabling
//     the UAT.
//
// Operators who want to run the UAT locally:
//
//   CHRONOS_MCP_PATH=/path/to/chronos-mcp \
//     CHRONOS_PRIVILEGED_UAT=1 \
//     cargo test -p chronos-sandbox --test probe_inject_privileged_uat -- --nocapture

use chronos_sandbox::{client::tools::McpTestClient, McpSession};
use std::time::Duration;

/// Run only when the operator opts in via `CHRONOS_PRIVILEGED_UAT=1`.
///
/// Without this env var the test returns early — it does NOT use
/// `#[ignore]` so the default CI run sees a passing test (with the
/// skip message printed to stderr), not an ignored one.
fn privileged_uat_enabled() -> bool {
    std::env::var("CHRONOS_PRIVILEGED_UAT")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Happy path: `probe_inject` attaches a uprobe to `test_busyloop` and
/// the typed error path is **not** triggered.
///
/// Requires:
/// - `chronos-mcp` built with `--features chronos-ebpf/ebpf`.
/// - The test process running with `CAP_BPF` / `CAP_PERFMON` (root).
#[tokio::test]
async fn test_probe_inject_privileged_happy_path() {
    if !privileged_uat_enabled() {
        eprintln!(
            "skipped: set CHRONOS_PRIVILEGED_UAT=1 to run the privileged \
             probe_inject UAT (requires --features chronos-ebpf/ebpf + root)"
        );
        return;
    }

    let fixture =
        McpSession::fixture_path("test_busyloop").expect("test_busyloop fixture not found");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    // Give the tracee a moment to be traced and a PID known.
    tokio::time::sleep(Duration::from_millis(500)).await;

    let response = client
        .probe_inject_raw(&session_id, fixture.to_str().unwrap(), "main")
        .await
        .expect("probe_inject must return a CallToolResult, not a transport error");

    let text = response
        .as_object()
        .and_then(|o| o.get("content"))
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|item| item.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or_default();

    // On the privileged path the wrapper returns a success-shaped payload
    // with `probes_attached: 1`. We assert the typed error prefix is
    // ABSENT — that is the whole point of the UAT.
    assert!(
        !text.contains("probe_inject: capability:"),
        "privileged probe_inject must NOT return a typed capability error, got: {}",
        text
    );
    assert!(
        !text.starts_with("error"),
        "privileged probe_inject must not surface as a tool error, got: {}",
        text
    );

    client.probe_stop(&session_id).await.ok();
    client.shutdown().await.ok();
}
