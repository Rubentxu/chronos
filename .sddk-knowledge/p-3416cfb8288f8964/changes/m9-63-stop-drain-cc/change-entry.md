# Change: m9-63 stop-then-drain ordering cross-check

## Summary

Closes the second m9-61 follow-up: added CC#52 (stop-then-drain ordering) to `vault-drift-sweep.md`. The CC scans service-layer files for `fn ... stop(...)` bodies where `drain_raw_events()` is invoked before `stop_probe()` on a `ProbeBackend`.

## Ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-63-stop-drain-cc` |
| Path | B-direct |
| Status | CLOSED |
| Base SHA | `52ee9a2f2ba6233f03e46c7e5c139721779ba8d4` |
| Head SHA | `8dc1063d1a8f24bd2f59fac501b848003eec63d5` |
| Tag | `v0.7.65` |

## Subject

- base_sha: `52ee9a2f2ba6233f03e46c7e5c139721779ba8d4`
- head_sha: `8dc1063d1a8f24bd2f59fac501b848003eec63d5`
- cycle: m9-63
- branch: `feat/m9-63-stop-drain-cc`
- date: 2026-09-13
- tag: `v0.7.65`
- findings_closed: 1 (FIND-M9-63-MISSING-STOP-DRAIN-CC)
- findings_introduced.no_action: 0

## Files changed

- (modified) `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md` — CC#52 appended
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-63-stop-drain-cc/apply-checkpoint.json`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-63-stop-drain-cc/verify-findings.json`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-63-stop-drain-cc/verify-report.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-63-stop-drain-cc/merge-receipt.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-63-stop-drain-cc/release-receipt.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-63-stop-drain-cc/release-report.md`
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-63-stop-drain-cc/change-entry.md` (this file)
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-63-stop-drain-cc/archive-manifest.md`
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` (m9-63 row added)
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` (Last archive bumped)

## Cross-checks

CC#1..CC#51: pass (no drift). CC#52 (new): pass on current code; self-tested with synthetic violation. CC#48 meta-check: pass (52 CCs all clean).
