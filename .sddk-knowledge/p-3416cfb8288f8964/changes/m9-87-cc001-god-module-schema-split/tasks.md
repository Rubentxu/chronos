# Tasks — m9-87-cc001-god-module-schema-split

## Slice 1 — create ce_schema submodule + delete moved section

### Task 1.1 — create ce_schema.rs

Create `crates/chronos-store/src/ce_schema.rs` containing:

Imports needed at top:
- `crate::ce_chunk_keys::{bundle_prefix, decode_chunk_key, decode_chunk_key_legacy, decode_chunk_value};`
  (these are `pub(super) use`'d from ce_chunk_keys at the parent).
- `crate::counterexample_storage::classify_read_table_error` (re-export of `crate::table_error::classify_read_table_error`).
- `crate::error::StoreError`.
- `redb::{ReadableTable, TableDefinition}`.

Wait — the parent imports `classify_read_table_error` directly from `crate::table_error::classify_read_table_error`. The submodule should do the same.

Let me check the current state:

Imports needed at top of `ce_schema.rs`:
- `crate::error::StoreError`.
- `crate::ce_chunk_keys::{bundle_prefix, decode_chunk_key, decode_chunk_key_legacy, decode_chunk_value};`
  (these are `pub(super) use`'d from ce_chunk_keys at the parent; sibling submodules access via `crate::counterexample_storage::{...}`).
- `crate::table_error::classify_read_table_error`.
- `redb::{ReadableTable, TableDefinition}`.

For sibling submodule access to helpers:
- `bundle_prefix`, `decode_chunk_key`, `decode_chunk_key_legacy`,
  `decode_chunk_value` are re-exported at the parent via `pub(super) use
  ce_chunk_keys::{...}` in the parent. From ce_schema (sibling of
  ce_chunk_keys), we access them via
  `crate::counterexample_storage::{bundle_prefix, decode_chunk_key, ...}`.

This works because `pub(super) use` makes items visible to the parent
module, and sibling submodules see parent items via `crate::parent::*`.

For `classify_read_table_error`: it's currently used in the parent via
`use crate::table_error::classify_read_table_error;`. ce_schema uses
the same path.

For `redb::ReadableTable` and `TableDefinition`: ce_schema needs both
(`TableDefinition` for the consts, `ReadableTable` for `.iter()` and
`.range()`).

### Task 1.2 — wire submodule in parent

In `counterexample_storage.rs`, add after the existing submodule
declarations:

```rust
/// Submodule owning the schema/table definitions, the chunk-size
/// constant, the 4 collect helpers, and the compile-time invariant
/// block binding `CURRENT_BUNDLE_SCHEMA_VERSION` to
/// `KNOWN_BUNDLE_SCHEMA_VERSIONS`.
///
/// Public items (`BUNDLE_EVENTS_CHUNK_SIZE`,
/// `CURRENT_BUNDLE_SCHEMA_VERSION`, `collect_bundle_chunks_range`) are
/// re-exported at the parent path so external callers continue to find
/// them at `chronos_store::counterexample_storage::*`. Private items
/// (TableDefinitions, the other collect helpers,
/// `KNOWN_BUNDLE_SCHEMA_VERSIONS`) are reachable by sibling submodules
/// via `crate::counterexample_storage::item_name`.
#[path = "ce_schema.rs"]
pub(crate) mod ce_schema;
pub use ce_schema::{
    collect_bundle_chunks_range, BUNDLE_EVENTS_CHUNK_SIZE,
    CURRENT_BUNDLE_SCHEMA_VERSION,
};
```

### Task 1.3 — delete the moved section

Delete lines 127-344 from `counterexample_storage.rs`:

- Lines 127-132: `COUNTEREXAMPLE_BUNDLES` const + doc comment.
- Lines 134-160: `COUNTEREXAMPLE_BUNDLE_EVENTS` const + doc comment.
- Lines 162-171: `BUNDLE_EVENTS_CHUNK_SIZE` const + doc comment.
- Lines 173-218: `collect_bundle_chunks_range` function + doc comment.
- Lines 220-260: `collect_v3_keys_for_bundle` function + doc comment.
- Lines 262-286: `collect_bundle_chunks_legacy` function + doc comment.
- Lines 288-312: `collect_bundle_chunks` function + doc comment.
- Line 314: `CURRENT_BUNDLE_SCHEMA_VERSION` const.
- Lines 316-324: `KNOWN_BUNDLE_SCHEMA_VERSIONS` const + doc comment.
- Lines 326-344: compile-time invariant block.

**Keep** lines 346-364 (`bundle_events_or_legacy` and
`bundle_events_count_or_legacy`) — these are standalone free functions
that reference types via the parent path, kept in parent.

### Task 1.4 — verify

- `cargo build -p chronos-store` (compile check).
- `cargo test -p chronos-store --lib --no-fail-fast` (77 tests).
- `cargo test -p chronos-services --lib --no-fail-fast` (264 tests;
  uses `cs::CURRENT_BUNDLE_SCHEMA_VERSION`).
- `cargo test -p chronos-cli --no-fail-fast` (35 tests; uses the
  constants via `replay_integration`).
- `cargo test -p chronos-native --lib -- --test-threads=1` (103 tests).
- `cargo build -p chronos-mcp` (sanity check for downstream mcp).
- `cargo clippy --workspace --all-targets -- -D warnings` (clean).
- `cargo fmt --all -- --check` (clean).

### Task 1.5 — commit + tag + merge

- Commit on `feat/m9-87-cc001-god-module-schema-split`.
- `--no-ff` merge into main.
- Tag `v0.7.89` (pre-created at the merge commit, per the
  fixpoint-cascade workaround).
- Push.
- Delete cycle branch.

### Task 1.6 — cycle-artifacts + handoff

Same as m9-86 (T6 + T7).

## Estimated work

- Task 1.1: ~5 min (1 file creation, mechanical).
- Task 1.2: ~3 min.
- Task 1.3: ~2 min.
- Task 1.4: ~5 min (build + 4 test suites + clippy + fmt).
- Task 1.5: ~5 min.
- Task 1.6: ~10 min.

Total: ~30 min wall time.

## Risk mitigation

- Round-trip: 77 + 264 + 35 + 103 test count match.
- Public surface preserved via `pub use` re-exports at the parent path.
- Compile-time invariant block moves with the constants (no behavior change).
- Lint + format clean via clippy + fmt.
