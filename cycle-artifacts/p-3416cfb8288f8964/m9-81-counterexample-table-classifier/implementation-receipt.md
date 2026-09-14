# Implementation Receipt: m9-81

> **Cycle**: `p-3416cfb8288f8964/m9-81-counterexample-table-classifier`
> **Path**: B-direct
> **Date**: 2026-09-14
> **Head SHA**: `a3f59ea0d1bd446cd20c12f3f62863d70deb0f3a`
> **Base SHA**: `45b53df132186b09de75b543b87cf0bab23bd26e`

## Summary

Closed FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION by routing 6
production read-path sites in `crates/chronos-store/src/counterexample_storage.rs`
through the canonical `table_error::classify_read_table_error().or_not_found(...)`
helper that m9-72 introduced for `cas.rs` and `storage.rs`.

## Commits

| SHA | Title |
|---|---|
| `80cca0d` | m9-81: vault (exploration-report + proposal + spec + tasks) |
| `a3f59ea` | m9-81: route 6 counterexample_storage read paths through table_error |

## Source diff (high-level)

`crates/chronos-store/src/counterexample_storage.rs`:

- Imports: `TableError` removed (no longer named); `classify_read_table_error` added.
- 6 sites converted from `match { Ok | TableDoesNotExist -> Ok(EMPTY) | else -> Err(Database) }`
  to `match { Ok | else -> classify_read_table_error(e).or_not_found(EMPTY) }`.

| Site | Function | "Absent" answer |
|---|---|---|
| 1 | `collect_bundle_chunks_range` | `Vec::new()` |
| 2 | `collect_v3_keys_for_bundle` | `Vec::new()` |
| 3 | `collect_bundle_chunks_legacy` | `Vec::new()` |
| 4 | `get_bundle_events_count` | `0_u64` |
| 5 | `load_counterexample_bundle` | `None::<CounterexampleBundleRecord>` |
| 6 | `list_counterexample_bundles` | `Vec::new()` |

Net delta: 11 insertions / 13 deletions = -2 lines.

## Acceptance against spec

| REQ | Status | Evidence |
|---|---|---|
| REQ-M9-81-01 (counterexample_storage uses canonical helper) | PASS | `grep -n 'classify_read_table_error' crates/chronos-store/src/counterexample_storage.rs` returns 6 matches (lines 238, 286, 325, 695, 872, 946); `grep -n 'redb::TableError::TableDoesNotExist\|TableError::TableDoesNotExist' crates/chronos-store/src/counterexample_storage.rs` returns 0 matches in production code (the 2 hits at lines 859, 933 are explanatory docstrings). |
| REQ-M9-81-02 (behavioural preservation) | PASS | `cargo test -p chronos-store --lib --no-fail-fast`: 74 passed / 0 failed. Baseline (cycle base `45b53df`): 74 passed / 0 failed. Round-trip verified via `git stash`. |
| REQ-M9-81-03 (lint/clippy clean) | PASS | `cargo fmt --all -- --check` exits 0; `cargo clippy --workspace --all-targets -- -D warnings` exits 0. |

## Carry-forward

- **Closed**: `FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION`.
- **New**: none.

## Out-of-scope items (intentionally untouched)

- `#[cfg(test)] mod tests` blocks in `counterexample_storage.rs` use
  `tx.open_table(...).unwrap()` — deliberate test-only panics in a
  controlled test environment, not production read paths, and FIND-M9-72
  does not cover them.
- m9+ carry-forwards unrelated to FIND-M9-72
  (e.g. FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE).
