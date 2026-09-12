# Change Entry — m9-55-apply-checkpoint-fabricated-sha

## Summary

m9-38's `base_sha` field referenced a non-existent commit (`6bc6781465f9dff5a59bd4d8e8a99930dba3e7e5`) in 7 artifact files. Replaced with the real parent-of-head in the cycle branch: `6bc67812d66548a3e0ee48f5323d45c4a1ed13d8`. Detection: `git cat-file -e <base_sha>` for every apply-checkpoint during cross-check sweep; m9-38 failed. Pre-existing cross-checks (m9-09..m9-54) validated SHA format (length/hex) but not reachability. m9-55 adds cross-check #47 to validate existence.

## Ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-55-apply-checkpoint-fabricated-sha` |
| Path | B-direct |
| Status | CLOSED |
| Base SHA | `cbb9384` |
| Head SHA | (f6733fdfe38b3c405dc74c9e92450b2976957153 — see release-report) |

## Subject

- base_sha: `cbb93847228a9062de8e093dfe452c257233228c`
- head_sha: `f6733fdfe38b3c405dc74c9e92450b2976957153`
- cycle: m9-55
- branch: `fix/m9-55-apply-checkpoint-fabricated-sha`
- date: 2026-09-12
- tag: `v0.7.53`
- findings_closed: 1 (FIND-M9-55-FABRICATED-BASE-SHA)
- findings_introduced.no_action: 0

## Files

- `cycle-artifacts/p-3416cfb8288f8964/m9-38-verify-report-title-format-normalize/apply-checkpoint.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-38-verify-report-title-format-normalize/merge-receipt.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-38-verify-report-title-format-normalize/release-receipt.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-38-verify-report-title-format-normalize/verify-findings.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-38-verify-report-title-format-normalize/verify-report.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-38-verify-report-title-format-normalize/change-entry.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-38-verify-report-title-format-normalize/archive-manifest.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-55-*/apply-checkpoint.json`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-55-*/verify-findings.json`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-55-*/verify-report.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-55-*/merge-receipt.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-55-*/release-receipt.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-55-*/release-report.md`
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-55-*/change-entry.md` (this file)
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-55-*/archive-manifest.md`
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md` (cross-check #47)
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` (m9-55 row)

## Cross-checks

none added by m9-55 cross-check section. m9-55 added new cross-check #47 (apply-checkpoint base_sha must exist in git). See vault-drift-sweep.md.
