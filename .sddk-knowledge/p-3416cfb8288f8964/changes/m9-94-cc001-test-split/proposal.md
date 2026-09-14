# Proposal — m9-94: cc-001 god-module test-split

## Identification

| Field | Value |
|---|---|
| Cycle ID | `m9-94-cc001-test-split` |
| Workspace | `p-3416cfb8288f8964` |
| Path | B-direct (mechanical test code extraction, no behavior change) |
| Status | proposed |
| Branch | `chore/m9-94-cc001-test-split` |
| Tag | `v0.7.96` |

## Subject

Extract the 1771-line `mod tests { ... }` block from
`crates/chronos-store/src/counterexample_storage.rs` into a sibling
file `crates/chronos-store/src/ce_storage_tests.rs` using the
`#[path = "..."]` submodule pattern already established by the
m9-84..m9-87 cc-001 production code splits.

## Problem

`crates/chronos-store/src/counterexample_storage.rs` is 1960 lines
but the production (non-test) code is only 188 lines. The bulk
(1771 lines, ~90% of the file) is the inline `mod tests { ... }`
block.

This is the **test half** of the cc-001 god-module smell that the
m9-84..m9-87 cycles did not address:

- m9-84 (keys-split): extracted `ce_chunk_keys.rs` — production code
- m9-85 (impl-split): extracted `ce_read.rs` + `ce_write.rs` + `ce_test_hooks.rs` — production code
- m9-86 (types-split): extracted `ce_types.rs` — production code
- m9-87 (schema-split): extracted `ce_schema.rs` — production code

After those splits, the production code was nicely factored into 5
sibling files, but the test block stayed monolithic in the parent
file.

## Approach

B-direct mechanical refactor using the existing `#[path = "..."]`
submodule pattern (same pattern as the m9-84..m9-87 splits):

1. Create `crates/chronos-store/src/ce_storage_tests.rs` (new sibling
   file).
2. Move the entire content of `mod tests { ... }` block from
   `counterexample_storage.rs` into `ce_storage_tests.rs` (with the
   `use super::*;` import — sibling `#[path]` submodules preserve the
   parent's scope).
3. Replace the inline `mod tests { ... }` block in
   `counterexample_storage.rs` with a one-line declaration:
   ```rust
   #[cfg(test)]
   #[path = "ce_storage_tests.rs"]
   mod tests;
   ```

This is a **purely mechanical refactor**:
- No public API change (tests are `#[cfg(test)]` only).
- No behavior change (every test moves verbatim).
- No fixture changes (`make_store()`, `make_event()`, `__force_collision_prefix()`
  helpers are all test-internal — they move with the test block).
- No re-export changes (the test block uses `super::*` which still
  resolves to the parent module's items including private types
  like `CounterexampleBundleRecord`, `CounterexampleBundleSummary`,
  etc.).

## Out-of-scope

- `crates/chronos-services/src/counterexample.rs` (2169 lines of
  tests) — separate cycle if/when warranted. The pattern will be
  identical, so m9-94 establishes the technique.
- `crates/chronos-services/src/counterexample_storage.rs` was
  deleted by m9-84..m9-87 splits; the production code lives in
  `crates/chronos-store/src/counterexample_storage.rs`.
- `cc-004-implicit-io-toctou` (deferred to m10+).
- `cc-001` production code split — already complete in m9-84..m9-87.

## Files changed

| Bucket | Files | Lines | Notes |
|---|---|---|---|
| chronos-store/src/ce_storage_tests.rs | 1 added | +1771 | verbatim copy of inline test block |
| chronos-store/src/counterexample_storage.rs | 1 modified | -1770 / +3 | removed test block, added `#[path]` declaration |

**Total**: 1 file added + 1 file modified; net ~0 LoC.

## Tier

- **T0**: `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings`.
- **T1**: `cargo test -p chronos-store --lib --no-fail-fast` (must still pass 77 tests, now from the new sibling file).

No sandbox needed (no probe/mcp touched; no behavior change).

## Verification

- All 77 chronos-store lib tests continue to pass.
- The new sibling file `ce_storage_tests.rs` compiles as `#[cfg(test)]`
  only and inherits the parent module's scope (private items
  accessible via `super::*`).
- No clippy warnings introduced (the tests use the same idioms as
  before).
