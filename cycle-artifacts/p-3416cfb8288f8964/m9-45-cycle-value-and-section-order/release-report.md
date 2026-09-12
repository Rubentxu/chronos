# Release Report — m9-45-cycle-value-and-section-order

**Cycle**: m9-45-cycle-value-and-section-order
**Path**: B-direct
**Tag**: v0.7.43

## What changed

Two drift classes closed:

1. **change-entry.md section order** (9 files: m9-34..m9-44).
2. **release-report.md cycle value** (11 files: m9-34..m9-44).

## Cross-checks added

- C37: change-entry section order + release-report cycle value.

## Verification

- C37: pass (after fixes applied)
- C1-C36: pass
- T0 fmt+clippy: clean
- T4 sandbox smoke: not run (metadata-only cycle)
