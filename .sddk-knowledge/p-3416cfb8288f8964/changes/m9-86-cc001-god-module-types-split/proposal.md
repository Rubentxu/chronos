# Proposal — m9-86-cc001-god-module-types-split

## Intent

Close the type definitions concern of `cc-001-god-module` by extracting
the 6 `pub` types + the `default_schema_version` helper from
`crates/chronos-store/src/counterexample_storage.rs` (lines 327-513,
~187 lines) into a new sibling submodule `ce_types.rs`.

## Approach

1. Create `crates/chronos-store/src/ce_types.rs` containing:
   - `pub enum MinimisedPayload`.
   - `pub enum ExistencePredicateWire`.
   - `pub struct HypothesisInputWire`.
   - `pub struct CounterexampleBundleFilter<'a>`.
   - `pub struct CounterexampleBundleSummary`.
   - `pub struct CounterexampleBundleRecord`.
   - `pub(crate) fn default_schema_version() -> u32`.

2. In `counterexample_storage.rs`:
   - Declare the new submodule via `#[path = "ce_types.rs"] pub(crate) mod ce_types;`.
   - Re-export the 6 types via
     `pub use ce_types::{CounterexampleBundleFilter, CounterexampleBundleRecord, CounterexampleBundleSummary, ExistencePredicateWire, HypothesisInputWire, MinimisedPayload};`.
   - Delete the moved type definitions and `default_schema_version`.

3. Update `#[serde(default = "default_schema_version")]` annotations
   to reference `crate::counterexample_storage::default_schema_version`
   if needed (depending on how serde resolves paths from within the
   submodule — see Risk Analysis in exploration-report.md).

## Path

A-min (cross-crate, bounded scope, single-crate change but with
several downstream consumers that must continue to compile).

## Tier required

T0 + T2 — lib tests + downstream crates (chronos-services,
chronos-cli, chronos-mcp use these types).

## Tier plan

- T0: `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings`.
- T1: `cargo test -p chronos-store --lib --no-fail-fast` (77 tests).
- T2: `cargo test -p chronos-services --lib --no-fail-fast` (264 tests).
- T3: `cargo test -p chronos-cli --no-fail-fast` (35 tests, including the replay_integration round-trip).

No sandbox needed (no probe/mcp touched).

## Carry-forward

Closes the type definitions concern of `cc-001-god-module`. Does NOT
close the debt finding itself (the file remains 2,287 - 187 + ~10 =
~2,110 lines after this cycle). The remaining concerns (free functions
`collect_*`, `KNOWN_BUNDLE_SCHEMA_VERSIONS`; tests ~1,772 lines) will
be split across m9-87+.

## Out of scope

- Tests (~1,772 lines) — kept in same file as the impl they cover.
- The standalone pub fns `bundle_events_or_legacy` and
  `bundle_events_count_or_legacy` (lines 478-513) — kept in parent since
  they reference types via the parent path.
- The free functions `collect_v3_keys_for_bundle`,
  `collect_bundle_chunks_legacy`, `collect_bundle_chunks`,
  `KNOWN_BUNDLE_SCHEMA_VERSIONS` (lines 217-326) — out of scope; could
  be split in m9-87+ if needed.
