# Archive Manifest — m9-30

| Field | Value |
|---|---|
| Cycle | m9-30-release-receipt-fields-normalize |
| Head SHA | `33acc58f9782c296f2f876cdfed280a024548188` |
| Base SHA | `f481a61169d3ec33a6ec0e236a938f231f303cf8` |
| Tag | `v0.7.28` |
| Path | B-direct |
| Date | 2026-09-12 |

## Summary

Normalizes release-receipt.md SHA fields (Head SHA, Remote tag, Remote
tag_peel, Peel match, Date) across all 25 m9 cycles (m9-03..m9-27) to the
canonical format established by m9-28. Adds cross-check #22 that
explicitly checks both field presence and SHA consistency.

## Artifact index

| Path | SHA-256 |
|---|---|
| `cycle-artifacts/p-3416cfb8288f8964/m9-30-release-receipt-fields-normalize/apply-checkpoint.json` | `69679d129144dff1ac357ce5db77c3d2e91b7d452bf6b0479b9d140fbc664a7d` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-30-release-receipt-fields-normalize/merge-receipt.md` | `b4bd95ac6c2656fb251f8c56edf72d7c8905eb7eebf600bdebee1a117b45f66d` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-30-release-receipt-fields-normalize/release-receipt.md` | `01e8e77b2d556e35da4f6929b269e7ceb25adcd0310b9803218dd953846fe434` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-30-release-receipt-fields-normalize/release-report.md` | `021aef501bbeeebe00cdb921c8875e62bffb60838c8342b77ba2be6f34c6d3c5` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-30-release-receipt-fields-normalize/verify-findings.json` | `f91a9b7c7d241fa39ab2c9da1b09193b6d2bc7a5f9410d540ca7a7dcc04eaa90` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-30-release-receipt-fields-normalize/verify-report.md` | `8a8dd1cb19fda6eae155759d1309bd5a78c3a5a96a35944013ba4f2152bde53b` |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-30-release-receipt-fields-normalize/change-entry.md` | `cd06e368178eb41bf6520dba81c98f5c721a4afaa75894dbf5574ef1ac221202` |

## Evidence bindings

Vault metadata only. Each cycle artifact listed with its computed
SHA-256 in the Artifact index above.

## Drift summary

25 release-receipt.md files (m9-03..m9-27) had inconsistent field
names and missing SHA fields. m9-30 normalizes all to canonical format.

## Cross-checks added

- #22 (`vault-drift-sweep.md`): release-receipt.md must have
  canonical SHA fields (Head SHA, Remote tag, Remote tag_peel,
  Peel match, Date).
