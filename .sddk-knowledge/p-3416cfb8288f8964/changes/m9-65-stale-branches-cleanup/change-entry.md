# Change: m9-65 stale branches cleanup

## Summary

Cleanup of 49 stale branches (25 local + 24 remote) across M0-M9 milestones that were merged into main but never deleted. CC#46 (vault drift sweep) only monitored `fix/m9-*` branches, so every milestone's per-cycle branches (and the `feat/ms-race-fix` branch from m9-61) accumulated. m9-65 added CC#53 (bash) to vault-drift-sweep.md that extends CC#46 to all prefixes (`chore/*`, `feat/mX-*`, `fix/mX-*`, `ms-*`), executed safe deletion via `git branch -d` and `git push origin --delete`, and recorded the tip SHA of every deleted branch in `branches-deleted.log` for recovery. 5 local + 19 remote not-merged branches were preserved and documented for human triage.

## Ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-65-stale-branches-cleanup` |
| Path | B-direct |
| Status | CLOSED |
| Base SHA | `6e7e39ec35f7bd6c882833206d72b2d46fab1199` |
| Head SHA | `97ff56e53c4e00768783f11d0df16878cf9c693e` |
| Tag | `v0.7.67` |

## Subject

- base_sha: `6e7e39ec35f7bd6c882833206d72b2d46fab1199`
- head_sha: `97ff56e53c4e00768783f11d0df16878cf9c693e`
- cycle: m9-65
- branch: `feat/m9-65-stale-branches-cleanup`
- date: 2026-09-13
- tag: `v0.7.67`
- findings_closed: 1 (FIND-M9-65-BRANCH-DRIFT)
- findings_introduced.no_action: 0

## Files changed

- (modified) `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md` — added CC#53 (bash block, extends CC#46 to all prefixes)
- (modified) `scripts/check_vault_drift.sh` — dynamic CC count (46 python + 6 bash + 1 self = 53 total); quoted heredoc to fix bash backtick expansion
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-65-stale-branches-cleanup/branches-deleted.log` — recovery log: 49 deleted SHAs + 24 preserved
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-65-stale-branches-cleanup/apply-checkpoint.json`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-65-stale-branches-cleanup/verify-findings.json`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-65-stale-branches-cleanup/verify-report.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-65-stale-branches-cleanup/merge-receipt.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-65-stale-branches-cleanup/release-receipt.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-65-stale-branches-cleanup/release-report.md`
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-65-stale-branches-cleanup/change-entry.md` (this file)
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-65-stale-branches-cleanup/archive-manifest.md`
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` (m9-65 row added)
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` (Last archive bumped)

## Cross-checks

CC#1..CC#52: pass (no drift). CC#53 (new, bash, not auto-executed by CC#48): pass when run manually (0 merged-stale).
