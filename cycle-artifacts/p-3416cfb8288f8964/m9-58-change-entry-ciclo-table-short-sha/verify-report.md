# Verify Report — m9-58

**Cycle**: m9-58-change-entry-ciclo-table-short-sha
**Path**: B-direct

## Summary

4 change-entry.md `## Ciclo` table cells had short SHA values (1-39 chars) in their `Base SHA` rows. The pre-existing CCs that scan SHA fields (CC#22, CC#23) use `len(head) != 40` as a precondition, which silently skipped these short-SHA rows. m9-58 expands each short SHA to its full 40-char form and adds cross-check #49 that explicitly detects short SHAs in SHA-keyed metadata cells, independent of the canonical-format precondition.

## Subject

| Base | Head (final) | Dirty diff digest | CWD | Verified at |
|---|---|---|---|---|
| `fc75ff3` | `c6ce0e678d2872001ffca68abaffe00b72d8c516` | `sha256:e3b0c44...` | `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos` | 2026-09-12T18:38:00Z |

Files changed (4 change-entry.md + 1 maintenance doc):

| File | old Base SHA | new Base SHA |
|---|---|---|
| `m8-07/change-entry.md` | `148f009` | `148f009a4a957ae67ac62a28bcd04e49d44db5f2` |
| `m9-54/change-entry.md` | `a24139e` | `a24139ec1410d6fff167c5b127c1d6fed019c192` |
| `m9-55/change-entry.md` | `cbb9384` | `cbb93847228a9062de8e093dfe452c257233228c` |
| `m9-56/change-entry.md` | `6bdf8ba` | `6bdf8ba506a61655edd82c997fca2137665ad8d6` |

## Findings

| ID | Severity | Category | Description | Status |
|---|---|---|---|---|
| F1 | medium | vault_hygiene.short_sha_in_metadata | 4 change-entry.md `## Ciclo` table cells had short SHAs (1-39 chars) in `Base SHA` rows; CCs #22/#23 silently skipped them due to `len(head) != 40` precondition | RESOLVED |

## Files Inventory

| Path |
|---|
| `cycle-artifacts/p-3416cfb8288f8964/m9-58-change-entry-ciclo-table-short-sha/apply-checkpoint.json` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-58-change-entry-ciclo-table-short-sha/verify-findings.json` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-58-change-entry-ciclo-table-short-sha/verify-report.md` (this file) |
| `cycle-artifacts/p-3416cfb8288f8964/m9-58-change-entry-ciclo-table-short-sha/merge-receipt.md` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-58-change-entry-ciclo-table-short-sha/release-receipt.md` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-58-change-entry-ciclo-table-short-sha/release-report.md` |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-58-change-entry-ciclo-table-short-sha/change-entry.md` |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-58-change-entry-ciclo-table-short-sha/archive-manifest.md` |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m8-07-hypothesis-reconstruction-fidelity/change-entry.md` (modified) |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-54-stale-branch-cleanup/change-entry.md` (modified) |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-55-apply-checkpoint-fabricated-sha/change-entry.md` (modified) |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-56-release-report-duplicate-cross-checks/change-entry.md` (modified) |
| `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md` (modified, CC#49 added) |

## Cross-checks

- CC#49: passes (0 DRIFT lines after fix)
- CC#48 meta-check: passes (0 DRIFT lines across all 49 CCs)
- All 4 expanded SHAs verified reachable via `git cat-file -t`

## Verification

- C1-C48: pass
- C49: pass (after fix)

## History

m9-58 was discovered during a post-m9-57 sweep that itself closed 5 other drift dimensions (v0.7.59). The short-SHA drift was found in `## Ciclo` table cells — a metadata location that CCs #22/#23 (which scan SHA fields in release-receipt and merge-receipt respectively) did not cover. CC#49 explicitly scans `## Ciclo` table cells for short SHAs, skipping narrative text inside markdown code blocks (where historical short-SHA references in archive-manifest narratives are legitimate).
