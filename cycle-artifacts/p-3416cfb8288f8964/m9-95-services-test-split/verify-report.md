# Verify Report — m9-95-services-test-split

> **Cycle**: m9-95-services-test-split
> **Path**: B-direct (mechanical test code extraction, no behavior change)
> **Date**: 2026-09-14
> **Tier**: T1 (T0 + T1 + T2)

## Subject

This verify report covers the m9-95-services-test-split cycle, a B-direct
mechanical refactor that closes FIND-M9-94-SERVICES-COUNTEREXAMPLE-TESTS-MONOLITHIC
(the test-half of cc-001-god-module on the services side; storage side
was closed by m9-94). The cycle:

1. Extracts the 2169-line inline `mod tests { ... }` block from
   `crates/chronos-services/src/counterexample.rs` (lines 1786-3955)
   into a sibling file `crates/chronos-services/src/ce_services_tests.rs`.
2. Replaces the inline block in the parent file with an 11-line
   `#[cfg(test)] #[path = "ce_services_tests.rs"] mod tests;` declaration
   (including a 7-line doc comment).

After m9-95: `counterexample.rs` shrinks from 3955 → 1794 lines
(production code only); `ce_services_tests.rs` holds the 45 test
functions + 4 test helpers in a sibling submodule that is reachable as
`counterexample::tests`. The Rust module graph is unchanged (same
items, same visibility, same resolution paths).

## Verification approach

Per AGENTS.md tier table for B-direct:

- **T0**: `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings`.
- **T1**: `cargo test -p chronos-services --lib --no-fail-fast` (the affected crate).
- **T2**: `cargo test --workspace --lib --no-fail-fast -- --test-threads=1` (broader regression sweep).

No sandbox needed (no probe/mcp touched; no behavior change).

## Pre-cycle baseline

m9-94 (just closed): workspace lib tests 1042; v0.7.96 released.

## Post-cycle result

| Tier | Result | Notes |
|---|---|---|
| T0 fmt | clean | no diff after `cargo fmt --all` |
| T0 clippy | clean | no warnings, no errors |
| T1 chronos-services lib | 268 pass | same count as m9-94 baseline (268). No test lost or duplicated. |
| T2 workspace lib | 1042 pass | same count as m9-94 baseline (1042). No regressions. |

## Cross-checks executed

| CC | Description | Result |
|---|---|---|
| CC#3 | apply-checkpoint era-awareness | pending (after merge + tag fixpoint) |
| CC#4 | Artifact SHA-256 consistency | pending (after archive-manifest.md + regen) |
| CC#8 | archive-manifest Head SHA single-line | pending (after archive-manifest.md written) |
| CC#22 | release-receipt canonical SHA fields | pending (after release-receipt.md written) |
| CC#23 | merge-receipt canonical SHA fields | pending (after merge --no-ff) |
| CC#39 | Total cycles consistency | pending (after cycles/index.md bump to 95) |
| CC#42 | release-receipt Remote tag_peel matches immutable tag | pending (after release-receipt.md + tag fixpoint) |
| CC#43 | release-receipt Head SHA matches apply-checkpoint | pending (after release-receipt.md written) |
| CC#51 | cycles/index.md cycle has cycle-artifacts/ folder | passed (m9-95 dir created with cycle artifacts in progress) |

## Findings

None — clean state. (m10-legacy-migration)

## Files Inventory

| Path | Change |
|---|---|
| `crates/chronos-services/src/ce_services_tests.rs` | NEW (2181 lines; verbatim copy of inline test block + 12-line doc comment header) |
| `crates/chronos-services/src/counterexample.rs` | modified (-2169 / +11 LoC; replaced 2170-line inline `mod tests { ... }` block with 11-line `#[cfg(test)] #[path = "ce_services_tests.rs"] mod tests;` declaration) |
| `cycle-artifacts/p-3416cfb8288f8964/m9-95-services-test-split/*.md` + `.json` | added (7 cycle artifacts) |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-95-services-test-split/*.md` | added (5 knowledge artifacts) |

## Summary

T0+T1+T2 all green. m9-95 ready for release + archive.

## Sign-off

Workspace lib tests green (1042 pass, no regressions). CC sweep pending
post-archive vault writes. Cycle ready for tag pre-creation +
--no-ff merge + archive + push.

## Cross-checks

- `bash scripts/check_vault_drift.sh`: PASS (CC#39 Part B satisfied; release-report.md has `## Cross-checks` section).
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
