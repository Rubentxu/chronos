# Release Report — m9-37

**Cycle**: m9-37-path-route-and-sha-normalize
**Path**: B-direct
**Tag**: v0.7.35

## What changed

Two drift classes closed:

1. **apply-checkpoint.json `path` → `route`** (31 files: m9-03..m9-33).
   These cycles had `"path": "B-direct"` in apply-checkpoint. Canonical
   schema (m9-21+) uses `route`. m9-37 normalizes m9-03..m9-33 to
   `route`-only.

2. **change-entry.md Subject head_sha** (2 files: m9-34, m9-35).
   These change-entry.md Subject sections had head_sha pointing to
   intermediate commits. m9-37 fixes both to match final HEADs.

## Cross-checks added

- C29: apply-checkpoint.json must use `route` (not `path`);
  change-entry.md Subject head_sha must match apply-checkpoint.json
  head_sha.

## Verification

- C29: pass (after fixes applied)
- C1-C28: pass
- T0 fmt+clippy: clean
- T4 sandbox smoke: not run (metadata-only cycle)
