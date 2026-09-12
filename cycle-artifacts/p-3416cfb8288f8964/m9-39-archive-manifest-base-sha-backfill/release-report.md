# Release Report — m9-39

**Cycle**: m9-39-archive-manifest-base-sha-backfill
**Path**: B-direct
**Tag**: v0.7.37

## What changed

Two drift classes closed:

1. **archive-manifest.md Base SHA field** (16 files: m9-11..m9-27).
   Added `| Base SHA |` line to each, sourced from apply-checkpoint.json.
2. **verify-report.md Cross-checks section** (1 file: m9-34).
   Added `## Cross-checks` section listing C26, C1-C25.

## Cross-checks added

- C31: archive-manifest.md must have `Base SHA` field;
  verify-report.md must have `## Cross-checks` section.

## Verification

- C31: pass (after fixes applied)
- C1-C30: pass
- T0 fmt+clippy: clean
- T4 sandbox smoke: not run (metadata-only cycle)
