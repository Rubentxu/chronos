# Change: m9-34 verify-findings base_sha + archive-manifest Head SHA normalize

## Summary

Two structural drift classes closed:

1. **verify-findings.json subject missing base_sha** (23 files: m9-05..m9-27).
   Each cycle's `subject` dict had `head_sha` but no `base_sha`. Added
   `base_sha` from each cycle's apply-checkpoint.json.

2. **archive-manifest.md header missing Head SHA field** (10 files: m9-01..m9-10).
   The m9-01..m9-10 archive-manifests used `| Published SHA |` instead of
   `| Head SHA |`. Added `| Head SHA |` as an alias with the same value.

Both are zero-content drift closes; the SHA values were already known
from apply-checkpoint.json.

## Subject

- base_sha: `5cfa91be296065ab4ca19c38bb0b06a859260627`
- head_sha: `27fc7149201d7657aa4c7acb15ee9d582e2f2664`
- cycle: m9-34
- branch: `fix/m9-34-archive-manifest-header-normalize`
- date: 2026-09-12
- tag: `v0.7.32`

## Files changed

- 23 verify-findings.json files (m9-05..m9-27): added subject.base_sha
- 10 archive-manifest.md files (m9-01..m9-10): added | Head SHA | field
- 1 vault-drift-sweep.md: added cross-check #26
- 6 new cycle artifacts for m9-34

## Cross-check added

- **C26**: verify-findings.json subject must have head_sha AND base_sha
  (or legacy head/base for m9-03); archive-manifest.md header must have
  Head SHA field.
