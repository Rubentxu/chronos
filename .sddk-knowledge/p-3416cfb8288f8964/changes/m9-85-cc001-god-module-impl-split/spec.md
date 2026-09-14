# Spec — m9-85-cc001-god-module-impl-split

## REQ-M9-85-01 — three submodules created

The 11 methods of `impl SessionStore { ... }` in
`crates/chronos-store/src/counterexample_storage.rs` (lines 448-909)
MUST be moved into 3 sibling submodules:

- `crates/chronos-store/src/ce_write.rs` — 2 write-path methods.
- `crates/chronos-store/src/ce_read.rs` — 5 read-path methods.
- `crates/chronos-store/src/ce_test_hooks.rs` — 3 test-chokepoint methods.

**Scenario**: `three_submodules_compile`
- Given the 3 new files exist
- When `cargo build -p chronos-store` runs
- Then it exits 0 with no warnings

## REQ-M9-85-02 — public surface preserved

The existing public surface at
`chronos_store::counterexample_storage::*` MUST remain identical:

- `save_counterexample_bundle` — still reachable.
- `save_bundle_record_and_events` — still callable from inside the
  parent module (it's `fn`, not `pub fn`, so only the parent and
  `ce_write` itself need to see it).
- `load_counterexample_bundle_events` — still reachable.
- `count_counterexample_bundle_events` — still reachable.
- `get_bundle_events_count` — still callable from inside `ce_read`.
- `load_counterexample_bundle` — still reachable.
- `list_counterexample_bundles` — still reachable.
- `insert_v2_chunk_for_test` — still reachable (test chokepoint).
- `count_v3_chunks_for_test` — still reachable (test chokepoint).
- `insert_bundle_record_for_test` — still reachable (test chokepoint).

**Scenario**: `public_api_unchanged`
- Given the refactor is applied
- When `cargo test -p chronos-cli --no-fail-fast` runs (uses all
  3 test chokepoints)
- Then it exits 0 with no compile errors or test failures.

## REQ-M9-85-03 — behaviour preservation

All 77 `chronos-store` lib unit tests, all 264 `chronos-services`
lib unit tests, and all 35 `chronos-cli` tests (3 binaries including
the replay_integration integration test) MUST pass before and after
the refactor (round-trip verification via `git stash`).

**Scenario**: `all_test_baselines_match`
- Given baseline counts: chronos-store 77/77, chronos-services
  264/264, chronos-cli 35/35
- When the refactor is applied
- Then post-refactor counts are identical
- And no test names change

## REQ-M9-85-04 — clippy clean

`cargo clippy --workspace --all-targets -- -D warnings` MUST exit 0
post-refactor.

**Scenario**: `clippy_clean`
- Given the refactor is applied
- When `cargo clippy --workspace --all-targets -- -D warnings` runs
- Then it exits 0 with no warnings or errors.

## REQ-M9-85-05 — downstream compilation

`cargo build --workspace` MUST exit 0 post-refactor. All downstream
crates that call methods on `SessionStore` (`chronos-services`,
`chronos-cli`) must compile unchanged.

**Scenario**: `workspace_compiles`
- Given the refactor is applied
- When `cargo build --workspace` runs
- Then it exits 0 with no warnings.

## REQ-M9-85-06 — net file size reduction

`counterexample_storage.rs` line count MUST decrease by approximately
460 lines (the moved `impl SessionStore` block plus the section
comments).

**Scenario**: `file_size_reduction`
- Given pre-refactor line count L (2720 post-m9-84)
- When the refactor is applied
- Then post-refactor line count is approximately L - 460 (±20 for
  comment adjustments + module declarations).

## Non-requirements

- Type definitions are NOT moved.
- Tests are NOT moved.
- The standalone pub fns `bundle_events_or_legacy` and
  `bundle_events_count_or_legacy` are NOT moved.
- No new public API is added.
- No new tests are added.
