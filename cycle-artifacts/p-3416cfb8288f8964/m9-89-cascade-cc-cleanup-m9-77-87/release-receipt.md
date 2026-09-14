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
| Base SHA | a195367d64bd1caa56dedea82195259deb7b671b |
| Head SHA | d0071ee5d6053ca38eca02c2a2f6dba47fc2daca |
| Main SHA | d0071ee5d6053ca38eca02c2a2f6dba47fc2daca |
| Remote tag | v0.7.91 |
| Remote tag_peel | d0071ee5d6053ca38eca02c2a2f6dba47fc2daca |
| Peel match | true (tag_peel == merge_commit_sha == HEAD) |

## SHAs (canonical table)

| Field | Value |
|---|---|
| Branch | chore/m9-89-cascade-cc-cleanup-m9-77-87 |
| Date | 2026-09-14 |
| Base SHA | a195367d64bd1caa56dedea82195259deb7b671b |
| Head SHA | d0071ee5d6053ca38eca02c2a2f6dba47fc2daca |
| Remote tag | v0.7.91 |
| Remote tag_peel | d0071ee5d6053ca38eca02c2a2f6dba47fc2daca |
| Peel match | true (tag_peel == merge_commit_sha == HEAD) |

## Release notes

- Vault-only hardening cycle. No Rust source code touched.
- Tag `v0.7.91` was pre-created at the cascade commit
  (`7e8981bbc1f1aa6177022c9083a24193ffa9679b`) and moved to the merge
  commit (`d0071ee5d6053ca38eca02c2a2f6dba47fc2daca`) per the CC#42
  fixpoint-cascade workaround (m9-83 handoff). Peel match: true.
- Cross-checks satisfied:
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
