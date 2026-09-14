# Spec: m9-81 chronos-store::counterexample_storage uses the canonical table_error helper

> Spec is the contract.

## Background

m9-72 introduced `crates/chronos-store/src/table_error.rs` as the
canonical helper for the read-path "table absent" / "propagate fault"
classification:

- `classify_read_table_error(redb::TableError) -> TableOpenFailure`
- `TableOpenFailure::or_not_found::<T>(not_found: T) -> Result<T, StoreError>`

m9-72 applied it to `crates/chronos-store/src/cas.rs` (2 sites) and
`crates/chronos-store/src/storage.rs` (2 sites). It missed
`crates/chronos-store/src/counterexample_storage.rs`, which still
hand-rolls the same `match` ladder at 6 read-path sites.

## REQ-M9-81-01 — counterexample_storage uses the canonical helper

`crates/chronos-store/src/counterexample_storage.rs` SHALL use
`crate::table_error::classify_read_table_error` at each of the 6
production read-path sites, replacing the inline
`match tx.open_table(…) { Ok | Err(TableDoesNotExist) -> Ok(EMPTY) | Err(e) -> Err(Database(e.into())) }`
ladder.

**Scenarios:**

- `ce_storage_uses_canonical_helper_in_collect_bundle_chunks_range` —
  `grep -n 'classify_read_table_error' crates/chronos-store/src/counterexample_storage.rs`
  returns >=1 match at or near line 237.
- `ce_storage_uses_canonical_helper_in_collect_v3_keys_for_bundle` —
  same grep at or near line 286.
- `ce_storage_uses_canonical_helper_in_collect_bundle_chunks_legacy` —
  same grep at or near line 326.
- `ce_storage_uses_canonical_helper_in_get_bundle_events_count` —
  same grep at or near line 697.
- `ce_storage_uses_canonical_helper_in_load_counterexample_bundle` —
  same grep at or near line 875.
- `ce_storage_uses_canonical_helper_in_list_counterexample_bundles` —
  same grep at or near line 950.
- `ce_storage_drops_inline_table_does_not_exist` — `grep -n
  'redb::TableError::TableDoesNotExist\|TableError::TableDoesNotExist'
  crates/chronos-store/src/counterexample_storage.rs` returns 0 matches
  in production code (cfg(test) is allowed to keep panics via
  `.unwrap()`).

## REQ-M9-81-02 — Behavioural preservation

The refactor SHALL NOT change the public API, the wire shape, or the
schema. The `Result<…, StoreError>` return type at each refactored site
SHALL be preserved (the helper produces the same shape).

**Scenarios:**

- `ce_storage_test_load_counterexample_bundle_passes` —
  `cargo test -p chronos-store --lib counterexample_storage::tests::load_counterexample_bundle`
  passes.
- `ce_storage_test_list_counterexample_bundles_passes` —
  `cargo test -p chronos-store --lib counterexample_storage::tests::list_counterexample_bundles`
  passes.
- `ce_storage_test_full_lib_suite_passes` —
  `cargo test -p chronos-store --lib --no-fail-fast` total passed
  matches the pre-refactor count (currently expected: ≥80 passed, 0
  failed).

## REQ-M9-81-03 — Lint and clippy clean

- `cargo fmt --all -- --check` passes.
- `cargo clippy --workspace --all-targets -- -D warnings` passes.

## Design notes

The refactor pattern per site (4 lines → 2 lines):

```rust
// Before:
let table = match tx.open_table(FOO) {
    Ok(t) => t,
    Err(redb::TableError::TableDoesNotExist(_)) => return Ok(EMPTY),
    Err(e) => return Err(StoreError::Database(e.into())),
};

// After:
let table = match tx.open_table(FOO) {
    Ok(t) => t,
    Err(e) => return classify_read_table_error(e).or_not_found(EMPTY),
};
```

Where `EMPTY` is one of:

| Site | Function | EMPTY |
|---|---|---|
| 237 | `collect_bundle_chunks_range` | `Vec::new()` |
| 286 | `collect_v3_keys_for_bundle` | `Vec::new()` (of `Vec<u8>`) |
| 326 | `collect_bundle_chunks_legacy` | `Vec::new()` |
| 697 | `get_bundle_events_count` | `0_u64` |
| 875 | `load_counterexample_bundle` | `None::<CounterexampleBundleRecord>` |
| 950 | `list_counterexample_bundles` | `Vec::new()` |

`or_not_found` already handles all of these — it's a generic
`<T>(not_found: T) -> Result<T, StoreError>`.
