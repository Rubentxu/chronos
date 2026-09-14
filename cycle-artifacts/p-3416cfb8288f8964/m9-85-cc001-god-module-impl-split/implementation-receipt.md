# Implementation Receipt — m9-85-cc001-god-module-impl-split

## Summary

Closed the second slice of `cc-001-god-module` (P2, MEDIUM, m9-04) by
splitting the 11-method `impl SessionStore { ... }` block in
`crates/chronos-store/src/counterexample_storage.rs` (lines 448-909,
~462 lines, post-m9-84) into 3 sibling submodules grouped by concern:

- `ce_write.rs` (148 lines) — write-path methods.
- `ce_read.rs` (276 lines) — read-path methods.
- `ce_test_hooks.rs` (99 lines) — m9-05 R4 test chokepoints.

`counterexample_storage.rs`: 2720 → 2287 lines (-433 lines net).

## Files changed

### Added

- `crates/chronos-store/src/ce_write.rs` (148 lines).
  Contains `impl SessionStore` with 2 methods:
  - `pub fn save_counterexample_bundle(&self, record) -> Result<String, StoreError>`.
  - `fn save_bundle_record_and_events(&self, record, events) -> Result<String, StoreError>`
    (private atomic wrapper, called from `save_counterexample_bundle`).

- `crates/chronos-store/src/ce_read.rs` (276 lines).
  Contains `impl SessionStore` with 5 methods:
  - `pub fn load_counterexample_bundle_events(&self, bundle_id)`.
  - `pub fn count_counterexample_bundle_events(&self, bundle_id)`.
  - `fn get_bundle_events_count(tx, bundle_id)` (private, takes `tx`).
  - `pub fn load_counterexample_bundle(&self, bundle_id)`.
  - `pub fn list_counterexample_bundles(&self, filter)`.

- `crates/chronos-store/src/ce_test_hooks.rs` (99 lines).
  Contains `impl SessionStore` with 3 `#[doc(hidden)] pub fn` methods:
  - `insert_v2_chunk_for_test`.
  - `count_v3_chunks_for_test`.
  - `insert_bundle_record_for_test`.

### Modified

- `crates/chronos-store/src/counterexample_storage.rs`:
  - Removed the `impl SessionStore { ... }` block (lines 448-909 pre-refactor).
  - Added `#[path = "ce_write.rs"] pub mod ce_write;` etc.
  - Dropped unused `std::mem` import (was used by deleted impl block).
  - Trimmed `pub(super) use` of helpers referenced only in deleted block.

## Submodule wiring

```rust
// In counterexample_storage.rs:
#[path = "ce_write.rs"]
pub mod ce_write;
#[path = "ce_read.rs"]
pub mod ce_read;
#[path = "ce_test_hooks.rs"]
pub mod ce_test_hooks;
```

**No `pub use` re-exports** (intentional — see FIND-M9-85-NO-PUB-USE-METHODS).
Methods inside `impl SessionStore` are inherent methods of the type, not
module-level items, so `pub use ce_write::save_counterexample_bundle` fails
with E0432. The methods remain callable via `instance.method_name(...)` or
`<SessionStore>::method_name` from any module that imports `SessionStore`.
Verified every real caller uses the `instance.method_name(...)` form.

## Risk and verification

- Build: `cargo build --workspace` succeeds.
- Tests: chronos-store 77/77, chronos-services 264/264, chronos-cli 35/35,
  chronos-native 103/103 (--test-threads=1).
- Lint: `cargo clippy --workspace --all-targets -- -D warnings` clean.
- Format: `cargo fmt --all -- --check` clean.
- Round-trip: `chronos-cli/tests/replay_integration.rs` exercises
  `save_counterexample_bundle` → `load_counterexample_bundle` → `list_*` →
  `insert_*_for_test` → `count_*_for_test` paths (2/2 pass).

## Carry-forward

The `cc-001-god-module` debt remains open. Counterexample_storage.rs is now
2,287 lines (was 2,821 pre-m9-84, 2,720 post-m9-84, 2,287 post-m9-85).
Remaining concerns to address in future cycles:

- Type definitions section (structs `CounterexampleBundleRecord`,
  `CounterexampleBundleSummary`, `CounterexampleBundleFilter`, etc.,
  ~140 lines).
- Tests (~1,770 lines) — kept in same file as the impl they cover; could
  be moved to `tests.rs` in a future cycle.
- The standalone pub fns `bundle_events_or_legacy` and
  `bundle_events_count_or_legacy` (128 lines) — kept in parent since they
  are free functions, not methods on `SessionStore`.
