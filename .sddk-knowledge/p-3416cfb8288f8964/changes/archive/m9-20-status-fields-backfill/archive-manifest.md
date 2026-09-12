# Archive Manifest: m9-20

| Field | Value |
|---|---|
| Cycle | m9-20-status-fields-backfill |
| Head SHA | `07febc1bae0d236ad47eef3c5b3172c4824cde04` |
| Base SHA | `d66569159fe3e47404aee3b4e13a3c5b883c7d69` |
| Tag | `v0.7.18` |
| Tag peel | `07febc1bae0d236ad47eef3c5b3172c4824cde04` |
| Peel match | true |
| Archived at | 2026-09-12T11:13:00Z |
| Path | B-direct |
| Cross-check added | #13 |
| Date | `2026-09-12` |

## Summary

Drift closure cycle. See release-receipt.md and change-entry.md for details.



## Cross-checks

- C1-C28: pass
## Evidence bindings

Vault metadata cycles only (or vault + minimal code). Each cycle artifact bound to its SHA-256:

- `cycle-artifacts/p-3416cfb8288f8964/m9-20-status-fields-backfill/apply-checkpoint.json` → `1129d6775f18d4c866f6ff90a6a186569139ecb7be5f5561fd0d71494b0698dc`
- `cycle-artifacts/p-3416cfb8288f8964/m9-20-status-fields-backfill/merge-receipt.md` → `d3ac4ec275fc0aa1365871702715553c45b89995db90b9468145d87b958d51be`
- `cycle-artifacts/p-3416cfb8288f8964/m9-20-status-fields-backfill/release-receipt.md` → `16d761fa9221fd54f5a0e67caaec81bbe22b9f17523366bae373e1bec7e9ba63`
- `cycle-artifacts/p-3416cfb8288f8964/m9-20-status-fields-backfill/release-report.md` → `0fc324538c8b14dbb9a456fb9053b2518657b4e4c9475df32882435ef38ddc50`
- `cycle-artifacts/p-3416cfb8288f8964/m9-20-status-fields-backfill/verify-findings.json` → `0bae19db35f740b77697e21c17b2e0cf133445b48aabda3486fb93446c657e12`
- `cycle-artifacts/p-3416cfb8288f8964/m9-20-status-fields-backfill/verify-report.md` → `0d8b325d4cae36d161bee72cd46bf2addc52beb87c2afa594ad2c15bd5f3e17f`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-20-status-fields-backfill/change-entry.md` → `2bc2019e5f89353133d6c56dd2f73852630dcd26bc368759ad9789477367bb0a`

## Archived artifacts

### Cycle artifacts (`cycle-artifacts/p-3416cfb8288f8964/m9-20-status-fields-backfill/`)

| File | SHA-256 |
|---|---|
| `apply-checkpoint.json` | `a8985dd10ec5ae9ceb6abad4064320d7e076359d9c73b6c2cbab0caaab6df7f8` |
| `merge-receipt.md` | `d3ac4ec275fc0aa1365871702715553c45b89995db90b9468145d87b958d51be` |
| `release-receipt.md` | `184927b93586ee6df7feaa4f7ef437c4e71e165a7460b88d693d14d8f5586aa1` |
| `release-report.md` | `0fc324538c8b14dbb9a456fb9053b2518657b4e4c9475df32882435ef38ddc50` |
| `verify-findings.json` | `02d4da4446b42cdfef02126d59b10018afdb686d5f7ec18b88c25b25b02af086` |
| `verify-report.md` | `0d8b325d4cae36d161bee72cd46bf2addc52beb87c2afa594ad2c15bd5f3e17f` |

### Knowledge artifacts

| File | SHA-256 |
|---|---|
| `change-entry.md` | `2bc2019e5f89353133d6c56dd2f73852630dcd26bc368759ad9789477367bb0a` |

### Modified cycle artifacts (17 prior cycles)

The following prior-cycle apply-checkpoint.json files were modified
to add `verify_status`, `release_status`, `archive_status`:

- m9-03-side-table-debt-cleanup
- m9-04-side-table-key-layout
- m9-05-side-table-overeng-cleanup
- m9-06-known-schema-versions-invariant
- m9-07-coup-01-invariant-assertion
- m9-08-list-load-schema-error-variant
- m9-09-vault-hygiene-active-disclosure-dedupe
- m9-10-m9-03-apply-checkpoint-rebuild
- m9-11-cycles-index-metadata-drift
- m9-12-terms-index-metadata-drift
- m9-13-change-entry-base-sha-drift
- m9-14-m9-11-fabricated-sha
- m9-15-m9-12-m9-13-short-sha
- m9-16-archive-manifest-short-and-fabricated-sha
- m9-17-verify-findings-and-markdown-sha-drift
- m9-18-apply-checkpoint-metadata-drift

## SHA verification

```bash
git rev-parse v0.7.18^{}  # → 07febc1bae0d236ad47eef3c5b3172c4824cde04
```
