# Archive Manifest — m9-61-ms-race-fix

## Summary

m9-61 closes the drain/stop race in the probe lifecycle. Single B-direct commit landed as 5d4c00d on feat/ms-race-fix. Race was observable as missing events at session stop on busy fixtures (test_busyloop). Fix is mechanical: blocking stop_probe + stop-then-drain reorder at the 2 service call sites. No trait signature change.

## Cycle

| Campo | Valor |
|---|---|
| Cycle | m9-61-ms-race-fix |
| Base SHA | `97ce507d209eb1bb5d61a4d5b41c0f8bc068343a` |
| Head SHA | `5d4c00d36b14e56349b615e0a1d65186b273c510` |
| Path | B-direct |
| Date | 2026-09-13T09:21Z |
| Branch | `feat/ms-race-fix` |
| Tag | `v0.7.63` |
| Tag peel SHA | `5d4c00d36b14e56349b615e0a1d65186b273c510` |
| Peel match | `5d4c00d36b14e56349b615e0a1d65186b273c510` |
| Status | CLOSED |

## Evidence bindings

- **`apply-checkpoint.json`**: `status: CLOSED`, `verify_status: passed`, `release_status: released`, `archive_status: archived`, `findings_closed: [FIND-MS-RACE-FIX-DRAIN-STOP]`
- **`verify-findings.json`**: 1 finding (FIND-MS-RACE-FIX-DRAIN-STOP), verdict `pass_with_findings`
- **`verify-report.md`**: Subject, Files Inventory, Gates (T0 + T2 + T4), Cross-checks, Notes, History
- **`merge-receipt.md`**: `Base SHA | 97ce507…`, `Head SHA | 5d4c00d…`
- **`release-receipt.md`**: `Remote tag | v0.7.63`, `Peel match | 5d4c00d…`

## Tangential modifications

5 source files changed (47 insertions, 47 deletions):

| File | Net change |
|---|---|
| `crates/chronos-domain/src/adapter.rs` | doc only |
| `crates/chronos-native/src/probe_backend.rs` | blocking rewrite |
| `crates/chronos-services/src/probe.rs` | reorder |
| `crates/chronos-services/src/browser_probe.rs` | reorder |
| `crates/chronos-browser/src/adapter.rs` | drop clear() |

## Cross-checks

- C1-C51: pass
- C48 meta-check: pass (0 DRIFT lines)
- No new CC added

## Follow-ups (deferred)

- **Bounded join with timeout** (m9+): current `handle.join()` is unbounded. If a probe thread is wedged in waitpid (parent ignores SIGKILL), the MCP server blocks indefinitely. The previous spawn+recv_timeout(10s) pattern (HIGH-5) was bounded; we lost that bound when switching to inline join. Tradeoff: bounded-join needs the timer outside the join to keep the API simple, OR we revert to async spawn+channel. Document in m9 follow-ups.
- **CC for stop-then-drain ordering**: optional static check enforcing the call order in service layer.
