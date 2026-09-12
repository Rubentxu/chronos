# Release Report — m9-40

**Cycle**: m9-40 release-report-path-and-cross-checks-backfill
**Path**: B-direct
**Tag**: v0.7.38

## What changed

Two drift classes closed:

1. **release-report.md Path field** (22 files: m9-03..m9-33).
   Added `**Path**: <route>` line to each.
2. **release-report.md Cross-checks section** (17 files: m9-11..m9-27).
   Added `## Cross-checks` section listing cross-checks that passed.

## Cross-checks added

- C32: release-report.md must have `Path` field and
  `## Cross-checks` / `## Verification` section.

## Verification

- C32: pass (after fixes applied)
- C1-C31: pass
- T0 fmt+clippy: clean
- T4 sandbox smoke: not run (metadata-only cycle)
