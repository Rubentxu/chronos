# Change: m9-38 four-class drift closure

## Summary

Four drift classes closed:

1. **verify-report.md title format** (29 files: m9-03..m9-37).
   Three different title formats used across eras. Normalized all
   to canonical `# Verify Report — m9-NN`.
2. **verify-findings.json verdict field** (24 files: m9-03..m9-27).
   Legacy schema had `summary.verdict` nested. Added top-level
   `verdict`.
3. **archive-manifest.md Cycle field** (18 files: m9-01..m9-18).
   Renamed `Cycle ID` to `Cycle`.
4. **change-entry.md Summary section** (18 files: m9-01..m9-18).
   Replaced Spanish `## Ciclo` with `## Summary`.

## Cross-check added

- **C30**: verify-report.md title, verify-findings verdict,
  archive-manifest Cycle field, change-entry Summary section.

## Files changed

- 29 verify-report.md files (m9-03..m9-37): title format normalize
- 24 verify-findings.json files (m9-03..m9-27): verdict field
- 18 archive-manifest.md files (m9-01..m9-18): Cycle field
- 18 change-entry.md files (m9-01..m9-18): Summary section
- 1 vault-drift-sweep.md: added cross-check #30
- 6 new cycle artifacts for m9-38

## Subject

- base_sha: `6bc6781465f9dff5a59bd4d8e8a99930dba3e7e5`
- head_sha: `e8f805f350b47b7b1cad6c1ac507ae868b13c07e`
- cycle: m9-38
- branch: `fix/m9-38-verify-report-title-format-normalize`
- date: 2026-09-12
- tag: `v0.7.36`
