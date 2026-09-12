# Verify Report — m9-37

## Summary

Two drift classes closed:

1. **apply-checkpoint.json `path` vs `route`** (31 files: m9-03..m9-33).
   These cycles had `"path": "B-direct"` in apply-checkpoint. m9-21+
   added `route` alongside `path` (both fields, same value). m9-34 dropped
   `path` entirely. m9-37 normalizes m9-03..m9-33 to `route`-only.

2. **change-entry.md Subject head_sha drift** (2 files: m9-34, m9-35).
   These change-entry.md Subject sections had head_sha values pointing
   to intermediate commits, not the cycle's final HEAD. m9-37 fixes both.

Cross-check #29 added to vault-drift-sweep.md.

## Findings

| ID | Severity | Category | Description | Status |
|---|---|---|---|---|
| F1 | low | schema-drift | 31 apply-checkpoint.json files (m9-03..m9-33) had `path` field instead of canonical `route`. | RESOLVED |
| F2 | low | metadata-drift | 2 change-entry.md files (m9-34, m9-35) had Subject `head_sha` pointing to intermediate commits instead of final HEAD. | RESOLVED |
| F3 | informational | process | Added cross-check #29 to vault-drift-sweep.md enforcing both schemas. | RESOLVED |

## Subject

- base_sha: `5225467421097d7b283a2b3e21d0ce38018c58bd`
- head_sha: see release-receipt
- cycle: m9-37
- branch: `fix/m9-37-path-route-and-sha-normalize`

## Cross-checks

- C29: pass (after fixes applied)
- C1-C28: pass
