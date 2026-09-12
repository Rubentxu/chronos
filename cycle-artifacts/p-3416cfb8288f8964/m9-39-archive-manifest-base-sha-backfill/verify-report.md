# Verify Report — m9-39

## Summary

Two drift classes closed:

1. **archive-manifest.md Base SHA field** (16 files: m9-11..m9-27).
   These archive-manifest.md files used a header table format that
   didn't include the `Base SHA` field. Added `| Base SHA |` line right
   after the `| Head SHA |` line, sourced from apply-checkpoint.json.

2. **verify-report.md Cross-checks section** (1 file: m9-34).
   m9-34's verify-report.md was written before the `## Cross-checks`
   section was standardized (introduced in m9-32). Added the section.

Cross-check #31 added to vault-drift-sweep.md.

## Findings

| ID | Severity | Category | Description | Status |
|---|---|---|---|---|
| F1 | low | schema-drift | 16 archive-manifest.md files (m9-11..m9-27) were missing the `Base SHA` field in the header table. | RESOLVED |
| F2 | low | schema-drift | m9-34 verify-report.md was missing `## Cross-checks` section. | RESOLVED |
| F3 | informational | process | Added cross-check #31 to vault-drift-sweep.md enforcing both schemas. | RESOLVED |

## Subject

- base_sha: `32f62d2462182faa241799e305907e761f211f85`
- head_sha: see release-receipt
- cycle: m9-39
- branch: `fix/m9-39-archive-manifest-base-sha-and-verify-report-cross-checks`

## Cross-checks

- C31: pass (after fixes applied)
- C1-C30: pass
