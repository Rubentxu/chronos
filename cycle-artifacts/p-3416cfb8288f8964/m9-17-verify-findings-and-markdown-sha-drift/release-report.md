# Release Report — m9-17-verify-findings-and-markdown-sha-drift

## Summary

Doc-only cycle that closes residual SHA drift across 4 file types
(verify-findings.json subject_sha, release-receipt.md Head/Remote
tag_peel, merge-receipt.md Head SHA, change-entry.md Head SHA) across
4 prior cycles (m9-11, m9-12, m9-13, m9-14). m9-17 also adds
cross-check #10 to the standing vault-drift-sweep procedure.

## Diff stats

```
.sddk-knowledge/p-3416cfb8288f8964/changes/m9-11-cycles-index-metadata-drift/change-entry.md       |   6 +-
.sddk-knowledge/p-3416cfb8288f8964/changes/m9-12-terms-index-metadata-drift/change-entry.md         |   6 +-
.sddk-knowledge/p-3416cfb8288f8964/changes/m9-13-change-entry-base-sha-drift/change-entry.md       |   8 +-
.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md                                                 |   5 +-
.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md                                |  91 +++++++++++++++++++++-
.sddk-knowledge/p-3416cfb8288f8964/terms/index.md                                                  |   4 +-
cycle-artifacts/p-3416cfb8288f8964/m9-11-cycles-index-metadata-drift/merge-receipt.md              |   4 +-
cycle-artifacts/p-3416cfb8288f8964/m9-11-cycles-index-metadata-drift/release-receipt.md            |   6 +-
cycle-artifacts/p-3416cfb8288f8964/m9-11-cycles-index-metadata-drift/release-report.md             |   2 +-
cycle-artifacts/p-3416cfb8288f8964/m9-11-cycles-index-metadata-drift/verify-findings.json          |   2 +-
cycle-artifacts/p-3416cfb8288f8964/m9-11-cycles-index-metadata-drift/verify-report.md              |   8 +-
cycle-artifacts/p-3416cfb8288f8964/m9-12-terms-index-metadata-drift/verify-findings.json           |   2 +-
cycle-artifacts/p-3416cfb8288f8964/m9-13-change-entry-base-sha-drift/verify-findings.json          |   2 +-
cycle-artifacts/p-3416cfb8288f8964/m9-14-m9-11-fabricated-sha/merge-receipt.md                     |   4 +-
cycle-artifacts/p-3416cfb8288f8964/m9-14-m9-11-fabricated-sha/release-receipt.md                   |   2 +-
15 files changed, 119 insertions(+), 33 deletions(-)
```

## Risks

None. Doc-only change.

## Gates

| Gate | Result |
|---|---|
| T0 (cargo fmt + clippy) | PASS (0 warnings, 0 diffs) |
| Vault drift sweep (10 cross-checks; C10 new) | PASS (all 10 clean post-fix) |

## Findings

No findings introduced or closed.

## Verdict

**RELEASED** at tag `v0.7.15`, head SHA `134dc7525312275c447db8f5996740ff7c102a02`.
