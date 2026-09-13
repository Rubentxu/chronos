# Verify Report — m9-61

**Cycle**: m9-61-ms-race-fix
**Path**: B-direct

## Summary

Single-commit B-direct cycle closes a drain/stop race in the probe lifecycle. Drain could be invoked while the probe thread was still emitting events, losing them in the window between drain_raw_events() return and probe-thread exit. Fix is purely a reordering + a blocking join; no trait signature change.

## Subject

| Base | Head (final) | Dirty diff digest | CWD | Verified at |
|---|---|---|---|---|
| `97ce507` | `5d4c00d36b14e56349b615e0a1d65186b273c510` | `sha256:135db361379b125d946ee04ddb38c2a220ced1113122dcf2171873c2cbe3cd3a` | `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos` | 2026-09-13T09:21:00Z |

## Files Inventory

5 files changed, 47 insertions(+), 47 deletions(-):

| File | Change |
|---|---|
| `crates/chronos-domain/src/adapter.rs` | ProbeBackend::stop_probe doc: non-blocking → blocking; cite ADR-0005 |
| `crates/chronos-native/src/probe_backend.rs` | NativeProbeBackend::stop_probe: replaced async spawn+channel+recv_timeout with synchronous handle.join() inline; removed detached-thread-with-timeout pattern |
| `crates/chronos-services/src/probe.rs` | ProbeService::stop: reorder drain-then-stop → stop-then-drain; eBPF detach moved after stop |
| `crates/chronos-services/src/browser_probe.rs` | BrowserProbeService::stop: same stop-then-drain reorder |
| `crates/chronos-browser/src/adapter.rs` | BrowserAdapter::stop_probe: removed event_buffer.clear() (drain now happens after stop, not before) |

## Gates

| Gate | Status | Evidence |
|---|---|---|
| T0: cargo fmt --check | PASS | no output |
| T0: cargo clippy --workspace --all-targets -- -D warnings | PASS | no warnings (1.09s cached) |
| T1: chronos-domain --lib | PASS | 149 passed; 0 failed |
| T2: chronos-services --lib | PASS | 263 passed; 0 failed |
| T2: chronos-browser --lib | PASS | 42 passed; 1 ignored |
| T2: chronos-native --lib (single-thread) | PASS | 99 passed; 0 failed |
| T4: chronos-sandbox program_scenarios test_infinite_loop_stopped_by_probe_stop | PASS | events captured after stop |
| T4: chronos-sandbox e2e_connectivity | PASS | 1 passed |
| T4: chronos-sandbox analytics_tools | PASS | 4 passed |
| T4: chronos-sandbox program_scenarios full | PASS | 11 passed |

## Cross-checks

- CC#1..CC#51: pass (no schema drift introduced; cycle is pure refactor)
- No new CCs needed — the race is closed by construction (stop_probe is now blocking by contract)

## Notes

- Pre-existing flake observed in chronos-native ptrace_tracer tests when run with parallel test threads; documented in AGENTS.md §6.5. Reproduces on main (c76b1096 family) and on feat/ms-race-fix alike. NOT a regression. Single-thread run yields 99/99 PASS.
- The blocking stop_probe has no documented timeout. If the probe thread is wedged in waitpid (e.g. parent process ignores SIGKILL), the MCP server would block on the join indefinitely. ADR-0005 calls this out as an implementation-defined bound. Future cycle: bounded join with timeout (HIGH-5 in original commit was the previous bounded pattern via spawn+channel).

## History

m9-61 was a hotfix discovered while integrating the probe lifecycle into session_lifecycle MCP tools. The race was observable as missing events at session stop when the target binary was emitting at high frequency (e.g. test_busyloop). MS-RACE-FIX is the internal tracker name; the cycle lives in m9 (concurrency intelligence section of the roadmap).
