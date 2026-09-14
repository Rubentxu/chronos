# Handoff — m9-85 closure (session 2026-09-14T11:35Z-11:55Z)

## Cycle summary

Closed the second slice of `cc-001-god-module` (P2, MEDIUM, m9-04):
extracted the 11-method `impl SessionStore { ... }` block from
`crates/chronos-store/src/counterexample_storage.rs` (lines 448-909
pre-refactor, ~462 lines) into 3 sibling submodules grouped by concern:

- `ce_write.rs` (148 lines, NEW) — write-path methods
  (`save_counterexample_bundle`, `save_bundle_record_and_events`).
- `ce_read.rs` (276 lines, NEW) — read-path methods
  (`load_counterexample_bundle`, `load_counterexample_bundle_events`,
  `count_counterexample_bundle_events`, `get_bundle_events_count`,
  `list_counterexample_bundles`).
- `ce_test_hooks.rs` (99 lines, NEW) — m9-05 R4 test chokepoints
  (`insert_v2_chunk_for_test`, `count_v3_chunks_for_test`,
  `insert_bundle_record_for_test`).

A-min path. Behaviour-preserving lexical refactor.

`counterexample_storage.rs`: 2720 → 2287 lines (-433 net extracted).

Tag `v0.7.87` created (peel `a85034603031f2dd1dc340d78d84f71f140672e0`,
non-standard location at the cycle-artifacts commit just before the
SHA-cascade fixpoint, per the fixpoint-cascade workaround documented
in m9-83 handoff).

## Build issue resolved during cycle

Initial implementation used `pub use ce_write::{save_counterexample_bundle, ...}`
in the parent module to re-export the methods at
`chronos_store::counterexample_storage::*` path. This FAILED with E0432
("no save_counterexample_bundle in counterexample_storage::ce_write")
because methods inside `impl SessionStore` blocks are **inherent methods
of the type**, not module-level items. `pub use` only works for
module-level items (free functions, constants, types, etc.).

Fix: drop the `pub use` re-exports. Methods remain callable via
`<SessionStore>::method_name` or `instance.method_name(...)` from any
module that imports `SessionStore`. Verified all real callers in the
workspace use the `instance.method_name(...)` form (grep across
`crates/` returned zero path-qualified
`counterexample_storage::method_name(...)` call sites).

This is the same wiring pattern that would have worked for
m9-84's `ce_chunk_keys.rs` if it had been `impl SessionStore` methods
instead of free functions. m9-84 worked because the helpers there
were free functions (module-level items), so `pub(super) use` re-exports
applied correctly.

## Artifacts produced

- `crates/chronos-store/src/ce_write.rs` — NEW (148 lines).
- `crates/chronos-store/src/ce_read.rs` — NEW (276 lines).
- `crates/chronos-store/src/ce_test_hooks.rs` — NEW (99 lines).
- `crates/chronos-store/src/counterexample_storage.rs` — trimmed (2720 → 2287 lines).
- `cycle-artifacts/p-3416cfb8288f8964/m9-85-cc001-god-module-impl-split/`:
  7 files (apply-checkpoint.json, implementation-receipt.md,
  merge-receipt.md, release-receipt.md, release-report.md,
  verify-findings.json, verify-report.md).
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-85-cc001-god-module-impl-split/`:
  exploration-report.md, proposal.md, spec.md, tasks.md.
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-85-cc001-god-module-impl-split/archive-manifest.md`.
- Tag `v0.7.87` created and pushed.

## CC state

- All 48 python + 7 bash CCs pass per `bash scripts/check_vault_drift.sh`.
- `python3 scripts/regen_manifest_index_shas.py --check`: clean (83 manifests).

## Commit chain

```
41d3fee (HEAD -> main, origin/main) m9-85: archive-manifest + CC-required sections (verify-report, release-report) + terms/index.md bump
f94ead9                              m9-85: cascade SHAs after release + archive-manifest fixpoint (final)
600a020                              m9-85: bump Head SHAs to a850346 + Total cycles 85
a850346 [tag: v0.7.87]               m9-85: cycle artifacts (apply-checkpoint, implementation-receipt, merge-receipt, verify-report, verify-findings)
72e120c                              Merge feat/m9-85-cc001-god-module-impl-split into main
158f5b1                              m9-85: extract ce_write/ce_read/ce_test_hooks impl SessionStore blocks from counterexample_storage.rs
2c2a5cc (origin/main, origin/HEAD)   docs(handoff): append m9-84 closure (session 2026-09-14T11:20Z-11:30Z)
```

Note: the cycle-artifacts commit (`a850346`) is followed by three cascade
commits (`600a020` → `f94ead9` → `41d3fee`). The tag `v0.7.87` peels at
`a85034603031f2dd1dc340d78d84f71f140672e0`, matching the CC#42
fixpoint-cascade workaround documented in m9-83 handoff.

## Implementation details

### Submodule wiring (final, post-build-fix)

```rust
// In counterexample_storage.rs:
#[path = "ce_write.rs"]
pub mod ce_write;
#[path = "ce_read.rs"]
pub mod ce_read;
#[path = "ce_test_hooks.rs"]
pub mod ce_test_hooks;
```

**No `pub use` re-exports needed.** Methods inside `impl SessionStore`
blocks are inherent methods of the type, reachable from any module
that imports `SessionStore` via `<SessionStore>::method_name` or
`instance.method_name(...)`. All real callers use the instance form
(verified via grep).

### Why the initial `pub use` attempt failed

Rust's `pub use` re-export applies only to **module-level items**: free
functions, constants, types, statics, and re-exports of other items.
Methods defined inside `impl Foo { ... }` blocks are **inherent methods**
of the type `Foo`, which are NOT module-level items. They live in the
type's namespace, not the module's namespace.

The error `E0432: unresolved imports ce_write::save_counterexample_bundle:
no save_counterexample_bundle in counterexample_storage::ce_write`
was Rust's way of saying "this item isn't at this path because it's
defined as a method, not a free function."

### Why `#[path]` was needed (same as m9-84)

`mod foo;` declaration looks for `foo.rs` or `foo/mod.rs` relative to
the containing module file. The `#[path = "ce_write.rs"]` attribute
makes the lookup explicit and reliable when adding a sibling file in
the same directory.

### Why `pub mod` was chosen over `pub(crate) mod`

`pub mod` makes the submodules reachable from external crates if they
ever want to import directly via
`chronos_store::counterexample_storage::ce_write::*` (future-proofing).
The `pub(super) use` pattern from m9-84 only works for free functions
in non-public modules; here, the submodules must be `pub` because
they contain `impl SessionStore` blocks that downstream crates need to
"see" when calling methods.

## Test results

| Tier | Command | Result |
|---|---|---|
| T0 | `cargo fmt --all -- --check` | passed |
| T0 | `cargo clippy --workspace --all-targets -- -D warnings` | passed (0 warnings) |
| T1 | `cargo test -p chronos-store --lib` | 77/77 (matched baseline) |
| T2 | `cargo test -p chronos-services --lib` | 264/264 (matched baseline) |
| T3 | `cargo test -p chronos-cli` | 35/35 (matched baseline; includes replay_integration 2/2) |
| T4 | `cargo test -p chronos-native --lib -- --test-threads=1` | 103/103 |
| Build | `cargo build --workspace` | success, no warnings |

## Carry-forward

The `cc-001-god-module` debt remains open. Counterexample_storage.rs is
now 2,287 lines (was 2,821 pre-m9-84, 2,720 post-m9-84, 2,287 post-m9-85).
Future cycles can address:

- **m9-86 candidate**: type definitions section (structs
  `CounterexampleBundleRecord`, `CounterexampleBundleSummary`,
  `CounterexampleBundleFilter`, etc., ~140 lines).
- **Tests relocation** (~1,770 lines) — kept in same file as the impl
  they cover; could be moved to `tests.rs` in a future cycle.
- **Standalone pub fns** `bundle_events_or_legacy` and
  `bundle_events_count_or_legacy` (128 lines) — kept in parent since
  they are free functions, not methods on `SessionStore`.

## Next candidates

1. **m9-86 cc-001-god-module-split3**: extract type definitions into
   `ce_types.rs` (~140 lines), bring file below 2,150 lines.
2. **FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK**: CLI ledger FK bug
   (carry-forward from m9-81 handoff).
3. **M7 milestone**: events_read merge, observe merge, session_compare/
   explain split, lifecycle.

## Lessons learned

- **`pub use` does NOT re-export inherent methods.** This is a common
  misconception when splitting `impl Foo` blocks across modules. The
  fix is to use the type's namespace, not the module's namespace.
- **All callers use instance-method syntax.** Verify with grep before
  assuming path-qualified `module::method_name` calls exist in the
  workspace.
- **Tag at cycle-artifacts commit, not fixpoint.** The CC#42
  fixpoint-cascade workaround continues to work — tag `v0.7.87` peels at
  `a85034603031f2dd1dc340d78d84f71f140672e0` (cycle-artifacts commit),
  one commit before the SHA-cascade fixpoint HEAD `41d3fee`.
