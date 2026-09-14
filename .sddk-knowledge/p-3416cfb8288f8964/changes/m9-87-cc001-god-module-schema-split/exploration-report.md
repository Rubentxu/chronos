# Exploration Report — m9-87-cc001-god-module-schema-split

## Context

`cc-001-god-module` is the P2 debt finding from m9-04:
`counterexample_storage.rs` was at 2,821 lines pre-m9-84, now at
2,156 lines post-m9-86. m9-84 extracted chunk key/value encoding helpers
(ce_chunk_keys.rs, 134 lines). m9-85 extracted the `impl SessionStore`
block into 3 sibling submodules (ce_write.rs 148, ce_read.rs 276,
ce_test_hooks.rs 99). m9-86 extracted the 6 `pub` types + the
`default_schema_version` helper (ce_types.rs, 165 lines).

This cycle (m9-87) extracts the schema/table/free-functions section.

## Current state of counterexample_storage.rs (post-m9-86)

- Lines 1-60: module-level doc comments.
- Lines 61-128: submodule declarations (ce_chunk_keys, ce_write,
  ce_read, ce_test_hooks, ce_types).
- Lines 127-344: **schema section** — 2 TableDefinitions, 2 `pub const`,
  1 private `const`, 4 `fn` collect helpers, compile-time invariant
  block.
- Lines 346-364: standalone free functions `bundle_events_or_legacy`
  and `bundle_events_count_or_legacy`.
- Lines 366-2156: `#[cfg(test)] mod tests` (1,790 lines).

## Scope decision: schema section as a unit

Lines 127-344 (~217 lines) hold the schema/table definitions and the
free functions that operate on them. They form a tightly coupled unit:

| Item | Lines | Visibility | Used by |
|---|---|---|---|
| `COUNTEREXAMPLE_BUNDLES` | 131-132 | private const | ce_write, ce_read, ce_test_hooks |
| `COUNTEREXAMPLE_BUNDLE_EVENTS` | 159-160 | private const | ce_write, ce_read, ce_test_hooks |
| `BUNDLE_EVENTS_CHUNK_SIZE` | 171 | `pub const` | ce_write, ce_types (doc reference) |
| `collect_bundle_chunks_range` | 181-218 | `pub fn` | ce_read, parent (`collect_bundle_chunks`) |
| `collect_v3_keys_for_bundle` | 229-260 | `fn` (private) | ce_write |
| `collect_bundle_chunks_legacy` | 268-286 | `fn` (private) | ce_write, parent (`collect_bundle_chunks`) |
| `collect_bundle_chunks` | 298-312 | `fn` (private) | ce_read |
| `CURRENT_BUNDLE_SCHEMA_VERSION` | 314 | `pub const` | ce_types (in `default_schema_version`), parent, downstream |
| `KNOWN_BUNDLE_SCHEMA_VERSIONS` | 324 | private const | const `_:` invariant |
| `const _:` invariant | 329-344 | compile-time | production build |

The compile-time invariant block (`const _:` at lines 329-344) binds
`CURRENT_BUNDLE_SCHEMA_VERSION` to `KNOWN_BUNDLE_SCHEMA_VERSIONS`. This
must stay together with both constants.

## Why this is a single submodule (not split further)

The constants and functions are tightly coupled by the compile-time
invariant. Splitting them across multiple submodules would either:

- (a) Duplicate the invariant block in each submodule (worse).
- (b) Move the invariant block to one submodule but reference the
  other submodule's constants (works but adds coupling).
- (c) Move all of them to a single `ce_schema.rs` submodule.

Option (c) is cleanest. The submodule owns:
- All table definitions (used by the 3 impl submodules).
- The chunk-size constant (used by ce_write).
- The 4 collect helpers (used by ce_write/ce_read + cross-call to each other).
- The schema-version constants (used by ce_types's `default_schema_version`).
- The compile-time invariant block.

## Risk analysis

### 1. Visibility

The TableDefinitions and the `collect_*` private functions are visible
to the submodules because submodules can access private items of their
parent. With the move to `ce_schema.rs` (also a submodule of
counterexample_storage), the privacy level remains the same: visible
within the parent module + its submodules.

For external reachability:
- `BUNDLE_EVENTS_CHUNK_SIZE` is `pub const` — re-export at parent via `pub use`.
- `CURRENT_BUNDLE_SCHEMA_VERSION` is `pub const` — re-export at parent via `pub use`.
- `collect_bundle_chunks_range` is `pub fn` — re-export at parent via `pub use`.

### 2. Compile-time invariant

The `const _:` block runs at compile time. Moving it to `ce_schema.rs`
keeps it in the same compilation unit as the constants it references.
No behavior change.

### 3. Cross-submodule calls

`collect_bundle_chunks` calls `collect_bundle_chunks_range` (public, in
same submodule after move). `collect_v3_keys_for_bundle` and
`collect_bundle_chunks_legacy` are called from ce_write (sibling
submodule). They're `fn` (private) — visible to the parent module and
its submodules via `super::*`.

After move: ce_write imports them via `use crate::counterexample_storage::{collect_v3_keys_for_bundle, collect_bundle_chunks_legacy};`.
The parent's `pub use ce_schema::{...}` re-exports only `pub` items,
so private `fn` items are reachable via the direct `crate::counterexample_storage::fn_name`
path (which works for sibling submodules).

### 4. Submodule access to TableDefinitions

ce_write.rs/ce_read.rs/ce_test_hooks.rs currently import via:
```rust
use crate::counterexample_storage::{
    encode_chunk_key, encode_chunk_key_legacy, encode_chunk_value,
    collect_bundle_chunks_legacy, collect_v3_keys_for_bundle,
    BUNDLE_EVENTS_CHUNK_SIZE, COUNTEREXAMPLE_BUNDLES, COUNTEREXAMPLE_BUNDLE_EVENTS,
    CURRENT_BUNDLE_SCHEMA_VERSION,
};
```

After move, the parent module re-exports the public items. For private
items (TableDefinitions, collect_v3_keys_for_bundle,
collect_bundle_chunks_legacy), the import path through the parent still
works because:
- The parent's `pub(crate) mod ce_schema;` declaration makes `ce_schema::*`
  reachable from the parent scope.
- The parent can re-export the items it wants with `pub use ce_schema::{...}`.
- For private items, the children can use `crate::counterexample_storage::item`
  directly (sibling submodule access).

## Strategy

Create `crates/chronos-store/src/ce_schema.rs` containing:

- `const COUNTEREXAMPLE_BUNDLES: TableDefinition<&[u8], &[u8]>`.
- `const COUNTEREXAMPLE_BUNDLE_EVENTS: TableDefinition<&[u8], &[u8]>`.
- `pub const BUNDLE_EVENTS_CHUNK_SIZE: usize = 256`.
- `pub fn collect_bundle_chunks_range(...)`.
- `fn collect_v3_keys_for_bundle(...)`.
- `fn collect_bundle_chunks_legacy(...)`.
- `fn collect_bundle_chunks(...)`.
- `pub const CURRENT_BUNDLE_SCHEMA_VERSION: u32 = 3`.
- `const KNOWN_BUNDLE_SCHEMA_VERSIONS: &[u32] = &[1, 2, 3]`.
- `const _:` compile-time invariant block.

In `counterexample_storage.rs`:

```rust
#[path = "ce_schema.rs"]
pub(crate) mod ce_schema;
pub use ce_schema::{
    BUNDLE_EVENTS_CHUNK_SIZE, CURRENT_BUNDLE_SCHEMA_VERSION,
    collect_bundle_chunks_range,
};
```

(`collect_v3_keys_for_bundle`, `collect_bundle_chunks_legacy`, and
`collect_bundle_chunks` stay private — they're only called from sibling
submodules via `crate::counterexample_storage::fn_name`.)

(TableDefinitions and `KNOWN_BUNDLE_SCHEMA_VERSIONS` stay private
inside the submodule; sibling submodules access them via
`crate::counterexample_storage::CONST_NAME`.)

## Submodule wiring

```rust
// In counterexample_storage.rs:
#[path = "ce_schema.rs"]
pub(crate) mod ce_schema;
pub use ce_schema::{
    BUNDLE_EVENTS_CHUNK_SIZE, CURRENT_BUNDLE_SCHEMA_VERSION,
    collect_bundle_chunks_range,
};
```

## Behaviour preservation

No semantic changes. Constants and functions move verbatim. Tests stay
in the same file (no test relocation).

## Risk

Low. The split is purely lexical. Compile-time invariant block moves
with the constants. The `pub use` re-exports preserve external reachability.

## Out of scope

- Tests (~1,790 lines) — kept in same file as the impl they cover.
- The standalone pub fns `bundle_events_or_legacy` and
  `bundle_events_count_or_legacy` (lines 346-364) — kept in parent since
  they reference types via the parent path and are not part of the
  schema section.
