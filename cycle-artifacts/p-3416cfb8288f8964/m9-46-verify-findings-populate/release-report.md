# Release Report — m9-46-verify-findings-populate

**Cycle**: m9-46-verify-findings-populate
**Path**: B-direct
**Tag**: v0.7.44

## What changed

Drift class closed: 12 verify-findings.json files (m9-34..m9-45) had
empty `findings` arrays. Populated from verify-report.md tables.

## Cross-checks added

- C38: verify-findings findings array consistency.

## Verification

- C38: pass (after fixes applied)
- C1-C37: pass
- T0 fmt+clippy: clean
- T4 sandbox smoke: not run (metadata-only cycle)
