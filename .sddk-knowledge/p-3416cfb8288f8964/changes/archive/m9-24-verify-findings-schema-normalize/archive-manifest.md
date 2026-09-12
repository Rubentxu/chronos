# Archive Manifest: m9-24

| Field | Value |
|---|---|
| Cycle | m9-24-verify-findings-schema-normalize |
| Head SHA | `8e0bfbf79d761452d7c186d855066b54a8176ace` |
| Base SHA | `dd389257d6fdf402a21845449e7d66727beafa6e` |
| Tag | `v0.7.22` |
| Tag peel | `8e0bfbf79d761452d7c186d855066b54a8176ace` |
| Peel match | true |
| Archived at | 2026-09-12T11:26:00Z |
| Path | B-direct |
| Cross-check added | #17 |

## Summary

Drift closure cycle. See release-receipt.md and change-entry.md for details.



## Cross-checks

- C1-C28: pass
## Evidence bindings

Vault metadata cycles only (or vault + minimal code). Each cycle artifact bound to its SHA-256:

- `cycle-artifacts/p-3416cfb8288f8964/m9-24-verify-findings-schema-normalize/apply-checkpoint.json` → `d16301f8348440120489b014bbd850394d07b472526dca0a1af9450eda164f5a`
- `cycle-artifacts/p-3416cfb8288f8964/m9-24-verify-findings-schema-normalize/merge-receipt.md` → `4087465589ee496d5e5d3bac56a783db3a29a4cfbaeaa41777c4b89b1139382b`
- `cycle-artifacts/p-3416cfb8288f8964/m9-24-verify-findings-schema-normalize/release-receipt.md` → `2a19b94a63970af0de95d7f31fe16f2a61e57cbf6817f505cf7bfd400e431d39`
- `cycle-artifacts/p-3416cfb8288f8964/m9-24-verify-findings-schema-normalize/release-report.md` → `0ab05a1137fea6a0b59747c18d80bf08ebdade92fc3e3a872ac6a404bdf91cd0`
- `cycle-artifacts/p-3416cfb8288f8964/m9-24-verify-findings-schema-normalize/verify-findings.json` → `c6af3356dd0c73f75617c2a097f2ee4c90c5a119a7e55370d7ae042bb9a1be1d`
- `cycle-artifacts/p-3416cfb8288f8964/m9-24-verify-findings-schema-normalize/verify-report.md` → `f2221efc204022077297f2bffa8d9fc3f015b0e926c150efdf5dd92526c809cf`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-24-verify-findings-schema-normalize/change-entry.md` → `bc9fa79fad4c5e02b340b677ce9970d6affc424f18b1d7a70b9aebdcf481952e`

## Archived artifacts

### Cycle artifacts

| File | SHA-256 |
|---|---|
| `apply-checkpoint.json` | `f2a8a8a0fa0bc350ade10f9f6a3b9ca174351ce68bb8e0750e544de2d8e9b279` |
| `merge-receipt.md` | `4087465589ee496d5e5d3bac56a783db3a29a4cfbaeaa41777c4b89b1139382b` |
| `release-receipt.md` | `42f5b62a3f5bf83f45a6ad5122eed22cec741286fe0dc6299b72cddc4f1a8fed` |
| `release-report.md` | `0ab05a1137fea6a0b59747c18d80bf08ebdade92fc3e3a872ac6a404bdf91cd0` |
| `verify-findings.json` | `7858e272c59f9c32e7a48d9c955f9f94db6550d3cb240e7e4f31030608f4c777` |
| `verify-report.md` | `f2221efc204022077297f2bffa8d9fc3f015b0e926c150efdf5dd92526c809cf` |

### Knowledge artifacts

| File | SHA-256 |
|---|---|
| `change-entry.md` | bc9fa79fad4c5e02b340b677ce9970d6affc424f18b1d7a70b9aebdcf481952e |

## SHA verification

```bash
git rev-parse v0.7.22^{}  # → 8e0bfbf79d761452d7c186d855066b54a8176ace
```
