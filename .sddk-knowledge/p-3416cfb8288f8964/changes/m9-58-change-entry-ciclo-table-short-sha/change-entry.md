# Change: m9-58 change entry ciclo table short sha

## Summary

Post-m9-57 sweep (which closed 5 other drift dimensions in v0.7.59) found 4 change-entry.md `## Ciclo` table cells with short SHA values (1-39 chars) in their `Base SHA` rows. m8-07, m9-54, m9-55, m9-56 each had a short SHA that pre-existing CCs (CC#22, CC#23) silently skipped due to their `len(head) != 40` precondition. m9-58 expands each to full 40-char form and adds cross-check #49 that explicitly detects short SHAs in SHA-keyed metadata cells.

## Ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-58-change-entry-ciclo-table-short-sha` |
| Path | B-direct |
| Status | CLOSED |
| Base SHA | `fc75ff3cbba48c6c4603d6b56b53664ad3849dbb` |
| Head SHA | `c6ce0e678d2872001ffca68abaffe00b72d8c516` |

## Subject

- base_sha: `fc75ff3cbba48c6c4603d6b56b53664ad3849dbb`
- head_sha: `c6ce0e678d2872001ffca68abaffe00b72d8c516`
- cycle: m9-58
- branch: `fix/m9-58-change-entry-ciclo-table-short-sha`
- date: 2026-09-12
- tag: `v0.7.60`
- findings_closed: 1 (FIND-M9-58-CHANGE-ENTRY-CICLO-TABLE-SHORT-SHA)
- findings_introduced.no_action: 0

## Files changed
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m8-07-hypothesis-reconstruction-fidelity/change-entry.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-54-stale-branch-cleanup/change-entry.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-55-apply-checkpoint-fabricated-sha/change-entry.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-56-release-report-duplicate-cross-checks/change-entry.md`
- `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md` (CC#49 added)
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-58-*/apply-checkpoint.json`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-58-*/verify-findings.json`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-58-*/verify-report.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-58-*/merge-receipt.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-58-*/release-receipt.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-58-*/release-report.md`
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-58-*/change-entry.md` (this file)
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-58-*/archive-manifest.md`
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` (m9-58 row added)
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` (Last archive bumped)

## Cross-checks

m9-58 added cross-check #49 (change-entry `## Ciclo` table cells must use full 40-char SHAs). See `vault-drift-sweep.md`.
