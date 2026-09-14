# Release Report — m9-94-cc001-test-split

> **Cycle**: m9-94-cc001-test-split
> **Path**: B-direct (mechanical refactor, no behavior change)
> **Date**: 2026-09-14
> **Tag**: v0.7.96

## Summary

m9-94 closes the **test half** of `cc-001-god-module` (the production
half was closed by m9-84..m9-87). Before m9-94, the parent file
`crates/chronos-store/src/counterexample_storage.rs` was 1960 lines but
production code was only 188 lines — the bulk (1771 lines, 90% of the
file) was an inline `mod tests { ... }` block. m9-94 mechanically
moves that block to a sibling file
`crates/chronos-store/src/ce_storage_tests.rs` using the same
`#[path = "..."]` submodule pattern that m9-84..m9-87 used for the
production code.

The result: the parent file is now 191 lines (production code + a
3-line `#[path]` declaration); the test block lives in a sibling
sibling submodule reachable as `counterexample_storage::tests`. The
Rust module graph is identical pre- and post-extraction (same items,
same visibility, same resolution paths).

## What shipped

| Change | File(s) | LoC |
|---|---|---|
| NEW sibling test file | `crates/chronos-store/src/ce_storage_tests.rs` | +1785 (1771 verbatim tests + 14-line doc header) |
| Replace inline test block with `#[path]` declaration | `crates/chronos-store/src/counterexample_storage.rs` | -1772 / +3 |
| Knowledge artifacts (proposal, spec, tasks, exploration, change-entry) | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-94-cc001-test-split/` | +5 files |
| Cycle artifacts (apply-checkpoint, implementation-receipt, merge-receipt, release-receipt, release-report, verify-findings, verify-report) | `cycle-artifacts/p-3416cfb8288f8964/m9-94-cc001-test-split/` | +7 files |

## What did not ship

- **FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK** (carried from m9-88, external `sddk` CLI bug).
- **FIND-M9-94-SERVICES-COUNTEREXAMPLE-TESTS-MONOLITHIC** (introduced m9-94, P2 MEDIUM, followup): `crates/chronos-services/src/counterexample.rs` still has a 2169-line inline test block; same `#[path]` pattern can be applied in a future m-cycle.
- **cc-004-implicit-io-toctou** (deferred m10+).

## Verification summary

| Tier | Command | Result |
|---|---|---|
| T0 fmt | `cargo fmt --all -- --check` | clean |
| T0 clippy | `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| T1 chronos-store | `cargo test -p chronos-store --lib --no-fail-fast` | 77 pass |
| T2 workspace | `cargo test --workspace --lib --no-fail-fast -- --test-threads=1` | 1042 pass (same as m9-93 baseline) |

No sandbox smoke required (no probe/mcp touched; no behavior change).

## Risk assessment

- **Public API**: no change (extraction is `#[cfg(test)]` only).
- **Module graph**: identical pre- and post-extraction.
- **Behavior**: identical (43 tests + 4 helpers moved verbatim).
- **Downstream crates**: unaffected (verified by T2 workspace lib).

## Rollback plan

The change is purely a file reorganization. Rollback would revert the
single source commit `3a84949` (Rust) plus the cycle-artifacts commits.
No production behavior is at risk; rollback would be safe at any point.

## Out-of-scope followups

1. **FIND-M9-94-SERVICES-COUNTEREXAMPLE-TESTS-MONOLITHIC** — apply the
   same `#[path]` pattern to `crates/chronos-services/src/counterexample.rs`
   (2169-line inline test block; 55% of 3955-line file).
2. **cc-001-god-module**: fully closed at this point (production +
   test halves both split). The finding can be moved from "open" to
   "closed" in a future housekeeping cycle.
3. **cc-004-implicit-io-toctou** — P3, LOW, deferred to m10+.

## Sign-off

Cycle complete. Ready for merge → archive → push.
