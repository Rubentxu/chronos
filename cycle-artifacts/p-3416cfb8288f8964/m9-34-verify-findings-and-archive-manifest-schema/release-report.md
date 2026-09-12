# Release Report — m9-34

**Cycle**: m9-34 verify-findings-and-archive-manifest-schema
**Path**: B-direct
**Tag**: v0.7.32

## What changed

Two structural drift classes closed:

1. **verify-findings.json subject schema** (23 files): m9-05..m9-27 had
   `subject.head_sha` but no `subject.base_sha`. Added `subject.base_sha`
   from each cycle's apply-checkpoint.json. m9-03 keeps the legacy
   `subject.base` / `subject.head` schema (accepted-by-design per C17).

2. **archive-manifest.md header** (10 files): m9-01..m9-10 used
   `| Published SHA |` instead of `| Head SHA |`. Added `| Head SHA |`
   as an alias with the same value so cross-references match m9-11+.

Both are zero-content drift closes; the SHA values were already known
from apply-checkpoint.json.

## Cross-checks added

- C26: verify-findings.json subject must have head/base AND
  archive-manifest.md header must have Head SHA.

## Verification

- C26: pass (after fixes applied)
- C1-C25: pass
- T0 fmt+clippy: clean
- T4 sandbox smoke: not run (metadata-only cycle)
