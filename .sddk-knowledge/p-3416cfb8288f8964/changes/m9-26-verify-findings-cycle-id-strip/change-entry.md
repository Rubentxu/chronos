# Change: m9-26 verify-findings.json cycle_id prefix strip


## Subject

- base_sha: `38e0e3aaacc9471b1c2550e548f2c24ae3f71025`
- head_sha: `a7d43e012be509fc44edae1ab2f05397e3452037`
- cycle: m9-26
- tag: `v0.7.24`
- route: B-direct
- date: 2026-09-12


## Summary

m9-23 stripped workspace prefix from apply-checkpoint.json cycle_id
but missed verify-findings.json. m9-04 verify-findings.json had
'p-3416cfb8288f8964/m9-04-side-table-key-layout'.

m9-26 strips the prefix and extends cross-check #16.

## Cross-check

Cross-check #16 extended to cover verify-findings.json.

## Files changed

- `cycle-artifacts/p-3416cfb8288f8964/m9-26-verify-findings-cycle-id-strip/apply-checkpoint.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-26-verify-findings-cycle-id-strip/merge-receipt.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-26-verify-findings-cycle-id-strip/release-receipt.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-26-verify-findings-cycle-id-strip/release-report.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-26-verify-findings-cycle-id-strip/verify-findings.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-26-verify-findings-cycle-id-strip/verify-report.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-26-verify-findings-cycle-id-strip/archive-manifest.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-26-verify-findings-cycle-id-strip/change-entry.md`
- `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`


## Verification

- T0 gate: PASS
- All 18 cross-checks: PASS
