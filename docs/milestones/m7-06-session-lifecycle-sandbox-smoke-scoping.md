# m7-06 — session_start + session_stop + capabilities sandbox smoke scoping

**Branch:** `feat/m7-06-session-lifecycle-sandbox-smoke-scoping`
**Cycle:** M7 (v2-spec sub-cycle), sixth deliverable (m7-05 dispatcher execute closed; smoke is the verification gate)
**Precedence:** `docs/milestones/m7-05-session-lifecycle-dispatchers-merge.md`; `docs/milestones/m7-05-session-lifecycle-dispatchers-scoping.md`; v2 spec `AGENT_API_V2.md` lines 11–13
**Status:** SCOPING — 2026-09-11

## Why this cycle

m7-05 shipped the dispatcher + MCP wrappers + v1 probe_start shim, but explicitly deferred the T4 sandbox smoke to a follow-up cycle (see `sddk/changes/m7-05-session-lifecycle-dispatchers-merge/apply-checkpoint.json` `followups[0]`). m7-06 picks up that follow-up and adds:

1. **Sandbox smoke for the v2 lifecycle surface** — exercise `session_start{action=spawn|load|attach}`, `session_stop`, `capabilities{target|session_id}` against real programs via the `chronos-mcp` binary.
2. **MCP integration tests** — `crates/chronos-mcp/tests/session_lifecycle_tools.rs` (new) exercises the wire format + DTO round-trip end-to-end.
3. **M7 close report** — `docs/milestones/m7-close-report.md` capturing the full M7 deliverables, v1 sunset bookkeeping, and the handoff to m8+.

This is the final deliverable in the M7 sub-cycle. After m7-06 ships, the M7 backlog reduces to:
- **v1 sunset bookkeeping** — no removals until 2027-09-11; tracked as a passive item.
- **m7-07+ follow-ups** — `single-call session_stop`, `attach` domain API, etc. (m7+ scope per the m7-04/m7-05 decisions).

## Scope (this cycle)

### 1. Sandbox client surface

Add to `chronos-sandbox/src/client/`:

- **types.rs**: `SessionStartResponse`, `SessionStopResponse`, `CapabilitiesResponse` (mirroring the wire-format shape of the v2 DTOs).
- **tools.rs**: client methods
  - `pub async fn session_start_spawn(&mut self, program, args, ...) -> Result<SessionStartResponse, McpSandboxError>`
  - `pub async fn session_start_load(&mut self, session_id) -> Result<SessionStartResponse, McpSandboxError>`
  - `pub async fn session_start_attach(&mut self, pid) -> Result<serde_json::Value, McpSandboxError>` (expects `Unsupported` error)
  - `pub async fn session_stop(&mut self, session_id, seal_tail, drain_subscriptions) -> Result<SessionStopResponse, McpSandboxError>`
  - `pub async fn capabilities_target(&mut self, program, language) -> Result<CapabilitiesResponse, McpSandboxError>`
  - `pub async fn capabilities_session(&mut self, session_id) -> Result<CapabilitiesResponse, McpSandboxError>`

### 2. MCP integration tests (new file)

`crates/chronos-mcp/tests/session_lifecycle_tools.rs`:

1. `test_session_start_spawn_via_v2` — happy-path against `/bin/echo`.
2. `test_session_start_load_via_v2` — save session → load returns metadata + capability snapshot.
3. `test_session_start_attach_returns_unsupported` — stub surface.
4. `test_session_stop_default_seal_tail_true_drain_subscriptions_true` — verify `sealed_at` populated.
5. `test_session_stop_seal_tail_false_does_not_seal` — verify metadata unchanged.
6. `test_capabilities_target_only_returns_static_capabilities`.
7. `test_capabilities_session_only_returns_dynamic_capabilities`.
8. `test_probe_start_v1_shim_routes_to_session_start_spawn` — backward-compat smoke.
9. `test_probe_stop_v1_shim_still_works` — backward-compat smoke.

### 3. Sandbox suite extensions

`chronos-sandbox/tests/probe_lifecycle.rs` (extend existing):

1. `test_session_start_via_v2_then_session_stop_via_v2` — happy-path round-trip on real `/bin/echo`.
2. `test_probe_start_v1_shim_still_works` — backward-compat smoke.
3. `test_probe_stop_v1_shim_still_works` — backward-compat smoke.

`chronos-sandbox/tests/session_lifecycle.rs` (extend existing):

1. `test_capabilities_static_then_dynamic_through_full_lifecycle` — start → drain → capabilities{dynamic} → stop → capabilities{tail_sealed=true}.

`chronos-sandbox/tests/program_scenarios.rs` (extend existing):

1. `test_session_lifecycle_in_full_session` — full v2 path on a real binary.

### 4. M7 close report

`docs/milestones/m7-close-report.md` (~400 LoC):

- **§1 Executive summary** — M7 closed in 6 cycles (m7-01 through m7-06).
- **§2 Cycle log** — what each cycle shipped, gate results, tags.
- **§3 v2 tool surface inventory** — events_read, observe, session_compare, session_explain, session_start, session_stop, capabilities (7 v2 tools).
- **§4 v1 deprecation bookkeeping** — list of v1 tools that route through v2 dispatchers; sunset date 2027-09-11.
- **§5 Architectural decisions log** — async-first dispatchers, capability snapshots, hardcoded engine_version, etc.
- **§6 Test pyramid state** — 800+ unit + 357 integration + (post-smoke) ~12 sandbox suites; green.
- **§7 m8+ handoff** — pointer to the M8 backlog (live probe improvements, attach domain API, etc.).

## Test plan

### Smoke subset

T4 sandbox subset: `probe_lifecycle.rs` + `session_lifecycle.rs` + `program_scenarios.rs` (3 suites, ~12 new tests).

### Pre-build

Pre-build `chronos-mcp` binary before sandbox run to avoid per-test binary-spawn latency:

```bash
cargo build --bin chronos-mcp
export CHRONOS_MCP_PATH="${CARGO_TARGET_DIR:-target}/debug/chronos-mcp"
```

### Background execution

The smoke suite runs ~5 minutes total. Run in background with `bg run_in_background: true` + `notify: true` + `max_wait_seconds: 600`.

## Risks and known limitations

- **Pre-existing chronos-native flake** (`ptrace_tracer::tests::test_launch_with_syscall_tracing`) is documented in `AGENTS.md` §6.5. Not exercised by m7-06 smoke; affects only chronos-native lib tests, not sandbox.
- **Sandbox smoke uses real programs** (`/bin/echo`, `/bin/sh`). Slow on CI but fast locally.
- **No `attach` domain API** — the v2 surface stub returns `Unsupported` (m7+).
- **No v1 probe_stop shim** — v1 probe_stop stays as-is (m7-05 decision; deferred to m7-07 for single-call path).
- **Sandbox smoke requires `chronos-mcp` binary** — pre-build is mandatory; per-test `cargo build` would 35× the runtime.

## Cross-references

* `docs/milestones/m7-05-session-lifecycle-dispatchers-merge.md` — dispatcher execute (parent).
* `docs/milestones/m7-05-session-lifecycle-dispatchers-scoping.md` — dispatcher scoping (parent).
* `sddk/changes/m7-05-session-lifecycle-dispatchers-merge/apply-checkpoint.json` `followups[0]` — explicit deferred-scope list.
* `docs/milestones/m6-close-report.md` §6 — origin of the three tools covered by the M7 sub-cycle.

---

— Submitted 2026-09-11. Awaits m7-06 execute cycle kickoff.
