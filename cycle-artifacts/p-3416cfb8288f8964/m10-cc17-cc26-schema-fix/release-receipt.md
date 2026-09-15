# Release Receipt — m10-cc17-cc26-schema-fix

**Cycle**: `p-3416cfb8288f8964/m10-cc17-cc26-schema-fix`
**Path**: B-direct
**Published subject (main HEAD)**: `ab0b873179e18ca3c9ccc04d47abe1d78bcb5e17` (merge commit)
**Tag**: `v0.7.107` (annotated)
**Tag peel (`vN^{commit}`)**: `ab0b873179e18ca3c9ccc04d47abe1d78bcb5e17` — matches main HEAD ✓
**Tag type**: annotated (`git tag -a`)
**Tag message**: `v0.7.107 — m10-cc17-cc26-schema-fix (CC#17 + CC#26 close)`

## Remote tag (verification)

| Field | Value |
|---|---|
| Remote tag | `refs/tags/v0.7.107` |
| Remote tag_peel (`v0.7.107^{commit}`) | `ab0b873179e18ca3c9ccc04d47abe1d78bcb5e17` |
| Match between local peel and remote peel? | YES |
| Push argv (tag) | `git push origin v0.7.107` |
| Push argv (main) | `git push origin main` |
| Push exit codes | 0 / 0 |
| Push output (main) | `09eae57e..ab0b8731  main -> main` |
| Push output (tag) | `* [new tag] v0.7.107 -> v0.7.107` |

## Diff snapshot (base → published main HEAD)

```
$ git log --oneline 09eae57e..ab0b8731
ab0b8731 Merge branch 'feat/m10-cc17-cc26-schema-fix' (B-direct, CC#17 + CC#26 close)
63a7061c chore(vault): normalize verify-findings.json schema across m9-66..m9-84 and m10 cycle artifacts (CC#17 + CC#26 close)

$ git diff --stat 09eae57e..ab0b8731
 .../m9-81-counterexample-table-classifier/archive-manifest.md           |   2 +-
 .../m9-82-degraded-store-disclosure/archive-manifest.md                 |   2 +-
 .../handoffs/verify-findings.json                                       |  51 +++++++-----
 .../m10-vault-handoff-relocate/verify-findings.json                     |  10 +-
 .../m10-vault-last-updated-backfill/verify-findings.json                |   8 ++-
 .../m9-66-bash-cc-meta-check/verify-findings.json                       |  13 +--
 .../m9-81-counterexample-table-classifier/verify-findings.json          |  35 ++++----
 .../m9-82-degraded-store-disclosure/verify-findings.json                |  40 +++++----
 .../m9-83-cc39-total-cycles/verify-findings.json                        |  35 ++++----
 .../m9-84-cc001-god-module-keys-split/verify-findings.json              |  28 +++---
 10 files changed, 212 insertions(+), 149 deletions(-)
```

Total: **+212/-149** (10 files). All changes are JSON schema normalizations + cascade SHA-row rewrites. No Rust source touched.
