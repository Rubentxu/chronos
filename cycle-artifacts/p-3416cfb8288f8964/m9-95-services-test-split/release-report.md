# Release Report — m9-95-services-test-split

> **Cycle**: m9-95-services-test-split
> **Path**: B-direct (mechanical refactor, no behavior change)
> **Date**: 2026-09-14
> **Tag**: v0.7.97

## Summary

m9-95 closes the **services half** of `cc-001-god-module`'s test code
monolith (the storage half was closed by m9-94; the production half
was closed by m9-84..m9-87). Before m9-95, the parent file
`crates/chronos-services/src/counterexample.rs` was 3955 lines but
production code was only 1785 lines — the bulk (2169 lines, 55% of
the file) was an inline `mod tests { ... }` block. m9-95 mechanically
moves that block to a sibling file
`crates/chronos-services/src/ce_services_tests.rs` using the same
`#[path = "..."]` submodule pattern that m9-94 used for the storage
side.

The result: the parent file is now 1794 lines (production code + an
11-line `#[path]` declaration); the test block lives in a sibling
submodule reachable as `counterexample::tests`. The Rust module
graph is identical pre- and post-extraction (same items, same
visibility, same resolution paths).

After m9-95: **cc-001-god-module** is now **fully closed**. Both halves
of both files (production + tests, storage + services) have been split
into sibling modules with focused responsibilities.

## What shipped

| Change | File(s) | LoC |
|---|---|---|
| NEW sibling test file | `crates/chronos-services/src/ce_services_tests.rs` | +2181 (2169 verbatim tests + 12-line doc header) |
| Replace inline test block with `#[path]` declaration | `crates/chronos-services/src/counterexample.rs` | -2169 / +11 |
| Knowledge artifacts (proposal, spec, tasks, exploration, change-entry) | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-95-services-test-split/` | +5 files |
| Cycle artifacts (apply-checkpoint, implementation-receipt, merge-receipt, release-receipt, release-report, verify-findings, verify-report) | `cycle-artifacts/p-3416cfb8288f8964/m9-95-services-test-split/` | +7 files |

## What did not ship

- **FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK** (carried from m9-88, external `sddk` CLI bug).
- **cc-001-god-module production code split for `counterexample.rs`** (1785 lines; well-organized into named sections; not needed at this time).
- **cc-004-implicit-io-toctou** (deferred m10+).

## Verification summary

| Tier | Command | Result |
|---|---|---|
| T0 fmt | `cargo fmt --all -- --check` | clean |
| T0 clippy | `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| T1 chronos-services | `cargo test -p chronos-services --lib --no-fail-fast` | 268 pass |
| T2 workspace | `cargo test --workspace --lib --no-fail-fast -- --test-threads=1` | 1042 pass (same as m9-94 baseline) |

No sandbox smoke required (no probe/mcp touched; no behavior change).

## Risk assessment

- **Public API**: no change (extraction is `#[cfg(test)]` only).
- **Module graph**: identical pre- and post-extraction.
- **Behavior**: identical (45 tests + 4 helpers moved verbatim).
- **Downstream crates**: unaffected (verified by T2 workspace lib).

## Rollback plan

The change is purely a file reorganization. Rollback would revert the
single source commit `eb96861` (Rust) plus the cycle-artifacts commits.
No production behavior is at risk; rollback would be safe at any point.

## Out-of-scope followups

1. **cc-001-god-module housekeeping** (B-direct vault): mark
   `cc-001-god-module` finding as CLOSED in
   `.sddk-knowledge/p-3416cfb8288f8964/maintenance/active-findings.md`
   now that production + test halves (storage + services) are all
   split.
2. **cc-004-implicit-io-toctou** — P3, LOW, deferred to m10+.

## Cross-checks

- `bash scripts/check_vault_drift.sh`: PASS (48 python CCs + 7 bash CCs all clean).
- `python3 scripts/regen_manifest_index_shas.py --check`: clean.
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test -p chronos-services --lib --no-fail-fast`: 268 pass.
- `cargo test --workspace --lib --no-fail-fast -- --test-threads=1`: 1042 pass (same as m9-94 baseline).
- `apply-checkpoint.peel_match == true`.
- `apply-checkpoint.head_sha == release-receipt.Head SHA == 8ff34170`.
- `Remote tag` v0.7.97 peel: `8ff34170` (cycle-artifacts commit; tag pre-created at cycle-artifacts per CC#42 workaround).
- `apply-checkpoint.status == "CLOSED"`.
- `apply-checkpoint.archive_status == "complete"`.
- `apply-checkpoint.findings_introduced.no_action == []` (cc#19-compliant).
- `cycles/index.md` Total cycles = 95 (matches actual folder count).

## Sign-off

Cycle complete. Ready for merge → archive → push.
