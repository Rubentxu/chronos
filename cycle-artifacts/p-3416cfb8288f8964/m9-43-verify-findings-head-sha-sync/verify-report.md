# Verify Report — m9-43

**Cycle**: m9-43-verify-findings-head-sha-sync
**Path**: B-direct


## Summary

One drift class closed:

1. **verify-findings.json subject.head_sha mismatch** (2 files: m9-34, m9-35).
   Both had fix-commit SHAs (since tags peel to fix commits) but
   should have docs-commit SHAs (per fix-peel convention). Synced
   to match apply-checkpoint.json head_sha.

Cross-check #35 added to vault-drift-sweep.md.

m9-03, m9-04 are exempt — they use legacy `sddk.verify-finding/v1`
schema with `subject.head` = fix commit (docs-peel convention).

## Findings

| ID | Severity | Category | Description | Status |
|---|---|---|---|---|
| F1 | low | metadata-drift | m9-34 verify-findings.json subject.head_sha was fix commit SHA; should be docs commit SHA. | RESOLVED |
| F2 | low | metadata-drift | m9-35 verify-findings.json subject.head_sha was fix commit SHA; should be docs commit SHA. | RESOLVED |
| F3 | informational | process | Added cross-check #35 to vault-drift-sweep.md. | RESOLVED |

## Subject

- base_sha: `19074fc24e0f545ff2200dd8dfafe74863af79c0`
- head_sha: see release-receipt
- cycle: m9-43
- branch: `fix/m9-43-verify-findings-head-sha-sync`

## Cross-checks

- C35: pass (after fixes applied)
- C1-C34: pass
