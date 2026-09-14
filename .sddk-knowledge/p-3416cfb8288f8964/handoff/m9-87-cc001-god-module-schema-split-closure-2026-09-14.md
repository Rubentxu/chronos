# m9-87-cc001-god-module-schema-split-closure-2026-09-14

## Cycle identity

- **Cycle ID:** m9-87-cc001-god-module-schema-split
- **Path:** A-min (lexical refactor)
- **Branch:** feat/m9-87-cc001-god-module-schema-split
- **Base SHA:** 8df5c57b93bef0ff612a20bad1301b2c5875fb5a (m9-86 handoff)
- **Head SHA:** 1bfd6a75493b7dc491866c76e52a2a6978e5af16
- **Merge SHA:** e222d854d97e4adae5f8b20410644609c191e545
- **Remote tag:** v0.7.89 (peel: e222d85)
- **Date:** 2026-09-14T13:34Z

## What m9-87 did

Extracted 9 items from `crates/chronos-store/src/counterexample_storage.rs`
into a new sibling submodule `crates/chronos-store/src/ce_schema.rs`:

| Item | Visibility change |
|---|---|
| `COUNTEREXAMPLE_BUNDLES` | const → pub (for re-export) |
| `COUNTEREXAMPLE_BUNDLE_EVENTS` | const → pub (for re-export) |
| `BUNDLE_EVENTS_CHUNK_SIZE` | pub (unchanged) |
| `collect_bundle_chunks_range` | pub (unchanged) |
| `collect_v3_keys_for_bundle` | fn → pub (for re-export) |
| `collect_bundle_chunks_legacy` | fn → pub (for re-export) |
| `collect_bundle_chunks` | fn → pub (for re-export) |
| `CURRENT_BUNDLE_SCHEMA_VERSION` | pub (unchanged) |
| `KNOWN_BUNDLE_SCHEMA_VERSIONS` | const → pub (for re-export) |
| `const _: () = { ... }` invariant block | moved verbatim |

counterexample_storage.rs: **2077 → 1954 lines (-123, -5.9%)**.

Public surface preserved: all 9 items re-exported at the parent path
via `pub use ce_schema::{...}` so callers continue to use
`chronos_store::counterexample_storage::*` unchanged.

## Cross-cycle cc-001 progress

| Cycle | Submodule created | Parent (lines) | Delta |
|---|---|---|---|
| m9-84 | ce_chunk_keys | 2821 → 2720 | -101 (key encoding helpers) |
| m9-85 | ce_write, ce_read, ce_test_hooks | 2720 → 2287 | -433 (impl blocks) |
| m9-86 | ce_types | 2287 → 2077 | -210 (types) |
| m9-87 | ce_schema | 2077 → 1954 | -123 (schema) |
| **Total** | **6 submodules** | **-766 (-28%)** | across 4 cycles |

The cc-001 god-module decomposition is now substantially complete.
The 1954-line parent is mostly: 3 `impl SessionStore` method blocks
across ce_write/ce_read/ce_test_hooks (~750 lines), 2 standalone free
helpers (`bundle_events_or_legacy`, `bundle_events_count_or_legacy`,
~100 lines), and integration tests (~1000 lines).

## Verification

- **T0** (`cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings`): clean.
- **T1** (`cargo test -p chronos-store --lib --no-fail-fast`): 77 / 77.
- **T2** (`cargo test -p chronos-services --lib --no-fail-fast`): 264 / 264.
- **T3** (`cargo test -p chronos-cli --no-fail-fast`): 11+22+2 = 35 / 35.
- **T4-serial** (`cargo test -p chronos-native --lib -- --test-threads=1`): 103 / 103.
- **Sanity** (`cargo build -p chronos-mcp`): succeeds.

No regressions.

## Smell audit

- 3 unused `use` statements in counterexample_storage.rs removed
  (`classify_read_table_error`, `ReadableTable`, `TableDefinition`).
- 1 deliberate `#[cfg(test)] #[allow(unused_imports)]` annotation
  for test-only re-exports (`bundle_prefix`, `decode_chunk_key`,
  `decode_chunk_value` from `ce_chunk_keys`).

No code-quality regressions.

## Discoveries worth noting

1. **`pub(super)` does not satisfy `pub use` re-export.** E0364 error
   when attempting to re-export private items via the parent's
   `pub use`. The `pub(super)` items become visible at the parent
   module but the parent's `pub use` requires them to be `pub`
   themselves. **Resolution:** promote all moved items to `pub` in
   the new submodule (matches the pattern from m9-84 / m9-86).

2. **`crate::ce_chunk_keys` is not visible from sibling submodules.**
   `ce_schema.rs` is at `crate::counterexample_storage::ce_schema`,
   so `crate::ce_chunk_keys` does not resolve from there. **Resolution:**
   use `use crate::counterexample_storage::ce_chunk_keys::{...}`.

3. **Test code in the parent file may still use moved items.** The
   m9-04 layout tests in counterexample_storage.rs use `bundle_prefix`,
   `decode_chunk_key`, `decode_chunk_value`. These were moved out
   of the parent's production body but the tests still need them.
   **Resolution:** wrap the parent's `pub(super) use` of test-only
   items in `#[cfg(test)] #[allow(unused_imports)]`.

## Cycle artifacts written

All under `cycle-artifacts/p-3416cfb8288f8964/m9-87-cc001-god-module-schema-split/`:

- apply-checkpoint.json
- implementation-receipt.md
- merge-receipt.md
- release-receipt.md
- release-report.md
- verify-findings.json
- verify-report.md

Plus `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-87-cc001-god-module-schema-split/archive-manifest.md`
with `## Evidence bindings` + `## Cross-checks` sections.

Plus `cycles/index.md` updated with m9-87 row and `Total cycles | 87`.

## Carry-forward

Open (not addressed by m9-87):

- **FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK** — CLI ledger FK bug.
- **M7 milestone** — events_read merge, observe merge,
  session_compare/explain split, lifecycle.

Open options for m9-88+:

- **cc-001 final slimming** — extract integration tests to a
  `tests.rs` submodule (would reduce parent by ~1000 lines, but
  diminishing returns and tests would lose access to private
  `impl SessionStore` items without `pub(super)`-all over the place).
- **ce_write.rs / ce_read.rs split into smaller units** — each
  is currently ~150-300 lines with multiple `impl SessionStore`
  methods; could be split per-method but tests are method-coupled.

## Vault drift status

9 CCs reported drift before m9-87 closure (CC#21, 24, 31, 32, 33,
39, 42, 44, 55). After m9-87 closure:

- 8 fixed: CC#21, 24, 31, 32, 33, 39, 42, 44 (via Evidence bindings,
  Cross-checks sections, Total cycles update, Remote tag_peel field,
  Summary section in verify-report.md, Subject in verify-report.md,
  Path/Cross-checks in release-report.md, Cross-checks in
  verify-report.md).
- 1 remaining: CC#55 — pre-existing drift in m9-66 (JSON parse error
  on line 35), m9-67 (bad base_sha), m9-85 (bad base_sha). NOT
  introduced by m9-87. The script counts these as 1 drift line.

These are pre-existing issues to address in a future cycle (e.g.
m9-88 cc-001 final slimming + drift remediation).

## Tag pre-creation pattern (CC#42 workaround)

Tag `v0.7.89` was pre-created at `1bfd6a7` (refactor commit), then
moved to `e222d85` (merge commit) per the CC#42 fixpoint-cascade
workaround documented in m9-83 handoff. This ensures
`git rev-parse v0.7.89^{commit}` returns the merge SHA at archive
time, matching `apply-checkpoint.json`'s `head_sha` + `main_sha`
after archive closure.

## Session timing

- Session start: ~12:35Z (post-m9-86 handoff).
- Cycle close: ~13:41Z.
- Wall time: ~66 min (mostly build/test waiting + drift remediation).
