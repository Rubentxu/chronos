# Release Report — m9-50-tag-and-peel-reconcile

**Cycle**: m9-50-tag-and-peel-reconcile
**Path**: B-direct
**Tag**: v0.7.48

## What changed

Three drift classes closed:

1. **Recreated missing tags**: v0.7.31 (at 837bc5c44) and v0.7.43 (at 3e70b346).
2. **Reconciled 47 release-receipt peel values**: use `vN^{commit}`.
3. **Updated index timestamps**: terms and cycles index.

## Cross-checks added

- C42: release-receipt peel + tag existence + index timestamps.

## Verification

- C42: pass (after fixes applied)
- C1-C41: pass
- T0 fmt+clippy: clean
- T4 sandbox smoke: not run (metadata-only cycle)
