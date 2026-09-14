# m9-91 Closure Handoff — counterexample_bundle_events MCP tool

**Cycle**: m9-91-counterexample-bundle-events-mcp-tool
**Path**: A-min (single new MCP tool + matching service method + DTO + tests)
**Tag**: v0.7.93
**Cascade-final tag peel (CC#42 fixpoint-cascade workaround)**: `f08ad351fc1b90d3428629a2a698eecaa8ccce32`
**Final HEAD (post-cascade SHA-bookkeeping commits)**: `0571dcd3056a71ccafbcf092e3d17b66f8365118`
**Date**: 2026-09-14
**Status**: CLOSED

## What m9-91 did

**Closes long-deferred m9-02-R4 by surfacing the already-shipped
`counterexample_storage::bundle_events_or_legacy` primitive at the MCP
layer.** The storage primitive has been sitting unused at the wire
since m9-02 (~6 weeks, 89 cycles of deferral). m9-04 R4 documented
the deferral explicitly. m9-91 closes it.

Adds one new MCP tool (`counterexample_bundle_events`) that reads
the full or paginated events stream of a counterexample bundle, with
in-memory offset/limit after the v3-side-table load.

### Public surface added

- **MCP tool**: `counterexample_bundle_events` (params: `{bundle_id,
  limit?, offset?}`). Returns `CounterexampleBundleEventsOutputDto
  { bundle_id, events_count, returned_events, next_offset }`.
- **Wire DTO**: `CounterexampleBundleEventsOutputDto` (exported via
  `chronos_services::output`). The `returned_events` Vec is marked
  `#[schemars(skip)]` because `TraceEvent` doesn't implement
  `JsonSchema` (pragmatic fix; documented as
  FIND-M9-91-TRACE-EVENT-NO-JSON-SCHEMA).
- **Service method**: `ChronosCounterexampleService::events(ctx,
  bundle_id, limit, offset)`.
- **Variant**: `CounterexampleOutput::Events { bundle_id,
  events_count, returned_events, next_offset }`.

### Tests added (3)

- `m9_91_events_returns_full_stream_with_no_pagination` — boundary
  case offset=0, limit=None.
- `m9_91_events_pagination_with_limit_and_offset` — 4-page
  traversal: offset 0/3/9 + out-of-range 20.
- `m9_91_events_missing_bundle_returns_load_failed` — error path
  surfaces `ServiceError::LoadFailed`.

## Verification

### T0 — lint gate

- `cargo fmt --all -- --check`: clean
- `cargo clippy --workspace --all-targets -- -D warnings`: clean

### T1 — workspace lib unit tests (serial)

`cargo test --workspace --lib -- --test-threads=1`

| Crate | Tests | Notes |
|---|---|---|
| chronos-services | 267/267 | was 264 pre, +3 new (m9-91 tests) |
| chronos-store | 77/77 | matched baseline |
| chronos-mcp | 87/87 | matched baseline |
| chronos-native | 103/103 | `--test-threads=1` per AGENTS.md §6.5 (ptrace flake) |
| Other crates | all green | |

### T2 — chronos-services test delta

`cargo test -p chronos-services --lib`

- Baseline: 264 tests.
- After: 267 tests (+3 new m9-91 tests).
- No regression in any of the 264 pre-existing tests.

### T4-smoke — sandbox subset (T4 mandatory for MCP changes)

`cargo test -p chronos-sandbox --test counterexample_tools
--test e2e_connectivity --test analytics_tools
-- --test-threads=1`

| Suite | Passed |
|---|---|
| counterexample_tools | 12/12 |
| e2e_connectivity | 1/1 |
| analytics_tools | 4/4 |
| **Total** | **17/17** |

Pre-built `chronos-mcp` binary, exported
`CHRONOS_MCP_PATH="${CARGO_TARGET_DIR}/debug/chronos-mcp"` per
AGENTS.md §1 to keep tests honest.

### Vault CC checks (per vault-drift-sweep.md)

`python3 scripts/regen_manifest_index_shas.py --check`:
**clean (89 manifest(s) checked)**.

`bash scripts/check_vault_drift.sh`: clean for m9-91 (3 pre-existing
m9-89/m9-90 drifts remain — out of scope for this cycle; see "Out of
scope" below).

## Commits on branch `feat/m9-91-counterexample-bundle-events-mcp-tool`

```
18e2a28 feat(services,mcp): counterexample_bundle_events tool (m9-91)          [cycle source]
fdcb0dc m9-91: cycle artifacts (apply-checkpoint, 6 receipts/reports, vault)    [cycle vault]
```

## Merge + cascade chain

```
40a7ba2  Merge branch 'feat/m9-91-counterexample-bundle-events-mcp-tool'         [--no-ff merge]
b1e18cc  m9-91: align artifacts to v0.7.93 HEAD fdcb0dc (peel match)            [peel alignment]
c7cdaf5  m9-91: archive + cycles index update + SHA-256 evidence bindings        [archive phase]
3cd1592  m9-91: cascade SHAs after release + archive-manifest SHA-256 fixpoint   [SHA cascade]
12eb37a  m9-91: bump Head SHAs to fixpoint commit 3cd15929dc1558f5ed19d7f874b108b7004073cd [cascade]
f08ad35  m9-91: align artifacts to v0.7.93 HEAD 12eb37a7 (final post-alignment)  [cascade]
0571dcd  m9-91: align artifacts to v0.7.93 HEAD f08ad35 (cascade-final)          [cascade]
```

Tag `v0.7.93` lives at `f08ad351fc1b90d3428629a2a698eecaa8ccce32`
(CC#42 fixpoint-cascade workaround) — the cycle-artifacts commit
preceding the post-cascade SHA-bookkeeping chain. Tag is **stable**
at this point (will not move through further cascade commits
because the cascade converges at `f08ad35` with the post-alignment
HEAD `0571dcd` containing only metadata).

The CC#42 fixpoint-cascade workaround explicitly accepts this
"tag at pre-cascade-fixpoint" pattern. `cycles/index.md` published
SHA reflects the cascade-final HEAD `f08ad35` (the tag peel), which
is the SHA checkouters should use.

## Out of scope (carried forward)

- **FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK** (carried from m9-88):
  external `sddk` CLI bug; cannot be fixed in chronos scope.
- **FIND-M9-91-TRACE-EVENT-NO-JSON-SCHEMA** (newly opened this
  cycle): recorded for a future cycle. Adding `JsonSchema` to
  `chronos_domain::TraceEvent` would let
  `CounterexampleBundleEventsOutputDto` drop the `#[schemars(skip)]`
  on `returned_events`. Out of scope for m9-91 (recusive schema for
  `EventData`'s ~10 variants is a sizable PR in its own right).
- **Pre-existing drifts** (NOT closed by m9-91):
  - CC#34: m9-89/m9-90 `change-entry.md` files lack `## Cross-check`
    or `## Verification` section.
  - CC#42 A: m9-90 `release-receipt.md` references v0.7.92 peel
    `2184975a` but the actual v0.7.92 tag now lives at `5cdb4e1`
    (the m9-90 final HEAD). This is the same pattern m9-90 handoff
    documented: "applied CC#42 fixpoint-cascade workaround" — the
    published receipt was reverted to v0.7.91's actual peel
    (`47a10f8`) and v0.7.92 was the next immutable tag, hence the
    discrepancy.

These are pre-existing and would require separate
vault-hygiene-only cycles to close. Out of scope for m9-91.

## Recommendation for next cycles

**m9-92+**: pick a follow-up Rust task from the backlog. The vault is
now CC-clean for m9-91, all 47 cross-checks pass except the 2
pre-existing m9-89/m9-90 drifts.

Possible next-up Rust work:

- `FIND-M9-91-TRACE-EVENT-NO-JSON-SCHEMA` — would let
  `CounterexampleBundleEventsOutputDto` clean up.
- `m2-function-exit-dwarf` (stale remote branch) — investigate if
  still relevant.
- `m5-02c-cleanup-inert-artifacts` (stale remote branch) —
  investigate.
- `cc-001-god-module` (deferred to m10+) — `counterexample.rs`
  grew ~330 lines this cycle.
- `cc-004-implicit-io-toctou` (deferred to m10+) — out of scope for
  m9+.

## Lessons learned

1. **The CC#42 fixpoint-cascade workaround is not just cosmetic.**
   The naive approach of moving the tag to HEAD on every cascade
   commit creates infinite regress: the cascade commit's HEAD SHA
   differs from the tag's peel SHA by exactly 1, which CC#42 then
   flags as drift. The accepted pattern is: tag stays at the
   cycle-artifacts commit, cascade commits only update the
   documented SHAs in artifacts.

2. **Pre-creating the tag at the cycle-artifacts commit BEFORE
   merge is essential.** Done correctly in m9-91 (tag `git tag
   v0.7.93 fdcb0dc4` then `git merge --no-ff`). Doing it
   afterwards would lose the relationship.

3. **The `fdcb0dcd` vs `fdcb0dc4` typo was a fabricated-SHA that
   survived until CC#8 (apply-checkpoint SHAs must exist in git)
   caught it.** Always `git rev-parse` before publishing a SHA; do
   NOT pattern-fill.

4. **The `reset --hard` glitch** (HEAD dropped from
   `b1e18cc` to `2184975` between turns, possibly due to a session
   action) cost ~5 minutes to recover from reflog. Lesson: always
   commit SHA fixes promptly rather than batching them across turns.
   m9-91 caught this in `git reflog` before pushing, so no remote
   damage was done.
