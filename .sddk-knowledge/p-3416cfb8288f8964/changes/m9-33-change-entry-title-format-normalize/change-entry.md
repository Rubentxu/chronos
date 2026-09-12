# Change: m9-33 change-entry title format normalization


## Subject

- base_sha: `5bfcfedaf9687050a8090a31f999738ff082a081`
- head_sha: `837bc5c44a501e4c2253cc6fd257cc578bebf8e0`
- cycle: m9-33
- tag: `v0.7.31`
- route: B-direct
- date: 2026-09-12


## Summary

m9-01 through m9-18 change-entry.md files used `# Change Entry — <slug>`
format (slug-based). m9-19+ used `# Change: <human-readable>` format.
Some m9-19+ cycles had inconsistencies (mixed case, missing prefix).

## Fix

1. Normalized all 18 affected change-entry.md files (m9-01..m9-18)
   to the canonical `# Change: m9-NN <human-readable>` format:
   - Strip the `m9-NN-` prefix from the slug
   - Replace dashes with spaces
   - Add the `# Change:` prefix
   - Add the `m9-NN` prefix to the title

2. Added cross-check #25 to vault-drift-sweep.md enforcing the
   canonical title format.

## Cross-check

Cross-check #25 added to
`.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`.

## Verification

- All 25 cross-checks: PASS
- 18 change-entry.md files each verified to have the canonical title format

## Lessons

The drift was caused by an agent who authored m9-01..m9-18 with the
slug-based title format and later m9-19+ with the human-readable
format. m9-33 normalizes all 32 cycles to the canonical format and
adds C25 to enforce it.

The deeper lesson: **convention evolution is a form of drift**. When
the agent switched from slug-based to human-readable titles, it didn't
backfill the prior cycles. C25 enforces that future cycles stay in
the canonical format and that prior cycles can be migrated.

## Files changed

- `cycle-artifacts/p-3416cfb8288f8964/m9-33-change-entry-title-format-normalize/apply-checkpoint.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-33-change-entry-title-format-normalize/merge-receipt.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-33-change-entry-title-format-normalize/release-receipt.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-33-change-entry-title-format-normalize/release-report.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-33-change-entry-title-format-normalize/verify-findings.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-33-change-entry-title-format-normalize/verify-report.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-33-change-entry-title-format-normalize/archive-manifest.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-33-change-entry-title-format-normalize/change-entry.md`
- `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`


## Risk

None. Vault metadata only; no code or runtime behavior affected.
