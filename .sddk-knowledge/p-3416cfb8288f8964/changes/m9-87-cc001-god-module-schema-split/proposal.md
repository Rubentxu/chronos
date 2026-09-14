# Proposal — m9-87-cc001-god-module-schema-split

## Intent

Close the schema concern of `cc-001-god-module` by extracting the
schema/table/collect-helpers section from
`crates/chronos-store/src/counterexample_storage.rs` (lines 127-344
post-m9-86, ~217 lines) into a new sibling submodule `ce_schema.rs`.

## Approach

1. Create `crates/chronos-store/src/ce_schema.rs` containing:
   - `const COUNTEREXAMPLE_BUNDLES: TableDefinition<&[u8], &[u8]>`.
   - `const COUNTEREXAMPLE_BUNDLE_EVENTS: TableDefinition<&[u8], &[u8]>`.
   - `pub const BUNDLE_EVENTS_CHUNK_SIZE: usize = 256`.
   - `pub fn collect_bundle_chunks_range(tx, bundle_id)`.
   - `fn collect_v3_keys_for_bundle(tx, bundle_id)`.
   - `fn collect_bundle_chunks_legacy(tx, bundle_id)`.
   - `fn collect_bundle_chunks(tx, bundle_id, events_count)`.
   - `pub const CURRENT_BUNDLE_SCHEMA_VERSION: u32 = 3`.
   - `const KNOWN_BUNDLE_SCHEMA_VERSIONS: &[u32] = &[1, 2, 3]`.
   - `const _:` compile-time invariant block.

2. In `counterexample_storage.rs`:
   - Declare `#[path = "ce_schema.rs"] pub(crate) mod ce_schema;`.
   - Re-export `pub use ce_schema::{BUNDLE_EVENTS_CHUNK_SIZE,
     CURRENT_BUNDLE_SCHEMA_VERSION, collect_bundle_chunks_range};`.
   - Delete the moved section (lines 127-344).

3. Sibling submodules (ce_write, ce_read, ce_test_hooks, ce_types)
   continue to import via `crate::counterexample_storage::{...}` —
   the parent's `pub use` re-exports keep `pub` items reachable at the
   parent path; sibling submodules reach private items via the parent's
   `pub(crate) mod ce_schema;` re-export of ce_schema items.

## Path

A-min (cross-crate, bounded scope, single-crate change but with
several downstream consumers that must continue to compile).

## Tier required

T0 + T2 — lib tests + downstream crates (chronos-services uses
`CURRENT_BUNDLE_SCHEMA_VERSION`).

## Tier plan

- T0: `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings`.
- T1: `cargo test -p chronos-store --lib --no-fail-fast` (77 tests).
- T2: `cargo test -p chronos-services --lib --no-fail-fast` (264 tests).
- T3: `cargo test -p chronos-cli --no-fail-fast` (35 tests, including the replay_integration round-trip).
- T4: `cargo test -p chronos-native --lib -- --test-threads=1` (103 tests, per AGENTS.md §6.5).

No sandbox needed (no probe/mcp touched).

## Carry-forward

Closes the schema concern of `cc-001-god-module`. Does NOT close the
debt finding itself (the file remains 2,156 - 217 + ~10 = ~1,950 lines
after this cycle). The remaining concerns (tests ~1,790 lines) will be
split across m9-88+.

## Out of scope

- Tests (~1,790 lines) — kept in same file as the impl they cover.
- The standalone pub fns `bundle_events_or_legacy` and
  `bundle_events_count_or_legacy` (lines 346-364) — kept in parent since
  they reference types via the parent path.
- Further splitting of the schema section (e.g., moving the
  collect_helpers to a separate submodule) — the compile-time invariant
  block binds the constants, so they must stay together.
