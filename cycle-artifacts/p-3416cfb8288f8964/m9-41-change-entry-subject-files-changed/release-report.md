# Release Report — m9-41

**Cycle**: m9-41 change-entry-subject-files-changed
**Path**: B-direct
**Tag**: v0.7.39

## What changed

Four drift classes closed:

1. **change-entry.md ## Subject section** (33 files).
2. **change-entry.md ## Files changed section** (33 files).
3. **archive-manifest.md ## Summary section** (27 files).
4. **verify-report.md ## Subject section** (6 files).

Short SHAs expanded to full 40-char SHAs in change-entry Subject sections.

## Cross-checks added

- C33: change-entry Subject/Files + archive-manifest Summary + verify-report Subject.

## Verification

- C33: pass (after fixes applied)
- C1-C32: pass
- T0 fmt+clippy: clean
- T4 sandbox smoke: not run (metadata-only cycle)
