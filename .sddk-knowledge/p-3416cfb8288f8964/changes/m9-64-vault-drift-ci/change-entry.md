# Change: m9-64 vault drift CI workflow

## Summary

Added CI automation for the 52 vault CCs. Before m9-64, the CCs only ran manually during SDDK cycles — any regression in vault files would go undetected until the next session's drift sweep. Now: `.github/workflows/vault-drift.yml` executes `scripts/check_vault_drift.sh` (which runs CC#48 meta-check) on every push to main and every PR touching vault files.

## Ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-64-vault-drift-ci` |
| Path | B-direct |
| Status | CLOSED |
| Base SHA | `598c914b4427657c264949037787cbae191db533` |
| Head SHA | `339f7b5e806167550355413cf570507925b774cf` |
| Tag | `v0.7.66` |

## Subject

- base_sha: `598c914b4427657c264949037787cbae191db533`
- head_sha: `339f7b5e806167550355413cf570507925b774cf`
- cycle: m9-64
- branch: `feat/m9-64-vault-drift-ci`
- date: 2026-09-13
- tag: `v0.7.66`
- findings_closed: 1 (FIND-M9-64-NO-VAULT-CI)
- findings_introduced.no_action: 0

## Files changed

- (new) `.github/workflows/vault-drift.yml` — CI workflow (37 lines)
- (new) `scripts/check_vault_drift.sh` — bash entry point (61 lines)
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-64-vault-drift-ci/apply-checkpoint.json`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-64-vault-drift-ci/verify-findings.json`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-64-vault-drift-ci/verify-report.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-64-vault-drift-ci/merge-receipt.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-64-vault-drift-ci/release-receipt.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-64-vault-drift-ci/release-report.md`
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-64-vault-drift-ci/change-entry.md` (this file)
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-64-vault-drift-ci/archive-manifest.md`
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` (m9-64 row added)
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` (Last archive bumped)

## Cross-checks

CC#1..CC#52: pass (no drift). No new CC added — uses CC#48 meta-check as entry point.
