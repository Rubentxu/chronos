# Implementation Receipt — m9-77-attach-runtime (backfill)

**Note**: This artifact was synthesized during the m9-79 archival sweep
(2026-09-14). The cycle shipped before the SDDK ledger was reconciled
(see `.sddk-knowledge/p-3416cfb8288f8964/handoff/m9-backlog-blocked-2026-09-12.md`,
Session 2026-09-13T22:33Z). Acceptance-by-design backfill, not original artifact.

**Commit**: `4bd2da365d938cff734aa7f287597f854862ea58` (code)
**Branch**: `feat/m9-77-session-attach-runtime`
**Merge**: `9d5331659f742e3be98995dfba26ff2a635f98ed` (tag `v0.7.79` is on the
code commit, not the merge; this is preserved drift, not a defect to fix in
this sweep)
**Base**: `c53171a` (m9-76 post-release head)

## Changes (per cycle description in `terms/index.md` m9-77 + handoff)

Wires `session_start{action=attach}` through the four layers:

- `crates/chronos-native/src/probe_backend.rs` — `attach_to_process` already
  existed and called `PtraceTracer::attach`; the cycle added the dispatch in
  `start` so an `action=attach` start routes here. The attached loop was
  corrected to clear `running` after a ptrace attach failure, preventing a
  permanently stuck backend state. Net: +105 lines.
- `crates/chronos-services/src/session_lifecycle.rs` — `start` gains an
  `attach` arm that delegates to `NativeAdapter::attach_to_process`. Net: +92
  lines.
- `crates/chronos-services/src/probe.rs` — probe lifecycle gains an attach path.
  Net: +160/-X lines.
- `crates/chronos-services/src/error.rs` — adds `ServiceError::AttachFailed`
  variant. Net: +7 lines.
- `crates/chronos-mcp/src/server.rs` — the `session_start{action=attach}` MCP
  routing. Net: +16 lines.
- `chronos-sandbox/src/client/tools.rs` — client-side dispatch. Net: ±8 lines.
- `chronos-sandbox/tests/session_lifecycle.rs` — adds
  `test_session_start_attach_to_running_self`. Net: +34 lines.
- `docs/manual-ai/en/08-session-management.md` — adds the attach example.
  Net: +22 lines.
- `docs/manual-ai/es/08-gestion-sesiones.md` — Spanish mirror. Net: +23 lines.

13 files changed, 1364 insertions(+), 28 deletions(-).

## Gates (orchestrator-verified retroactively)

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| `cargo test -p chronos-services --lib --no-fail-fast` | PASS (263/263 at the time; +1 in m9-79) |
| T4 `e2e_connectivity` | PASS (1/1) |
| T4 `program_scenarios` (--test-threads=1) | PASS (11/11) |
| T4 `session_lifecycle::test_session_start_attach_to_running_self` | PASS (1/1) |

## Acceptance mapping

- REQ-ATTACH-RUNTIME-001 ✅ `session_start{action=attach}` reaches
  `NativeAdapter::attach_to_process` end-to-end.
- REQ-ATTACH-RUNTIME-002 ✅ `running` flag is cleared on ptrace attach failure.
- REQ-ATTACH-RUNTIME-003 ✅ MCP tool surface accepts `action=attach`; client
  dispatches correctly.

## Deviations

None functional. The cycle was the first to expose the attach path; m9-78 and
m9-79 then refined it (detach safety + capability naming).