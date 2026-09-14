# Implementation Receipt — m9-94-cc001-test-split

## Identification

| Field | Value |
|---|---|
| Cycle | m9-94-cc001-test-split |
| Path | B-direct (mechanical test code extraction, no behavior change) |
| Branch | chore/m9-94-cc001-test-split |
| Date | 2026-09-14 |
| Base SHA | 645eedb530f1b827a600544d01e501d82b410820 |
| Head SHA | 3a8494967c366761f10da6caf89745971fce9f78 |

## Work performed

Single source commit (Rust only):

| Bucket | Files | Lines | Notes |
|---|---|---|---|
| crates/chronos-store/src/ce_storage_tests.rs | 1 added | +1785 | NEW sibling file; verbatim copy of inline test block (1771 lines) + 14-line doc comment header |
| crates/chronos-store/src/counterexample_storage.rs | 1 modified | -1772 / +3 | Replaced 1773-line inline `mod tests { ... }` block (lines 188-1960) with 3-line `#[cfg(test)] #[path = "ce_storage_tests.rs"] mod tests;` declaration |

**Total**: 1 file added + 1 file modified; net +16 LoC (1783 insertions, 1772 deletions, ~14 lines of doc comments added to the sibling file).

## Tests added

None. All 43 existing test functions + 4 test helpers moved verbatim to the sibling file. No test logic changed.

## Test inventory

43 test functions + 4 helpers moved:

- **Roundtrip / lifecycle** (4): `save_then_load_roundtrip`, `load_unknown_returns_none`, `list_filters_by_property_kind`, `list_with_limit_truncates`, `list_cursor_paginates_forward`, `list_cursor_unknown_id_returns_all`, `save_rejects_empty_bundle_id`
- **m8-07 target hypothesis** (2): `m8_07_save_then_load_preserves_target_hypothesis`, `m8_07_pre_m8_07_bundle_has_no_target_hypothesis`
- **m9-02 schema versioning** (5): `m9_02_save_writes_current_schema_version`, `m9_02_legacy_bundle_deserializes_with_default_schema_version`, `m9_02_future_version_load_is_rejected`, `m9_07_loader_rejects_envelope_summary_version_mismatch`, `m9_02_save_overwrites_callers_schema_version`, `m9_02_list_includes_future_versioned_row_best_effort`
- **m9-02 side-table events** (7): `m9_02_save_writes_events_to_side_table_not_blob`, `m9_02_load_events_concatenates_chunks_in_order`, `m9_02_save_events_idempotent_overwrites_prior_chunks`, `m9_02_count_events_handles_partial_last_chunk`, `m9_02_legacy_bundle_with_events_in_blob_loads_normally`, `m9_02_post_m9_02_bundle_loads_events_from_side_table`, `m9_02_summary_events_count_default_zero_for_legacy`, `m9_02_load_unknown_bundle_returns_empty_events_vec`
- **m9-04 v3 key layout** (10): `m9_04_v3_key_layout_fixed_width`, `m9_04_bundle_prefix_is_16_bytes`, `m9_04_decode_chunk_key_roundtrip`, `m9_04_decode_chunk_key_rejects_wrong_length`, `m9_04_encode_decode_value_carries_bundle_id`, `m9_04_v3_save_load_roundtrip`, `m9_04_v2_bundle_loads_via_fallback`, `m9_04_resave_migrates_v2_to_v3`, `m9_04_range_scan_covers_all_chunk_indices`, `m9_04_collision_containment_via_bundle_id`, `m9_04_fresh_store_ghost_bundle_returns_empty`, `m9_04_saved_v3_schema_and_future_rejection`, `m9_04_v2_bundle_with_zero_events_count_loads_normally`, `m9_04_forced_prefix_collision_is_contained_by_identity_check`, `m9_04_r4_v2_bundle_never_resaved_uses_legacy_path`, `m9_04_known_bundle_schema_versions_pinned`, `m9_04_known_bundle_schema_versions_count_and_pinning`, `m9_06_current_schema_version_is_listed_as_known`

(Counting is approximate; the test inventory is documented in the sibling file directly via `grep -nE "^    fn [a-z]" crates/chronos-store/src/ce_storage_tests.rs`.)

4 test helpers:
- `make_store()` → `crate::storage::SessionStore`
- `make_event(id: u64, func: &str)` → `chronos_domain::TraceEvent`
- `__force_collision_prefix(...)` (test-only hook from ce_test_hooks.rs)
- (plus inline `assert_send_*` style helpers within tests)

## Drift line delta

None — m9-94 is purely a mechanical file reorganization. The Rust module graph is identical pre- and post-extraction (same items, same visibility, same resolution paths). No vault CC drift introduced or closed by the source commit itself; the change-entry + 4 supporting knowledge artifacts + 7 cycle artifacts are vault work that lands separately.

## Out-of-scope

- **FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK** (carried from m9-88): external `sddk` CLI bug; cannot be fixed in chronos scope.
- **FIND-M9-94-SERVICES-COUNTEREXAMPLE-TESTS-MONOLITHIC** (introduced): `crates/chronos-services/src/counterexample.rs` still has 2169-line inline test block; same `#[path]` pattern can be applied in a future m-cycle.

## Verification

- **T0** (lint gate): `cargo fmt --all -- --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean.
- **T1** (chronos-store lib unit): `cargo test -p chronos-store --lib --no-fail-fast` = 77 passed.
- **T2** (workspace lib): `cargo test --workspace --lib --no-fail-fast -- --test-threads=1` = 1042 passed (matches m9-93 baseline of 1042 on `main`).

## Carry-forward findings

**Introduced**:
- `FIND-M9-94-SERVICES-COUNTEREXAMPLE-TESTS-MONOLITHIC` — P2, MEDIUM, followup.

**Carried (external-deferred)**:
- `FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK` (from m9-88, external sddk CLI bug).

## Status

Implementation complete. m9-94 cycle ready for release + archive.
