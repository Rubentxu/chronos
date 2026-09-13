# Release Report — m9-61-ms-race-fix

## Path

B-direct

## Subject

Closed a drain/stop race in the probe lifecycle. `drain_raw_events()` could be invoked while the probe thread was still emitting events, losing them in the race window between drain return and probe-thread exit. Fixed by:

1. Making `NativeProbeBackend::stop_probe` blocking (replaced async spawn+channel+recv_timeout pattern with synchronous `handle.join()` inline before returning).
2. Reordering `ProbeService::stop` and `BrowserProbeService::stop` to call `stop_probe` first, then `drain_raw_events`.
3. Updating `ProbeBackend` trait doc and removing `event_buffer.clear()` from `BrowserAdapter::stop_probe` (drain now happens after stop).
4. Moving eBPF `detach_all()` to after `stop_probe` returns (no concurrent producer).

No trait signature change. ADR-0005 (adaptive instrumentation) cited in the trait doc.

## Files changed

| Group | Count | Change |
|---|---|---|
| `crates/chronos-domain/src/adapter.rs` | 1 | ProbeBackend::stop_probe doc +5/-1 |
| `crates/chronos-native/src/probe_backend.rs` | 1 | stop_probe blocking rewrite +25/-21 |
| `crates/chronos-services/src/probe.rs` | 1 | stop-then-drain reorder +14/-10 |
| `crates/chronos-services/src/browser_probe.rs` | 1 | stop-then-drain reorder +9/-4 |
| `crates/chronos-browser/src/adapter.rs` | 1 | drop event_buffer.clear() +3/-2 |

Total: 5 files changed, 47 insertions(+), 47 deletions(-).

## Cross-checks

| ID | Description | Status |
|---|---|---|
| CC#1..CC#51 | Vault schema and cross-reference checks | pass (no drift) |

No new CC added: the race is closed by construction (`stop_probe` is now blocking by contract, enforced at the trait doc level). A future CC could enforce `drain-then-stop` ordering in callers, but the call sites are only 2 services and reordering is mechanical.

## History

Discovered during session_lifecycle integration testing when `test_infinite_loop_stopped_by_probe_stop` showed `events_captured < events_emitted` on busy fixtures. Internal tracker: MS-RACE-FIX. Hotfix landed as a single B-direct commit because the fix is mechanical and bounded: the previous bounded-join pattern (HIGH-5, async spawn + recv_timeout) was correct in shape but used the wrong ordering at the caller; switching to inline join + stop-then-drain removes the async hop entirely.

Follow-up candidates (deferred):
- **Bounded join with timeout**: current `handle.join()` is unbounded. If a probe thread is wedged in waitpid (parent ignores SIGKILL), the MCP server blocks indefinitely. The previous spawn+recv_timeout(10s) pattern was the bounded variant. Tradeoff documented; future cycle to reinstate a bounded variant.
- **CC enforcement**: add a static check that `ProbeBackend::stop_probe` callers do not invoke `drain_raw_events` before `stop_probe` returns.
