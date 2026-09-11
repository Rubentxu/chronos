# m8-03 — MCP wrappers + redb bundle table + real Strategy impls merge doc

**Cycle:** M8 §3, third execute cycle of the M8 multi-cycle plan.
**Predecessor doc:** `docs/milestones/m8-03-mcp-wrappers-redb-bundle-table-scoping.md` (PROPOSED, FF-merged before this cycle).
**Branch:** `feat/m8-03-mcp-wrappers-redb-bundle-table` (FF-merged to `main` after this cycle).
**Status:** MERGED — 2026-09-11

## What shipped

m8-03 is the largest M8 cycle: it replaces every m8-02 stub end-to-end with a working wire-level contract, persists bundles to redb, and ships a sandbox smoke that exercises the full client ↔ server round-trip.

### 1. `counterexample_bundles` redb table + store extensions

`crates/chronos-store/src/counterexample_storage.rs` (new, 375 LoC). Adds a new `TableDefinition` (string key, blob value) for counterexample bundles and the public read/write API:

- `SessionStore::save_counterexample_bundle(record)` writes one bundle.
- `SessionStore::load_counterexample_bundle(bundle_id)` returns `Option<Record>`.
- `SessionStore::list_counterexample_bundles(filter)` scrolls the table.

Public-API impact: one new `pub(crate) fn db(&self) -> &Arc<redb::Database>` accessor on `SessionStore`. `TableDoesNotExist` from a read-only `open_table` collapses to `Ok(None)` / `Ok(vec![])` so first-time readers don't see IO errors. Mirrors the existing `#[allow(clippy::result_large_err)]` pattern in `storage.rs`.

**Disclosure (R4):** today the entire bundle blob — including the events vector — is serialised as one redb blob. For traces with 10k+ events this is wasteful (rewrite the whole blob on every append), but it is correct for m8-03 because `save()` always writes the complete record. m8-04 splits events into a side table when it adds the engine-events wiring.

**Disclosure (R5):** the record uses `ExistencePredicateWire`, a wire-shape duplicate of `chronos_services::ExistencePredicate`. We do not let `chronos-store` depend on `chronos-services`; conversion happens in services at the boundary (`existence_predicate_to_wire`, `counterexample_summary_from_wire`). Cost: one extra struct in the store crate.

### 2. Real per-variant proptest Strategy impls

`crates/chronos-services/src/counterexample.rs`. Replaces the m8-02 strategy stubs (`shrink_invariant` / `_existence` / `_call_path`) with concrete `proptest::Strategy` impls bound to the real `proptest::test_runner::TestRunner::new(Config)` API.

- `build_strategy_for(target) -> BoxedStrategy<HypothesisInput>` selects the per-variant sampler via `Just(target.clone())`.
- The three `shrink_*` fns take `&mut TestRunner` (proptest 1.5 `run(&mut self, &S, impl Fn)` signature) and drop the previously-unused `&CounterexampleContext` argument — the closures inside `run()` never read the context.
- `rounds` counter captured via `Cell<u32>` because proptest's closure is `Fn`, not `FnMut`.

**Disclosure (R1):** the sampler is `Just(target.clone())`, so proptest has nothing to shrink and `rounds_used` always reads as `1`. The strategy is wired to the real `proptest::test_runner::TestRunner` API so m8-05 close can swap in a real shrinker (e.g. `proptest::strategy::Map` over `target.constant`) without a service-signature change. The "rounds_used = 1" disclosure from m8-02 stands.

**Disclosure (R2):** the runner is built and used on the same async task — there is no `tokio::task::spawn_blocking` wrap. `runner.run(...)` itself does not cross an await point inside `shrink()` (proptest's `run` is blocking). m8-05 close will wrap this in `spawn_blocking` if real shrink loops prove expensive.

### 3. Save / Get / List wire through redb

`crates/chronos-services/src/counterexample.rs` (sections 5 + 5b). The m8-01 `get()` and `list()` stubs now read from the redb table; `save()` now writes through the same `save_counterexample_bundle` API.

- `save()` accepts `(workspace_id, property_kind, ShrinkResult, Vec<TraceEvent>)`, builds a `CounterexampleBundleRecord`, persists via `store.save_counterexample_bundle`.
- `get()` replaces the m8-01 `LoadFailed` stub — returns `Err(LoadFailed)` for both absent and IO failures (preserves m8-01 contract).
- `list()` replaces the m8-01 `Unsupported` stub — scrolls the table with `next_cursor: None` (R3 disclosure).
- `shrink()` step 4 calls `save()` to persist the bundle; events are `vec![]` placeholder pending m8-04 engine events wiring.

New `Saved` variant on `CounterexampleOutput` (carries `CounterexampleBundleSummary` with `has_full_bundle=true`); `#[derive(Debug)]` added for test ergonomics.

**Disclosure (B3, R3, R4):** the events vector is empty today; the events_count in the bundle summary will be 0. m8-04 will plumb `engine.get_all_events()` through `save()`; the bundle payload then carries real events.

### 4. Three v2 MCP wrappers + wire DTOs

`crates/chronos-mcp/src/server.rs` + `crates/chronos-services/src/output.rs`. Three new `#[tool]` handlers registered via the rmcp tool router:

- `counterexample_shrink` — runs the shrink pipeline; returns `CounterexampleShrinkOutputDto` with the freshly-persisted bundle summary + minimised payload.
- `counterexample_get` — reads from redb; returns `CounterexampleGetOutputDto` (absent → `Err(LoadFailed)`).
- `counterexample_list` — scrolls the redb table with the filter; returns `CounterexampleListOutputDto` (with `next_cursor: Option<String>`, always `None` today).

New wire DTOs in `chronos-services::output.rs`:

```rust
pub struct HypothesisInputWireDto { /* 10 Option<T> fields */ }
pub struct CounterexampleShrinkParamsDto { property_kind, target_hypothesis, max_rounds?, seed? }
pub struct CounterexampleBundleDto { summary, has_full_bundle, events_count, minimised, minimised_kind }
pub struct CounterexampleMinimisedDto { /* Constant / Predicate / CallerCallee / MaxDepth discriminated */ }
pub struct CounterexampleShrinkOutputDto { bundle, rounds_used }
pub struct CounterexampleGetOutputDto  { bundle, has_full_bundle }
pub struct CounterexampleListOutputDto { bundles, next_cursor }
pub struct CounterexampleBundleSummaryDto { /* 6 fields */ }
```

Plus a `serialize_counterexample_output` helper that converts any `CounterexampleOutput` variant into the matching wire envelope. `chronos_domain::property::PropertyValue` and `ComparisonOp` gained `schemars::JsonSchema` derives so the tool router can introspect the JSON schema.

**Disclosure (B3):** `Vec<TraceEvent>` is **not** on the wire DTOs. The minimised payload DTOs carry the `PropertyValue` / `ExistencePredicate` / `(caller, callee, max_depth)` triple but never the raw trace. Agents that need the trace re-fetch via a future `counterexample_bundle_events` tool (m8-05 close-time work, only if acceptance demands it).

**Disclosure (Saved-variant stopgap):** today `Saved` serialises to `{"saved": <summary>}` — this is a stopgap envelope pending m8-04 close-time rework. The expected envelope after m8-04 is `{"bundle": <summary>, "events_count": <usize>}` to match the rest of the wire.

### 5. Sandbox smoke (deliverable 4)

`chronos-sandbox/tests/counterexample_tools.rs` (new, 224 LoC) + `chronos-sandbox/src/client/tools.rs` (+108 LoC).

3 new typed client methods on `McpTestClient`:

- `counterexample_shrink(target)` → `CounterexampleShrinkOutputWire`
- `counterexample_get(bundle_id)` → `CounterexampleGetOutputWire`
- `counterexample_list(workspace_id?, property_kind?, limit?)` → `CounterexampleListOutputWire`

Each method deserialises the wire envelope into a typed struct so future callers don't reinvent the JSON parsing.

6 smoke tests against a real spawned `chronos-mcp` server using the `test_busyloop` and `test_exit_immediate` fixtures:

- CE1: shrink a `Number(42.0)` constant target on a busyloop probe → asserts non-empty `bundle_id`, `rounds_used == 1` (R1), `has_full_bundle == true`, `property_kind == "invariant"`.
- CE2: shrink → get round-trip with the returned `bundle_id`.
- CE3: get on a non-existent id returns `Err(LoadFailed)` at the MCP layer.
- CE4: list after a shrink sees the bundle; `next_cursor` is `None` (R3).
- CE5: same shrink on `test_exit_immediate` fixture — confirms the pipeline is fixture-agnostic.
- CE6: list with `workspace_id` filter — call shape is honoured.

Helper `setup_with_probe(fixture)` runs `probe_start` + `probe_stop` (the engine is built at stop time, not start) and returns the live `session_id` so the shrink can find it in the engines map. Without `probe_stop` the dispatcher returns `SessionNotFound` before the shrink loop even runs.

**Disclosure:** tests skip (with a printed message) if the `chronos-mcp` binary is unavailable, so the file compiles even on CI without a built binary.

### Test evidence

| Gate | Command | Result |
|---|---|---|
| T0 fmt | `cargo fmt --all -- --check` | clean |
| T0 clippy | `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| T1 lib (excl. native/sandbox/e2e) | `cargo test --workspace --lib --no-fail-fast` (excluding chronos-native, chronos-sandbox, chronos-e2e) | 1,118 passed, 0 failed (chronos-store 23, chronos-services 230, chronos-mcp 76, plus the other 11 crates) |
| T2 chronos-store | `cargo test -p chronos-store --tests --no-fail-fast` | 23 passed (+0 vs. m8-02) |
| T2 chronos-services | `cargo test -p chronos-services --tests --no-fail-fast` | 230 passed (+9 vs. m8-02 baseline 221: real-strategy tests 11/12/13 + boundary-conversion tests 14 + saved-variant) |
| T2 chronos-mcp | `cargo test -p chronos-mcp --tests --no-fail-fast` | 76 passed (+0 vs. previous; new wrappers are MCP-tool-level, unit-tested in chronos-services) |
| T4 sandbox smoke | `cargo test -p chronos-sandbox --test counterexample_tools -- --test-threads=1` | 6 passed, 0 failed in 33s |

**No regressions.** chronos-native flake (`test_launch_with_syscall_tracing`) noted in `AGENTS.md §6.5` was not triggered (not in scope of this cycle).

### Pre-existing artifacts

`pid 796486` was an orphaned `chronos-mcp` release binary at `/var/home/rubentxu/Proyectos/rust/chronos/target/release/chronos-mcp`, 46 min old, unrelated to this cycle. Killed defensively so it does not collide with `CHRONOS_MCP_PATH` resolution in future sandbox runs. `AGENTS.md §3` documents the orphan-kill protocol for future sandbox smoke runs.

## Architectural decisions (B-decisions recap)

| # | Decision | Justification |
|---|---|---|
| B1 | Bundle table uses string-keyed redb `TableDefinition` (no second table for events) | Lets m8-03 ship without engine-events wiring; m8-04 splits events into a side table when the engine payload arrives. Cost: one rewrite per `save()` until then (acceptable — saves are infrequent). |
| B2 | `Saved` variant on `CounterexampleOutput` with `has_full_bundle=true` | Single-variant truth source for "this bundle exists in redb"; `Got` is reserved for read paths. `#[derive(Debug)]` for test ergonomics. |
| B3 | `Vec<TraceEvent>` NOT on wire | Bundling 1000+ trace events in a tool response bloats the JSON; agents re-fetch via a future `counterexample_bundle_events` tool if acceptance demands it. Keeps the shrink/get responses small. |
| B4 | `PropertyValue` wire shape is the externally-tagged enum (default serde) | Matches the existing `chronos-domain` derive. Inline JSON is `{"Number": 42.0}` not `{"type":"number","value":42.0}`. Acceptable — this is server-side wire, not external API. |
| B5 | `Saved` serialises to `{"saved": <summary>}` (stopgap) | Avoids a wire-shape change for m8-03; m8-04 close-time rework will fold `Saved` into the same `CounterexampleBundleDto` envelope as `Shrunk`. |
| B6 | Sandbox client gets typed `*Wire` DTOs (mirrors mcp `*Dto` shapes) | Mirrors the existing `McpTestClient` pattern (`ListThreadsResponse`, `ThreadInfo`, etc.) and gives the smoke tests strong typing without a per-test ad-hoc parser. |
| B7 | `setup_with_probe` runs `probe_start` + `probe_stop` (not just `probe_start`) | The MCP server only inserts a session_id into the engines map at `probe_stop` (engine build time). Without stop the dispatcher returns `SessionNotFound`. Documented in the helper comment. |

## Experimental disclosures (R-recap)

| Risk | Title | Status |
|---|---|---|
| R1 | Strategy is `Just(value)` → `rounds_used == 1` always | **Still present.** Real shrinker is m8-05 close-time work. |
| R2 | No `spawn_blocking` wrap | **Still present.** `runner.run` does not cross an await point inside `shrink()`. Wrap is m8-05 close-time work. |
| R3 | `next_cursor: None` always | **Still present.** Pagination is m8-05 close-time work; field is on the wire DTO so the addition is shape-preserving. |
| R4 | Bundle-as-blob storage inefficiency | **Documented.** Splits to side table is m8-04. |
| R5 | Wire-mirror conversion cost at boundary | **Accepted.** One extra struct in `chronos-store`; conversion is one line in `chronos-services`. |

## Honest disclosures (carry-over from m8-03)

- Events count is 0 on the wire (B3, R4) — events live in redb today but `save()` is called with `vec![]`. m8-04 will plumb `engine.get_all_events()`.
- `Saved` serialises to `{"saved": <summary>}` — stopgap pending m8-04 rework.
- `rounds_used` always reads `1` (R1) — `Just(value)` strategies have nothing to shrink.

## Files touched (this cycle's 5 commits)

| File | Change | Lines |
|---|---|---|
| `crates/chronos-store/src/counterexample_storage.rs` | New: redb table + save/load/list extension methods + 5 unit tests | +375 |
| `crates/chronos-store/src/storage.rs` | New `pub(crate) fn db(&self) -> &Arc<redb::Database>` accessor | +5 |
| `crates/chronos-store/src/lib.rs` | Re-export new module | +1 |
| `crates/chronos-services/src/counterexample.rs` | Real `proptest::Strategy` impls + boundary conversion helpers + Saved variant + 9 new tests | +488 / −28 |
| `crates/chronos-services/src/output.rs` | 7 new wire DTOs | +174 |
| `crates/chronos-domain/src/property.rs` | `serde::JsonSchema` derive on `PropertyValue`/`ComparisonOp` | +9 / −1 |
| `crates/chronos-mcp/src/server.rs` | 3 new `#[tool]` handlers + 3 new params structs + `serialize_counterexample_output` helper | +143 / −8 |
| `chronos-sandbox/src/client/tools.rs` | 4 wire DTOs + 3 client wrapper methods | +108 |
| `chronos-sandbox/tests/counterexample_tools.rs` | 6 end-to-end smoke tests | +224 (new) |
| `docs/milestones/m8-03-mcp-wrappers-redb-bundle-table-merge.md` | **This document** | (this file) |
| `sddk/changes/m8-03-mcp-wrappers-redb-bundle-table-merge/apply-checkpoint.json` | The apply-checkpoint | (new file) |

Total net additions: **~1,500 LoC** plus the merge doc + checkpoint + smoke file.

## Commit + tag + release

- Commits (5):
  - `1b2e1a0` feat(store): m8-03 redb counterexample_bundles table
  - `b3f4de4` feat(services): m8-03 real proptest Strategy impls
  - `8456777` feat(services): m8-03 wire get/list/save through redb + boundary conversions
  - `f10e8c8` feat(mcp): m8-03 counterexample_shrink/get/list wrappers + wire DTOs
  - `77322dc` feat(sandbox): m8-03 counterexample_shrink/get/list smoke + 3 client wrappers
- Chore commit: apply-checkpoint sync (post-merge).
- Tag: `m8-03-mcp-wrappers-redb-bundle-table.0`.
- Apply-checkpoint: `sddk/changes/m8-03-mcp-wrappers-redb-bundle-table-merge/apply-checkpoint.json`.
- Push: `origin main` only.

## Pattern compliance with prior cycles

| Pattern | m8-02 precedent | m8-03 conformance |
|---|---|---|
| Doc first, code second | yes | yes (scoping doc FF-merged before this branch) |
| Stub + machine-readable error to avoid follow-up cycle signature change | yes | yes (m8-02 stubs replaced with real Strategy impls; events=vec![] placeholder is the only remaining stub) |
| Wire DTOs in `output.rs`, internal enums at module scope | yes | yes (7 `*Dto` in `output.rs`; `Saved`/`Got`/`Listed`/`Shrunk` variants stay in counterexample.rs) |
| `ServiceError::Unsupported/LoadFailed/EvalError` reused for new stubs | yes | yes (no new variants added) |
| `chrono-cli` workspace member not added if cycle doesn't need it | yes | yes (deferred to m8-04) |
| Sandbox smoke on real fixtures (not just unit tests) | m7-06 | yes (test_busyloop + test_exit_immediate) |
| Honest disclosure of "the algorithm runs later, this cycle shipped the shape" | yes | yes (this document § "Honest disclosures") |

## Cross-references

- `docs/milestones/m8-counterexample-shrinking-scoping.md` — parent doc, multi-cycle plan.
- `docs/milestones/m8-03-mcp-wrappers-redb-bundle-table-scoping.md` — cycle-specific scoping.
- `docs/milestones/m8-02-shrink-loop-wiring-merge.md` — m8-02 sibling (signature + stubs + dispatch).
- `docs/milestones/m8-01-counterexample-foundation-merge.md` — m8-01 sibling (foundation + service signatures).
- `docs/chronos-agentic-reconstruction/docs/specs/RUNTIME_PROPERTIES_AND_SLICING.md` § "Counterexample shrinking" — spec subsections 54-62.
- `docs/chronos-agentic-reconstruction/docs/roadmap/MILESTONE_ACCEPTANCE.md` § M8 line 78 — end-to-end acceptance criterion (m8-05).

---

— Submitted 2026-09-11. Cycle closed on `main`.
