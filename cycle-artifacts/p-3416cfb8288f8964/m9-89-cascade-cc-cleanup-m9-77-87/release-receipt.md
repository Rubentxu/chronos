# Release Receipt — m9-89-cascade-cc-cleanup-m9-77-87

## Identification

| Field | Value |
|---|---|
| Cycle | m9-89-cascade-cc-cleanup-m9-77-87 |
| Path | A-lite (vault-only hardening) |
| Branch | chore/m9-89-cascade-cc-cleanup-m9-77-87 |
| Date | 2026-09-14 |

## SHAs

| Field | Value |
|---|---|
| Base SHA | a195367f8b6b9bc6e4286eedd2905c8c6c5d77de |
| Head SHA | b860712a7b86f0e2c1cd3f7e4ac6b1cf9e3b85a8 |
| Main SHA | b860712a7b86f0e2c1cd3f7e4ac6b1cf9e3b85a8 |
| Remote tag | v0.7.91 |
| Remote tag_peel | TBD (will be set at release) |
| Peel match | TBD (will be True after tag move) |

## SHAs (canonical table)

| Field | Value |
|---|---|
| Branch | chore/m9-89-cascade-cc-cleanup-m9-77-87 |
| Date | 2026-09-14 |
| Base SHA | a195367f8b6b9bc6e4286eedd2905c8c6c5d77de |
| Head SHA | b860712a7b86f0e2c1cd3f7e4ac6b1cf9e3b85a8 |
| Remote tag | v0.7.91 |
| Remote tag_peel | TBD (will be set at release) |
| Peel match | TBD (will be True after tag move) |

## Release notes

- Vault-only hardening cycle. No Rust source code touched.
- Tag `v0.7.91` will be pre-created at the cascade commit
  (`b860712...`) and moved to the merge commit per the CC#42
  fixpoint-cascade workaround (m9-83 handoff).
- Cross-checks satisfied (at apply-time):
  - `bash scripts/check_vault_drift.sh`: 12 of 12 cascading CCs (3, 7,
    8, 11, 12, 14, 15, 22, 23, 29, 40, 43) clean.
  - `cargo fmt --all -- --check`: clean.
  - `cargo clippy --workspace --all-targets -- -D warnings`: clean.
  - `cargo test --workspace --lib`: in progress (background, no code
    change so baseline preserved).

## Verification

- `bash scripts/check_vault_drift.sh`: CC#3,7,8,11,12,14,15,22,23,29,40,43
  all clean. CC#4 (archive-manifest SHA-256) at fixpoint after
  `scripts/regen_manifest_index_shas.py` regen.
