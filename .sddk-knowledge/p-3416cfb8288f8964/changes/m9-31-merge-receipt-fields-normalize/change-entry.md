# Change: m9-31 Merge receipt fields normalization

| Field | Value |
|---|---|
| Cycle | m9-31-merge-receipt-fields-normalize |
| Base SHA | `6c428f7` |
| Head SHA | `5c286b6` |
| Tag | `v0.7.29` (peels to `5c286b6`) |
| Path | B-direct |
| Date | 2026-09-12 |
| Author | jcode (auto-mode B-direct) |

## Summary

Merge-receipt.md files for m9-19 through m9-27 were authored with a
minimal format containing only `Cycle`, `Path`, `Base SHA`, `Branch`,
`Merge target`, `Merged at` — no Head SHA field at all.

m9-03..m9-10 used a different format with `Main SHA` (different field
name) instead of `Head SHA`.

m9-11..m9-18 used Spanish `Campo` header.

m9-28+ used the canonical English format.

## Fix

1. Normalized all 25 prior merge-receipts (m9-03 through m9-27) to
   the canonical format established by m9-28+:

   ```
   | Field | Value |
   |---|---|
   | Cycle | <slug> |
   | Base SHA | `<base>` |
   | Head SHA | `<head>` |
   | Branch | `<slug>` |
   | Date | <archived_at> |
   ```

2. Added cross-check #23 that explicitly checks **both** field
   presence AND SHA consistency in merge-receipt.md (parallel to C22
   for release-receipt.md).

## Cross-check

Cross-check #23 added to
`.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`.

## Verification

- All 23 cross-checks: PASS
- 25 merge-receipt.md files each verified to have the canonical SHA
  fields, with SHAs matching apply-checkpoint.json

## Lessons

The drift was caused by an agent who authored m9-19..m9-27 with a
different merge-receipt structure than the original convention.
m9-28+ (this session) used the canonical format; m9-31 backfills it
for the 25 prior cycles.

The deeper lesson: **silent gaps are dangerous**. There was no
cross-check for merge-receipt.md SHA fields at all (C10 only checked
release-receipt.md). The fact that C22 found drift in release-receipt
made me realize that the same drift pattern would apply to
merge-receipt. C23 closes this gap.

## Risk

None. Vault metadata only; no code or runtime behavior affected.
