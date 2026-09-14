# Release Report — m9-86-cc001-god-module-types-split

## Identification

| Field | Value |
|---|---|
| Cycle | m9-86-cc001-god-module-types-split |
| Path | A-min |
| Branch | feat/m9-86-cc001-god-module-types-split |
| Date | 2026-09-14 |
| Base SHA | 43e48b927eebb30be5cd2c6fcffbd6dcbabcbc7d |
| Head SHA | 434f2b4db74e94488f180100a8e8c8db50fd7fa3 |
| Remote tag | v0.7.88 |
| ff_merged | false (--no-ff merge commit db2147d) |

## Cycle value

| Field | Value |
|---|---|
| Cycle | m9-86-cc001-god-module-types-split |
| Tier required | T2 |
| Tiers run | T0, T1, T2, T3, T4 (chronos-native serial) |
| Status | released → archived |

## Path

A-min (cross-crate lexical refactor, single-crate source change but
with several downstream consumers that must continue to compile without
modification).

## Cycle

m9-86-cc001-god-module-types-split (P2, MEDIUM, m9-04 cc-001-god-module
third slice).

## What shipped

Lexical refactor of `crates/chronos-store/src/counterexample_storage.rs`:

- Removed the 6 `pub` type definitions + `default_schema_version` helper
  (lines 327-513 post-m9-85, ~187 lines).
- Added `ce_types.rs` (165 lines, NEW) holding all the moved types +
  the helper.
- Wired submodule via `#[path = "ce_types.rs"] pub(crate) mod ce_types;`
  + `pub use ce_types::{CounterexampleBundleFilter,
  CounterexampleBundleRecord, CounterexampleBundleSummary,
  ExistencePredicateWire, HypothesisInputWire, MinimisedPayload};`.
- Added `#[cfg(test)] pub(crate) use ce_types::default_schema_version;`
  so the test module's `use super::*;` can call the helper.
- Trimmed `counterexample_storage.rs` from 2,287 to 2,156 lines
  (-131 net).

## What did NOT ship

- Free functions `collect_v3_keys_for_bundle`, `collect_bundle_chunks_legacy`,
  `collect_bundle_chunks`, `KNOWN_BUNDLE_SCHEMA_VERSIONS` (~110 lines) —
  out of scope; could be split in m9-87+ if needed.
- Tests (~1,750 lines) — kept in same file as the impl they cover;
  out of scope.
- The standalone pub fns `bundle_events_or_legacy` and
  `bundle_events_count_or_legacy` (35 lines) — kept in parent since
  they reference types via the parent path.
- New public API. No public API removed.

## Cross-checks / Verification

| Tier | Command | Result |
|---|---|---|
| T0 | `cargo fmt --all -- --check` | passed |
| T0 | `cargo clippy --workspace --all-targets -- -D warnings` | passed |
| T1 | `cargo test -p chronos-store --lib` | 77/77 (matched baseline) |
| T2 | `cargo test -p chronos-services --lib` | 264/264 (matched baseline) |
| T3 | `cargo test -p chronos-cli` | 35/35 (matched baseline; includes replay_integration round-trip) |
| T4 | `cargo test -p chronos-native --lib -- --test-threads=1` | 103/103 |
| Build | `cargo build --workspace` | success, no warnings |
| Build | `cargo build -p chronos-mcp` | success, no warnings (downstream consumer) |

### Cross-checks

- **apply-checkpoint.base_sha** == **release-receipt.base_sha** == `43e48b927eebb30be5cd2c6fcffbd6dcbabcbc7d`. **PASS.**
- **apply-checkpoint.head_sha** == **remote_tag_peel** == **tag_peel_sha** == `434f2b4db74e94488f180100a8e8c8db50fd7fa3`. **PASS.**
- **cycles/index.md** has m9-86 row referencing `434f2b4db74e94488f180100a8e8c8db50fd7fa3`. **PASS.**
- **Total cycles** = 86 (matches row count). **PASS (CC#39).**
- **v0.7.88** tag peel == cycle-artifacts commit `434f2b4db74e94488f180100a8e8c8db50fd7fa3`. **PASS (CC#42 fixpoint-cascade workaround).**

## Files Inventory

- `crates/chronos-store/src/ce_types.rs` — NEW (165 lines).
- `crates/chronos-store/src/counterexample_storage.rs` — modified (2287 → 2156 lines, -131).
- `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` — m9-86 row added, Total cycles 86.
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-86-cc001-god-module-types-split/` — exploration-report, proposal, spec, tasks.
- `cycle-artifacts/p-3416cfb8288f8964/m9-86-cc001-god-module-types-split/` — 7 cycle artifacts.
- Tag `v0.7.88` at `434f2b4db74e94488f180100a8e8c8db50fd7fa3` (cycle-artifacts commit, pre-cascade).

## Carry-forward

The `cc-001-god-module` debt remains open. counterexample_storage.rs is
now 2,156 lines (was 2,821 pre-m9-84, 2,720 post-m9-84, 2,287 post-m9-85,
2,156 post-m9-86). Future cycles can address:

- Free functions section (`collect_*`, `KNOWN_BUNDLE_SCHEMA_VERSIONS`).
- Tests relocation.
- Standalone pub fns.

## Artifacts

- `cycle-artifacts/p-3416cfb8288f8964/m9-86-cc001-god-module-types-split/`:
  apply-checkpoint.json, implementation-receipt.md, merge-receipt.md,
  release-receipt.md, release-report.md, verify-findings.json,
  verify-report.md.
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-86-cc001-god-module-types-split/`:
  exploration-report.md, proposal.md, spec.md, tasks.md.
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-86-cc001-god-module-types-split/archive-manifest.md` (to be added at archive).
- Tag `v0.7.88` at `434f2b4db74e94488f180100a8e8c8db50fd7fa3` (cycle-artifacts
  commit, pre-cascade).
