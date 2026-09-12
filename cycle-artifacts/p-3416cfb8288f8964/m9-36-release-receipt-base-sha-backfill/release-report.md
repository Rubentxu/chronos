# Release Report — m9-36

**Cycle**: m9-36-release-receipt-base-sha-backfill
**Path**: B-direct
**Tag**: v0.7.34

## What changed

Two related drift classes closed in m9-03..m9-33 release-receipt.md:

1. **Missing Base SHA field** (31 files). The markdown table format used
   in m9-03..m9-33 omitted the `Base SHA` field.
2. **Format drift** — pipe-separated canonical format (introduced in
   m9-34) vs markdown table format (used m9-03..m9-33).

Both classes fixed by normalizing m9-03..m9-33 to the canonical pipe-separated
format with all fields: Cycle, Base SHA, Head SHA, Branch, Date, Remote tag,
Remote tag_peel, Peel match.

## Cross-checks added

- C28: release-receipt.md must have `Base SHA` field.

## Verification

- C28: pass (after fixes applied)
- C1-C27: pass
- T0 fmt+clippy: clean
- T4 sandbox smoke: not run (metadata-only cycle)

## Follow-up: title format normalize

While verifying m9-36, cross-check #27 caught additional drift in
release-report.md title format for m9-03..m9-18. Normalized all 16
release-report titles to canonical `# Release Report — m9-NN` format
in this same cycle.
