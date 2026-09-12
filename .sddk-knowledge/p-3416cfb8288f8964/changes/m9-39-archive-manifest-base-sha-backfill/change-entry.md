# Change: m9-39 archive-manifest Base SHA backfill + verify-report Cross-checks

## Summary

Two drift classes closed:

1. **archive-manifest.md Base SHA field** (16 files: m9-11..m9-27).
   Added `| Base SHA |` line to each, sourced from apply-checkpoint.json.
2. **verify-report.md Cross-checks section** (1 file: m9-34).
   Added `## Cross-checks` section listing C26, C1-C25.

## Cross-check added

- **C31**: archive-manifest.md must have `Base SHA` field;
  verify-report.md must have `## Cross-checks` section.

## Files changed

- 16 archive-manifest.md files (m9-11..m9-27): added Base SHA
- 1 verify-report.md (m9-34): added Cross-checks section
- 1 vault-drift-sweep.md: added cross-check #31
- 6 new cycle artifacts for m9-39

## Subject

- base_sha: `32f62d2462182faa241799e305907e761f211f85`
- head_sha: `75aec1a3ff6d06dffd8188c0efe5d59926809d2e`
- cycle: m9-39
- branch: `fix/m9-39-archive-manifest-base-sha-and-verify-report-cross-checks`
- date: 2026-09-12
- tag: `v0.7.37`
