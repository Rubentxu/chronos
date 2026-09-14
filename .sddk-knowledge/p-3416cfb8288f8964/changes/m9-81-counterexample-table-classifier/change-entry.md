# Change: m9-81 counterexample_storage uses the canonical table_error helper

## Summary

Closes FIND-M9-72 by routing 6 production read-path sites in
`crates/chronos-store/src/counterexample_storage.rs` through
`chronos_store::table_error::classify_read_table_error().or_not_found(...)`.
Behaviour-preserving refactor (74 / 0 lib unit test counts match before
and after). No public API, schema, or wire change.

## Subject

- Cycle: `m9-81-counterexample-table-classifier`
- Tag: `v0.7.83`
- Merge SHA: `fdc5accf64be1fcf780913243aec0496ad48e7fe`
- Path: B-direct
- Tier required: T1

## Files changed

- `crates/chronos-store/src/counterexample_storage.rs` — 6 read-path
  sites refactored; imports trimmed (TableError removed,
  classify_read_table_error added). Net -2 lines (11 inserts / 13 deletes).
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-81-counterexample-table-classifier/` —
  new exploration-report.md, proposal.md, spec.md, tasks.md,
  change-entry.md (5 new files).
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-81-counterexample-table-classifier/archive-manifest.md` —
  new archive-manifest.md.
- `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` — appended m9-81 row; Total cycles 82 → 83.
- `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` — Last updated + Last archive bumped; FIND-M9-72 closure recorded.
- `cycle-artifacts/p-3416cfb8288f8964/m9-81-counterexample-table-classifier/` —
  apply-checkpoint.json, implementation-receipt.md, verify-findings.json,
  verify-report.md, release-receipt.md, merge-receipt.md, release-report.md
  (7 cycle-artifacts).

## Cross-check

- CC#12: `main_sha == head_sha == remote_tag_peel == fdc5accf64be1fcf780913243aec0496ad48e7fe`
- CC#28: `Base SHA = 45b53df132186b09de75b543b87cf0bab23bd26e` in release-receipt.md
- CC#30: verify-findings.json has `verdict: "passed"`; archive-manifest.md
  has `## Cycle`; change-entry.md has `## Summary`
- CC#31: archive-manifest.md has `Base SHA` field; verify-report.md has
  `## Cross-checks` section
- CC#42: `tag_peel_sha = fdc5accf64be1fcf780913243aec0496ad48e7fe` (full 40-char hex)
- CC#51: cycle-artifacts folder exists with 7 artifacts
- CC#55: Files Inventory present in verify-report.md and archive-manifest.md

> **Cycle**: `p-3416cfb8288f8964/m9-81-counterexample-table-classifier`
> **Tag**: `v0.7.83`
> **Merge SHA**: `fdc5accf64be1fcf780913243aec0496ad48e7fe`
> **Date**: 2026-09-14
> **Status**: CLOSED

## What changed

`crates/chronos-store/src/counterexample_storage.rs`: 6 production
read-path sites now route through `chronos_store::table_error::classify_read_table_error().or_not_found(EMPTY)`
instead of hand-rolling the `match { Ok | TableDoesNotExist -> Ok(EMPTY) | else -> Err(Database) }`
ladder at each site.

| Site | Function | "Absent" answer |
|---|---|---|
| 1 | `collect_bundle_chunks_range` | `Vec::new()` |
| 2 | `collect_v3_keys_for_bundle` | `Vec::new()` |
| 3 | `collect_bundle_chunks_legacy` | `Vec::new()` |
| 4 | `get_bundle_events_count` | `0_u64` |
| 5 | `load_counterexample_bundle` | `None::<CounterexampleBundleRecord>` |
| 6 | `list_counterexample_bundles` | `Vec::new()` |

Imports: `TableError` removed (no longer named); `classify_read_table_error` added.

## Why

m9-72 introduced `crates/chronos-store/src/table_error.rs` as the canonical
helper for the read-path `TableDoesNotExist` / else-propagate policy and
applied it to `cas.rs` and `storage.rs`. `counterexample_storage.rs`
was missed; FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION
tracked the follow-up.

## Carry-forward

- **Closed**: FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION.
- **New (out of scope)**: FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK — the
  `sddk cycle evaluate-gate` CLI is unable to record admission events
  on this project (FOREIGN KEY constraint + duplicate event_id), which
  forced the use of the manual vault-tracked workflow this cycle.
  Recommend a separate follow-up cycle.

## Out-of-scope items (intentionally untouched)

- `#[cfg(test)] mod tests` blocks in `counterexample_storage.rs` use
  `tx.open_table(...).unwrap()` — deliberate test-only panics, not
  production read paths, and FIND-M9-72 does not cover them.
- m9+ carry-forwards unrelated to FIND-M9-72
  (e.g. FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE).

## Public API / wire impact

None. No public API, schema, or wire shape change.
