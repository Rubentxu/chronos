# Archive Manifest — m9-18-apply-checkpoint-metadata-drift

| Campo | Valor |
|---|---|
| Cycle | `m9-18-apply-checkpoint-metadata-drift` |
| Archived at | 2026-09-12T10:44:45Z |
| Tag | `v0.7.16` |
| Head SHA | `6dce3736df06d4fe09db861ad43a3667c0f0bc25` |
| Base SHA | `cc995cdd2cf41bd0638a548fc3db169763bfc38d` |
| Release-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-18-apply-checkpoint-metadata-drift/release-receipt.md` |
| Path | `B-direct` |
| Date | `2026-09-12` |

## Summary

Drift closure cycle. See release-receipt.md and change-entry.md for details.



## Cross-checks

- C1-C25: pass
## Evidence bindings

Vault metadata cycles only (or vault + minimal code). Each cycle artifact bound to its SHA-256:

- `cycle-artifacts/p-3416cfb8288f8964/m9-18-apply-checkpoint-metadata-drift/apply-checkpoint.json` → `faccfff745dce93bb70a0bbdf7fd4e228ccf2e13943d444434387efcc6a83d91`
- `cycle-artifacts/p-3416cfb8288f8964/m9-18-apply-checkpoint-metadata-drift/merge-receipt.md` → `3e6cae2aecefceccf42165eace4ed7463b710318e8f3f83d6fb9969f34906c8c`
- `cycle-artifacts/p-3416cfb8288f8964/m9-18-apply-checkpoint-metadata-drift/release-receipt.md` → `1dbf45683604153dbb63c29d0866a3b94df1f6f968399b69d46122327542d7a6`
- `cycle-artifacts/p-3416cfb8288f8964/m9-18-apply-checkpoint-metadata-drift/release-report.md` → `465c2845340554a2fa9d0ff88279aee0c25ea96e22d5b53b126d68779cae699d`
- `cycle-artifacts/p-3416cfb8288f8964/m9-18-apply-checkpoint-metadata-drift/verify-findings.json` → `fe56ac7ffb2219a1b6eccd23893c2f2c51349550c97d5cfd5ee5eb2f6d7485c3`
- `cycle-artifacts/p-3416cfb8288f8964/m9-18-apply-checkpoint-metadata-drift/verify-report.md` → `50a33c48930383c8569440465accfcdb38d4cb9148e36e7dc3619d163d36a3f7`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-18-apply-checkpoint-metadata-drift/change-entry.md` → `17aa0f178f60c4eeaf5d1419687117cb745d4fbcd7438824751d2eb985142c12`

## Artifact index (SHA-256)

| File | SHA-256 |
|---|---|
| `cycle-artifacts/p-3416cfb8288f8964/m9-18-apply-checkpoint-metadata-drift/merge-receipt.md` | `3e6cae2aecefceccf42165eace4ed7463b710318e8f3f83d6fb9969f34906c8c` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-18-apply-checkpoint-metadata-drift/release-receipt.md` | `1dbf45683604153dbb63c29d0866a3b94df1f6f968399b69d46122327542d7a6` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-18-apply-checkpoint-metadata-drift/release-report.md` | `465c2845340554a2fa9d0ff88279aee0c25ea96e22d5b53b126d68779cae699d` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-18-apply-checkpoint-metadata-drift/verify-report.md` | `50a33c48930383c8569440465accfcdb38d4cb9148e36e7dc3619d163d36a3f7` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-18-apply-checkpoint-metadata-drift/apply-checkpoint.json` | `5a95b34c9386591e5b5eea044ed75e2826ca243b4be91387761a59424bc664f6` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-18-apply-checkpoint-metadata-drift/verify-findings.json` | `d52e67c5ac6b34cb8e373fcad34d8079d2feb3687a48c6c4e21b7ed1dc4ef6c9` |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-18-apply-checkpoint-metadata-drift/change-entry.md` | `17aa0f178f60c4eeaf5d1419687117cb745d4fbcd7438824751d2eb985142c12` |

## Notes

m9-18 closes 3 classes of apply-checkpoint.json metadata drift
across 10 prior cycles: archived_at backfill (3 cycles),
findings_introduced field additions (7 cycles), and status
normalization (8 cycles). Adds cross-check #11 to vault-drift-sweep.md
to enforce the post-m9-11 schema going forward.
