# Release Report — m9-62-bounded-stop-probe

## Path

B-direct

## Subject

Restored the HIGH-5 bounded-join timeout in `NativeProbeBackend::stop_probe`. m9-61's inline `handle.join()` switched the join semantics but lost the 10s upper bound. Fixed by reintroducing the previous pattern: spawn a waiter that joins the thread and signals via `mpsc::channel`; `recv_timeout(Duration::from_secs(10))` on the caller's stack. On `Timeout`, warn and abandon the waiter (MCP server response path must not block indefinitely). On `Disconnected`, warn about panic. On `Ok`, log clean exit.

No API change. Trait contract now reads: "blocking, bounded by 10s timeout, MS-RACE-FIX + HIGH-5 + ADR-0005".

## Files changed

| Group | Count | Change |
|---|---|---|
| `crates/chronos-native/src/probe_backend.rs` | 1 | Bounded join restored in stop_probe; doc updated to cite HIGH-5 |

Total: 1 file changed, 20 insertions(+), 5 deletions(-).

## Cross-checks

| ID | Description | Status |
|---|---|---|
| CC#1..CC#51 | Vault schema and cross-reference checks | pass (no drift) |

No new CC added: the bounded join is enforced by construction (`recv_timeout` returns `Timeout` after 10s and the caller proceeds with a warn! log; no deadlock possible).

## History

This was a planned follow-up to m9-61, recorded in the m9-61 handoff: "Bounded join with timeout — current `handle.join()` is unbounded. If a probe thread is wedged in waitpid (parent ignores SIGKILL), the MCP server blocks indefinitely. The previous spawn+recv_timeout(10s) pattern was bounded; we lost that bound when switching to inline join."

m9-62 closes that follow-up by restoring the original HIGH-5 pattern, now combined with stop-then-drain semantics. The waiter thread outlives the caller on timeout but eventually finishes when the probe thread exits; detached, no zombie risk.

Remaining follow-up from m9-61:
- **CC for stop-then-drain caller ordering**: optional static check that `ProbeBackend::stop_probe` callers do not invoke `drain_raw_events` before `stop_probe` returns. Defer to m9-63+ unless the call site drift becomes a problem.
