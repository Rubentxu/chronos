# Change: m9-22 created_at + title + summary backfill

| Field | Value |
|---|---|
| Cycle | m9-22-created-at-summary-title-backfill |
| Base SHA | `71e7d11` |
| Head SHA | `e87a25c` |
| Tag | `v0.7.20` (peels to `e87a25c`) |
| Path | B-direct |
| Date | 2026-09-12 |
| Author | jcode (auto-mode B-direct) |

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

## Verification

- T0 gate: cargo fmt --check + cargo clippy -- -D warnings → PASS
- All 15 cross-checks → PASS
