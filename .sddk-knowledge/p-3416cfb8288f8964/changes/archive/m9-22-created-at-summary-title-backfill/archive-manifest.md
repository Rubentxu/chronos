# Archive Manifest: m9-22

| Field | Value |
|---|---|
| Cycle | m9-22-created-at-summary-title-backfill |
| Head SHA | `e87a25c29b4529b94e7823430e1cd1df3b5cbd0a` |
| Base SHA | `71e7d1171d40b6f137504db470c96e65a9be7f17` |
| Tag | `v0.7.20` |
| Tag peel | `e87a25c29b4529b94e7823430e1cd1df3b5cbd0a` |
| Peel match | true |
| Archived at | 2026-09-12T11:23:00Z |
| Path | B-direct |
| Cross-check added | #15 |

## Summary

Drift closure cycle. See release-receipt.md and change-entry.md for details.



## Cross-checks

- C1-C28: pass
## Evidence bindings

Vault metadata cycles only (or vault + minimal code). Each cycle artifact bound to its SHA-256:

- `cycle-artifacts/p-3416cfb8288f8964/m9-22-created-at-summary-title-backfill/apply-checkpoint.json` → `54db313065c2910184c09051c1acd6ae15deef638cc2c3e09d77aa8bff306d83`
- `cycle-artifacts/p-3416cfb8288f8964/m9-22-created-at-summary-title-backfill/merge-receipt.md` → `3e391a0c592f995bec5b7e877b6cef791ebc8920ed53fcf8a5c2ec63d4be47ed`
- `cycle-artifacts/p-3416cfb8288f8964/m9-22-created-at-summary-title-backfill/release-receipt.md` → `d1ca5105c2ab086939a301251c9412d29b797e0ae857f4152c15bbc3a1ce5778`
- `cycle-artifacts/p-3416cfb8288f8964/m9-22-created-at-summary-title-backfill/release-report.md` → `c581be95d1018ccf64fefa31fdf8619e1deb2f8935f92fad795f80184d0fd928`
- `cycle-artifacts/p-3416cfb8288f8964/m9-22-created-at-summary-title-backfill/verify-findings.json` → `f7076ad7c74802a509cc7a190f701708f6d0d2e2dce83e05baa50984ebec75ea`
- `cycle-artifacts/p-3416cfb8288f8964/m9-22-created-at-summary-title-backfill/verify-report.md` → `6ef67660e69c5b67fbb6ddc693328b2f9b494cbdf761015bde29e8df5efc663e`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-22-created-at-summary-title-backfill/change-entry.md` → `0c96c95a83f36fe1edbc72d29d969dcdd55c4880206e077d2cd32ab6f177a39b`

## Archived artifacts

### Cycle artifacts

| File | SHA-256 |
|---|---|
| `apply-checkpoint.json` | `0fce7c6a5f1c3eb4125dc527c33b697dfcfbabe4d866e413912489fb3d67ae1b` |
| `merge-receipt.md` | `3e391a0c592f995bec5b7e877b6cef791ebc8920ed53fcf8a5c2ec63d4be47ed` |
| `release-receipt.md` | `96a1cae2cfc026d743dfbb9b98991ee38c8e307bf2c61fafd7965f210f9eda73` |
| `release-report.md` | `c581be95d1018ccf64fefa31fdf8619e1deb2f8935f92fad795f80184d0fd928` |
| `verify-findings.json` | `24c89ab09180fd9f74fe38ffe5b52cc00b613f88d75eba97128d33233ac8ab68` |
| `verify-report.md` | `6ef67660e69c5b67fbb6ddc693328b2f9b494cbdf761015bde29e8df5efc663e` |

### Knowledge artifacts

| File | SHA-256 |
|---|---|
| `change-entry.md` | `0c96c95a83f36fe1edbc72d29d969dcdd55c4880206e077d2cd32ab6f177a39b` |

### Modified cycle artifacts (16 prior cycles)

created_at, title, summary added to m9-03..m9-18.

## SHA verification

```bash
git rev-parse v0.7.20^{}  # → e87a25c29b4529b94e7823430e1cd1df3b5cbd0a
```
