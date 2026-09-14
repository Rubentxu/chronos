# Release Report: m9-81

> **Cycle**: `p-3416cfb8288f8964/m9-81-counterexample-table-classifier`
> **Tag**: `v0.7.83`
> **Merge SHA**: `fdc5accf64be1fcf780913243aec0496ad48e7fe`
> **Date**: 2026-09-14

## Summary

m9-81 is the 82nd closed cycle (chronological; m9-80 was 81st) and the
smallest of the recent m9 cycles: a behaviour-preserving refactor that
routes 6 read-path sites in `crates/chronos-store/src/counterexample_storage.rs`
through the canonical `chronos_store::table_error::classify_read_table_error()`
helper. The helper was introduced by m9-72 for `cas.rs` and `storage.rs`
but `counterexample_storage.rs` was missed; FIND-M9-72 carried the
follow-up until this cycle closed it.

## Behavioural delta

- `cargo test -p chronos-store --lib`: 74 / 0 (unchanged; round-trip
  verified via `git stash`).
- `cargo test -p chronos-services --lib`: 264 / 0 (unchanged; downstream
  smoke).
- `cargo clippy --workspace --all-targets -- -D warnings`: 0 warnings.
- `cargo fmt --all -- --check`: 0 diffs.

## Source delta

- `crates/chronos-store/src/counterexample_storage.rs`: 11 insertions /
  13 deletions (net -2 lines). 6 sites refactored. Imports:
  `TableError` removed (no longer named); `classify_read_table_error`
  added.
- No other source files touched.
- No public API change, no schema change, no wire change.

## Vault delta

- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-81-counterexample-table-classifier/`:
  exploration-report.md, proposal.md, spec.md, tasks.md (4 new files).
- `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md`: m9-81 row
  appended (initially OPEN, will be closed in archive phase).
- `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md`: Last updated
  timestamp bumped.
- `cycle-artifacts/p-3416cfb8288f8964/m9-81-counterexample-table-classifier/`:
  apply-checkpoint.json, implementation-receipt.md,
  verify-findings.json, verify-report.md, release-receipt.md,
  merge-receipt.md, release-report.md (7 cycle-artifacts).

## Carry-forward

- **Closed**: FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION.
- **New (out of scope)**: FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK — the
  `sddk cycle evaluate-gate` CLI is unable to record admission events
  on this project (FOREIGN KEY constraint + duplicate event_id), which
  forced the use of the manual vault-tracked workflow. Recommend a
  separate follow-up cycle once m9-81 reaches archive phase.

## Release process

1. **Pre-merge**: confirmed `origin/main == 45b53df == cycle base`.
2. **Merge**: `git -c user.name='Chronos Maintainer' -c user.email='maintainer@chronos-rs.local' merge --no-ff feat/m9-81-counterexample-table-classifier -m "..."` → merge SHA `fdc5accf64be1fcf780913243aec0496ad48e7fe` (no conflicts).
3. **Tag**: `git tag -a v0.7.83 -m "..."` on the merge commit. Peel match verified: `v0.7.83^{commit} == fdc5accf64be1fcf780913243aec0496ad48e7fe`.
4. **Push**: pending (next step).

## Archive phase (next)

- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-81-counterexample-table-classifier/change-entry.md` (new).
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-81-counterexample-table-classifier/archive-manifest.md` (new).
- `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md`: m9-81 row OPEN → CLOSED; Total cycles 82 → 83.
- `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md`: Last archive bumped; FIND-M9-72 closure recorded under "Findings closed in m9-81".
- CC#4 regen: re-cascade `scripts/regen_manifest_index_shas.py` to fixpoint.
- `bash scripts/check_vault_drift.sh`: confirm only pre-existing CC#39 off-by-one remains.
- Cycle branch `feat/m9-81-counterexample-table-classifier` deleted (local + remote).
