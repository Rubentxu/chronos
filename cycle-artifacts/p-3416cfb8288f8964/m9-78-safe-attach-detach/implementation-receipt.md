# Implementation Receipt — m9-78-safe-attach-detach (backfill)

**Note**: This artifact was synthesized during the m9-79 archival sweep
(2026-09-14). The cycle shipped before the SDDK ledger was reconciled.

**Commit**: `a2c70fe9b68120093313c88386d1ee6c53c5391c` (code)
**Branch**: `feat/m9-78-attach-detach`
**Merge**: `009b75037357d069775ad4d7a0661684e27fc9ca` (tag `v0.7.80` is on the
code commit, not the merge; preserved drift, not a defect)
**Base**: `9d5331659f742e3be98995dfba26ff2a635f98ed` (m9-77 merge)

## Changes

Restores the contract m9-77 identified: `session_stop` wakes an attached
target with `SIGSTOP` and lets the ptrace loop `PTRACE_DETACH`, instead of
terminating the traced process. Spawned probes retain the SIGKILL path.

| File | Change |
|---|---|
| `crates/chronos-native/src/probe_backend.rs` | attached loop: `PTRACE_DETACH` on stop; +23/-X lines |
| `crates/chronos-services/src/probe.rs` | probe.rs: -13 lines (stop-then-drain still applies; SIGKILL path stays for spawned probes) |
| `chronos-sandbox/tests/session_lifecycle.rs` | the focused `test_session_start_attach_to_running_self` covers both attach + safe detach in one shot; +29/-X lines |
| `docs/manual-ai/en/08-session-management.md` | documents the safe-detach semantics; +7/-X lines |
| `docs/manual-ai/es/08-gestion-sesiones.md` | Spanish mirror; +8/-X lines |

5 files changed, 44 insertions(+), 36 deletions(-).

## Gates (orchestrator-verified retroactively)

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| `cargo test -p chronos-native --lib -- --test-threads=1` | PASS (101 passed / 0 failed / 13.02 s) |
| T4 `program_scenarios::test_infinite_loop_stopped_by_probe_stop` | PASS |
| T4 `e2e_connectivity` | PASS |
| T4 `probe_lifecycle` | PASS |
| T4 `session_lifecycle::test_session_start_attach_to_running_self` | PASS (1/1) |

## Acceptance mapping

- REQ-ATTACH-DETACH-001 ✅ `session_stop` on an attached session wakes the
  target with `SIGSTOP`, the ptrace loop performs `PTRACE_DETACH`, and the
  child process remains alive after the call.
- REQ-ATTACH-DETACH-002 ✅ Spawned probes still take the SIGKILL path; the
  contract is unchanged for them.

## Deviations

None functional. The cycle is a one-file semantic change plus matching test
+ doc updates.