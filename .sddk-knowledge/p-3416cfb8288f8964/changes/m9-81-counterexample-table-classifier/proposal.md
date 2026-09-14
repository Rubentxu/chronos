# Proposal: m9-81 chronos-store::counterexample_storage uses the canonical table_error helper

> **Cycle**: `p-3416cfb8288f8964/m9-81-counterexample-table-classifier`
> **Status**: proposal-complete; spec/apply/release/archive pending
> **Date**: 2026-09-14
> **Path**: B-direct
> **Tier**: T1
> **Carry-forward FIND**: FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION (m9-72)

## Intent

m9-72 introduced `crates/chronos-store/src/table_error.rs` as the canonical
helper for the read-path `TableDoesNotExist` / else-propagate policy. It
was applied to `cas.rs` and `storage.rs`. `counterexample_storage.rs` was
missed: it still hand-rolls the same `match` ladder at 6 production
read-path sites. This cycle closes the gap.

## Scope

- **Touched file**: `crates/chronos-store/src/counterexample_storage.rs`
  (6 read-path sites)
- **New import**: `use crate::table_error::classify_read_table_error;`
- **No public API change.** No schema change. No wire change.
- **No new tests** — the existing 6 read-path tests at lines 715+
  already cover each refactored function; behavioural preservation is
  what they assert.

## Acceptance

- `counterexample_storage.rs` contains 0 occurrences of
  `redb::TableError::TableDoesNotExist` (regex: `redb::TableError::TableDoesNotExist`)
- Each of the 6 sites uses the helper shape:
  `Err(e) => return classify_read_table_error(e).or_not_found(EMPTY)`
  with `EMPTY` = `Vec::new()`, `0_u64`, or `None` per the function's
  return type.
- `cargo fmt --all -- --check` passes.
- `cargo clippy --workspace --all-targets -- -D warnings` passes.
- `cargo test -p chronos-store --lib --no-fail-fast` passes with no
  regressions relative to `45b53df` (the cycle base).

## Out of scope

- `#[cfg(test)] mod tests` blocks in `counterexample_storage.rs` use
  `tx.open_table(...).unwrap()` — those are deliberate test-only panics,
  not production read paths, and FIND-M9-72 does not cover them.
- m9+ carry-forwards unrelated to FIND-M9-72.

## Carry-forward

- Closes: **FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION**
- No new FINDs introduced.

## Risks

- **Mechanically equivalent refactor.** The helper has the same
  input/output types as the inline ladder. The compiler will accept
  only valid substitutions.
- **Single-crate, reversible.** `git revert` restores the file in one
  command.
