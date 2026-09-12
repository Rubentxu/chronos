# Archive Manifest — m9-11-cycles-index-metadata-drift

| Campo | Valor |
|---|---|
| Cycle ID | `m9-11-cycles-index-metadata-drift` |
| Path | B-direct |
| Status | ARCHIVED |
| Tag | `v0.7.9` |
| Head SHA | `cd0115fd8f942058cde109c72a975cab7ea7473c` |

## Artifact index

| Path | SHA-256 |
|---|---|
| `cycle-artifacts/p-3416cfb8288f8964/m9-11-cycles-index-metadata-drift/merge-receipt.md` | `af46ede170a04a85cc43d34e6c1eb11a0f9c7be77518c942776e109fefbd3e38` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-11-cycles-index-metadata-drift/release-receipt.md` | `f5ab5f83395d9df40695e2fba889da0f33afe6133494526b4cf2f0d863e29eb1` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-11-cycles-index-metadata-drift/release-report.md` | `6f419ff96cc632a8bf58e63e6d18e42ec6eecc6400982152cc74367448a8331e` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-11-cycles-index-metadata-drift/verify-report.md` | `08a1de461b9cb44a59bccb67be178e1cb0d28ee5d099182c81b87633d810ce32` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-11-cycles-index-metadata-drift/verify-findings.json` | `9bfb26516732fc32424ecb25108eb6facac4c1a3cd8785fbb51335531cf06417` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-11-cycles-index-metadata-drift/apply-checkpoint.json` | `5e642c5de5e5ca05872b47e230c1fd7fa50bc7390df3cfb67c67b204aa3a29bb` |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-11-cycles-index-metadata-drift/change-entry.md` | `8b04ea8f526c8774ea5c8ce8845eed05a7c9702453e66dc093fdf6407ba0a2e5` |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-11-cycles-index-metadata-drift/archive-manifest.md` | (this file) |

## Drift summary

The `Total cycles` metadata field in `cycles/index.md` was drifting for
4 cycles (m9-07 through m9-10). The previous standing procedure
(`vault-drift-sweep.md` checks 1-4) had no cross-check for this metadata
field, so the drift went uncaught across multiple archive commits.

This cycle closes the drift and extends the standing procedure with
cross-check #5 to prevent recurrence.

## Cycles index update

`cycles/index.md` updated:

```
| m9-10 | m9-10-m9-03-apply-checkpoint-rebuild | B-rebuild | `v0.7.8` | `69f200e2144bea2cd305c38903feb4814fb38806` | CLOSED |
| m9-11 | m9-11-cycles-index-metadata-drift | B-direct | `v0.7.9` | `cd0115fd8f942058cde109c72a975cab7ea7473c` | CLOSED |
```

`Total cycles` field updated: 22 → 26 → 27 (after adding m9-11 row).
`Last updated` updated: 2026-09-12T08:21:30Z.

## Vault index update

`terms/index.md` not modified (no findings closed/introduced).

## Procedure extension

`maintenance/vault-drift-sweep.md` extended:

- Added **cross-check #5** (cycles/index.md metadata consistency).
- Updated reference list: m9-11 → cross-check #5.
- Updated "When to escalate" wording (4 → cross-checks, generic).

## Cross-references

- Merge receipt: `cycle-artifacts/p-3416cfb8288f8964/m9-11-cycles-index-metadata-drift/merge-receipt.md`
- Release receipt: `cycle-artifacts/p-3416cfb8288f8964/m9-11-cycles-index-metadata-drift/release-receipt.md`
- Release report: `cycle-artifacts/p-3416cfb8288f8964/m9-11-cycles-index-metadata-drift/release-report.md`
- Verify report: `cycle-artifacts/p-3416cfb8288f8964/m9-11-cycles-index-metadata-drift/verify-report.md`
- Verify findings: `cycle-artifacts/p-3416cfb8288f8964/m9-11-cycles-index-metadata-drift/verify-findings.json`
- Apply checkpoint: `cycle-artifacts/p-3416cfb8288f8964/m9-11-cycles-index-metadata-drift/apply-checkpoint.json`
- Change entry: `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-11-cycles-index-metadata-drift/change-entry.md`
