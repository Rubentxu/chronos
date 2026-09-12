# Archive Manifest — m9-58-change-entry-ciclo-table-short-sha

## Summary

m9-58 closes the "short SHA in SHA-field metadata" drift class. 4 change-entry.md `## Ciclo` table cells had short SHA values (1-39 chars) in `Base SHA` rows; CCs #22/#23 silently skipped them due to `len(head) != 40` precondition. Each expanded to full 40-char form. Cross-check #49 added to detect this drift class going forward.

## Cycle

| Campo | Valor |
|---|---|
| Cycle | m9-58-change-entry-ciclo-table-short-sha |
| Base SHA | `fc75ff3cbba48c6c4603d6b56b53664ad3849dbb` |
| Head SHA | `c6ce0e678d2872001ffca68abaffe00b72d8c516` |
| Path | B-direct |
| Date | 2026-09-12T18:38Z |
| Branch | `fix/m9-58-change-entry-ciclo-table-short-sha` |
| Tag | `v0.7.60` |
| Tag peel SHA | `c6ce0e678d2872001ffca68abaffe00b72d8c516` |
| Peel match | `c6ce0e678d2872001ffca68abaffe00b72d8c516` |
| Status | CLOSED |

## Evidence bindings

- **`apply-checkpoint.json`**: `cycle-artifacts/p-3416cfb8288f8964/m9-58-*/apply-checkpoint.json` — `status: CLOSED`, `verify_status: passed`, `release_status: released`, `archive_status: archived`, `findings_closed: [FIND-M9-58-CHANGE-ENTRY-CICLO-TABLE-SHORT-SHA]`
- **`verify-findings.json`**: 1 finding (FIND-M9-58-CHANGE-ENTRY-CICLO-TABLE-SHORT-SHA), verdict `pass_with_findings`
- **`verify-report.md`**: Subject table, Findings section, Files Inventory (13 rows), Cross-checks (#49), Verification (3 rows), History
- **`merge-receipt.md`**: `Base SHA | fc75ff3…`, `Head SHA | c6ce0e678d2872001ffca68abaffe00b72d8c516`
- **`release-receipt.md`**: `Remote tag | v0.7.60`, `Peel match | c6ce0e678d2872001ffca68abaffe00b72d8c516`
- **`release-report.md`**: `# m9-58: change-entry ## Ciclo table short SHA expansion`, Path B-direct, Subject, Files (4 modified + 1 maintenance + 8 new), Cross-checks (#49), History

## Tangential modifications

4 change-entry.md files modified to expand short SHAs:

| File | Expansion |
|---|---|
| `m8-07-hypothesis-reconstruction-fidelity/change-entry.md` | `148f009` → `148f009a4a957ae67ac62a28bcd04e49d44db5f2` |
| `m9-54-stale-branch-cleanup/change-entry.md` | `a24139e` → `a24139ec1410d6fff167c5b127c1d6fed019c192` |
| `m9-55-apply-checkpoint-fabricated-sha/change-entry.md` | `cbb9384` → `cbb93847228a9062de8e093dfe452c257233228c` |
| `m9-56-release-report-duplicate-cross-checks/change-entry.md` | `6bdf8ba` → `6bdf8ba506a61655edd82c997fca2137665ad8d6` |

1 maintenance doc modified:

| File | Change |
|---|---|
| `vault-drift-sweep.md` | Added CC#49 (short SHA detection) |

## Cross-checks

- C1-C48: pass
- C49: pass (after fix)
- C48 meta-check: pass (0 DRIFT lines across all 49 CCs)
