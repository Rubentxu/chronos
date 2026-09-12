# Change: m9-31 Merge receipt fields normalization


## Subject

- base_sha: `6c428f76ec2f57bd210e936ab7b2f9a40e207d63`
- head_sha: `5c286b6b19684cc59a0c8e7935280051d14e0e54`
- cycle: m9-31
- tag: `v0.7.29`
- route: B-direct
- date: 2026-09-12


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

## Files changed

- `cycle-artifacts/p-3416cfb8288f8964/m9-31-merge-receipt-fields-normalize/apply-checkpoint.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-31-merge-receipt-fields-normalize/merge-receipt.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-31-merge-receipt-fields-normalize/release-receipt.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-31-merge-receipt-fields-normalize/release-report.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-31-merge-receipt-fields-normalize/verify-findings.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-31-merge-receipt-fields-normalize/verify-report.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-31-merge-receipt-fields-normalize/archive-manifest.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-31-merge-receipt-fields-normalize/change-entry.md`
- `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`


## Risk

None. Vault metadata only; no code or runtime behavior affected.
