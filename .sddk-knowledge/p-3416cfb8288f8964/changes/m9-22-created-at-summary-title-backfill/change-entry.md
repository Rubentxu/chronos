# Change: m9-22 created_at + title + summary backfill


## Subject

- base_sha: `71e7d1171d40b6f137504db470c96e65a9be7f17`
- head_sha: `e87a25c29b4529b94e7823430e1cd1df3b5cbd0a`
- cycle: m9-22
- tag: `v0.7.20`
- route: B-direct
- date: 2026-09-12


## Summary

m9-19 introduced three new fields in apply-checkpoint.json
(`created_at`, `title`, `summary`) but did not backfill the 16
prior CLOSED cycles (m9-03..m9-18), which had them missing.

m9-22 closes this drift by backfilling:

- `created_at`: extracted from git log (first commit touching the
  apply-checkpoint.json file), normalized from +02:00 local time to
  UTC.
- `title`: derived from the cycle folder slug (kebab-case → spaces).
- `summary`: extracted from the cycle's merge-receipt.md first
  non-heading, non-table, non-code paragraph.

## Cross-check

Cross-check #15 added to vault-drift-sweep.md.

## Files changed

- `cycle-artifacts/p-3416cfb8288f8964/m9-22-created-at-summary-title-backfill/apply-checkpoint.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-22-created-at-summary-title-backfill/merge-receipt.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-22-created-at-summary-title-backfill/release-receipt.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-22-created-at-summary-title-backfill/release-report.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-22-created-at-summary-title-backfill/verify-findings.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-22-created-at-summary-title-backfill/verify-report.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-22-created-at-summary-title-backfill/archive-manifest.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-22-created-at-summary-title-backfill/change-entry.md`
- `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`


## Verification

- T0 gate: cargo fmt --check + cargo clippy -- -D warnings → PASS
- All 15 cross-checks → PASS
