# Release Report — m9-15

**Cycle**: m9-15-m9-12-m9-13-short-sha
**Path**: B-direct


## Summary

Doc-only cycle that expands short 7-character SHAs in m9-12 and m9-13
cycle artifacts to full 40-character SHAs (matching the format used
in `main_sha` and `base_sha` of those same files), corrects the same
SHAs in `cycles/index.md` Published SHA columns, and tightens
cross-check #3 in the standing vault-drift-sweep procedure to
explicitly require full-SHA storage.

## Diff stats

```
.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md                                       |  9 ++---
.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md                      | 24 ++++++----
.sddk-knowledge/p-3416cfb8288f8964/terms/index.md                                        |  4 +-
.../m9-12-terms-index-metadata-drift/apply-checkpoint.json                              |  4 +-
.../m9-12-terms-index-metadata-drift/merge-receipt.md                                   |  6 +--
.../m9-12-terms-index-metadata-drift/release-receipt.md                                 |  6 +--
.../m9-12-terms-index-metadata-drift/release-report.md                                  |  2 +-
.../m9-12-terms-index-metadata-drift/verify-report.md                                   |  4 +-
.../m9-13-change-entry-base-sha-drift/apply-checkpoint.json                              |  4 +-
.../m9-13-change-entry-base-sha-drift/merge-receipt.md                                   |  4 +-
.../m9-13-change-entry-base-sha-drift/release-receipt.md                                 |  6 +--
.../m9-13-change-entry-base-sha-drift/release-report.md                                  |  2 +-
.../m9-13-change-entry-base-sha-drift/verify-report.md                                   |  4 +-
13 files changed, 43 insertions(+), 36 deletions(-)
```

## Risks

None. Doc-only change, no production code touched.

## Gates

| Gate | Result |
|---|---|
| T0 (`cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings`) | PASS (0 warnings, 0 diffs) |
| Vault drift sweep (8 cross-checks; C3 now strict) | PASS (all 8 clean post-fix) |

## Findings

No findings introduced or closed. This is a hygiene cycle that closes
a documentation format inconsistency (short-SHA storage).

## Verdict

**RELEASED** at tag `v0.7.13`, head SHA `2441f6f3c679555dc4106ea2e8a422ed407a26a0`.
## Cross-checks

- C3: pass

