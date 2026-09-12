# Change: m9-37 apply-checkpoint path->route + change-entry SHA normalize

## Summary

Two drift classes closed:

1. **apply-checkpoint.json `path` vs `route`** (31 files: m9-03..m9-33).
   These cycles had `"path": "B-direct"` in apply-checkpoint. Canonical
   schema (m9-21+) uses `route`. m9-37 normalizes m9-03..m9-33 to
   `route`-only.

2. **change-entry.md Subject head_sha** (2 files: m9-34, m9-35).
   These change-entry.md Subject sections had head_sha pointing to
   intermediate commits. m9-37 fixes both to match final HEADs.

## Cross-check added

- **C29**: apply-checkpoint.json must use `route` (not `path`);
  change-entry.md Subject head_sha must match apply-checkpoint.json
  head_sha.

## Files changed

- 31 apply-checkpoint.json files (m9-03..m9-33): path -> route
- 2 change-entry.md files (m9-34, m9-35): Subject head_sha fix
- 1 vault-drift-sweep.md: added cross-check #29
- 6 new cycle artifacts for m9-37

## Subject

- base_sha: `5225467421097d7b283a2b3e21d0ce38018c58bd`
- head_sha: `0d570b3ff534fcecf490fbbda6384d5b30721384`
- cycle: m9-37
- branch: `fix/m9-37-path-route-and-sha-normalize`
- date: 2026-09-12
- tag: `v0.7.35`
