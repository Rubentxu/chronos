# Exploration Report — m9-84-cc001-god-module-keys-split

## Context

`cc-001-god-module` is a P2 debt finding from m9-04: `counterexample_storage.rs`
is at 2,821 lines (post-m9-74 growth), spans 5 distinct concerns, and is the
biggest file in `chronos-store`. The M7 milestone is blocked on splitting it.

## Scope decision: incremental, first-pass

A full 5-way split is too large for one cycle. This cycle extracts the
**chunk key/value encoding + decoding helpers** into a sibling submodule.
These are pure functions (~125 lines), have no dependency on the
`SessionStore` impl block, and are the cleanest separable concern.

## Findings from file inspection

`crates/chronos-store/src/counterexample_storage.rs` (2,821 lines):

| Section | Lines | Concern | Extractability |
|---|---|---|---|
| Module doc + imports | 1-99 | Module setup | keep |
| v3/v2 key+value encoding (helpers) | 100-225 | Pure encode/decode | **extract** |
| Chunk collection (range scan, v2 fallback) | 232-396 | Storage-coupled helpers | keep (uses `COUNTEREXAMPLE_BUNDLE_EVENTS`) |
| Schema version constants | 365-396 | Bundle envelope | keep |
| Type definitions (MinimisedPayload, etc.) | 406-547 | Public types | keep |
| `impl SessionStore` (30+ methods) | 549-1021 | Storage impl | keep (next split) |
| Standalone pub fns (`bundle_events_or_legacy`) | 1022-1049 | Public helpers | keep |
| Tests | 1050-2821 | Unit + integration | keep |

The encoding helpers (lines 100-225) are referenced by:
- 4 functions inside `counterexample_storage.rs` (collect_bundle_chunks_*,
  save_bundle_record_and_events).
- 12 test functions in the same file.
- Zero external callers (all are `fn`, not `pub fn`).

Public surface that must remain unchanged:
- `pub fn encode_chunk_key_legacy` (used by 4 test functions + 1
  internal storage method). Must stay accessible at
  `chronos_store::counterexample_storage::encode_chunk_key_legacy`.

## Strategy

1. Create `crates/chronos-store/src/ce_chunk_keys.rs` (new sibling file).
2. Move lines 100-225 verbatim. Change `fn` to `pub(super) fn` so the
   parent module can use them. Change `pub fn encode_chunk_key_legacy`
   to `pub fn encode_chunk_key_legacy` (kept public, the file is
   `mod ce_chunk_keys` declared in `counterexample_storage.rs` so
   `pub fn` inside the submodule is accessible via
   `crate::counterexample_storage::encode_chunk_key_legacy`).
3. In `counterexample_storage.rs`: add `pub mod ce_chunk_keys;` at the
   top. The `pub mod` re-exports the submodule's items at the parent
   path, so existing imports keep working unchanged.
4. Remove the moved lines from `counterexample_storage.rs`.

## Behaviour preservation

No semantic changes. All code is moved verbatim. Tests stay in the
same file (no test relocation).

## Risk

Low. The move is purely lexical. The encoding functions have no
side effects, no `self` methods, no state. The only verification needed
is:
- `cargo test -p chronos-store --lib` (77 tests pass before/after)
- `cargo clippy -p chronos-store --lib -- -D warnings` (clean)
- `cargo test -p chronos-services --lib` (downstream callers)

## Out of scope

- The `impl SessionStore` block (472 lines, ~30 methods). This is the
  next split target (m9-85+).
- Type definitions section (~140 lines). Could split but they are
  referenced by both the encoding layer and the storage impl, so
  splitting them creates friction.
- Tests (~1,770 lines). Could split but tests are tied to the impl
  they cover.
