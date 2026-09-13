# Release Report — m9-63-stop-drain-cc

## Path

B-direct

## Subject

Closed the second m9-61 follow-up: added CC#52 to `vault-drift-sweep.md`. CC#52 is a static cross-check that scans service-layer files for `fn ... stop(...)` bodies where `drain_raw_events()` is invoked before `stop_probe()` on a `ProbeBackend`. If found, it emits a DRIFT line pointing at `file:line:fn_name`.

Implementation: regex + brace-walking over `crates/chronos-services/src/probe.rs` and `crates/chronos-services/src/browser_probe.rs` (the 2 known consumers of `ProbeBackend`).

## Files changed

| Group | Count | Change |
|---|---|---|
| `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md` | 1 | CC#52 appended (+74 lines) |

Total: 1 file changed, 74 insertions(+), 0 deletions(-).

## Cross-checks

| ID | Description | Status |
|---|---|---|
| CC#52 (new) | ProbeBackend stop-then-drain ordering at service call sites | closed by m9-63; self-tested |
| CC#1..CC#51 | Vault schema and cross-reference checks | pass (no drift) |
| CC#48 | meta-check | pass (52 CCs all clean) |

## History

This was a planned follow-up to m9-61, recorded in the m9-61 handoff:
> "**CC enforcement**: add a static check that `ProbeBackend::stop_probe` callers do not invoke `drain_raw_events` before `stop_probe` returns."

m9-63 closes that follow-up by adding CC#52. The race itself remains closed (m9-61 + m9-62). CC#52 is forward-looking defense: any future refactor or new consumer is caught at drift-sweep time.

The CC's call_sites list is small (2 files). If a new `ProbeBackend` consumer is added, the list must be extended. This is a manual update, not auto-discovered — the heuristic trade-off (regex over AST parsing) keeps the CC maintainable.
