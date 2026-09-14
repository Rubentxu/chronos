# Implementation Receipt — m9-95-services-test-split

## Identification

| Field | Value |
|---|---|
| Cycle | m9-95-services-test-split |
| Path | B-direct (mechanical test code extraction, no behavior change) |
| Branch | chore/m9-95-services-test-split |
| Date | 2026-09-14 |
| Base SHA | 72bff2808457a4ead6f4caec233dc404a20c35d8 |
| Head SHA | eb96861b17711f7d525cedacccc17905bc17e6e0 |

## Work performed

Single source commit (Rust only):

| Bucket | Files | Lines | Notes |
|---|---|---|---|
| crates/chronos-services/src/ce_services_tests.rs | 1 added | +2181 | NEW sibling file; verbatim copy of inline test block (2169 lines) + 12-line doc comment header |
| crates/chronos-services/src/counterexample.rs | 1 modified | -2169 / +11 | Replaced 2170-line inline `mod tests { ... }` block (lines 1786-3955) with 11-line `#[cfg(test)] #[path = "ce_services_tests.rs"] mod tests;` declaration |

**Total**: 1 file added + 1 file modified; net +23 LoC (2182 insertions, 2169 deletions, ~13 lines of doc comments added to the sibling file).

## Tests added

None. All 45 existing test functions + 4 test helpers moved verbatim to the sibling file. No test logic changed.

## Test inventory

45 test functions + 4 helpers moved:

- **Type-system invariants** (10): `counterexample_context_is_send_when_inner_refs_are_send`, `counterexample_get_signature_accepts_bundle_id_string`, `counterexample_list_filter_default_has_zero_limit`, `counterexample_input_default_max_rounds_is_64`, `counterexample_bundle_summary_has_full_bundle_false_is_m8_01_state`, ...
- **Shrink config mapping** (3): `m8_02_shrink_config_default_matches_pinned_max_rounds`, `m8_02_shrink_config_to_proptest_enforces_case_ceiling`, `m8_02_run_error_maps_to_documented_service_error_variants`
- **m8-02 roundtrip / proptest integration** (~10): hypothesis_input roundtrip invariants
- **m8-06 strategy shrinking** (~10): `m8_06_call_path_strategy_shrinks_caller_and_callee`, `m8_06_call_path_strategy_max_depth_shrinks_to_none`, `m8_06_drive_strategy_with_number_shrinker_terminates_at_zero`, `m8_06_drive_strategy_with_text_shrinker_terminates_at_empty`
- **m8-07 hypothesis input roundtrip** (~10): existence, call_path, invariant, property_target, all_predicate_variants
- **m9-01 schema version** (~2): `m9_01_save_persists_schema_version_1`

(The exact list is greppable: `grep -nE "^    fn [a-z]" crates/chronos-services/src/ce_services_tests.rs`.)

4 test helpers:
- `assert_send<T: Send>(_: T) {}` (compile-time Send bound)
- `dummy_target_invariant(c: PropertyValue) -> HypothesisInput` (test fixture)
- (plus inline `#[test]` body-local helpers)

## Drift line delta

None — m9-95 is purely a mechanical file reorganization. The Rust module graph is identical pre- and post-extraction (same items, same visibility, same resolution paths). No vault CC drift introduced or closed by the source commit itself; the change-entry + 4 supporting knowledge artifacts + 7 cycle artifacts are vault work that lands separately.

## Out-of-scope

- **FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK** (carried from m9-88): external `sddk` CLI bug; cannot be fixed in chronos scope.
- **cc-001-god-module production code split for `counterexample.rs`** (1785 lines; well-organized into named sections; not needed at this time).

## Verification

- **T0** (lint gate): `cargo fmt --all -- --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean.
- **T1** (chronos-services lib unit): `cargo test -p chronos-services --lib --no-fail-fast` = 268 passed.
- **T2** (workspace lib): `cargo test --workspace --lib --no-fail-fast -- --test-threads=1` = 1042 passed (matches m9-94 baseline).

## Carry-forward findings

**Closed**: `FIND-M9-94-SERVICES-COUNTEREXAMPLE-TESTS-MONOLITHIC` — opened m9-94, closed m9-95.

**Carried (external-deferred)**: `FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK` (from m9-88, external sddk CLI bug).

## Status

Implementation complete. m9-95 cycle ready for release + archive.
