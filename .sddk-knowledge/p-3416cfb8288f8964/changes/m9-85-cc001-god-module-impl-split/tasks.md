# Tasks — m9-85-cc001-god-module-impl-split

## Slice 1 — create 3 submodules + remove impl SessionStore block

### Task 1.1 — create ce_write.rs

Create `crates/chronos-store/src/ce_write.rs` containing the
write-path `impl SessionStore` block:

- `pub fn save_counterexample_bundle(&self, record: ...) -> ...`
- `fn save_bundle_record_and_events(&self, record, events) -> ...` (private)

Imports needed:
- `crate::cas::ContentHash` (no — used elsewhere? check)
- `crate::counterexample_storage::*` or `super::*` (for the helpers
  `encode_chunk_key`, `encode_chunk_value`, `encode_chunk_key_legacy`,
  `decode_chunk_payload`, `collect_bundle_chunks_legacy`,
  `collect_v3_keys_for_bundle`, `COUNTEREXAMPLE_BUNDLES`,
  `COUNTEREXAMPLE_BUNDLE_EVENTS`, `BUNDLE_EVENTS_CHUNK_SIZE`,
  `CURRENT_BUNDLE_SCHEMA_VERSION`).
- `crate::error::StoreError`.
- `crate::storage::SessionStore` (the impl target).
- `crate::table_error::classify_read_table_error` (not used in
  write path, but `save_bundle_record_and_events` reads via
  `collect_v3_keys_for_bundle` which uses it internally — fine).
- `chronos_domain::TraceEvent`.
- `redb::ReadableTable` (for `tx.open_table`).
- `std::mem` (for `mem::take`).

Wait — the helpers (`encode_chunk_key`, etc.) are `pub(super) use`'d
at the parent module. The submodule needs to import them too. The
submodule can either:
- (a) Use `use crate::counterexample_storage::*;` (the parent module's
  re-exports), OR
- (b) Use `use crate::ce_chunk_keys::*;` directly.

Option (a) keeps the submodule agnostic of where the helpers live.
Going with (a).

### Task 1.2 — create ce_read.rs

Create `crates/chronos-store/src/ce_read.rs` containing the
read-path `impl SessionStore` block:

- `fn get_bundle_events_count(tx, bundle_id) -> Result<u64, StoreError>` (private, takes `tx`)
- `pub fn load_counterexample_bundle_events(&self, bundle_id) -> ...`
- `pub fn count_counterexample_bundle_events(&self, bundle_id) -> ...`
- `pub fn load_counterexample_bundle(&self, bundle_id) -> ...`
- `pub fn list_counterexample_bundles(&self, filter) -> ...`

Imports needed:
- Same as ce_write, plus `crate::table_error::classify_read_table_error`
  (used in `load_counterexample_bundle` and `list_counterexample_bundles`).

`Self::get_bundle_events_count` (called from the two `*_events` methods)
works inside the same `impl SessionStore` block in `ce_read.rs`.

### Task 1.3 — create ce_test_hooks.rs

Create `crates/chronos-store/src/ce_test_hooks.rs` containing the
test-chokepoint `impl SessionStore` block:

- `#[doc(hidden)] pub fn insert_v2_chunk_for_test(&self, bundle_id, chunk_idx, events) -> ...`
- `#[doc(hidden)] pub fn count_v3_chunks_for_test(&self, bundle_id) -> ...`
- `#[doc(hidden)] pub fn insert_bundle_record_for_test(&self, record) -> ...`

Imports needed: minimal (table access, bincode, store error).

### Task 1.4 — wire submodules in parent

In `counterexample_storage.rs`, add after the existing `ce_chunk_keys`
declaration:

```rust
#[path = "ce_write.rs"]
pub mod ce_write;
#[path = "ce_read.rs"]
pub mod ce_read;
#[path = "ce_test_hooks.rs"]
pub mod ce_test_hooks;
```

**No `pub use` re-exports.** Methods inside `impl SessionStore` blocks
are not module-level items, so `pub use ce_write::save_counterexample_bundle`
fails with E0432 ("no `save_counterexample_bundle` in
`counterexample_storage::ce_write`"). The methods remain callable
via `instance.save_counterexample_bundle(...)` or
`<SessionStore>::save_counterexample_bundle(...)` from any module
that imports `SessionStore` — which is the pattern every real caller
already uses (verified: every call site uses `.method_name(...)` form,
no path-qualified `counterexample_storage::method_name(...)` call
exists in the workspace).

### Task 1.5 — remove the impl SessionStore block

Delete lines 448-909 from `counterexample_storage.rs` (the
`impl crate::storage::SessionStore { ... }` block including the
section comment and the m9-05 R4 chokepoint comment).

### Task 1.6 — verify

- `cargo build -p chronos-store` (compile check).
- `cargo test -p chronos-store --lib --no-fail-fast` (77 tests).
- `cargo test -p chronos-services --lib --no-fail-fast` (264 tests).
- `cargo test -p chronos-cli --no-fail-fast` (35 tests; uses the
  test chokepoints via `replay_integration`).
- `cargo build -p chronos-mcp` (sanity check for downstream mcp).
- `cargo clippy --workspace --all-targets -- -D warnings` (clean).
- `cargo fmt --all -- --check` (clean).

### Task 1.7 — commit + tag + merge

- Commit on `feat/m9-85-cc001-god-module-impl-split`.
- `--no-ff` merge into main.
- Tag `v0.7.87` (pre-created at the merge commit, per the
  fixpoint-cascade workaround).
- Push.
- Delete cycle branch.

### Task 1.8 — cycle-artifacts + handoff

Same as m9-84 (T6 + T7).

## Estimated work

- Task 1.1-1.3: ~10 min (3 file creations, mechanical).
- Task 1.4: ~3 min.
- Task 1.5: ~2 min.
- Task 1.6: ~5 min (build + 3 test suites + clippy + fmt).
- Task 1.7: ~5 min.
- Task 1.8: ~10 min.

Total: ~35 min wall time.

## Risk mitigation

- Round-trip via `git stash` for behaviour preservation (77+264+35
  test count match).
- Public surface preserved via `pub use` re-exports at the parent path.
- Integration test (`chronos-cli/tests/replay_integration.rs`) exercises
  all 3 test chokepoints, providing real acceptance validation of the
  refactor.
- Lint + format clean via clippy + fmt.
