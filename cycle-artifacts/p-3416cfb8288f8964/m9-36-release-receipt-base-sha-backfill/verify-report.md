# Verify Findings — m9-36 release-receipt-base-sha-backfill

## Summary

Two related drift classes closed in m9-03..m9-33 release-receipt.md:

1. **Missing Base SHA field** (31 files). The markdown table format used
   in m9-03..m9-33 omitted the `Base SHA` field.
2. **Format drift** — pipe-separated canonical format (introduced in
   m9-34) vs markdown table format (used m9-03..m9-33).

Both classes fixed by normalizing m9-03..m9-33 to the canonical pipe-separated
format with all fields: Cycle, Base SHA, Head SHA, Branch, Date, Remote tag,
Remote tag_peel, Peel match.

Cross-check #28 added to vault-drift-sweep.md.

## Findings

| ID | Severity | Category | Description | Status |
|---|---|---|---|---|
| F1 | low | schema-drift | 31 release-receipt.md files (m9-03..m9-33) were missing the `Base SHA` field. | RESOLVED |
| F2 | low | schema-drift | 31 release-receipt.md files (m9-03..m9-33) used markdown table format instead of canonical pipe-separated format. | RESOLVED |
| F3 | informational | process | Added cross-check #28 to vault-drift-sweep.md enforcing `Base SHA` presence. | RESOLVED |

## Subject

- base_sha: `d7fd733064999cf93eea429a78939a02a1d4d27c`
- head_sha: see release-receipt
- cycle: m9-36
- branch: `fix/m9-36-release-receipt-base-sha-backfill`

## Cross-checks

- C28: pass (after fixes applied)
- C1-C27: pass
