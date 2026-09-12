# Archive Manifest — m9-28

| Field | Value |
|---|---|
| Cycle | m9-28-m9-19-fabricated-base-sha |
| Head SHA | `709e3c470d50064b9a2c819b5761a2972eadafcb` |
| Base SHA | `8eb648779071f9a634e2439754034f1288929c62` |
| Tag | `v0.7.26` |
| Path | B-direct |
| Date | 2026-09-12 |

## Summary

Replaces fabricated `base_sha` in m9-19's apply-checkpoint.json with
the real value (parent of m9-19's fix commit). Adds cross-check #20
to vault-drift-sweep.md enforcing `base_sha == head_sha^` for
fix-peel cycles (m9-07+), exempting docs-peel cycles (m9-03..m9-06).

## Artifact index

| Path | SHA-256 |
|---|---|
| `cycle-artifacts/p-3416cfb8288f8964/m9-28-m9-19-fabricated-base-sha/apply-checkpoint.json` | `5717f59daa31170d010a4036f7633095977e5ebd6741f65b6c7a115d3294e2d7` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-28-m9-19-fabricated-base-sha/merge-receipt.md` | `abe066cfe97dc4172b3d06f5d00cc8b5744892b86f9953c8ab248f75b5576348` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-28-m9-19-fabricated-base-sha/release-receipt.md` | `5159923a7032bc2a65b0a61d699bafe7e259c8e1833272766959b61afb97b3b4` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-28-m9-19-fabricated-base-sha/release-report.md` | `c2941bf99177430b12493d45e208cd28ecfb49d1523d5a1544a0db7003006331` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-28-m9-19-fabricated-base-sha/verify-findings.json` | `a7d233db0140afd20241288d50da945956fd7d3e6c28d2151b8fa7868976e31b` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-28-m9-19-fabricated-base-sha/verify-report.md` | `aada5cd94b383f4f6ef71ca0a0c1b39de02d316b0233d9bb9a2dec37976ef53a` |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-28-m9-19-fabricated-base-sha/change-entry.md` | `38e5fc187baade07be9657e9e9cb574e393008306b63607e6883d8060a9c0ed3` |

## Evidence bindings

Vault metadata only. No code/runtime evidence (this cycle does not
modify any `.rs` file).

## Drift summary

None. This cycle **closes** drift:
- m9-19's `base_sha` was `6dce3739b7e2f0fcbdb2c10c0a35b27cfb2b8a37`
  (fabricated, doesn't exist in repo). Now `735c57b7178c93ea25f9cb603a3cb97b9ca7f81e`.

## Cross-checks added

- #20 (`vault-drift-sweep.md`): era-aware `base_sha == head_sha^`
  check for fix-peel cycles.
