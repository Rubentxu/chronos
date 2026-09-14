# Exploration Report — m9-86-cc001-god-module-types-split

## Context

`cc-001-god-module` is the P2 debt finding from m9-04:
`counterexample_storage.rs` was at 2,821 lines pre-m9-84, now at
2,287 lines post-m9-85. m9-84 extracted chunk key/value encoding
helpers (`ce_chunk_keys.rs`, 134 lines). m9-85 extracted the
`impl SessionStore` block into 3 sibling submodules
(`ce_write.rs` 148, `ce_read.rs` 276, `ce_test_hooks.rs` 99).

This cycle (m9-86) extracts the type definitions section.

## Current state of counterexample_storage.rs (post-m9-85)

- Lines 1-60: module-level doc comments (`//!`).
- Lines 61-104: submodule declarations + `pub(super) use` re-exports
  for the 4 submodules (ce_chunk_keys, ce_write, ce_read, ce_test_hooks).
- Lines 105-329: standalone free functions (`collect_v3_keys_for_bundle`,
  `collect_bundle_chunks_legacy`, `collect_bundle_chunks`,
  `KNOWN_BUNDLE_SCHEMA_VERSIONS`, `default_schema_version`,
  table definitions, chunk-size constants).
- Lines 330-513: **type definitions** — 6 `pub` items.
- Lines 478-513: standalone free functions `bundle_events_or_legacy`
  and `bundle_events_count_or_legacy`.
- Lines 515-2287: `#[cfg(test)] mod tests` (1,772 lines).

## Scope decision: types only

Lines 335-513 (~178 lines) hold 6 `pub` types:

| Type | Lines | Kind | Visibility |
|---|---|---|---|
| `MinimisedPayload` | 335-350 | enum | `pub` |
| `ExistencePredicateWire` | 358-362 | enum | `pub` |
| `HypothesisInputWire` | 381-398 | struct | `pub` |
| `CounterexampleBundleFilter<'a>` | 412-422 | struct | `pub` |
| `CounterexampleBundleSummary` | 429-448 | struct | `pub` |
| `CounterexampleBundleRecord` | 452-476 | struct | `pub` |

Plus the `default_schema_version()` helper at line 327 (used by
`#[serde(default = "...")]` on `summary.schema_version` and
`record.schema_version`).

## Risk analysis

### 1. `pub use` re-export pattern works for types

Unlike the `impl SessionStore` methods in m9-85 (which are type-level
items), these are `pub struct`/`pub enum` — module-level items. The
m9-84 `pub(super) use` pattern applies cleanly.

### 2. `#[serde(default = "...")]` path resolution

`default_schema_version` is referenced by string in `#[serde(default = "...")]`
attributes on the type definitions. When the struct moves to a submodule,
serde resolves the function in the **struct's module scope**. Two options:

- (a) Move `default_schema_version` to `ce_types.rs` alongside the structs.
- (b) Reference via full path `#[serde(default = "crate::counterexample_storage::default_schema_version")]`.

Option (a) is cleaner: keep related code in the same submodule. The
function becomes a `pub(crate) fn` so the parent's tests (line 925
`assert_eq!(default_schema_version(), ...)`) can still reach it via
`use super::*;` or `crate::counterexample_storage::default_schema_version`.

### 3. Tests `use super::*;` resolves submodule items

The test module at line 516 uses `use super::*;` which brings in all
parent items. After moving types to `ce_types.rs`, the test module's
`use super::*;` will not pick up `CounterexampleBundleRecord` etc.
directly. Solution: in `ce_types.rs`, the items are `pub`, and the
parent re-exports them with `pub use ce_types::{CounterexampleBundleRecord, ...};`
— then `use super::*;` in tests picks them up via re-exports.

Alternatively, change tests to `use crate::counterexample_storage::*;`
explicitly — but that's a larger churn. Re-exports are the
minimally-invasive path.

### 4. Downstream consumers

Downstream crates use:

- `chronos_store::counterexample_storage::CounterexampleBundleRecord`
  (replay.rs:28, counterexample.rs:41, replay_integration.rs).
- `chronos_store::counterexample_storage::MinimisedPayload`
  (replay.rs:28).
- `chronos_store::counterexample_storage::ExistencePredicateWire`
  (replay.rs:28, counterexample.rs:41).

If `pub use ce_types::{CounterexampleBundleRecord, ...};` re-exports
at the parent module, the downstream paths remain byte-identical.

### 5. Submodule usage of types

`ce_write.rs`, `ce_read.rs`, `ce_test_hooks.rs` already import types
via `use crate::counterexample_storage::{CounterexampleBundleRecord, ...};`.
These paths continue to work after the re-export.

## Strategy

Create `crates/chronos-store/src/ce_types.rs` containing:

- `pub enum MinimisedPayload { ... }`.
- `pub enum ExistencePredicateWire { ... }`.
- `pub struct HypothesisInputWire { ... }`.
- `pub struct CounterexampleBundleFilter<'a> { ... }`.
- `pub struct CounterexampleBundleSummary { ... }`.
- `pub struct CounterexampleBundleRecord { ... }`.
- `pub(crate) fn default_schema_version() -> u32 { CURRENT_BUNDLE_SCHEMA_VERSION }`.

In `counterexample_storage.rs`:

```rust
#[path = "ce_types.rs"]
pub(crate) mod ce_types;
pub use ce_types::{
    CounterexampleBundleFilter, CounterexampleBundleRecord, CounterexampleBundleSummary,
    ExistencePredicateWire, HypothesisInputWire, MinimisedPayload,
};
```

Why `pub(crate) mod` + `pub use` (not `pub mod`): the types are only
needed within the workspace (`pub` re-exports at parent path give
external reachability). `pub(crate)` keeps the submodule private to
the crate, matching the m9-84 `ce_chunk_keys` pattern.

## Submodule wiring

```rust
// In counterexample_storage.rs:
#[path = "ce_types.rs"]
pub(crate) mod ce_types;
pub use ce_types::{
    CounterexampleBundleFilter, CounterexampleBundleRecord, CounterexampleBundleSummary,
    ExistencePredicateWire, HypothesisInputWire, MinimisedPayload,
};
```

## Behaviour preservation

No semantic changes. Types move verbatim. `default_schema_version`
moves alongside. Tests stay in the same file (no test relocation).

## Risk

Low. The split is purely lexical. `pub use` re-export of `pub struct`/
`pub enum` is well-supported Rust.

## Out of scope

- Tests (~1,772 lines) — kept in same file as the impl they cover.
- The standalone pub fns `bundle_events_or_legacy` and
  `bundle_events_count_or_legacy` (lines 478-513) — kept in parent since
  they reference types via the parent path and are not types themselves.
- The free functions `collect_v3_keys_for_bundle`,
  `collect_bundle_chunks_legacy`, `collect_bundle_chunks`,
  `KNOWN_BUNDLE_SCHEMA_VERSIONS` (lines 217-326) — out of scope; could
  be split in m9-87+ if needed.
