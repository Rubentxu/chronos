# Change: m9-20 Status fields backfill


## Subject

- base_sha: `d66569159fe3e47404aee3b4e13a3c5b883c7d69`
- head_sha: `07febc1bae0d236ad47eef3c5b3172c4824cde04`
- cycle: m9-20
- tag: `v0.7.18`
- route: B-direct
- date: 2026-09-12


## Summary

m9-19 introduced three new fields in `apply-checkpoint.json`:
`verify_status`, `release_status`, and `archive_status`. m9-19 set
these fields on its own apply-checkpoint.json but did not backfill
the 17 prior CLOSED cycles (m9-03..m9-18), which had them as `null`.

m9-20 closes this drift class by setting:

- `verify_status: "passed"`
- `release_status: "released"`
- `archive_status: "archived"`

on all 17 prior CLOSED cycles. Values are derived from the
`status: CLOSED` fact (a CLOSED cycle must have been verified,
released, and archived).

## Cross-check

Cross-check #13 added to
`.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`
enforcing these three fields on CLOSED cycles.

## Verification

- T0 gate: `cargo fmt --check` + `cargo clippy -- -D warnings` → PASS
- All 13 cross-checks → PASS (0 drift)

## Files changed

- `cycle-artifacts/p-3416cfb8288f8964/m9-20-status-fields-backfill/apply-checkpoint.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-20-status-fields-backfill/merge-receipt.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-20-status-fields-backfill/release-receipt.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-20-status-fields-backfill/release-report.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-20-status-fields-backfill/verify-findings.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-20-status-fields-backfill/verify-report.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-20-status-fields-backfill/archive-manifest.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-20-status-fields-backfill/change-entry.md`
- `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`


## Pattern

Each cycle that introduces a new schema field should also add a
cross-check that enforces the field. m9-19 added the field but no
cross-check (since the field wasn't yet "officially required"). m9-20
formalizes the field requirement and adds the cross-check.
