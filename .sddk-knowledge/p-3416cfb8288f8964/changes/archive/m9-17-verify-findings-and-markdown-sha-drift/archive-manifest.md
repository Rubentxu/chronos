# Archive Manifest — m9-17-verify-findings-and-markdown-sha-drift

| Campo | Valor |
|---|---|
| Cycle | `m9-17-verify-findings-and-markdown-sha-drift` |
| Archived at | 2026-09-12T10:33:15Z |
| Tag | `v0.7.15` |
| Head SHA | `134dc7525312275c447db8f5996740ff7c102a02` |
| Base SHA | `0ed8f874f7b6f973505fc3997475fc398fcfa4cc` |
| Release-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-17-verify-findings-and-markdown-sha-drift/release-receipt.md` |
| Path | `B-direct` |
| Date | `2026-09-12` |

## Summary

Drift closure cycle. See release-receipt.md and change-entry.md for details.



## Cross-checks

- C1-C25: pass
## Evidence bindings

Vault metadata cycles only (or vault + minimal code). Each cycle artifact bound to its SHA-256:

- `cycle-artifacts/p-3416cfb8288f8964/m9-17-verify-findings-and-markdown-sha-drift/apply-checkpoint.json` → `4a066388f5d218f135d1db0a7b7c903ba416879cdb820a105ba1bd4a296557fb`
- `cycle-artifacts/p-3416cfb8288f8964/m9-17-verify-findings-and-markdown-sha-drift/merge-receipt.md` → `18e120b52702c6b1936e4a1200696ac6e46c145ebb0d3355c64f64ddeb3a12dc`
- `cycle-artifacts/p-3416cfb8288f8964/m9-17-verify-findings-and-markdown-sha-drift/release-receipt.md` → `8e727977a6298df060e7f2ad4f047712e0f08d6930233276407547b4160d729f`
- `cycle-artifacts/p-3416cfb8288f8964/m9-17-verify-findings-and-markdown-sha-drift/release-report.md` → `1d345d12a817e7cb40a7e656375665fde688f16d90d3434980b203e533fcb441`
- `cycle-artifacts/p-3416cfb8288f8964/m9-17-verify-findings-and-markdown-sha-drift/verify-findings.json` → `e56991e8d55121c9c94c8b87b92ae9b05c263c5f7ca5e545e511d97d893e3d15`
- `cycle-artifacts/p-3416cfb8288f8964/m9-17-verify-findings-and-markdown-sha-drift/verify-report.md` → `4451c99de7cca438e65a343af461bb326d1da52eb2e550c2621bc0288747848d`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-17-verify-findings-and-markdown-sha-drift/change-entry.md` → `ee7321c5c87bab5069fa08260227a5398f5f01a607c4c95873efacd7f44e6424`

## Artifact index (SHA-256)

| File | SHA-256 |
|---|---|
| `cycle-artifacts/p-3416cfb8288f8964/m9-17-verify-findings-and-markdown-sha-drift/merge-receipt.md` | `18e120b52702c6b1936e4a1200696ac6e46c145ebb0d3355c64f64ddeb3a12dc` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-17-verify-findings-and-markdown-sha-drift/release-receipt.md` | `8e727977a6298df060e7f2ad4f047712e0f08d6930233276407547b4160d729f` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-17-verify-findings-and-markdown-sha-drift/release-report.md` | `1d345d12a817e7cb40a7e656375665fde688f16d90d3434980b203e533fcb441` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-17-verify-findings-and-markdown-sha-drift/verify-report.md` | `4451c99de7cca438e65a343af461bb326d1da52eb2e550c2621bc0288747848d` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-17-verify-findings-and-markdown-sha-drift/apply-checkpoint.json` | `bd4a4a337bc57efb609e843c934c82b813c5ce3870be0c406b776d593b93bcaa` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-17-verify-findings-and-markdown-sha-drift/verify-findings.json` | `c7a68ffb2f5970f4ec09b5e6a468450ad1ab879d08f8325ffbc520be9337c0a3` |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-17-verify-findings-and-markdown-sha-drift/change-entry.md` | `ee7321c5c87bab5069fa08260227a5398f5f01a607c4c95873efacd7f44e6424` |

## Notes

m9-17 closes residual SHA drift across 4 file types
(verify-findings.json, release-receipt.md, merge-receipt.md,
change-entry.md) across 4 prior cycles (m9-11, m9-12, m9-13, m9-14).
m9-03 and m9-04 schema-v1 verify-findings are accepted-by-design drift
under the pre-m9-11 docs-peel convention. Adds cross-check #10 to
vault-drift-sweep.md.
