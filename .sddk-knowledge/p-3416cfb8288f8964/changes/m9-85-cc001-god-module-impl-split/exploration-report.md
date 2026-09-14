# Exploration Report — m9-85-cc001-god-module-impl-split

## Context

`cc-001-god-module` is the P2 debt finding from m9-04:
`counterexample_storage.rs` is at 2,720 lines post-m9-84 (was 2,821),
spans 5 concerns. m9-84 extracted the chunk key/value encoding helpers
into `ce_chunk_keys.rs`. This cycle extracts the `impl SessionStore`
methods into focused submodules.

## Scope decision: incremental, second slice

The `impl SessionStore` block (lines 448-909, ~462 lines, 11 methods
on `SessionStore`) covers 3 logical concerns:

1. **Write path** (2 methods: `save_counterexample_bundle`,
   `save_bundle_record_and_events`) — ~115 lines.
2. **Read path** (5 methods: `load_counterexample_bundle_events`,
   `count_counterexample_bundle_events`, `get_bundle_events_count`,
   `load_counterexample_bundle`, `list_counterexample_bundles`) —
   ~250 lines.
3. **Test chokepoints** (3 methods: `insert_v2_chunk_for_test`,
   `count_v3_chunks_for_test`, `insert_bundle_record_for_test`) —
   ~100 lines (m9-05 R4).

## Findings from file inspection

Methods by concern:

| Method | Concern | Visibility | Lines |
|---|---|---|---|
| `save_counterexample_bundle` | Write | `pub` | 459-486 |
| `save_bundle_record_and_events` | Write | `fn` (private) | 499-572 |
| `load_counterexample_bundle_events` | Read | `pub` | 611-637 |
| `count_counterexample_bundle_events` | Read | `pub` | 646-662 |
| `get_bundle_events_count` | Read | `fn` (private) | 588-608 |
| `load_counterexample_bundle` | Read | `pub` | 760-821 |
| `list_counterexample_bundles` | Read | `pub` | 838-908 |
| `insert_v2_chunk_for_test` | Test chokepoint | `pub` `#[doc(hidden)]` | 681-704 |
| `count_v3_chunks_for_test` | Test chokepoint | `pub` `#[doc(hidden)]` | 712-719 |
| `insert_bundle_record_for_test` | Test chokepoint | `pub` `#[doc(hidden)]` | 729-749 |

## Strategy

Create 3 sibling submodules, each holding one `impl SessionStore` block:

1. `crates/chronos-store/src/ce_write.rs` — write-path methods
   (save + atomic wrapper).
2. `crates/chronos-store/src/ce_read.rs` — read-path methods
   (load, count, list).
3. `crates/chronos-store/src/ce_test_hooks.rs` — test chokepoint
   methods (m9-05 R4).

All three use `impl crate::storage::SessionStore { ... }` (Rust
allows multiple `impl` blocks for the same type across modules —
this is exactly what trait extensions and `impl` blocks in different
files are designed for).

Visibility: each method keeps its original visibility:
- `pub fn` stays `pub` (reachable as `<SessionStore>::method_name` or
  `instance.method_name()` from any module that imports `SessionStore`).
- `fn` (private) stays private within its submodule.
- `#[doc(hidden)] pub fn` test chokepoints stay public at the type
  level so `chronos-cli/tests/replay_integration.rs` can call them.

**Implementation note (correction):** `pub use ce_write::save_*` from
the parent module **does not work** for methods defined inside
`impl SessionStore` blocks. Methods are inherent methods of the type,
not module-level items, so `pub use ce_write::save_counterexample_bundle`
fails with E0432 ("no `save_counterexample_bundle` in
`counterexample_storage::ce_write`"). The correct wiring is:

```rust
#[path = "ce_write.rs"]
pub mod ce_write;
#[path = "ce_read.rs"]
pub mod ce_read;
#[path = "ce_test_hooks.rs"]
pub mod ce_test_hooks;
```

**No `pub use` re-exports needed.** All real callers in the workspace
use the `instance.method_name(...)` form (verified — see grep result
in `tasks.md` Task 1.4). The type-level path
`<SessionStore>::save_counterexample_bundle` and
`SessionStore::save_counterexample_bundle` work from any module
that has `use crate::storage::SessionStore;` — independent of which
file the `impl` block lives in.

## Submodule wiring (final, after build fix)

```rust
// In counterexample_storage.rs:
#[path = "ce_write.rs"]
pub mod ce_write;
#[path = "ce_read.rs"]
pub mod ce_read;
#[path = "ce_test_hooks.rs"]
pub mod ce_test_hooks;
```

The submodule declarations use `pub mod` (not `pub(crate) mod`)
so that submodules are reachable from external crates if they ever
want to import directly via
`chronos_store::counterexample_storage::ce_write::*` (future-proofing).

**No `pub use` re-exports** — methods inside `impl SessionStore` are
not module-level items. The actual current callers all use
`instance.method_name(...)` form (verified via grep — see Task 1.4),
so the public path is `<SessionStore>::method_name` regardless of
which file the `impl` block lives in.

Initial attempt with `pub use ce_write::save_counterexample_bundle`
FAILED with E0432 ("no `save_counterexample_bundle` in
`counterexample_storage::ce_write`"). This is correct Rust behaviour:
methods are not items at the module level, only at the type level.
The fix is to drop the `pub use` lines and rely on the inherent
method call syntax.

## Behaviour preservation

No semantic changes. Methods move verbatim. Tests stay in the same
file (no test relocation).

## Risk

Low. The split is purely lexical. Methods have `&self` access to
`SessionStore`, all using the existing `db()` accessor. The
submodule's `impl SessionStore` block sees the same `Self` (i.e.,
`crate::storage::SessionStore`) type via `crate::storage::SessionStore`
import or via `use super::*;`.

Wait — `Self::get_bundle_events_count` is called from
`load_counterexample_bundle_events` and `count_counterexample_bundle_events`.
If `get_bundle_events_count` lives in `ce_read.rs` and the callers
live in `ce_read.rs`, then `Self::get_bundle_events_count` works
inside the same impl block. If we split read into two submodules
(e.g., events-read vs record-load vs list), this gets more complex.
The chosen 3-way split (write / read / test-hooks) keeps related
methods in the same impl block.

## Out of scope

- Type definitions section (~140 lines, lines 406-547) — could be
  split but reference coupling (encoding helpers, storage impl)
  makes it lower priority.
- Tests (~1,770 lines, lines 1050-end) — kept in same file as the
  impl they cover.
- The standalone pub fns `bundle_events_or_legacy` and
  `bundle_events_count_or_legacy` (lines 921-1049) — kept in the
  parent since they are not methods on `SessionStore`.
