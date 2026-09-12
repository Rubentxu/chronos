# Change: m9-30 Release receipt fields normalization


## Subject

- base_sha: `f481a61169d3ec33a6ec0e236a938f231f303cf8`
- head_sha: `33acc58f9782c296f2f876cdfed280a024548188`
- cycle: m9-30
- tag: `v0.7.28`
- route: B-direct
- date: 2026-09-12


## Summary

Release-receipt.md files for m9-19 through m9-27 were authored with a
minimal format containing only `Cycle`, `Tag`, `Pee` (typo), and
`Released at` — no Head SHA field at all. This drift was hidden from
the original C10 cross-check because C10 only verified SHA consistency
when the field existed; m9-19..m9-27 had no Head SHA field, so C10
incorrectly reported PASS.

m9-03..m9-10 used a different format with `Peel SHA` (no `Head SHA`).
m9-11..m9-18 used Spanish `Campo` header.

## Fix

1. Normalized all 25 prior release-receipts (m9-03 through m9-27) to
   the canonical format established by m9-28:

   ```
   | Field | Value |
   |---|---|
   | Cycle | <slug> |
   | Head SHA | `<sha>` |
   | Remote tag | `<v0.7.N-2>` |
   | Remote tag_peel | `<sha>` |
   | Peel match | true |
   | Date | <released_at> |
   ```

2. Added cross-check #22 that explicitly checks **both** field presence
   AND SHA consistency in release-receipt.md. m9-30 closes this drift
   class.

## Cross-check

Cross-check #22 added to
`.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`.

## Verification

- All 22 cross-checks: PASS
- 25 release-receipt.md files each verified to have the canonical SHA
  fields, with SHAs matching apply-checkpoint.json

## Lessons

The drift was caused by an agent who authored m9-19..m9-27 with a
different release-receipt structure than the original convention.
m9-28 (this session) restored the canonical format; m9-30 backfills
it for the 9 prior cycles.

The C10 cross-check was **silently passing** because it didn't
distinguish between "field exists with wrong SHA" and "field doesn't
exist at all". C22 fixes this by checking field presence first, then
SHA consistency.

The deeper lesson: **silent passes are dangerous**. C10's regex
`re.search(rf'\| {field} \| `([a-f0-9]+)`', content)` returns `None`
when the field is absent, but the conditional `if m:` correctly skips
the comparison — so the test reports PASS even when the field is
absent. This is a subtle semantic difference: C10 means "no
disagreement found", not "field is present and correct". C22 makes
the presence check explicit.

## Files changed

- `cycle-artifacts/p-3416cfb8288f8964/m9-30-release-receipt-fields-normalize/apply-checkpoint.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-30-release-receipt-fields-normalize/merge-receipt.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-30-release-receipt-fields-normalize/release-receipt.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-30-release-receipt-fields-normalize/release-report.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-30-release-receipt-fields-normalize/verify-findings.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-30-release-receipt-fields-normalize/verify-report.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-30-release-receipt-fields-normalize/archive-manifest.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-30-release-receipt-fields-normalize/change-entry.md`
- `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`


## Risk

None. Vault metadata only; no code or runtime behavior affected.
