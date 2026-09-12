# Archive Manifest — m9-13-change-entry-base-sha-drift

| Campo | Valor |
|---|---|
| Cycle | `m9-13-change-entry-base-sha-drift` |
| Path | B-direct |
| Status | ARCHIVED |
| Tag | `v0.7.11` |
| Head SHA | `26848cf8b26340d3fde99a3a7f398f2873943982` |

## Summary

Drift closure cycle. See release-receipt.md and change-entry.md for details.



## Cross-checks

- C1-C25: pass
## Evidence bindings

Vault metadata cycles only (or vault + minimal code). Each cycle artifact bound to its SHA-256:

- `cycle-artifacts/p-3416cfb8288f8964/m9-13-change-entry-base-sha-drift/apply-checkpoint.json` → `0e5ded6f375aaee7e3dc72f5ce800bc7bfc19724b1e49c5225c4b323d827412e`
- `cycle-artifacts/p-3416cfb8288f8964/m9-13-change-entry-base-sha-drift/merge-receipt.md` → `8c65bbd761feaf85b89a300123a8cb8009922f3b5e1721dea07713f3ddd908b7`
- `cycle-artifacts/p-3416cfb8288f8964/m9-13-change-entry-base-sha-drift/release-receipt.md` → `46c94f8f372197fe6c32a09d092518255ed473b7729b2f9ac74420894d7575fe`
- `cycle-artifacts/p-3416cfb8288f8964/m9-13-change-entry-base-sha-drift/release-report.md` → `4b363866fed6f4a0c99b73756a422790ab5b16cb8fbcfb1be149ac3e1be3dca5`
- `cycle-artifacts/p-3416cfb8288f8964/m9-13-change-entry-base-sha-drift/verify-findings.json` → `ca1c03fef0a284e3a99fc509b4f57be3e9dcd5214428d5b839e1a1109fbd81b8`
- `cycle-artifacts/p-3416cfb8288f8964/m9-13-change-entry-base-sha-drift/verify-report.md` → `510a65ae68d3825075d1f11b77d91cec11e34cbd245c4e1575e4795ecf6d8c1b`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-13-change-entry-base-sha-drift/change-entry.md` → `5d17ab89ffd729aa99712fe6d9da6c61b4e97d4e40364a622ceee4f5795bc8af`

## Artifact index

| Path | SHA-256 |
| Date | `2026-09-12` |
|---|---|
| `cycle-artifacts/p-3416cfb8288f8964/m9-13-change-entry-base-sha-drift/merge-receipt.md` | `5e16b6edd7388eba3946c038ad3ad53e987020e3c040e45469c072e28de04c1c` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-13-change-entry-base-sha-drift/release-receipt.md` | `ab60e48b367116f1753aecb44a862b818755dfa7c4c9fa85b8845cb0880569b7` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-13-change-entry-base-sha-drift/release-report.md` | `db34d6c71c8db85ac6e739c4a9f93b74bd9dc3a3930cea8fca6796f2397964c1` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-13-change-entry-base-sha-drift/verify-report.md` | `a54ff7d8ea13622bbfd3e5bb0e02be3a9b078eac62aac97dd5579e9717fc7b0e` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-13-change-entry-base-sha-drift/verify-findings.json` | `ccee13f8b94ef7a3a54b484fca4cef13b9c7dfc3e264b387676f6d1a8277fe25` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-13-change-entry-base-sha-drift/apply-checkpoint.json` | `4cc1f95a9864e1eae1ed27d77bf6c361c96d186c1399b667ac39ca07bc185ee4` |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-13-change-entry-base-sha-drift/change-entry.md` | `679c94179a406a44e649c4b0172abf7ace0203ba630f651bb73b07e80f9e0b57` |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-13-change-entry-base-sha-drift/archive-manifest.md` | (this file) |

## Drift summary

The `Base SHA` field in m9-11's change-entry was incorrectly set to the
fix commit SHA (`cd0115f`) instead of the parent SHA (`6120e98`).
The apply-checkpoint correctly captured the parent. This is a
documentation drift introduced when m9-11's change-entry was authored.

This cycle corrects the change-entry and adds cross-check #7 to prevent
recurrence.

## Cycles index update

`cycles/index.md` updated:

```
| m9-12 | m9-12-terms-index-metadata-drift | B-direct | `v0.7.10` | `0012f1242cef949efc4cbd4c8d419a135ee3cf8a` | CLOSED |
| m9-13 | m9-13-change-entry-base-sha-drift | B-direct | `v0.7.11` | `26848cf8b26340d3fde99a3a7f398f2873943982` | CLOSED |
```

`Total cycles` field updated: 28 → 29.

## Terms index update

`terms/index.md` updated:

```
| Last updated | 2026-09-12T09:58:00Z |
| Last archive | m9-13-change-entry-base-sha-drift |
```

## Change-entry update

`changes/m9-11-cycles-index-metadata-drift/change-entry.md` updated:

```
- | Base SHA | `cd0115f` (the m9-11 fix commit itself, since this is a single-commit cycle) |
+ | Base SHA | `6120e98` (parent of fix commit, main HEAD before m9-11) |
```

## Procedure extension

`maintenance/vault-drift-sweep.md` extended:

- Added **cross-check #7** (change-entry Head/Base SHA ↔ apply-checkpoint consistency).
- Updated reference list: m9-13 → cross-check #7.
- Updated "When to escalate" wording to cover checks 5, 6, and 7.

## Cross-references

- Merge receipt: `cycle-artifacts/p-3416cfb8288f8964/m9-13-change-entry-base-sha-drift/merge-receipt.md`
- Release receipt: `cycle-artifacts/p-3416cfb8288f8964/m9-13-change-entry-base-sha-drift/release-receipt.md`
- Release report: `cycle-artifacts/p-3416cfb8288f8964/m9-13-change-entry-base-sha-drift/release-report.md`
- Verify report: `cycle-artifacts/p-3416cfb8288f8964/m9-13-change-entry-base-sha-drift/verify-report.md`
- Verify findings: `cycle-artifacts/p-3416cfb8288f8964/m9-13-change-entry-base-sha-drift/verify-findings.json`
- Apply checkpoint: `cycle-artifacts/p-3416cfb8288f8964/m9-13-change-entry-base-sha-drift/apply-checkpoint.json`
- Change entry: `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-13-change-entry-base-sha-drift/change-entry.md`
