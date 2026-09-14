# Tasks — m9-86-cc001-god-module-types-split

## Slice 1 — create ce_types submodule + delete moved section

### Task 1.1 — create ce_types.rs

Create `crates/chronos-store/src/ce_types.rs` containing the 6 `pub`
types + `pub(crate) fn default_schema_version`.

Imports needed at top of `ce_types.rs`:
- `crate::cas::ContentHash`.
- `crate::counterexample_storage::CURRENT_BUNDLE_SCHEMA_VERSION` —
  referenced by `default_schema_version` (or move the constant too —
  see Task 1.3).

Note: `ce_types.rs` is a child of `counterexample_storage`, so the
path `crate::counterexample_storage::CURRENT_BUNDLE_SCHEMA_VERSION`
is a self-reference from a child to its parent. Rust allows this for
items that exist by the time the child is type-checked.

For `default_schema_version`, the simplest fix: since the function is
defined IN `ce_types.rs`, it doesn't need to import itself. It needs
access to `CURRENT_BUNDLE_SCHEMA_VERSION` (defined in the parent at
line 302). Use `use crate::counterexample_storage::CURRENT_BUNDLE_SCHEMA_VERSION;`.

For the `#[serde(default = "default_schema_version")]` annotations on
the structs (defined IN `ce_types.rs`), serde resolves the function
name in the **scope of the struct's module** = `ce_types`. So
`default_schema_version` is directly reachable.

### Task 1.2 — wire submodule in parent

In `counterexample_storage.rs`, add after the existing submodule
declarations:

```rust
/// Submodule owning the 6 `pub` types used by counterexample bundle
/// persistence + the `default_schema_version` serde helper.
///
/// Types are `pub` in the submodule; this parent module re-exports
/// them with `pub use` so internal callers and external crates
/// continue to find them at
/// `chronos_store::counterexample_storage::*` (path unchanged by this
/// split).
#[path = "ce_types.rs"]
pub(crate) mod ce_types;
pub use ce_types::{
    CounterexampleBundleFilter, CounterexampleBundleRecord,
    CounterexampleBundleSummary, ExistencePredicateWire,
    HypothesisInputWire, MinimisedPayload,
};
```

### Task 1.3 — delete the moved section

Delete lines 327-513 from `counterexample_storage.rs`:

- Line 327: `fn default_schema_version() -> u32 { ... }` (move to ce_types.rs).
- Lines 330-350: `MinimisedPayload` enum + doc comments.
- Lines 352-362: `ExistencePredicateWire` enum + doc comments.
- Lines 364-398: `HypothesisInputWire` struct + doc comments.
- Lines 400-422: `CounterexampleBundleFilter` struct + doc comments.
- Lines 424-448: `CounterexampleBundleSummary` struct + doc comments.
- Lines 450-476: `CounterexampleBundleRecord` struct + doc comments.

**Keep** lines 478-513 (`bundle_events_or_legacy` and
`bundle_events_count_or_legacy`) — these are standalone free functions
that reference the types via the parent path, kept in parent.

### Task 1.4 — verify

- `cargo build -p chronos-store` (compile check).
- `cargo test -p chronos-store --lib --no-fail-fast` (77 tests).
- `cargo test -p chronos-services --lib --no-fail-fast` (264 tests).
- `cargo test -p chronos-cli --no-fail-fast` (35 tests; uses the
  types via `replay_integration`).
- `cargo build -p chronos-mcp` (sanity check for downstream mcp).
- `cargo clippy --workspace --all-targets -- -D warnings` (clean).
- `cargo fmt --all -- --check` (clean).

### Task 1.5 — commit + tag + merge

- Commit on `feat/m9-86-cc001-god-module-types-split`.
- `--no-ff` merge into main.
- Tag `v0.7.88` (pre-created at the merge commit, per the
  fixpoint-cascade workaround).
- Push.
- Delete cycle branch.

### Task 1.6 — cycle-artifacts + handoff

Same as m9-85 (T6 + T7).

## Estimated work

- Task 1.1: ~5 min (1 file creation, mechanical).
- Task 1.2: ~3 min.
- Task 1.3: ~2 min.
- Task 1.4: ~5 min (build + 3 test suites + clippy + fmt).
- Task 1.5: ~5 min.
- Task 1.6: ~10 min.

Total: ~30 min wall time.

## Risk mitigation

- Round-trip: 77 + 264 + 35 test count match.
- Public surface preserved via `pub use` re-exports at the parent path.
- Serde default function resolution: the `#[serde(default = "...")]`
  attribute resolves the function in the struct's module scope.
  Since `default_schema_version` lives in the same `ce_types.rs` as the
  structs, it resolves directly.
- Lint + format clean via clippy + fmt.
