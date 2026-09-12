# Archive Manifest — m9-12-terms-index-metadata-drift

| Campo | Valor |
|---|---|
| Cycle | `m9-12-terms-index-metadata-drift` |
| Path | B-direct |
| Status | ARCHIVED |
| Tag | `v0.7.10` |
| Head SHA | `0012f1242cef949efc4cbd4c8d419a135ee3cf8a` |
| Base SHA | `f4818d13928c091daeaadcb65dca38558c612e7c` |

## Summary

Drift closure cycle. See release-receipt.md and change-entry.md for details.



## Cross-checks

- C1-C25: pass
## Evidence bindings

Vault metadata cycles only (or vault + minimal code). Each cycle artifact bound to its SHA-256:

- `cycle-artifacts/p-3416cfb8288f8964/m9-12-terms-index-metadata-drift/apply-checkpoint.json` → `91d01e8c90b9d173ec950305eacdc598ddb6a30b6f6b9302f1f63d8f8a54d138`
- `cycle-artifacts/p-3416cfb8288f8964/m9-12-terms-index-metadata-drift/merge-receipt.md` → `c043a0310f6e443f086586ab982b91ef6aacdc0cb856b2bdf93f82b9ae0152f9`
- `cycle-artifacts/p-3416cfb8288f8964/m9-12-terms-index-metadata-drift/release-receipt.md` → `0484a414dcc03ab7575b2d3b362bd2f1f21f5b1436efd892b4f7b00c7d58e93d`
- `cycle-artifacts/p-3416cfb8288f8964/m9-12-terms-index-metadata-drift/release-report.md` → `1068a566d19c17c68f41959748e1f98a2fb6b9701014a18f34aa58212a3266d2`
- `cycle-artifacts/p-3416cfb8288f8964/m9-12-terms-index-metadata-drift/verify-findings.json` → `352be0b40d072e1f58d75ff4c931d95969ef533390e7c07839cedcbf08196b08`
- `cycle-artifacts/p-3416cfb8288f8964/m9-12-terms-index-metadata-drift/verify-report.md` → `4a01107b9fd0d7a0ee69583110e344d4f5471d602a3cfdfa570993b48776a65b`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-12-terms-index-metadata-drift/change-entry.md` → `f89193e313e333f48e6fcbf06f4cd7c7c69ddd0e450513a9e9d32638c2592f63`

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
| m9-11 | m9-11-cycles-index-metadata-drift | B-direct | `v0.7.9` | `cd0115fd8f942058cde109c72a975cab7ea7473c` | CLOSED |
| m9-12 | m9-12-terms-index-metadata-drift | B-direct | `v0.7.10` | `0012f1242cef949efc4cbd4c8d419a135ee3cf8a` | CLOSED |
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
