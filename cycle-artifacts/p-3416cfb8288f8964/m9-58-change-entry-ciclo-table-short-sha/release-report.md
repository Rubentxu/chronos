# Release Report — m9-58-change-entry-ciclo-table-short-sha

## Path

B-direct

## Subject

Expand 4 short SHAs (1-39 chars) in `## Ciclo` table `Base SHA` cells across 4 change-entry.md files (m8-07, m9-54, m9-55, m9-56). Add cross-check #49 to detect this drift class automatically going forward.

## Files changed

| File | Change |
|---|---|
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m8-07-hypothesis-reconstruction-fidelity/change-entry.md` | `Base SHA`: `148f009` → `148f009a4a957ae67ac62a28bcd04e49d44db5f2` |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-54-stale-branch-cleanup/change-entry.md` | `Base SHA`: `a24139e` → `a24139ec1410d6fff167c5b127c1d6fed019c192` |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-55-apply-checkpoint-fabricated-sha/change-entry.md` | `Base SHA`: `cbb9384` → `cbb93847228a9062de8e093dfe452c257233228c` |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-56-release-report-duplicate-cross-checks/change-entry.md` | `Base SHA`: `6bdf8ba` → `6bdf8ba506a61655edd82c997fca2137665ad8d6` |
| `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md` | Added cross-check #49 (short SHA detection) |

## Cross-checks

| ID | Description | Status |
|---|---|---|
| #49 | change-entry.md `## Ciclo` table cells must use full 40-char SHAs (independent of canonical-format precondition) | closed by m9-58 |

## History

m9-58 closes the "short SHA in SHA-field metadata" drift class. Detection during post-m9-57 sweep (which had already closed 5 other drift dimensions in v0.7.59): 4 change-entry.md `## Ciclo` tables had short Base SHA values that CCs #22/#23 silently skipped due to their `len(head) != 40` precondition. CC#49 explicitly scans SHA-keyed metadata cells independent of that precondition, only inspecting table rows (lines starting with `|`) and JSON property assignments, skipping narrative text inside markdown code blocks.
