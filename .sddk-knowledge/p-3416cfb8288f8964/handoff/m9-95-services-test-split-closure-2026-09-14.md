# m9-95-services-test-split — Closure Handoff (2026-09-14)

## Cycle summary

| Field | Value |
|---|---|
| Cycle ID | m9-95-services-test-split |
| Path | B-direct (mechanical refactor, no behavior change) |
| Branch | chore/m9-95-services-test-split |
| Tag | v0.7.97 |
| Tag peel (immutable) | 8ff34170fe98fd14cc1e10e30e95d842fe67f0f0 (pre-cascade-fixpoint per CC#42 workaround) |
| Merge SHA | a33b84768026044b4500d677793b5654c410887b (--no-ff merge into main) |
| Status | CLOSED, archived, cycle branch deleted |
| Tier | T1 (T0 + T1 + T2) |

## What this cycle did

Closes **FIND-M9-94-SERVICES-COUNTEREXAMPLE-TESTS-MONOLITHIC** (the
**services half** of cc-001-god-module's test code monolith; storage
half was closed by m9-94; production halves were closed by
m9-84..m9-87).

Extracts the 2169-line inline `mod tests { ... }` block from
`crates/chronos-services/src/counterexample.rs` (lines 1786-3955)
into a sibling file
`crates/chronos-services/src/ce_services_tests.rs` using the
`#[path = "..."]` submodule pattern established by m9-94.

After m9-95: `counterexample.rs` shrinks from 3955 → 1794 lines
(production code only); `ce_services_tests.rs` holds the 45 test
functions + 4 test helpers in a sibling submodule reachable as
`counterexample::tests`. The Rust module graph is identical pre- and
post-extraction (same items, same visibility, same resolution paths).

## Drift delta

None — m9-95 is a Rust-only cycle. No vault CC drift introduced or
closed.

## Commit chain

1. **`eb96861`** — m9-95: extract counterexample services tests to ce_services_tests.rs sibling (Rust source)
2. **`8ff34170`** — m9-95: cycle artifacts + knowledge files (apply-checkpoint + change-entry)
3. **`38fba83c`** — m9-95: align artifacts to cycle-artifacts commit SHA 8ff34170 (SHA-cascade fixpoint per CC#42)
4. **`3a195111`** — m9-95: add cycle row + bump Total to 95; update Last archive
5. **`a33b8476`** — Merge branch 'chore/m9-95-services-test-split' into main (--no-ff)
6. **`a5fa9333`** — m9-95: fill merge SHA a33b8476 in merge-receipt.md
7. **`ff786ace`** — m9-95: archive-manifest + SHA-256 fixpoint cascade (24 rows)

Tag v0.7.97 pre-created at cycle-artifacts commit 8ff34170 per CC#42
fixpoint-cascade workaround (m9-83 handoff). The tag stays at
8ff34170 even after merge + cascade commits to avoid infinite regress.

## Cycle artifacts

All 7 cycle artifacts written under `cycle-artifacts/p-3416cfb8288f8964/m9-95-services-test-split/`:

- `apply-checkpoint.json`
- `implementation-receipt.md`
- `merge-receipt.md`
- `release-receipt.md`
- `release-report.md` (with `## Cross-checks` section per CC#39 Part B)
- `verify-findings.json`
- `verify-report.md`

Archive-manifest with SHA-256 evidence bindings written at
`.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-95-services-test-split/archive-manifest.md`.

## Lessons learned

1. **The `#[path]` test-split pattern is now a reliable routine**: m9-94
   established the technique on
   `crates/chronos-store/src/counterexample_storage.rs` (1771 lines of
   tests); m9-95 applies the identical pattern to the larger
   `crates/chronos-services/src/counterexample.rs` (2169 lines of
   tests). No surprises; the pattern works at the second occurrence
   without modification.
2. **Proptest imports survive the extraction without issue**: the
   sibling `#[path]` submodule inherits the parent's scope, but the
   proptest crate imports (`use proptest::strategy::{Strategy, ValueTree};`,
   `use proptest::test_runner::Config;`) are crate-level (not
   module-level), so they don't need to be re-imported in the
   sibling file. The `use super::*;` is sufficient.
3. **`cargo fmt` post-extraction rewrites a small number of lines** in
   the new sibling file (whitespace from the doc-comment header
   insertion). Cosmetic only.
4. **cc-001-god-module is now fully closed**: production halves
   (m9-84..m9-87) + test halves (m9-94 + m9-95). The finding can be
   marked CLOSED in a future housekeeping cycle.
5. **B-direct cycles still need `## Cross-checks` in release-report.md**
   (CC#39 Part B). m9-94 had to add it after the first vault sweep;
   m9-95 included it from the start.

## Out-of-scope / Carry-forward

- **FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK** (carried from m9-88): external `sddk` CLI bug; cannot be fixed in chronos scope.
- **cc-001-god-module production code split for `counterexample.rs`** (1785 lines; well-organized into named sections): not needed at this time. The production code is structured into CounterexampleContext, CounterexampleShrinkInput, CounterexampleOutput, CounterexampleRunError, ShrinkConfig, custom proptest strategies + ValueTree impls, and the ChronosCounterexampleService struct + impl methods. Each section has a clear doc-comment header.
- **cc-004-implicit-io-toctou** (deferred to m10+): out of scope for m9+.

## Final state

- `bash scripts/check_vault_drift.sh`: PASS (48 python CCs + 7 bash CCs all clean).
- `python3 scripts/regen_manifest_index_shas.py --check`: clean.
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test -p chronos-services --lib --no-fail-fast`: 268 pass.
- `cargo test --workspace --lib --no-fail-fast -- --test-threads=1`: 1042 pass (same as m9-94 baseline).
- v0.7.97 → 8ff34170 (cycle-artifacts; CC#42 workaround).
- m9-95 row added to `cycles/index.md`; Total cycles = 95.
- `terms/index.md` Last archive = m9-95-services-test-split.
- Cycle branch `chore/m9-95-services-test-split` deleted per CC#53.

## Next steps

m9-96 candidates (from m9-95 carry-forward + cc-001 housekeeping):

1. **cc-001-god-module housekeeping** (B-direct vault): mark
   `cc-001-god-module` finding as CLOSED in
   `.sddk-knowledge/p-3416cfb8288f8964/maintenance/active-findings.md`
   now that production + test halves (storage + services) are all
   split. **Strong candidate for m9-96**: small vault-only cycle
   (A-lite scope), no Rust touched.
2. **cc-004-implicit-io-toctou** (deferred to m10+): explicit-IO
   refactor across chronos-services / chronos-store / chronos-mcp.
   First m10 cycle.
3. **External** (sddk CLI): FIND-M9-81 cannot be fixed in chronos
   scope.

Recommend m9-96 = cc-001-god-module housekeeping (option 1). The
finding has been fully resolved across 5 cycles (m9-84..m9-87 + m9-94
+ m9-95); updating active-findings.md closes it. Smallest possible
next cycle; sets up the m10+ backlog to start clean.
