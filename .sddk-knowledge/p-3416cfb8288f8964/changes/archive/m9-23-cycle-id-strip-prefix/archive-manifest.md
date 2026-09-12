# Archive Manifest: m9-23

| Field | Value |
|---|---|
| Cycle | m9-23-cycle-id-strip-prefix |
| Head SHA | `b549c7462932647b03cd9cd2f28da0fc03c9ed64` |
| Base SHA | `f5fbbd7584398d0fda1f090fe9295bcb9fdde045` |
| Tag | `v0.7.21` |
| Tag peel | `b549c7462932647b03cd9cd2f28da0fc03c9ed64` |
| Peel match | true |
| Archived at | 2026-09-12T11:24:00Z |
| Path | B-direct |
| Cross-check added | #16 |
| Date | `2026-09-12` |

## Summary

Drift closure cycle. See release-receipt.md and change-entry.md for details.



## Cross-checks

- C1-C28: pass
## Evidence bindings

Vault metadata cycles only (or vault + minimal code). Each cycle artifact bound to its SHA-256:

- `cycle-artifacts/p-3416cfb8288f8964/m9-23-cycle-id-strip-prefix/apply-checkpoint.json` → `e25c2e7933e64f6351b05a93f5359b3dab6e18d81de187cc455f964fbfa53713`
- `cycle-artifacts/p-3416cfb8288f8964/m9-23-cycle-id-strip-prefix/merge-receipt.md` → `0e04b7d134c8c9db1bc272643f8aaf3b2f903bdfdb181866a349e12a8b64510e`
- `cycle-artifacts/p-3416cfb8288f8964/m9-23-cycle-id-strip-prefix/release-receipt.md` → `f6be8862e0a217c4be507238d73e0f0de340a5d50aa4e69bbecc998d970228af`
- `cycle-artifacts/p-3416cfb8288f8964/m9-23-cycle-id-strip-prefix/release-report.md` → `a8fbe3a4906cb8b97654c49b126b29abf491e9c53cb9ddd6e50316357ae7ee15`
- `cycle-artifacts/p-3416cfb8288f8964/m9-23-cycle-id-strip-prefix/verify-findings.json` → `bae71fa1298a3f5c556c1adc27455c0978766bff510d71b54956ad612d07fe63`
- `cycle-artifacts/p-3416cfb8288f8964/m9-23-cycle-id-strip-prefix/verify-report.md` → `26ede9a4ec01309cfece5309bcb5b45a10dad3d4fe6a8886dfb737c39f59a415`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-23-cycle-id-strip-prefix/change-entry.md` → `e72643d3bb82cce85a63940dfec29612a447c377e5b738d0b42fd398cecbe732`

## Archived artifacts

### Cycle artifacts

| File | SHA-256 |
|---|---|
| `apply-checkpoint.json` | `02fc752b3ccadb99daf929b86ff7bc269ca2466998d67867fc24bafdc7d1bf50` |
| `merge-receipt.md` | `0e04b7d134c8c9db1bc272643f8aaf3b2f903bdfdb181866a349e12a8b64510e` |
| `release-receipt.md` | `36ca92a84c25dfcf94e15f719028ad0271bf6eebba6ea41bac5e70da2e6fdc6a` |
| `release-report.md` | `a8fbe3a4906cb8b97654c49b126b29abf491e9c53cb9ddd6e50316357ae7ee15` |
| `verify-findings.json` | `c8ef664ce66bde4acab4c8f8207c4fa45eb717f72d5c496e1ee3254f29c7beab` |
| `verify-report.md` | `26ede9a4ec01309cfece5309bcb5b45a10dad3d4fe6a8886dfb737c39f59a415` |

### Knowledge artifacts

| File | SHA-256 |
|---|---|
| `change-entry.md` | `e72643d3bb82cce85a63940dfec29612a447c377e5b738d0b42fd398cecbe732` |

## SHA verification

```bash
git rev-parse v0.7.21^{}  # → b549c7462932647b03cd9cd2f28da0fc03c9ed64
```
