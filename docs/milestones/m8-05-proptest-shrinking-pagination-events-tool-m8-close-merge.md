# M8-05 merge doc — Real proptest shrinking + pagination + events tool + M8 close

**Cycle:** `m8-05-real-proptest-shrinking-pagination-events-tool-m8-close-merge`
**Branch:** `feat/m8-05-real-proptest-shrinking-execute`
**Base:** `a87d6a4` (m8-05 scoping FF-merge from prior cycle)
**Head:** `41f6ad2` (docs(milestones): M8 close report)
**Tag:** `m8-05-real-proptest-shrinking.0`
**Status:** COMPLETE — 2026-09-11

## §1 What this cycle ships

m8-05 closes four explicit deliverables from the scoping doc:

| Deliverable | B-item | Commit | What |
|---|---|---|---|
| Real per-variant proptest shrinking | B1 (closes m8-03 R1+R2) | `260b4ac` | `build_strategy_for` dispatches per `target.kind`; `drive_strategy` rebuilds strategy + tree each iter (R7 work-around for `dyn ValueTree: !Send`); `rounds_used == 2` end-to-end |
| List pagination cursor | B2 (closes m8-03 R3) | `464e689` | `cursor: Option<String>` on store filter + service filter + MCP params; store skips rows `bundle_id <= cursor` (uuid::v7 = chronological); service emits `next_cursor = Some(last.bundle_id)` when page is full |
| `counterexample_events_count` MCP tool | B3 | `27f95b5` | new DTO + `CounterexampleOutput::EventsCount` + service method + MCP `#[tool]` + sandbox ce9 |
| M8 close report | B4 | `41f6ad2` | `docs/milestones/M8-CLOSE.md` following m7 close precedent; 7 sections; m9+ handoff |

## §2 Cycle commits

| Commit | Subject | Tests added |
|---|---|---|
| `a87d6a4` | Merge branch 'feat/m8-05-proptest-shrinking-pagination-events-tool-m8-close' (scoping only) | — |
| `260b4ac` | feat(services): real per-variant proptest shrinking (m8-05 execute 1) | +6 services unit + modified 1 |
| `464e689` | feat(store+mcp+sandbox): counterexample list pagination cursor (m8-05 execute 2) | +5 (2 store + 3 services) + ce8 |
| `27f95b5` | feat(services+mcp+sandbox): counterexample_events_count lightweight tool (m8-05 execute 3) | +2 services + ce9 |
| `41f6ad2` | docs(milestones): M8 close report | — |

## §3 Architectural decisions (per-cycle summary)

### Execute 1 — real per-variant proptest shrinking

- **`build_strategy_for(target) -> SBoxedStrategy<HypothesisInput>`** dispatches per `target.kind`:
  - `Invariant` → `build_strategy_for_invariant`
  - `Existence` → `build_strategy_for_existence`
  - `CallPath` → `build_strategy_for_call_path`
- All three helpers currently return `Just(base.clone())` (m8-05 R3 honest disclosure).
- **`drive_strategy<F, Fut>`** is an async helper that drives proptest's `ValueTree` manually from async land.
- **`SBoxedStrategy`** required for `Send` (m8-05 R5): `BoxedStrategy` is `!Send` because it boxes `dyn Strategy` without a Send bound. The MCP `#[tool]` wrapper requires Send.
- **R7 work-around** (also m8-05): `dyn ValueTree` is also `!Send`, so the value tree cannot be held across `.await`. `drive_strategy` rebuilds the strategy + tree each iteration. For `Just(base)` strategies, `simplify()` returns `false` immediately, so the loop terminates after the initial validation + 1 simplify attempt (`rounds_used == 2`).

### Execute 2 — list pagination cursor

- **Cursor = `bundle_id` (uuid::v7)**: forward pagination via lexicographic `>`. Caller follows `next_cursor` until the page is partial.
- **Standard semantics**: `next_cursor = Some(last.bundle_id)` when `summaries.len() == limit`, `None` otherwise. Caller fetches an empty page to discover end (no `total_count` / `has_more`).

### Execute 3 — events_count tool

- **Dedicated events_count accessor**: lightweight tool that avoids the bundle-deserialize round-trip on the LLM side.
- **Persisted-at-save count** (m8-05 R-honest-disclosure): the returned count is the value persisted when the bundle was written. NOT a live re-read.

## §4 Test pyramid state

- **Unit (chronos-services lib)**: 243/243 pass (was 232 pre-m8-05; +11 across executes 1+2+3 + m8-03's already-merged tests).
- **Per-crate integration**:
  - chronos-store: 25/25 pass (was 23 pre-m8-05; +2 in execute 2)
  - chronos-mcp: 11/11 pass (was 11; no new tests added in m8-05 because pagination / events_count changes are covered by sandbox ce8/ce9 + service tests)
  - chronos-cli: 18/18 pass (unchanged from m8-04)
- **Sandbox smoke (T4)**: 9/9 ce tests pass + 1/1 e2e_connectivity. Total: ~64s wall time for both.

## §5 Disclosures

These are m8-05's open items; full text in apply-checkpoint.json.

| Risk | State | Plan |
|---|---|---|
| **R1**: counterexample persistence via mainline save path requires populated engines map | closed in m8-05 (test uses test fixtures) | — |
| **R2**: empty engines map → `SessionNotFound` + target unchanged | accepted | documented in service module docs |
| **R3**: per-variant strategy returns `Just(base)`, no real shrinking | open, m9+ | future cycle swaps bodies without changing signatures |
| **R4**: `HypothesisInput: Send + Sync + 'static` requirement | open, m9+ | the path of least resistance — no decision needed yet |
| **R5**: `SBoxedStrategy` requirement on the strategy's shape | open, accepted | constraint, not a bug |
| **R6**: shrink requires populated engines map | accepted | identical to R2 |
| **R7**: `dyn ValueTree: !Send`; strategy tree rebuilt each iteration | open, m9+ | future real-shrinker cycle must address (e.g., `ValueTree: Send` feature flag, or move loop into `spawn_blocking`) |

## §6 Smoke subset chosen

Per AGENTS.md §2, the cycle touches the wire envelope + list + events_count surface, so the smoke subset is:

- `counterexample_tools` (9 tests, covers shrink / get / list / list-filter / list-pagination / events_count)
- `e2e_connectivity` (1 test, server-startup canary)

Both green. Total wall time: ~64s. ce1..ce9 covers the new wire envelope (m8-04 B5 Saved-variant rework), pagination (B2), events_count (B3). e2e_connectivity confirms the MCP server still boots.

## §7 What's NOT closed by m8-05

- **m8-04 R-hypothesis-reconstruction-fidelity** (target_hypothesis persistence for byte-faithful replay): m9+ scope. The m8-04 close report already documented this as m9+.
- **Real shrinkers** (m8-05 R3 + R7): m9+ scope. The scaffolding is in place so future cycles only need to fill in the strategy helpers.

## §8 Tag and merge

- Tag: `m8-05-real-proptest-shrinking.0`
- Merge method: `git merge --no-ff` (per project convention; the literal "FF" is the git fast-forward semantics applied to `a87d6a4` itself during the scoping cycle).
- After merge, `main` HEAD = merge commit. The branch is preserved on origin.
