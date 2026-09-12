# Change: m9-24 verify-findings schema normalization


## Subject

- base_sha: `dd389257d6fdf402a21845449e7d66727beafa6e`
- head_sha: `8e0bfbf79d761452d7c186d855066b54a8176ace`
- cycle: m9-24
- tag: `v0.7.22`
- route: B-direct
- date: 2026-09-12


## Summary

8 prior CLOSED cycles (m9-11..m9-18) had verify-findings.json with
legacy schema (subject_sha at top level + lens_summary + verdict +
evidence). m9-19+ use the simpler new schema with subject dict.

m9-24 normalizes the 8 prior cycles. Legacy fields are preserved
under _legacy for traceability.

## Cross-check

Cross-check #17 added to vault-drift-sweep.md.

## Files changed

- `cycle-artifacts/p-3416cfb8288f8964/m9-24-verify-findings-schema-normalize/apply-checkpoint.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-24-verify-findings-schema-normalize/merge-receipt.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-24-verify-findings-schema-normalize/release-receipt.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-24-verify-findings-schema-normalize/release-report.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-24-verify-findings-schema-normalize/verify-findings.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-24-verify-findings-schema-normalize/verify-report.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-24-verify-findings-schema-normalize/archive-manifest.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-24-verify-findings-schema-normalize/change-entry.md`
- `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`


## Verification

- T0 gate: cargo fmt --check + cargo clippy -- -D warnings → PASS
- All 17 cross-checks → PASS
