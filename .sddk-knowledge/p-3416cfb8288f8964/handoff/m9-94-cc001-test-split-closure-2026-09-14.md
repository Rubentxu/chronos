# m9-94-cc001-test-split — Closure Handoff (2026-09-14)

## Cycle summary

| Field | Value |
|---|---|
| Cycle ID | m9-94-cc001-test-split |
| Path | B-direct (mechanical refactor, no behavior change) |
| Branch | chore/m9-94-cc001-test-split |
| Tag | v0.7.96 |
| Tag peel (immutable) | 9e15dd3fbfc5319df8bd31ba540d570c8fd75ad8 (pre-cascade-fixpoint per CC#42 workaround) |
| Merge SHA | 380a452fd0dc6d7e5e85f99dec97efbe47cf607c (--no-ff merge into main) |
| Status | CLOSED, archived, cycle branch deleted |
| Tier | T1 (T0 + T1 + T2) |

## What this cycle did

Closes **FIND-M9-94-CC001-TEST-CODE-MONOLITHIC** (the **test half** of
cc-001-god-module; production half was closed by m9-84..m9-87).

Extracts the 1771-line inline `mod tests { ... }` block from
`crates/chronos-store/src/counterexample_storage.rs` (lines 188-1960)
into a sibling file `crates/chronos-store/src/ce_storage_tests.rs`
using the `#[path = "..."]` submodule pattern established by
m9-84..m9-87.

After m9-94: `counterexample_storage.rs` shrinks from 1960 → 191 lines
(production code only); `ce_storage_tests.rs` holds the 43 test
functions + 4 test helpers in a sibling submodule reachable as
`counterexample_storage::tests`. The Rust module graph is identical
pre- and post-extraction (same items, same visibility, same
resolution paths).

## Drift delta

None — m9-94 is a Rust-only cycle. No vault CC drift introduced or
closed.

## Commit chain

1. **`3a84949`** — m9-94: extract cc-001 storage tests to ce_storage_tests.rs sibling (Rust source)
2. **`9e15dd3`** — m9-94: cycle artifacts + knowledge files (apply-checkpoint + change-entry)
3. **`dea23db`** — m9-94: align artifacts to cycle-artifacts commit SHA 9e15dd3 (SHA-cascade fixpoint per CC#42)
4. **`8c25fe3`** — m9-94: add cycle row + bump Total to 94; update Last archive
5. **`380a452`** — Merge branch 'chore/m9-94-cc001-test-split' into main (--no-ff)
6. **`f5ccae0`** — m9-94: fill merge SHA 380a452f in merge-receipt.md
7. **`94c8678`** — m9-94: archive-manifest + SHA-256 fixpoint cascade (24 rows)
8. **`81c1610`** — m9-94: add Cross-checks section to release-report.md (CC#39)

Tag v0.7.96 pre-created at cycle-artifacts commit 9e15dd3 per CC#42
fixpoint-cascade workaround (m9-83 handoff). The tag stays at
9e15dd3 even after merge + cascade commits to avoid infinite regress.

## Cycle artifacts

All 7 cycle artifacts written under `cycle-artifacts/p-3416cfb8288f8964/m9-94-cc001-test-split/`:

- `apply-checkpoint.json`
- `implementation-receipt.md`
- `merge-receipt.md`
- `release-receipt.md`
- `release-report.md` (with `## Cross-checks` section per CC#39 Part B)
- `verify-findings.json`
- `verify-report.md`

Archive-manifest with SHA-256 evidence bindings written at
`.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-94-cc001-test-split/archive-manifest.md`.

## Lessons learned

1. **`#[path = "..."]` works for `#[cfg(test)]` submodules**: the
   m9-84..m9-87 cycles used `#[path]` for production code submodules.
   m9-94 establishes the same pattern for test submodules. The
   sibling file's items become a submodule of the parent; `use super::*;`
   inside the test block continues to resolve to the parent's private
   items (CounterexampleBundleRecord, encode_chunk_key via ce_chunk_keys,
   etc.). No `pub(crate)` widening required.
2. **The `#[path]` submodule graph matches inline mod tests**: tests
   reachable as `counterexample_storage::tests::*` pre-extraction and
   post-extraction. No call-site changes needed.
3. **`cargo fmt` post-extraction rewrites a handful of test lines**
   (12 files affected in ce_storage_tests.rs, mostly whitespace from
   the doc-comment header insertion). The diff is small and cosmetic;
   no behavior changes.
4. **B-direct cycles still need `## Cross-checks` in release-report.md**
   (CC#39 Part B). Even when there's no T4-smoke or external integration
   to mention, the cross-checks section enumerates T0+T1+T2 plus
   peel_match, status, archive_status, no_action findings, and cycles
   Total consistency.
5. **Pre-creating the tag at the cycle-artifacts commit** (not the
   source commit) is the m9-93 pattern. m9-94 follows the same pattern
   to avoid the infinite regress of moving the tag through SHA-cascade
   fixpoint commits.

## Out-of-scope / Carry-forward

- **FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK** (carried from m9-88): external `sddk` CLI bug; cannot be fixed in chronos scope.
- **FIND-M9-94-SERVICES-COUNTEREXAMPLE-TESTS-MONOLITHIC** (introduced m9-94, P2 MEDIUM, followup): `crates/chronos-services/src/counterexample.rs` still has a 2169-line inline `mod tests { ... }` block (55% of 3955-line file). Same `#[path]` pattern established by m9-94 can be applied in a future m-cycle. The parent file is much larger (2169 lines of tests + 1786 lines of production code) so the extraction will need a more careful review than m9-94's mechanical approach.
- **cc-001-god-module production code re-split** — already complete (m9-84..m9-87). The finding can be marked closed in a future housekeeping cycle now that m9-94 closes the test half.
- **cc-004-implicit-io-toctou** (deferred to m10+): out of scope for m9+.

## Final state

- `bash scripts/check_vault_drift.sh`: PASS (48 python CCs + 7 bash CCs all clean).
- `python3 scripts/regen_manifest_index_shas.py --check`: clean.
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test -p chronos-store --lib --no-fail-fast`: 77 pass.
- `cargo test --workspace --lib --no-fail-fast -- --test-threads=1`: 1042 pass (same as m9-93 baseline).
- v0.7.96 → 9e15dd3 (cycle-artifacts; CC#42 workaround).
- m9-94 row added to `cycles/index.md`; Total cycles = 94.
- `terms/index.md` Last archive = m9-94-cc001-test-split.
- Cycle branch `chore/m9-94-cc001-test-split` deleted per CC#53.

## Next steps

m9-95 candidates (from m9-94 carry-forward + current backlog):

1. **FIND-M9-94-SERVICES-COUNTEREXAMPLE-TESTS-MONOLITHIC** (P2 MEDIUM,
   followup): apply the same `#[path]` pattern to
   `crates/chronos-services/src/counterexample.rs` (2169-line inline
   test block; 55% of 3955-line file). The parent file is much larger
   than the m9-94 source file, so the extraction will be a B-direct
   cycle but with more care needed to verify nothing is lost. **Strong
   candidate for m9-95**.
2. **cc-001-god-module housekeeping** (B-direct vault): mark
   `cc-001-god-module` finding as CLOSED in
   `.sddk-knowledge/p-3416cfb8288f8964/maintenance/active-findings.md`
   now that production + test halves are both split.
3. **cc-004-implicit-io-toctou** (deferred to m10+): explicit-IO
   refactor across chronos-services / chronos-store / chronos-mcp.
   First m10 cycle.
4. **External** (sddk CLI): FIND-M9-81 cannot be fixed in chronos
   scope.

Recommend m9-95 = services-counterexample test-split (option 1).
Largest mechanical refactor remaining in the cc-001 family. Closes
the carry-forward finding from m9-94 directly. Same B-direct path
mechanics, but with a larger file (2169 lines of tests vs m9-94's
1771 lines).
