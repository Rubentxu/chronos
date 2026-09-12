# Verify Report — m9-50-tag-and-peel-reconcile

**Path**: B-direct

## Summary

Three drift classes closed:

1. **Recreated missing tags**: v0.7.31 at 837bc5c44..., v0.7.43 at 3e70b346...
2. **Reconciled 47 release-receipt.md peel values**: now use `vN^{commit}` instead of `git rev-parse` (which returns tag-object SHA).
3. **Updated index timestamps**: terms and cycles index both 2026-09-12T14:30Z.

Cross-check #42 added.

## Findings

| ID | Severity | Category | Description | Status |
|---|---|---|---|---|
| F1 | low | schema-drift | v0.7.31 and v0.7.43 tags missing from git. | RESOLVED |
| F2 | medium | schema-drift | 47 release-receipt.md had wrong peel (tag-object SHA). | RESOLVED |
| F3 | informational | metadata-drift | terms/cycles index Last updated stale. | RESOLVED |
| F4 | informational | process | Added cross-check #42. | RESOLVED |

## Subject

- base_sha: `109e302286e6e53dfe51c8296dcfe59c16d77c3a`
- head_sha: see release-receipt
- cycle: m9-50
- branch: `fix/m9-50-tag-and-peel-reconcile`

## Cross-checks

- C42: pass (after fixes applied)
- C1-C41: pass
