# m7-06 — session_start + session_stop + capabilities sandbox smoke merge

**Branch:** `feat/m7-06-session-lifecycle-sandbox-smoke-merge`
**Cycle:** M7 (v2-spec sub-cycle), sixth deliverable
**Precedence:** `docs/milestones/m7-06-session-lifecycle-sandbox-smoke-scoping.md` (parent); `docs/milestones/m7-05-session-lifecycle-dispatchers-merge.md`
**Status:** PROPOSED — 2026-09-11

## Why this cycle

This is the **executor** for the m7-06 scope: T4 sandbox smoke + MCP integration tests for the v2 lifecycle surface, plus the M7 close report. It closes out the M7 sub-cycle.

After m7-06 ships, the M7 backlog reduces to:
- **v1 sunset bookkeeping** (passive; no work until 2027-09-11).
- **m7-07+ follow-ups** (`single-call session_stop`, `attach` domain API, etc.).

## Scope (this cycle)

- Sandbox client methods (`chronos-sandbox/src/client/{types,tools}.rs`).
- Sandbox client v2 response types (`SessionStartResponse`, `SessionStopResponse`, `CapabilitiesResponse`).
- MCP integration tests (`crates/chronos-mcp/tests/session_lifecycle_tools.rs`, 9 tests).
- 3 sandbox suite extensions (`probe_lifecycle.rs` + `session_lifecycle.rs` + `program_scenarios.rs`).
- M7 close report (`docs/milestones/m7-close-report.md`).

## Test plan

### MCP integration tests (`crates/chronos-mcp/tests/session_lifecycle_tools.rs`)

1. `test_session_start_spawn_via_v2`
2. `test_session_start_load_via_v2`
3. `test_session_start_attach_returns_unsupported`
4. `test_session_stop_default_seal_tail_true_drain_subscriptions_true`
5. `test_session_stop_seal_tail_false_does_not_seal`
6. `test_capabilities_target_only_returns_static_capabilities`
7. `test_capabilities_session_only_returns_dynamic_capabilities`
8. `test_probe_start_v1_shim_routes_to_session_start_spawn`
9. `test_probe_stop_v1_shim_still_works`

### Sandbox smoke

Pre-build `chronos-mcp` binary + export `CHRONOS_MCP_PATH` (mandatory).

`chronos-sandbox/tests/probe_lifecycle.rs` (extend existing):

1. `test_session_start_via_v2_then_session_stop_via_v2`
2. `test_probe_start_v1_shim_still_works`
3. `test_probe_stop_v1_shim_still_works`

`chronos-sandbox/tests/session_lifecycle.rs` (extend existing):

1. `test_capabilities_static_then_dynamic_through_full_lifecycle`

`chronos-sandbox/tests/program_scenarios.rs` (extend existing):

1. `test_session_lifecycle_in_full_session`

### T4 smoke run command

```bash
cargo build --bin chronos-mcp
export CHRONOS_MCP_PATH="${CARGO_TARGET_DIR:-target}/debug/chronos-mcp"
cargo test -p chronos-sandbox --test probe_lifecycle --test session_lifecycle --test program_scenarios -- --test-threads=1
```

## Risks and known limitations

- **Pre-existing chronos-native flake** (`ptrace_tracer::tests::test_launch_with_syscall_tracing`) is documented in `AGENTS.md` §6.5. Not exercised by m7-06 smoke.
- **Sandbox smoke uses real programs** (`/bin/echo`, `/bin/sh`). Slow on CI but fast locally.
- **No `attach` domain API** — the v2 surface stub returns `Unsupported` (m7+).
- **No v1 probe_stop shim** — v1 probe_stop stays as-is (m7-05 decision).
- **Sandbox smoke requires `chronos-mcp` binary** — pre-build is mandatory.
- **Background execution** — sandbox smoke runs ~5 minutes; use `bg run_in_background: true`.

## Cross-references

* `docs/milestones/m7-06-session-lifecycle-sandbox-smoke-scoping.md` — parent doc.
* `docs/milestones/m7-05-session-lifecycle-dispatchers-merge.md` — dispatcher execute.
* `sddk/changes/m7-05-session-lifecycle-dispatchers-merge/apply-checkpoint.json` `followups[0]`.

---

— Submitted 2026-09-11. Awaits m7-06 execute cycle kickoff.
