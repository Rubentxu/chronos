# Spec — m9-87-cc001-god-module-schema-split

## REQ-M9-87-01 — ce_schema submodule created

The schema/table/collect-helpers section in
`crates/chronos-store/src/counterexample_storage.rs` (lines 127-344
post-m9-86, ~217 lines) MUST be moved into a new sibling submodule
`crates/chronos-store/src/ce_schema.rs`.

**Scenario**: `ce_schema_module_compiles`
- Given `ce_schema.rs` exists
- When `cargo build -p chronos-store` runs
- Then it exits 0 with no warnings

## REQ-M9-87-02 — public surface preserved

The existing public surface at
`chronos_store::counterexample_storage::*` MUST remain identical:

- `BUNDLE_EVENTS_CHUNK_SIZE` — still reachable as `pub const`.
- `CURRENT_BUNDLE_SCHEMA_VERSION` — still reachable as `pub const`
  (used by chronos-services via `cs::CURRENT_BUNDLE_SCHEMA_VERSION`).
- `collect_bundle_chunks_range` — still reachable as `pub fn`.

**Scenario**: `public_api_unchanged`
- Given the refactor is applied
- When `cargo build -p chronos-services` runs (uses
  `counterexample_storage::CURRENT_BUNDLE_SCHEMA_VERSION`)
- Then it exits 0 with no errors.

## REQ-M9-87-03 — compile-time invariant preserved

The `const _:` compile-time invariant block at lines 329-344 (binding
`CURRENT_BUNDLE_SCHEMA_VERSION` to `KNOWN_BUNDLE_SCHEMA_VERSIONS`)
MUST continue to enforce the relationship after the move.

**Scenario**: `compile_time_invariant_holds`
- Given the refactor is applied
- When `cargo build -p chronos-store` runs
- Then it exits 0 (the invariant is satisfied; if not, the compile
  would fail with "CURRENT_BUNDLE_SCHEMA_VERSION must be listed in
  KNOWN_BUNDLE_SCHEMA_VERSIONS").

## REQ-M9-87-04 — behaviour preservation

All 77 `chronos-store` lib unit tests, all 264 `chronos-services` lib
unit tests, all 35 `chronos-cli` tests, and all 103 `chronos-native`
serial tests MUST pass before and after the refactor.

**Scenario**: `all_test_baselines_match`
- Given baseline counts: chronos-store 77/77, chronos-services 264/264,
  chronos-cli 35/35, chronos-native 103/103
- When the refactor is applied
- Then post-refactor counts are identical
- And no test names change

## REQ-M9-87-05 — clippy clean

`cargo clippy --workspace --all-targets -- -D warnings` MUST exit 0
post-refactor.

**Scenario**: `clippy_clean`
- Given the refactor is applied
- When `cargo clippy --workspace --all-targets -- -D warnings` runs
- Then it exits 0 with no warnings or errors.

## REQ-M9-87-06 — net file size reduction

`counterexample_storage.rs` line count MUST decrease by approximately
215 lines (the moved section + section comments).

**Scenario**: `file_size_reduction`
- Given pre-refactor line count L (2156 post-m9-86)
- When the refactor is applied
- Then post-refactor line count is approximately L - 205 (±15 for
  comment adjustments + module declarations).

## Non-requirements

- Tests are NOT moved.
- The standalone pub fns `bundle_events_or_legacy` and
  `bundle_events_count_or_legacy` are NOT moved.
- No new public API is added.
- No new tests are added.
