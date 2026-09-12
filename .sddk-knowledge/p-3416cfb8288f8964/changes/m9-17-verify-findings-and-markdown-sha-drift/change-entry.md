# Change: m9-17 verify findings and markdown sha drift

## Summary

Drift closure cycle for this milestone.


| Campo | Valor |
|---|---|
| Cycle ID | `m9-17-verify-findings-and-markdown-sha-drift` |
| Path | B-direct |
| Status | CLOSED |
| Base SHA | `0ed8f874f7b6f973505fc3997475fc398fcfa4cc` (main HEAD before cycle) |
| Head SHA | `134dc7525312275c447db8f5996740ff7c102a02` (fix commit on main) |
| Tag | `v0.7.15` |
| Published | 2026-09-12T10:33:00Z |

## What changed

m9-14..m9-16 each fixed one file type's SHA drift (apply-checkpoint,
archive-manifest) but missed other file types in the same cycle:
verify-findings.json subject_sha, release-receipt.md Head/Remote
tag_peel, merge-receipt.md Head SHA, change-entry.md Head SHA.

m9-17 closes the residual drift across 4 file types × 4 prior cycles:

**verify-findings.json:**
- m9-11: fabricated → real
- m9-12, m9-13: short → full
- m9-03, m9-04: schema-v1 (accepted-by-design)

**Cycle markdown:**
- m9-11 release-receipt/merge-receipt/release-report/verify-report: short → full
- m9-14 merge-receipt/release-receipt: short → full (m9-14 self-drift)
- m9-11/12/13 change-entry: Head SHA + fix-commit table row + "Tag peeled" narrative

Adds cross-check #10 to vault-drift-sweep.md covering all four file
types (verify-findings, release-receipt, merge-receipt, change-entry).

## Files touched

- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-11-cycles-index-metadata-drift/change-entry.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-12-terms-index-metadata-drift/change-entry.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-13-change-entry-base-sha-drift/change-entry.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-11-cycles-index-metadata-drift/{merge,release}-receipt.md`, `release-report.md`, `verify-findings.json`, `verify-report.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-12-terms-index-metadata-drift/verify-findings.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-13-change-entry-base-sha-drift/verify-findings.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-14-m9-11-fabricated-sha/{merge,release}-receipt.md`
- `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` (add m9-17 row + bump counters)
- `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` (Last archive → m9-17)
- `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md` (cross-check #10)
