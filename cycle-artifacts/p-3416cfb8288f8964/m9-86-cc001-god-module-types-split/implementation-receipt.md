# Implementation Receipt — m9-86-cc001-god-module-types-split

## Summary

Closed the third slice of `cc-001-god-module` (P2, MEDIUM, m9-04) by
extracting the 6 `pub` types + the `default_schema_version` serde helper
from `crates/chronos-store/src/counterexample_storage.rs` (lines
327-513 post-m9-85, ~187 lines) into a new sibling submodule
`crates/chronos-store/src/ce_types.rs`.

`counterexample_storage.rs`: 2287 → 2156 lines (-131 net extracted).

## Files changed

### Added

- `crates/chronos-store/src/ce_types.rs` (165 lines, NEW). Contains:
  - `pub enum MinimisedPayload`.
  - `pub enum ExistencePredicateWire`.
  - `pub struct HypothesisInputWire`.
  - `pub struct CounterexampleBundleFilter<'a>`.
  - `pub struct CounterexampleBundleSummary`.
  - `pub struct CounterexampleBundleRecord`.
  - `pub(crate) fn default_schema_version() -> u32`.

### Modified

- `crates/chronos-store/src/counterexample_storage.rs`:
  - Removed the 6 type definitions and `default_schema_version` (lines
    327-513 post-m9-85).
  - Added `#[path = "ce_types.rs"] pub(crate) mod ce_types;`.
  - Added `pub use ce_types::{CounterexampleBundleFilter,
    CounterexampleBundleRecord, CounterexampleBundleSummary,
    ExistencePredicateWire, HypothesisInputWire, MinimisedPayload};`.
  - Added `#[cfg(test)] pub(crate) use ce_types::default_schema_version;`
    (test-only re-export so `mod tests { use super::*; }` can call
    `default_schema_version()` directly).
  - Marked test-only imports (`PropertyValue`) with `#[cfg(test)]`.

## Submodule wiring

```rust
// In counterexample_storage.rs:
#[path = "ce_types.rs"]
pub(crate) mod ce_types;
pub use ce_types::{
    CounterexampleBundleFilter, CounterexampleBundleRecord, CounterexampleBundleSummary,
    ExistencePredicateWire, HypothesisInputWire, MinimisedPayload,
};
#[cfg(test)]
pub(crate) use ce_types::default_schema_version;
```

Why `pub(crate) mod` (not `pub mod`): the types are reachable at
`chronos_store::counterexample_storage::*` via the `pub use` re-exports.
`pub(crate)` keeps the submodule internal, matching the m9-84
`ce_chunk_keys` pattern.

## Serde default resolution

`#[serde(default = "default_schema_version")]` annotations on
`CounterexampleBundleSummary.schema_version` and
`CounterexampleBundleRecord.schema_version` resolve `default_schema_version`
in the struct's module scope (`ce_types.rs`), where the function now
lives. Verified by `m9_02_legacy_bundle_deserializes_with_default_schema_version`
test passing (legacy bundle gets `schema_version = 3`).

## Risk and verification

- Build: `cargo build --workspace` succeeds.
- Tests: chronos-store 77/77, chronos-services 264/264, chronos-cli
  35/35, chronos-native 103/103 (--test-threads=1).
- Lint: `cargo clippy --workspace --all-targets -- -D warnings` clean.
- Format: `cargo fmt --all -- --check` clean.
- Round-trip: `chronos-cli/tests/replay_integration.rs` continues to
  exercise save/load paths (2/2 pass).

## Carry-forward

The `cc-001-god-module` debt remains open. counterexample_storage.rs is
now 2,156 lines (was 2,821 pre-m9-84, 2,720 post-m9-84, 2,287 post-m9-85,
2,156 post-m9-86). Remaining concerns:

- Free functions `collect_v3_keys_for_bundle`, `collect_bundle_chunks_legacy`,
  `collect_bundle_chunks`, `KNOWN_BUNDLE_SCHEMA_VERSIONS` (~110 lines)
  — out of scope; could be split in m9-87+ if needed.
- Tests (~1,750 lines) — kept in same file as the impl they cover.
- Standalone pub fns `bundle_events_or_legacy` and
  `bundle_events_count_or_legacy` (35 lines) — kept in parent since
  they reference types via the parent path.
