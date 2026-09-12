# Archive Manifest — m9-31

| Field | Value |
|---|---|
| Cycle | m9-31-merge-receipt-fields-normalize |
| Head SHA | `5c286b6b19684cc59a0c8e7935280051d14e0e54` |
| Base SHA | `6c428f76ec2f57bd210e936ab7b2f9a40e207d63` |
| Tag | `v0.7.29` |
| Path | B-direct |
| Date | 2026-09-12 |

## Summary

Normalizes merge-receipt.md SHA fields (Head SHA, Base SHA, Branch,
Date) across all 25 m9 cycles (m9-03..m9-27) to the canonical format
established by m9-28+. Adds cross-check #23 that explicitly checks
both field presence and SHA consistency.

## Artifact index

| Path | SHA-256 |
|---|---|
| `cycle-artifacts/p-3416cfb8288f8964/m9-31-merge-receipt-fields-normalize/apply-checkpoint.json` | `bf2eb57b595be9f5343816d018573c68fdbf94322d2211c23c43cd5dc52e876f` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-31-merge-receipt-fields-normalize/merge-receipt.md` | `fa1effe9cf59c216bb1601477fd9596ad9343155601400cf80358cb027c5e3cf` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-31-merge-receipt-fields-normalize/release-receipt.md` | `ad59c61cd3fbb471361a3b0102dd24c33cfe6f9760ba3f13eadb790b7f26052e` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-31-merge-receipt-fields-normalize/release-report.md` | `17c88c3305bda3e8a28669f2f676e537f58e368fc198adae0546d506f4fdcfe0` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-31-merge-receipt-fields-normalize/verify-findings.json` | `793ac9fcfc7410fe8e7991dfb482e11d08eb386d196013c5821cd6fcbd400194` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-31-merge-receipt-fields-normalize/verify-report.md` | `0bff5d5dd45813dd5882b68a118a08c482e91b055556359a7c2bd5966321348c` |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-31-merge-receipt-fields-normalize/change-entry.md` | `728eef117d89d4f4397ec91ba7f71a2463bab2558efc200fd0587de28de671f5` |

## Evidence bindings

Vault metadata only. Each cycle artifact listed with its computed
SHA-256 in the Artifact index above.

## Drift summary

25 merge-receipt.md files (m9-03..m9-27) had inconsistent field names
and missing SHA fields. m9-31 normalizes all to canonical format.

## Cross-checks added

- #23 (`vault-drift-sweep.md`): merge-receipt.md must have canonical
  SHA fields (Head SHA, Base SHA, Branch, Date).
