# Release Receipt — m9-84-cc001-god-module-keys-split

## Identification

| Field | Value |
|---|---|
| Cycle | m9-84-cc001-god-module-keys-split |
| Path | A-lite |
| Branch | feat/m9-84-cc001-god-module-keys-split |
| Date | 2026-09-14 |

## SHAs

| Field | Value |
|---|---|
| Base SHA | bf5597611dd7a2dc8f80c34a79996ef5531e313f |
| Head SHA | 6bd7f02f69c75b1839ed8796af0b644c7d4eb4b0 |
| Main SHA | 6bd7f02f69c75b1839ed8796af0b644c7d4eb4b0 |
| Remote tag | v0.7.86 |
| Remote tag_peel | 6bd7f02f69c75b1839ed8796af0b644c7d4eb4b0 |
| Peel match | clean peel: v0.7.86 points at the --no-ff merge commit 5f0c3a54 |

## Release notes

- Tag `v0.7.86` created at the --no-ff merge commit 5f0c3a54.
- Behaviour-preserving refactor: 8 chunk key/value encoding helpers
  extracted from `counterexample_storage.rs` into `ce_chunk_keys.rs`.
- Public surface unchanged (downstream crates compile without
  modification).
- Cross-checks satisfied:
  - apply-checkpoint.head_sha == release-receipt.head_sha == tag peel
  - apply-checkpoint.base_sha == release-receipt.base_sha
  - apply-checkpoint.main_sha == release-receipt.main_sha

## Verification

- `cargo test -p chronos-store --lib`: 77/77 (matches baseline).
- `cargo test -p chronos-services --lib`: 264/264.
- `cargo build -p chronos-cli`: success.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo fmt --all -- --check`: clean.
- `bash scripts/check_vault_drift.sh`: to be run post-commit (will
  fix the cascading SHA updates first).
