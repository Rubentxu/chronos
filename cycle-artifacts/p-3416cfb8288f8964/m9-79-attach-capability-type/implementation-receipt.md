# Implementation Receipt — m9-79-attach-capability-type

**Commit**: 917fcb036a56d6051af6429dd09225e7d0da4fe6 (feat/m9-79-attach-capability-type, base 009b750)
**Branch**: feat/m9-79-attach-capability-type

## Changes

Distinguish the ptrace attach path in `ChronosSessionLifecycleService::attach`
capability snapshots. Before this cycle every session, including those produced
by `session_start{action=attach}`, reported `probe_type: "ebpf_user"` — a value
that describes an eBPF user-probe, which is not what an attach session actually
runs. The new value is `probe_type: "ptrace_attach"`.

| File | Diff |
|---|---|
| `crates/chronos-services/src/session_lifecycle.rs` | `probe_type: Some("ebpf_user".to_string())` → `Some("ptrace_attach".to_string())` in the `attach` arm of `start` (`session_lifecycle.rs:198`). |
| `chronos-sandbox/tests/session_lifecycle.rs` | The `test_session_start_attach_to_running_self` assertion updated to expect `Some("ptrace_attach")` (line 376). |
| `docs/manual-ai/en/08-session-management.md` | Attach example now states the snapshot carries `probe_type: "ptrace_attach"`. |
| `docs/manual-ai/es/08-gestion-sesiones.md` | Spanish mirror of the same sentence. |

No public API surface change: `CapabilitySnapshot::probe_type` is `Option<String>`
and downstream readers compare it as a string. The single literal that changed
is documented in both manuals.

## Gates (orchestrator-verified 2026-09-14T06:25Z)

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS (0 warnings, 0 errors) |
| `cargo test -p chronos-services --lib --no-fail-fast` | PASS (264/264) |
| T4 `e2e_connectivity` (`--test-threads=1`) | PASS (1/1) |
| T4 `session_lifecycle::test_session_start_attach_to_running_self` (`--test-threads=1`) | PASS (1/1, 15.26 s) |

`CHRONOS_MCP_PATH=/var/home/rubentxu/cargo-targets/debug/chronos-mcp` is set
explicitly; the binary's mtime predates the cycle but the change lives entirely
in `chronos-services`, which the test client relinks.

## Acceptance mapping

The cycle has a single observable change: the literal string reported in
`probe_type` after a successful `session_start{action=attach}`. The assertion
in `test_session_start_attach_to_running_self` was updated to assert exactly
that literal, and the test ran green against the rebuilt library.

The attached-process safety behaviour from m9-78 (SIGSTOP + PTRACE_DETACH,
target remains alive after `session_stop`) is exercised by the same focused
test: it spawns `sleep 30`, attaches, calls `session_stop`, and asserts the
child PID is still in `TASK_RUNNING` after the call.

## Deviations

None. The cycle is a one-literal rename with matching test, doc, and manual
updates.