# Exploration Report — m9-94: cc-001 god-module test-split

## Context

m9-84..m9-87 (cc-001 production code splits) extracted 5 production
concerns from `counterexample_storage.rs` into sibling submodules:

- m9-84: `ce_chunk_keys.rs` (key encoding helpers)
- m9-85: `ce_read.rs` + `ce_write.rs` + `ce_test_hooks.rs` (impl methods)
- m9-86: `ce_types.rs` (6 pub types + helpers)
- m9-87: `ce_schema.rs` (table defs + chunk constants)

After these splits, the parent file `counterexample_storage.rs` was
~1960 lines. **188 lines** are production code (module header,
sub-module declarations, helpers `bundle_events_or_legacy` and
`bundle_events_count_or_legacy`). **1771 lines** are the inline
`mod tests { ... }` block.

The m9-93 handoff recommended m9-94 = "cc-001 god-module follow-up
(keys-split is smallest/lowest-risk)". The keys-split was the
smallest production split (m9-84); the **test-split** is the
smallest remaining split (no production behavior change).

## Current state

`wc -l` on `crates/chronos-store/src/`:

```
148 ce_read.rs           (production; m9-85)
165 ce_types.rs          (production; m9-86)
196 ce_schema.rs         (production; m9-87)
1960 counterexample_storage.rs  (188 prod + 1771 tests)
148 ce_write.rs          (production; m9-85)
70 ce_test_hooks.rs      (test hooks; m9-85)
```

The remaining 1771 lines of test code in the parent file is the
largest contributor to file length by far (90% of the file).

## Pattern: `#[path = "..."]` submodule

The m9-84..m9-87 splits used this Rust idiom:

```rust
// In counterexample_storage.rs
#[path = "ce_chunk_keys.rs"]
pub mod ce_chunk_keys;
```

The sibling file's items become accessible at the parent's path
(calls to `ce_chunk_keys::foo()` resolve to the sibling file's
contents). For test code, the equivalent is:

```rust
// In counterexample_storage.rs
#[cfg(test)]
#[path = "ce_storage_tests.rs"]
mod tests;
```

The sibling file's contents become a submodule named `tests` of
the parent, accessible at `parent::tests::*`. Since the test code
uses `use super::*;`, the parent's scope (including private items
like `CounterexampleBundleRecord`) is preserved.

This is **purely a file-level refactor**. No code logic changes.
The Rust module graph after extraction is identical to the graph
before — same items, same visibility, same resolution paths.

## What stays vs. what moves

### Stays in counterexample_storage.rs (188 lines):

- Module doc comments (lines 1-72)
- `#[cfg(test)] pub(super) use ce_chunk_keys::{...}` (line 80)
- 5 sibling submodule declarations (lines 90-149): `ce_write`, `ce_read`, `ce_test_hooks`, `ce_types`, `ce_schema`
- 2 free functions: `bundle_events_or_legacy`, `bundle_events_count_or_legacy` (lines 161-186)
- New `#[path]` declaration for `tests` (3 lines)

### Moves to ce_storage_tests.rs (1772 lines):

- `mod tests { ... }` body verbatim (1771 lines + 1 for the new file's outer wrapper)

## Test inventory (43 test functions)

By category:

- **Roundtrip / lifecycle** (4 tests): `save_then_load_roundtrip`, `load_unknown_returns_none`, `save_rejects_empty_bundle_id`, ...
- **List / filter / cursor** (4 tests): `list_filters_by_property_kind`, `list_with_limit_truncates`, `list_cursor_paginates_forward`, `list_cursor_unknown_id_returns_all`
- **m8-07 target hypothesis** (2 tests): `m8_07_save_then_load_preserves_target_hypothesis`, `m8_07_pre_m8_07_bundle_has_no_target_hypothesis`
- **m9-02 schema versioning** (5 tests): save/load/list/future-rejection/mismatch
- **m9-02 side-table events** (7 tests): chunk save/load/overwrite/count/legacy/post-m9-02/zero-events
- **m9-04 v3 key layout** (10 tests): prefix/keys/encode/decode/range/collision/ghost/fresh/schema/v2-fallback
- **m9-04 R4 chokepoint** (3 tests): v2-never-resaved, known-schema-versions, current-schema-versions-listed

Total: 43 tests + 4 helpers (`make_store`, `make_event`, `__force_collision_prefix`, `assert_send_*`).

All 43 tests and 4 helpers move verbatim.

## Why B-direct (not A-min)

- No public API change.
- No behavior change.
- No architectural decision.
- Pure mechanical file reorganization using an established pattern.
- Tier T1 only (chronos-store lib unit tests).
- No sandbox needed.

## Risk analysis

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| `#[path]` attribute breaks compile (e.g., path resolution) | very low | medium | Verified the pattern works in m9-84..m9-87. cargo build before commit will catch any issue. |
| Private items become inaccessible in the new sibling | low | medium | `use super::*;` in the test block already preserves parent scope; sibling `#[path]` modules inherit this. |
| Test count changes (test lost or duplicated) | very low | low | `cargo test -p chronos-store --lib` before + after — must report 77 either way. |
| File ends up with stray whitespace or comment headers | low | very low | `cargo fmt` runs as part of T0. |

## Recommendation

Proceed with m9-94 as B-direct. Single commit (Rust source) +
standard cycle artifacts + push.

## Out-of-scope (deferred to future cycles)

- `crates/chronos-services/src/counterexample.rs` test extraction (2169 lines of tests). Same pattern, but larger file; establish m9-94 first.
- cc-004-implicit-io-toctou (P3, LOW): explicit-IO refactor. m10+.
- cc-001 production code re-split: already complete in m9-84..m9-87.
