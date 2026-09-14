# Spec — m9-86-cc001-god-module-types-split

## REQ-M9-86-01 — ce_types submodule created

The 6 `pub` types + `default_schema_version` helper in
`crates/chronos-store/src/counterexample_storage.rs` (lines 327-513
post-m9-85) MUST be moved into a new sibling submodule
`crates/chronos-store/src/ce_types.rs`.

**Scenario**: `ce_types_module_compiles`
- Given `ce_types.rs` exists
- When `cargo build -p chronos-store` runs
- Then it exits 0 with no warnings

## REQ-M9-86-02 — public surface preserved

The existing public surface at
`chronos_store::counterexample_storage::*` MUST remain identical for the
6 types:

- `MinimisedPayload` — still reachable.
- `ExistencePredicateWire` — still reachable.
- `HypothesisInputWire` — still reachable.
- `CounterexampleBundleFilter` — still reachable.
- `CounterexampleBundleSummary` — still reachable.
- `CounterexampleBundleRecord` — still reachable.

**Scenario**: `public_api_unchanged`
- Given the refactor is applied
- When `cargo build -p chronos-cli` and `cargo build -p chronos-services`
  run (consumers use `chronos_store::counterexample_storage::TypeName`)
- Then both exit 0 with no errors.

## REQ-M9-86-03 — behaviour preservation

All 77 `chronos-store` lib unit tests, all 264 `chronos-services` lib
unit tests, and all 35 `chronos-cli` tests MUST pass before and after
the refactor.

**Scenario**: `all_test_baselines_match`
- Given baseline counts: chronos-store 77/77, chronos-services 264/264,
  chronos-cli 35/35
- When the refactor is applied
- Then post-refactor counts are identical
- And no test names change

## REQ-M9-86-04 — clippy clean

`cargo clippy --workspace --all-targets -- -D warnings` MUST exit 0
post-refactor.

**Scenario**: `clippy_clean`
- Given the refactor is applied
- When `cargo clippy --workspace --all-targets -- -D warnings` runs
- Then it exits 0 with no warnings or errors.

## REQ-M9-86-05 — serde default function path resolution

The `#[serde(default = "default_schema_version")]` annotations on
`CounterexampleBundleSummary.schema_version` and
`CounterexampleBundleRecord.schema_version` MUST continue to resolve
correctly after the types move to `ce_types.rs`.

**Scenario**: `serde_defaults_resolve`
- Given the refactor is applied
- When a legacy bundle (no `schema_version` field) is deserialized
- Then the field defaults to `CURRENT_BUNDLE_SCHEMA_VERSION` (3).

This is exercised by `m9_02_legacy_bundle_deserializes_with_default_schema_version`
in `counterexample_storage.rs::tests`.

## REQ-M9-86-06 — net file size reduction

`counterexample_storage.rs` line count MUST decrease by approximately
185 lines (the 6 type definitions + `default_schema_version` +
section comments).

**Scenario**: `file_size_reduction`
- Given pre-refactor line count L (2287 post-m9-85)
- When the refactor is applied
- Then post-refactor line count is approximately L - 175 (±15 for
  comment adjustments + module declarations).

## Non-requirements

- Free functions `collect_v3_keys_for_bundle`, `collect_bundle_chunks_legacy`,
  `collect_bundle_chunks`, `KNOWN_BUNDLE_SCHEMA_VERSIONS` are NOT moved.
- The standalone pub fns `bundle_events_or_legacy` and
  `bundle_events_count_or_legacy` are NOT moved.
- Tests are NOT moved.
- No new public API is added.
- No new tests are added.
