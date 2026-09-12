# Archive Manifest — m9-37

| Field | Value |
|---|---|
| Cycle | m9-37-path-route-and-sha-normalize |
| Head SHA | `0d570b3ff534fcecf490fbbda6384d5b30721384` |
| Base SHA | `5225467421097d7b283a2b3e21d0ce38018c58bd` |
| Tag | `v0.7.35` |
| Path | B-direct |
| Date | 2026-09-12 |

## Summary

Two drift classes closed:

1. **apply-checkpoint.json `path` vs `route`** (31 files: m9-03..m9-33).
   Normalized all to canonical `route` field.
2. **change-entry.md Subject head_sha** (2 files: m9-34, m9-35).
   Fixed to match final HEAD.

Added cross-check #29 to vault-drift-sweep.md.

## Cross-checks

- C29: pass (after fixes applied)
- C1-C28: pass

## Evidence bindings

- `cycle-artifacts/p-3416cfb8288f8964/m9-37-path-route-and-sha-normalize/apply-checkpoint.json` → sha256: `61caef3740121e8cd6aec068a5c2a5a4c4be750b83e97fe9672ee9da717161a2`
- `cycle-artifacts/p-3416cfb8288f8964/m9-37-path-route-and-sha-normalize/merge-receipt.md` → sha256: `f6ca03e16f07569d6f8387e32cf2a7e6b1e8c38ef7e072bd3ffd773369fc1ad1`
- `cycle-artifacts/p-3416cfb8288f8964/m9-37-path-route-and-sha-normalize/release-receipt.md` → sha256: `6a8d01e4e6ba7a4831ab061ea3aa78c82108fae37f57500e1235290b28d749e9`
- `cycle-artifacts/p-3416cfb8288f8964/m9-37-path-route-and-sha-normalize/release-report.md` → sha256: `ac22cfaae6a30b64cb6bfe1eb21aeab72e6cfe260a57206e24b93d8093f45c25`
- `cycle-artifacts/p-3416cfb8288f8964/m9-37-path-route-and-sha-normalize/verify-findings.json` → sha256: `74c493cec07d39ac932c1c5958f9bad55295739b4ec295ddad190251ea6d8d83`
- `cycle-artifacts/p-3416cfb8288f8964/m9-37-path-route-and-sha-normalize/verify-report.md` → sha256: `3311d244c1c4e6c9c3a2c3cbd14d4aef51fa491d6cd1004df6a99f8a8ce0f06c`
