# Archive Manifest — m9-36

| Field | Value |
|---|---|
| Cycle | m9-36-release-receipt-base-sha-backfill |
| Head SHA | `ced90eccf62f10e27ef0d76cab5b83bfc933172c` |
| Base SHA | `d7fd733064999cf93eea429a78939a02a1d4d27c` |
| Tag | `v0.7.34` |
| Path | B-direct |
| Date | 2026-09-12 |

## Summary

Two related drift classes closed in m9-03..m9-33 release-receipt.md:

1. **Missing Base SHA field** (31 files). The markdown table format
   used in m9-03..m9-33 omitted the `Base SHA` field.
2. **Format drift** — pipe-separated canonical format (introduced in
   m9-34) vs markdown table format (used m9-03..m9-33).

Both classes fixed by normalizing m9-03..m9-33 to the canonical
pipe-separated format with all fields.

Added cross-check #28 to vault-drift-sweep.md to enforce `Base SHA`
field presence in release-receipt.md going forward.

## Cross-checks

- C28: pass (after fixes applied)
- C1-C27: pass

## Evidence bindings

- `cycle-artifacts/p-3416cfb8288f8964/m9-36-release-receipt-base-sha-backfill/apply-checkpoint.json` → sha256: `58599b8a011a149d277231235c6e0caaf131765de0391dbb26d6d4caf71b09a2`
- `cycle-artifacts/p-3416cfb8288f8964/m9-36-release-receipt-base-sha-backfill/merge-receipt.md` → sha256: `0cff8f2de25075d454e1c2723daa029ff0d317644e3672cb7150dc876656dd3f`
- `cycle-artifacts/p-3416cfb8288f8964/m9-36-release-receipt-base-sha-backfill/release-receipt.md` → sha256: `07a9700ac10e9d99360b015e51895a94eef8fc63ca9b6ea183dfb1579d383ac4`
- `cycle-artifacts/p-3416cfb8288f8964/m9-36-release-receipt-base-sha-backfill/release-report.md` → sha256: `951a4a291133361cb25f74838080a60d4213b8cd0685969f294d706f8ef06039`
- `cycle-artifacts/p-3416cfb8288f8964/m9-36-release-receipt-base-sha-backfill/verify-findings.json` → sha256: `b2e6428d5ac2ad0736dc5d8bfd9a963ee9140eb23ff30ed6732df14208519aa1`
- `cycle-artifacts/p-3416cfb8288f8964/m9-36-release-receipt-base-sha-backfill/verify-report.md` → sha256: `13dcd9aa1afd41d381418b6f87fb490d7d1c126fbe007d25d84c00b2183f5f80`
