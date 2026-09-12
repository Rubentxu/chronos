# Release Report — m9-16-archive-manifest-short-and-fabricated-sha

## Summary

Doc-only cycle that closes a follow-up drift left over by m9-14 and
m9-15: both fixed apply-checkpoint.json SHA fields but missed the
corresponding archive-manifest.md files. m9-16 expands three
archive-manifests' Head SHA fields plus cross-references, plus
expands two cycles/index.md Published SHA columns to full 40-char
SHAs, and adds cross-check #9 to the standing vault-drift-sweep
procedure.

## Diff stats

```
.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-11-cycles-index-metadata-drift/archive-manifest.md |  4 +--
.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-12-terms-index-metadata-drift/archive-manifest.md    |  6 ++--
.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-13-change-entry-base-sha-drift/archive-manifest.md    |  6 ++--
.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md                                                          |  9 ++---
.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md                                         | 40 ++++++++++++++++++++++++--
.sddk-knowledge/p-3416cfb8288f8964/terms/index.md                                                           |  4 +--
6 files changed, 52 insertions(+), 17 deletions(-)
```

## Risks

None. Doc-only change, no production code touched.

## Gates

| Gate | Result |
|---|---|
| T0 (`cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings`) | PASS (0 warnings, 0 diffs) |
| Vault drift sweep (9 cross-checks; C3 strict, C9 new) | PASS (all 9 clean post-fix) |

## Findings

No findings introduced or closed.

## Verdict

**RELEASED** at tag `v0.7.14`, head SHA `eb5110dfe1f1d14eab85e6052f1f7cbb86e2f384`.
