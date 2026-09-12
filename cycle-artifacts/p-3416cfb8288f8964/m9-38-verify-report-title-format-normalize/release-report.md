# Release Report — m9-38

**Cycle**: m9-38-verify-report-title-format-normalize
**Path**: B-direct
**Tag**: v0.7.36

## What changed

Four drift classes closed:

1. **verify-report.md title format** (29 files: m9-03..m9-37). Three
   different title formats used across eras. Normalized all to canonical
   `# Verify Report — m9-NN`.
2. **verify-findings.json verdict field** (24 files: m9-03..m9-27).
   Legacy schema had `summary.verdict` nested. Added top-level `verdict`.
3. **archive-manifest.md Cycle field** (18 files: m9-01..m9-18). Renamed
   `Cycle ID` to `Cycle`.
4. **change-entry.md Summary section** (18 files: m9-01..m9-18). Replaced
   Spanish `## Ciclo` with `## Summary`.

## Cross-checks added

- C30: verify-report.md title, verify-findings verdict, archive-manifest
  Cycle field, change-entry Summary section.

## Verification

- C30: pass (after fixes applied)
- C1-C29: pass
- T0 fmt+clippy: clean
- T4 sandbox smoke: not run (metadata-only cycle)
