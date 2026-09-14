# Spec — m9-84-cc001-god-module-keys-split

## REQ-M9-84-01 — ce_chunk_keys submodule created

The encoding/decoding helpers from `counterexample_storage.rs`
(lines 100-225) MUST be moved to a new file
`crates/chronos-store/src/ce_chunk_keys.rs`.

**Scenario**: `ce_chunk_keys_pure_functions_compile`
- Given the new file exists with the moved functions
- When `cargo check -p chronos-store` runs
- Then it exits 0 with no warnings

## REQ-M9-84-02 — public surface preserved

The existing public surface at
`chronos_store::counterexample_storage::*` MUST remain identical:

- `pub fn encode_chunk_key_legacy(bundle_id, chunk_index) -> Vec<u8>` — still accessible
  at `chronos_store::counterexample_storage::encode_chunk_key_legacy`.
- All other helpers (`bundle_prefix`, `encode_chunk_key`, `decode_chunk_key`,
  `encode_chunk_value`, `decode_chunk_value`, `decode_chunk_payload`,
  `decode_chunk_key_legacy`) — still callable from inside
  `counterexample_storage.rs` without `crate::` paths.

**Scenario**: `public_api_unchanged`
- Given the refactor is applied
- When `cargo test -p chronos-services --lib --no-fail-fast` runs
- Then all tests that import from `chronos_store::counterexample_storage::*`
  compile and pass without modification.

## REQ-M9-84-03 — behaviour preservation

All 77 `chronos-store` lib unit tests MUST pass before and after the
refactor (round-trip verification via `git stash`).

**Scenario**: `lib_tests_baseline_match`
- Given baseline count from `cargo test -p chronos-store --lib` is N
- When the refactor is applied
- Then post-refactor count is also N
- And no test names change

## REQ-M9-84-04 — clippy clean

`cargo clippy --workspace --all-targets -- -D warnings` MUST exit 0
post-refactor.

**Scenario**: `clippy_clean`
- Given the refactor is applied
- When `cargo clippy --workspace --all-targets -- -D warnings` runs
- Then it exits 0 with no warnings or errors.

## REQ-M9-84-05 — downstream compilation

`cargo build --workspace` MUST exit 0 post-refactor. All downstream
crates that import from `chronos_store::counterexample_storage::*`
(`chronos-services`, `chronos-cli`) must compile unchanged.

**Scenario**: `workspace_compiles`
- Given the refactor is applied
- When `cargo build --workspace` runs
- Then it exits 0 with no warnings.

## REQ-M9-84-06 — net file size reduction

`counterexample_storage.rs` line count MUST decrease by ~125 lines
(the moved section + the section comments).

**Scenario**: `file_size_reduction`
- Given pre-refactor line count L (2821)
- When the refactor is applied
- Then post-refactor line count is approximately L - 125 (±10 for
  comment adjustments).

## Non-requirements

- The `impl SessionStore` block is NOT split in this cycle.
- Type definitions are NOT moved.
- Tests are NOT moved.
- No new public API is added.
- No new tests are added (existing tests still cover the moved code).
