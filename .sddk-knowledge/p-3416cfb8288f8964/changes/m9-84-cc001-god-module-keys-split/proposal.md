# Proposal — m9-84-cc001-god-module-keys-split

## Intent

Close `cc-001-god-module` (P2, MEDIUM, m9-04) by extracting the chunk
key/value encoding + decoding helpers from `counterexample_storage.rs`
into a sibling submodule `ce_chunk_keys.rs`.

## Approach

1. Create `crates/chronos-store/src/ce_chunk_keys.rs` (new sibling file).
2. Move the v3/v2 key+value encode/decode helpers verbatim.
3. Declare `pub mod ce_chunk_keys;` at the top of `counterexample_storage.rs`
   so all existing internal references and the one external
   `encode_chunk_key_legacy` reference keep working unchanged.
4. Remove the moved lines from `counterexample_storage.rs`.

This is a behaviour-preserving refactor. The encoding helpers are pure
functions with no state, no `self` methods, no side effects.

## Path

A-lite (bounded, cross-cutting within one crate, no architectural fork).

## Tier required

T1 (lib unit only — no integration tests touch the moved functions
externally, but downstream crates `chronos-services` and
`chronos-cli` reference types from the parent module and must still
compile + pass).

## Tier plan

- T0: `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings`.
- T1: `cargo test -p chronos-store --lib --no-fail-fast` (77 tests).
- T2: `cargo test -p chronos-services --lib --no-fail-fast` (downstream callers).
- T3: `cargo test -p chronos-cli --lib --no-fail-fast` (downstream callers).

No sandbox needed (no probe/mcp touched).

## Carry-forward

Closes the encoding-helper concern of `cc-001-god-module`. Does NOT
close the debt finding itself (the file remains >2,500 lines after
this cycle). The remaining 4 concerns will be split across m9-85+.

## Out of scope

- The `impl SessionStore` block (472 lines, ~30 methods) — m9-85+.
- Type definitions section (~140 lines) — could split but reference
  coupling makes it lower priority.
- Tests (~1,770 lines) — kept in same file as the impl they cover.
