# Release Report — m9-43

**Cycle**: m9-43 verify-findings-head-sha-sync
**Path**: B-direct
**Tag**: v0.7.41

## What changed

One drift class closed:

1. **verify-findings.json subject.head_sha mismatch** (2 files: m9-34, m9-35).
   Both had fix-commit SHAs; should have docs-commit SHAs. Synced.

## Cross-checks added

- C35: verify-findings.json subject.head_sha must match apply-checkpoint.json head_sha.

## Verification

- C35: pass (after fixes applied)
- C1-C34: pass
- T0 fmt+clippy: clean
- T4 sandbox smoke: not run (metadata-only cycle)
