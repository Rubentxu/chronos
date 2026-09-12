# Release Report — m9-18-apply-checkpoint-metadata-drift

## Summary

Doc-only cycle that closes 3 classes of apply-checkpoint.json metadata
drift across 10 prior cycles: archived_at backfill (3 cycles),
findings_introduced field additions (7 cycles), and status
normalization (8 cycles). Adds cross-check #11 to the standing
vault-drift-sweep procedure to enforce the post-m9-11 schema going
forward.

## Diff stats

```
.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md                            |   5 +-
.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md           |  58 ++++++++++++++++++++--
.sddk-knowledge/p-3416cfb8288f8964/terms/index.md                             |   4 +-
cycle-artifacts/p-3416cfb8288f8964/m9-03-side-table-debt-cleanup/apply-checkpoint.json    |   5 +-
cycle-artifacts/p-3416cfb8288f8964/m9-04-side-table-key-layout/apply-checkpoint.json      |   4 +-
cycle-artifacts/p-3416cfb8288f8964/m9-05-side-table-overeng-cleanup/apply-checkpoint.json |   7 ++-
cycle-artifacts/p-3416cfb8288f8964/m9-06-known-schema-versions-invariant/apply-checkpoint.json |   7 ++-
cycle-artifacts/p-3416cfb8288f8964/m9-07-coup-01-invariant-assertion/apply-checkpoint.json |   5 +-
cycle-artifacts/p-3416cfb8288f8964/m9-08-list-load-schema-error-variant/apply-checkpoint.json |   5 +-
cycle-artifacts/p-3416cfb8288f8964/m9-09-vault-hygiene-active-disclosure-dedupe/apply-checkpoint.json |   5 +-
cycle-artifacts/p-3416cfb8288f8964/m9-10-m9-03-apply-checkpoint-rebuild/apply-checkpoint.json |   5 +-
cycle-artifacts/p-3416cfb8288f8964/m9-11-cycles-index-metadata-drift/apply-checkpoint.json |   2 +-
cycle-artifacts/p-3416cfb8288f8964/m9-12-terms-index-metadata-drift/apply-checkpoint.json |   2 +-
cycle-artifacts/p-3416cfb8288f8964/m9-13-change-entry-base-sha-drift/apply-checkpoint.json |   2 +-
14 files changed, 94 insertions(+), 22 deletions(-)
```

## Risks

None. Doc-only change, no production code touched.

## Gates

| Gate | Result |
|---|---|
| T0 (cargo fmt + clippy) | PASS (0 warnings, 0 diffs) |
| Vault drift sweep (11 cross-checks; C11 new) | PASS (all 11 clean post-fix) |

## Findings

No findings introduced or closed.

## Verdict

**RELEASED** at tag `v0.7.16`, head SHA `6dce3736df06d4fe09db861ad43a3667c0f0bc25`.
