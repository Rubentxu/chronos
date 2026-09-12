# Archive Manifest — m9-12-terms-index-metadata-drift

| Campo | Valor |
|---|---|
| Cycle ID | `m9-12-terms-index-metadata-drift` |
| Path | B-direct |
| Status | ARCHIVED |
| Tag | `v0.7.10` |
| Head SHA | `0012f12` |

## Artifact index

| Path | SHA-256 |
|---|---|
| `cycle-artifacts/p-3416cfb8288f8964/m9-12-terms-index-metadata-drift/merge-receipt.md` | `3cd492fc4d56c07a8c4774ff6fe6c81d1fb760b1f9e5ba6ea389329bb825d352` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-12-terms-index-metadata-drift/release-receipt.md` | `7fe66801690da3908f30ae3f0d688e6ec7e3c5df768eaf93edac9585675ff11f` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-12-terms-index-metadata-drift/release-report.md` | `5f6718ccc268be6c3833df80b5e3676edfba9c352e985bcd3eef2efadb811180` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-12-terms-index-metadata-drift/verify-report.md` | `d096bfa96f5d4168a105f77146f22b431abe528ff634615a02b85222684c5e09` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-12-terms-index-metadata-drift/verify-findings.json` | `132b17b2447d555f46c709243235d792151401d0bea5d0ad4e3241c45bf24d3f` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-12-terms-index-metadata-drift/apply-checkpoint.json` | `0e306d1557cecd47afeb93c61fc04b05d7d838307c26ffeefc78a2d961f27115` |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-12-terms-index-metadata-drift/change-entry.md` | `7118b26c88d62dfad5f0b6e9e64c309c32de82a84a5bfef1c5f67ec6445ae7fb` |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-12-terms-index-metadata-drift/archive-manifest.md` | (this file) |

## Drift summary

The `Last archive` metadata field in `terms/index.md` was drifting for
1 cycle (m9-11 added itself to cycles/index.md but didn't bump
terms/index.md's `Last archive` pointer). The standing procedure
(`vault-drift-sweep.md` checks 1-5) had no cross-check for this metadata
field, so the drift went uncaught in the m9-11 archive commit.

This cycle closes the drift and extends the standing procedure with
cross-check #6 to prevent recurrence.

## Cycles index update

`cycles/index.md` updated:

```
| m9-11 | m9-11-cycles-index-metadata-drift | B-direct | `v0.7.9` | `cd0115f8c93bddcae06e5a57f4e7e91d3a4fbb33` | CLOSED |
| m9-12 | m9-12-terms-index-metadata-drift | B-direct | `v0.7.10` | `0012f12` | CLOSED |
```

`Total cycles` field updated: 27 → 28.
`Last updated` updated: 2026-09-12T08:25:00Z.

## Terms index update

`terms/index.md` updated:

```
| Last updated | 2026-09-12T08:52:00Z |
| Last archive | m9-11-cycles-index-metadata-drift |
```

(Was: `Last updated | 2026-09-12T07:24:00Z`, `Last archive | m9-10-m9-03-apply-checkpoint-rebuild`.)

## Procedure extension

`maintenance/vault-drift-sweep.md` extended:

- Added **cross-check #6** (terms/index.md "Last archive" ↔ cycles/index.md most-recent).
- Updated reference list: m9-12 → cross-check #6.
- Updated "When to escalate" wording to cover both check 5 and check 6.

## Cross-references

- Merge receipt: `cycle-artifacts/p-3416cfb8288f8964/m9-12-terms-index-metadata-drift/merge-receipt.md`
- Release receipt: `cycle-artifacts/p-3416cfb8288f8964/m9-12-terms-index-metadata-drift/release-receipt.md`
- Release report: `cycle-artifacts/p-3416cfb8288f8964/m9-12-terms-index-metadata-drift/release-report.md`
- Verify report: `cycle-artifacts/p-3416cfb8288f8964/m9-12-terms-index-metadata-drift/verify-report.md`
- Verify findings: `cycle-artifacts/p-3416cfb8288f8964/m9-12-terms-index-metadata-drift/verify-findings.json`
- Apply checkpoint: `cycle-artifacts/p-3416cfb8288f8964/m9-12-terms-index-metadata-drift/apply-checkpoint.json`
- Change entry: `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-12-terms-index-metadata-drift/change-entry.md`
