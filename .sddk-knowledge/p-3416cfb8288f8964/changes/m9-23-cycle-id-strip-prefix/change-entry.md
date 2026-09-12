# Change: m9-23 cycle_id workspace prefix strip


## Subject

- base_sha: `f5fbbd7584398d0fda1f090fe9295bcb9fdde045`
- head_sha: `b549c7462932647b03cd9cd2f28da0fc03c9ed64`
- cycle: m9-23
- tag: `v0.7.21`
- route: B-direct
- date: 2026-09-12


## Summary

16 prior CLOSED cycles (m9-03..m9-18) had `cycle_id` set to the
full path `p-3416cfb8288f8964/m9-NN-slug`. m9-19+ use bare slugs
without the workspace prefix. m9-23 strips the prefix from the 16
prior cycles.

## Cross-check

Cross-check #16 added to vault-drift-sweep.md.

## Files changed

- `cycle-artifacts/p-3416cfb8288f8964/m9-23-cycle-id-strip-prefix/apply-checkpoint.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-23-cycle-id-strip-prefix/merge-receipt.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-23-cycle-id-strip-prefix/release-receipt.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-23-cycle-id-strip-prefix/release-report.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-23-cycle-id-strip-prefix/verify-findings.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-23-cycle-id-strip-prefix/verify-report.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-23-cycle-id-strip-prefix/archive-manifest.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-23-cycle-id-strip-prefix/change-entry.md`
- `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`


## Verification

- T0 gate: cargo fmt --check + cargo clippy -- -D warnings → PASS
- All 16 cross-checks → PASS
