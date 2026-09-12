# Verify Report — m9-34

## Summary

Two drift classes closed:

1. **verify-findings.json subject missing base_sha** (23 files: m9-05..m9-27).
   The `subject` dict only had `head_sha`; the `base_sha` field was added
   from the corresponding apply-checkpoint.json. m9-03 keeps its legacy
   `subject.base` / `subject.head` schema (accepted-by-design per C17).
2. **archive-manifest.md header missing Head SHA field** (10 files: m9-01..m9-10).
   The m9-01..m9-10 archive-manifests used `| Published SHA |` instead of
   `| Head SHA |`. Added `| Head SHA |` as an alias with the same value.

Cross-check #26 added to vault-drift-sweep.md.

## Findings

| ID | Severity | Category | Description | Status |
|---|---|---|---|---|
| F1 | low | schema-drift | 23 verify-findings.json files (m9-05..m9-27) had `subject` dict missing `base_sha`; only `head_sha` was present. | RESOLVED |
| F2 | low | schema-drift | 10 archive-manifest.md files (m9-01..m9-10) had `| Published SHA |` instead of `| Head SHA |` in the header table. | RESOLVED |
| F3 | informational | process | Added cross-check #26 to vault-drift-sweep.md enforcing both drift classes. | RESOLVED |

## Subject

- base_sha: `7ffa57ab6bed79c9300b35d81a323e0f874746b2`
- head_sha: see release-receipt
- cycle: m9-34
- branch: `fix/m9-34-archive-manifest-header-normalize`

## Cross-checks

- C26: pass (after fixes applied)
- C1-C25: pass
