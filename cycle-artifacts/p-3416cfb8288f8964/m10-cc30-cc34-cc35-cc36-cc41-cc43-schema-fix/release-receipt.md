# Release Receipt — m10-cc30-cc34-cc35-cc36-cc41-cc43-schema-fix

**Cycle**: `p-3416cfb8288f8964/m10-cc30-cc34-cc35-cc36-cc41-cc43-schema-fix`
**Path**: B-direct
**Published subject (main HEAD)**: `c91f010594f74f4a1ff573aa4bb2705e639689e5` (merge commit)
**Tag**: `v0.7.108` (annotated)
**Tag peel**: `c91f010594f74f4a1ff573aa4bb2705e639689e5` — matches main HEAD ✓
**Tag type**: annotated (`git tag -a`)
**Tag message**: `v0.7.108 — m10-cc30-cc34-cc35-cc36-cc41-cc43-schema-fix (CC partial close, 41→13)`

## Remote tag (verification)

| Field | Value |
|---|---|
| Remote tag | `refs/tags/v0.7.108` |
| Remote tag_peel | `c91f010594f74f4a1ff573aa4bb2705e639689e5` |
| Match local/remote peel? | YES |
| Push argv (tag) | `git push origin v0.7.108` |
| Push argv (main) | `git push origin main` |
| Push exit codes | 0 / 0 |
| Push output (main) | `2cb2d2ce..c91f0105  main -> main` |
| Push output (tag) | `* [new tag] v0.7.108 -> v0.7.108` |

## Diff snapshot (base → published main HEAD)

```
$ git log --oneline 2cb2d2ce..c91f0105
c91f0105 Merge branch 'feat/m10-cc30-cc34-cc35-cc36-cc41-cc43-schema-fix' (B-direct, CC#30/34/35/36/41/43 partial close)
29f6f524 chore(vault): normalize verify-findings.json + change-entry.md + archive-manifest.md schema (CC#30 + CC#34 + CC#35 + CC#36 + CC#41 + CC#43 mechanical close)

$ git diff --stat 2cb2d2ce..c91f0105 | tail -3
 120 files changed, 2018 insertions(+), 1453 deletions(-)
```

Total: **+2018/-1453** (120 files). Schema normalizations + cascade SHA-row rewrites. No Rust source touched.
