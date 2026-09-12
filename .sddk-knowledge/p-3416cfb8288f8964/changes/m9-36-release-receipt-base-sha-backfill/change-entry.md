# Change: m9-36 release-receipt Base SHA backfill + format normalize

## Summary

Two related drift classes closed in m9-03..m9-33 release-receipt.md:

1. **Missing Base SHA field** (31 files). The markdown table format used
   in m9-03..m9-33 omitted the `Base SHA` field.
2. **Format drift** — pipe-separated canonical format (introduced in
   m9-34) vs markdown table format (used m9-03..m9-33).

Both classes fixed by normalizing m9-03..m9-33 to the canonical pipe-separated
format with all fields: Cycle, Base SHA, Head SHA, Branch, Date, Remote tag,
Remote tag_peel, Peel match.

## Subject

- base_sha: `d7fd733064999cf93eea429a78939a02a1d4d27c`
- head_sha: `ced90eccf62f10e27ef0d76cab5b83bfc933172c`
- cycle: m9-36
- branch: `fix/m9-36-release-receipt-base-sha-backfill`
- date: 2026-09-12
- tag: `v0.7.34`

## Files changed

- 31 release-receipt.md files (m9-03..m9-33): format + Base SHA field
- 1 vault-drift-sweep.md: added cross-check #28
- 6 new cycle artifacts for m9-36

## Cross-check added

- **C28**: release-receipt.md must have `Base SHA` field.
