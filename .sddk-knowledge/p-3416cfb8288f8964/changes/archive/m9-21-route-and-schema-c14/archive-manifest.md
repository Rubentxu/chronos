# Archive Manifest: m9-21

| Field | Value |
|---|---|
| Cycle | m9-21-route-and-schema-c14 |
| Head SHA | `14ecf16ded816896cb612721875e97380bcadbc3` |
| Base SHA | `efb9d93da368c4c6e1473c4668bbc945dff850c6` |
| Tag | `v0.7.19` |
| Tag peel | `14ecf16ded816896cb612721875e97380bcadbc3` |
| Peel match | true |
| Archived at | 2026-09-12T11:15:00Z |
| Path | B-direct |
| Cross-check added | #14 |
| Date | `2026-09-12` |

## Summary

Drift closure cycle. See release-receipt.md and change-entry.md for details.



## Cross-checks

- C1-C28: pass
## Evidence bindings

Vault metadata cycles only (or vault + minimal code). Each cycle artifact bound to its SHA-256:

- `cycle-artifacts/p-3416cfb8288f8964/m9-21-route-and-schema-c14/apply-checkpoint.json` → `8cf3ad52b86e4682293faea75429cf77d839aa640064f0954073a85f6851db22`
- `cycle-artifacts/p-3416cfb8288f8964/m9-21-route-and-schema-c14/merge-receipt.md` → `62f7c41d2adc63fa38181f826c71437fb9a8dfdf13601c7e774bc48aa9cd1280`
- `cycle-artifacts/p-3416cfb8288f8964/m9-21-route-and-schema-c14/release-receipt.md` → `1c1cfc40faa68fa880bfa1bcb09f7e2a3dd77e4b35bbafba248385fcafab58cd`
- `cycle-artifacts/p-3416cfb8288f8964/m9-21-route-and-schema-c14/release-report.md` → `47ddbb6c7bd7ae8885bc2eb8b326f441be4722f117504f7296ad8148b7e43d58`
- `cycle-artifacts/p-3416cfb8288f8964/m9-21-route-and-schema-c14/verify-findings.json` → `4e1a71fe6b121a3834331e1e9b3dd23a0a29e77fc9675d03b6bd38a8828aac0a`
- `cycle-artifacts/p-3416cfb8288f8964/m9-21-route-and-schema-c14/verify-report.md` → `2b9c23e0224d07d71f2ee9e666b1f23a61aaae957e501784e80c784ce274ca7f`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-21-route-and-schema-c14/change-entry.md` → `b5bc5cc1216a39677b6cc0d0bb6c9091fbcb921aa4b93d88431c5e111432c2e3`

## Archived artifacts

### Cycle artifacts

| File | SHA-256 |
|---|---|
| `apply-checkpoint.json` | `6c0e2792dd123c9cff71f1de74455e4679a338bddd853e875baaae2655d6d196` |
| `merge-receipt.md` | `62f7c41d2adc63fa38181f826c71437fb9a8dfdf13601c7e774bc48aa9cd1280` |
| `release-receipt.md` | `e1669872ddcbc4ef9f2596d5f6c8213e26d106b0b0e21e1f6dc081a3129b3d43` |
| `release-report.md` | `47ddbb6c7bd7ae8885bc2eb8b326f441be4722f117504f7296ad8148b7e43d58` |
| `verify-findings.json` | `cabd875bbf928c36ab0923cdde57a9a938c2ab7bbf2a9097ca60df4b503b2825` |
| `verify-report.md` | `2b9c23e0224d07d71f2ee9e666b1f23a61aaae957e501784e80c784ce274ca7f` |

### Knowledge artifacts

| File | SHA-256 |
|---|---|
| `change-entry.md` | `b5bc5cc1216a39677b6cc0d0bb6c9091fbcb921aa4b93d88431c5e111432c2e3` |

### Modified cycle artifacts (8 prior cycles)

Route field normalized on m9-11..m9-18.

## SHA verification

```bash
git rev-parse v0.7.19^{}  # → 14ecf16ded816896cb612721875e97380bcadbc3
```
