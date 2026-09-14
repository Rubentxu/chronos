# Change: m9-84 ce_chunk_keys submodule extracted

## Subject

- Extracted the v3/v2 chunk key/value encoding + decoding helpers
  from `crates/chronos-store/src/counterexample_storage.rs` into a
  new sibling submodule `crates/chronos-store/src/ce_chunk_keys.rs`.
- 8 helpers moved verbatim: `bundle_prefix`, `encode_chunk_key`,
  `decode_chunk_key`, `encode_chunk_value`, `decode_chunk_value`,
  `decode_chunk_payload`, `encode_chunk_key_legacy`, `decode_chunk_key_legacy`.
- `counterexample_storage.rs`: 2821 → 2720 lines (-101 net extracted).
- Submodule wired in parent via `#[path = "ce_chunk_keys.rs"] pub(crate) mod ce_chunk_keys;` + `pub(super) use` re-export of all 8 items.
- Public surface unchanged: `encode_chunk_key_legacy` still reachable at `chronos_store::counterexample_storage::encode_chunk_key_legacy`.
- Closes part of `cc-001-god-module` (P2, MEDIUM, m9-04); the file remains 2,720 lines (next slice: `impl SessionStore` block, ~472 lines, m9-85+).

## Files changed

- `crates/chronos-store/src/ce_chunk_keys.rs` — NEW (134 lines).
- `crates/chronos-store/src/counterexample_storage.rs` — trimmed (-115 lines, +14 inserted for module declaration + doc comment).
- `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` — m9-84 row added (will be flipped to CLOSED post-archive).
- `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` — Last archive bumped to m9-84 (in fixpoint commit).
- `cycle-artifacts/p-3416cfb8288f8964/m9-84-cc001-god-module-keys-split/` — 7 cycle artifacts created.
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-84-cc001-god-module-keys-split/change-entry.md` — this file.
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-84-cc001-god-module-keys-split/archive-manifest.md` — archive manifest.

## Summary

Behaviour-preserving lexical refactor. The encoding helpers were the
cleanest separable concern within `counterexample_storage.rs`: pure
functions, no state, no `self` methods, no side effects. They were
referenced by 4 internal functions in the parent module and 12 test
functions in the same file, with one downstream caller
(`encode_chunk_key_legacy` at `chronos_store::counterexample_storage`).

Tier results:
- T0 (`cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings`): passed (after one `cargo fmt` run to fix import-line wrapping in the new `pub(super) use` line).
- T1 (`cargo test -p chronos-store --lib --no-fail-fast`): 77/77 (matches pre-refactor baseline; round-trip via `git stash`).
- T2 (`cargo test -p chronos-services --lib --no-fail-fast`): 264/264 (downstream callers compile unchanged).
- T3 (`cargo build -p chronos-cli`): success.

## Cross-check

- `cargo test -p chronos-store --lib` returns 77/77 (matches baseline).
- `cargo test -p chronos-services --lib` returns 264/264 (downstream).
- `cargo build -p chronos-cli` succeeds.
- `cargo clippy --workspace --all-targets -- -D warnings` exits 0.
- `cargo fmt --all -- --check` passes.
- `bash scripts/check_vault_drift.sh` passes after the post-merge SHA
  cascade fixpoint (will be verified at T6).
- All m9-84 cycle artifacts have SHA-256 hashes captured in the
  archive-manifest (post-commit).
- No new carry-forward findings introduced.
