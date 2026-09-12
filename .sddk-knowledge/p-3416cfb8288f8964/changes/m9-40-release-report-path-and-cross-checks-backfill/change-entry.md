# Change: m9-40 release-report Path + Cross-checks backfill

## Summary

Two drift classes closed:

1. **release-report.md Path field** (22 files: m9-03..m9-33).
   Added `**Path**: <route>` line to each.
2. **release-report.md Cross-checks section** (17 files: m9-11..m9-27).
   Added `## Cross-checks` section listing cross-checks that passed.

## Cross-check added

- **C32**: release-report.md must have `Path` field and
  `## Cross-checks` / `## Verification` section.

## Files changed

- 22 release-report.md files (m9-03..m9-33): added Path
- 17 release-report.md files (m9-11..m9-27): added Cross-checks
- 1 vault-drift-sweep.md: added cross-check #32
- 6 new cycle artifacts for m9-40

## Subject

- base_sha: `9170deacbe463c0e81886b9083ef5920006afc3a`
- head_sha: `839e9d3b06e3823bb0bbee99c3cbab3ed5a8d8f1`
- cycle: m9-40
- branch: `fix/m9-40-release-report-path-and-cross-checks`
- date: 2026-09-12
- tag: `v0.7.38`
