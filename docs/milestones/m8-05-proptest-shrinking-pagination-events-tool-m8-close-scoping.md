# m8-05 — real proptest shrinking + pagination + events tool + M8 close scoping

**Branch:** `feat/m8-05-proptest-shrinking-pagination-events-tool-m8-close`
**Cycle:** M8 §5, fifth (and final) execute cycle of the M8 multi-cycle plan.
**Predecessor doc:** `docs/milestones/m8-counterexample-shrinking-scoping.md` (M8 master plan). `m8-04-chronos-cli-engine-events-saved-rework-scoping.md` lists the four work items this cycle ships. All M8 execute cycles m8-01..m8-04 are merged.
**Status:** SCOPING — 2026-09-11

## Why this cycle

m8-01..m8-04 shipped the **plumbing**: services dispatcher, MCP wrappers, redb bundle table, `chronos-cli` binary, engine-events wiring, and the Saved-variant rework. But three items from the m8-04 scoping doc remain on the deferred list, and the M8 close report still needs to be written. m8-05 ships the four outstanding items in one cycle:

1. **Real per-variant proptest shrinking** (closes m8-03 R1, m8-04 deferred).
2. **Pagination on `list_counterexample_bundles`** (closes m8-03 commit msg note "m8-05 close-time work").
3. **`counterexample_events_count` tool exposure** (closes m8-04 deferred — the field is on the wire envelope but has no dedicated accessor).
4. **M8 close report** (`docs/milestones/M8-CLOSE.md`) — milestone-level acceptance run against a known-fixture failing input, following the m5-close / m6-close / m7-close precedent.

This is the final M8 cycle. After it ships, M8 is officially closed and the project moves to M9 (chronos-roadmap M9, which is out of M8 scope).

## What's already in place (foundations m8-05 builds on)

- **`chronos-services::counterexample`** (m8-01..m8-04, ~1300 LoC): `shrink`, `get`, `list`, `save`, `pull_engine_events`. Per-variant shrinkers (`shrink_invariant`, `shrink_existence`, `shrink_call_path`) all return `Just(target.clone())` — the M8-03 R1 stopgap.
- **`chronos-store::counterexample_storage`** (m8-03): `counterexample_bundles` redb table, `save_counterexample_bundle`, `load_counterexample_bundle`, `list_counterexample_bundles`. `list_counterexample_bundles` returns `Vec<CounterexampleBundleSummary>` (no cursor).
- **`chronos-mcp::server`** (m8-03..m8-04): `counterexample_shrink`, `counterexample_get`, `counterexample_list` tools with `serialize_counterexample_output` envelope. `Saved` and `Shrunk` now carry `events_count: usize` (m8-04 rework).
- **`chronos-cli`** (m8-04): `chronos test replay <bundle_id>` (working), `chronos test run` (m9+ stub).
- **M8 master plan** (`docs/milestones/m8-counterexample-shrinking-scoping.md`): the five roadmap work items + 6th ("bundle artifact persistence") are all wired.

## Architectural decisions

### B1 — Strategy shape per variant (real shrinking)

Today every per-variant shrinker uses `Just(target.clone())` — exactly one sample, no shrinkage. m8-05 replaces each with a real `proptest::strategy::Strategy` that produces smaller variants of the captured hypothesis while preserving the kind+session invariants.

- **Invariant**: shrink `target.constant` (`PropertyValue`). `Number` shrinks toward `0.0`; `Bool` shrinks toward `false`; `Text` shrinks via proptest's `String` strategy (lexicographic). `scope` and `comparison` are kept as-is (they are configuration, not inputs).
- **Existence**: shrink `target.predicate`. `EventTypeEquals { event_type }` shrinks the string toward `""`. `ThreadEquals { thread_id }` shrinks toward `0`. `PropertyKeyEquals { target }` shrinks toward `""`. (The variant itself is fixed — we don't switch to a different predicate kind.)
- **CallPath**: shrink `target.caller` and `target.callee` toward `""`; `max_depth` shrinks toward `Some(1)` then `None`. (The pair is fixed — we don't invent a different frame.)

The shrinker uses `proptest::strategy::Strategy::boxed()` to wrap the typed `HypothesisInput`. Each shrinker returns a tuple `(rounds_used, minimised_constant, minimised_predicate, minimised_call_path)` matching the m8-03 `ShrinkResult` shape. The `rounds_used` counter now reflects the **actual** proptest iteration count, not the `Just(value)`-forced 1.

**Disclosure (m8-05 R1):** shrinkage may not converge to a single value for all hypothesis shapes — e.g. an Invariant with `scope = LatencyMs` may require a non-zero `PropertyValue::Number`. We follow proptest's `Config::with_cases(64)` / `Config::fork()` defaults and surface any non-convergence as a `ServiceError::EvalError` rather than infinite-looping.

### B2 — Pagination cursor

Add a cursor to `list_counterexample_bundles`. The cursor is the **bundle_id of the last row returned** in the previous page (a string token). The store returns `(Vec<CounterexampleBundleSummary>, Option<String>)` where `next_cursor: Some(bundle_id)` indicates more pages exist.

Why bundle_id as cursor: bundle_id is a uuid::v7 (m8-02), so lexicographic ordering on bundle_id matches chronological order. Using the same key as the cursor means we don't need a separate index — the existing iter is sufficient.

Why not offset-based pagination: redb has no native offset; an offset would require either scanning N+limit rows (O(N) per page) or maintaining a secondary index. bundle_id-as-cursor is O(limit) per page.

**Disclosure (m8-05 R2):** cursors are opaque strings in the wire envelope but are **bundle_ids** under the hood. Documenting this so callers don't try to "decode" them — they just round-trip the value back into the next call.

### B3 — `counterexample_events_count` tool

New MCP tool that returns the `events_count` of a persisted bundle without re-emitting the full summary.

```rust
#[tool(name = "counterexample_events_count", description = "...")]
async fn counterexample_events_count(
    &self,
    params: Parameters<CounterexampleEventsCountParams>,
) -> Result<CallToolResult, rmcp::ErrorData>
```

The wire envelope:

```json
{ "bundle_id": "cb-...", "events_count": 42 }
```

Why a dedicated tool: today `events_count` only surfaces on the Saved/Shrunk envelopes. After a session is stopped, callers (LLM agents) often want to know "how many events does this bundle carry?" without re-fetching the full bundle. A dedicated tool saves the LLM one round-trip and one bundle-deserialize on the wire.

**Disclosure (m8-05 R3):** `events_count` is read-only; the tool does not take a snapshot of the live engine. The count is the one persisted at `save()` time (m8-04 m9+ shortcut may differ — see m8-04 deferred list).

### B4 — M8 close report

`docs/milestones/M8-CLOSE.md` (~150-200 LoC) following the m7-close-report.md precedent. Sections:

1. **Cycle shape** — five execute cycles (m8-01..m8-05), one master scoping doc.
2. **What shipped** — per-cycle summary with commit SHAs and line counts.
3. **Roadmap acceptance check** — `MILESTONE_ACCEPTANCE.md` § M8 acceptance criterion: "A generated failing input shrinks while preserving the same property violation." Verified against a known-fixture failing input (the m7-07 `test_busyloop` busy-loop fixture re-used; it produces a multi-event trace where the Invariant hypothesis can be shrunk).
4. **Disclosure index** — R1 (Just(value) strategies, closed by m8-05 R1), R2 (no spawn_blocking, still open — see below), R3 (next_cursor=None, closed by m8-05 R2), R4 (bundle-as-blob → side table, deferred to m9+), R5 (wire-mirror ExistencePredicateWire, still open), and the m8-04 additions: R-run-stub, R-in-memory-engines, R-tilde-expansion, R-hypothesis-reconstruction-fidelity.
5. **Test counts** — `counterexample_tools` (7 smoke), `chronos-cli` (18 lib), `chronos-services` lib (~232 + m8-05 additions), `chronos-store` lib (~23), plus the 16 m8-05 unit tests (see § Test plan below).
6. **Outstanding m9+ backlog** — R4 (bundle split), persisting original target_hypothesis (closes R-hypothesis-reconstruction-fidelity), `chronos test run` live-probe plumbing, R2 spawn_blocking.

**Disclosure (m8-05 R4):** R2 (no spawn_blocking wrap) was carried over from m8-03 and is NOT closed by m8-05. The m8-05 strategy runs on the test-runner thread inside `shrink()`; if a future strategy adapter blocks on I/O, the dispatcher will need a `tokio::task::spawn_blocking` wrap. m9+.

## Concrete API surface (additions only)

### Store

```rust
impl SessionStore {
    pub fn list_counterexample_bundles_page(
        &self,
        filter: CounterexampleBundleFilter<'_>,
        cursor: Option<&str>,
    ) -> Result<(Vec<CounterexampleBundleSummary>, Option<String>), StoreError>;
}
```

The existing `list_counterexample_bundles(filter)` is kept as a thin wrapper that calls `list_counterexample_bundles_page(filter, None)` and discards the cursor — backward-compatible with m8-03 sandbox smoke that doesn't pass a cursor.

### Services

```rust
pub struct CounterexampleListFilter { /* existing */ cursor: Option<String> }

pub enum CounterexampleOutput {
    /* existing variants */
    EventsCount { bundle_id: String, events_count: usize },
}

impl ChronosCounterexampleService {
    pub fn events_count(&self, ctx: &CounterexampleContext<'_>, bundle_id: &str)
        -> Result<CounterexampleOutput, ServiceError>;
}
```

### Wire DTOs

```rust
pub struct CounterexampleListOutputDto {
    pub bundles: Vec<CounterexampleBundleSummaryDto>,
    pub next_cursor: Option<String>,  // was always None in m8-03; now sometimes Some.
}

pub struct CounterexampleEventsCountOutputDto {
    pub bundle_id: String,
    pub events_count: usize,
}

pub struct CounterexampleEventsCountParams { pub bundle_id: String }
```

### MCP tool

```rust
#[tool(name = "counterexample_events_count", description = "...")]
async fn counterexample_events_count(
    &self,
    params: Parameters<CounterexampleEventsCountParams>,
) -> Result<CallToolResult, rmcp::ErrorData>;
```

## Test plan

| Bucket | Count | Notes |
|---|---|---|
| chronos-store lib (pagination) | +4 | cursor=None returns first page; cursor=Some(last_id) skips to next page; filter+cursor combined; empty cursor mismatch returns empty + next_cursor=None. |
| chronos-services lib (real shrinking) | +6 | invariant_constant_shrinks_toward_zero, existence_predicate_string_shrinks, existence_predicate_thread_shrinks, call_path_callee_shrinks, max_depth_shrinks, rounds_used_now_reflects_proptest_iterations. |
| chronos-services lib (events_count) | +2 | returns count for known bundle, returns NotFound for unknown. |
| chronos-mcp integration (events_count wire) | +1 | smoke test serialise → wire → deserialize → assert. |
| chronos-sandbox (counterexample_tools, pagination) | +3 | ce8_first_page_cursor_works, ce9_second_page_via_cursor, ce10_pagination_exhausts_with_none. |
| chronos-sandbox (counterexample_tools, events_count) | +1 | ce11_events_count_tool_returns_n. |
| **Total m8-05 tests** | **+17** | All in T2 / T4-smoke tiers. |

T4-smoke subset per AGENTS.md §2: `counterexample_tools` (the only suite touching the wire envelope for the new pagination + events_count paths). Optionally `e2e_connectivity` if any MCP plumbing changes affect the start/respond round-trip (they don't, so we skip it).

## Risks and known limitations

- **Strategy shape uncertainty.** Per-variant shrinkers may not shrink toward the "intuitive" minimum (e.g. an Invariant with `comparison = GreaterEqual` should not shrink toward `0.0` if that would *create* a false-positive violation; the shrinker needs to respect the comparison direction). We document each variant's shrinker in inline doc comments and add unit tests for the three comparison directions.
- **Cursor is opaque.** Operators / LLM agents must not interpret the cursor as anything other than a token to round-trip. This is enforced by the wire schema (`Option<String>`) and documented in the tool description.
- **`events_count` is a snapshot.** It does not reflect the current state of the live engine. If a session is still running and the bundle has not been re-saved, the count reflects the bundle's persist-time state, not the live one.
- **M8 close report acceptance run.** The acceptance fixture must be deterministic — we use `test_busyloop` (m7-07) which produces a fixed multi-event trace. If a future fixture change breaks determinism, the acceptance run breaks. We pin the fixture hash in the close report.
- **R2 (no spawn_blocking) carries over.** The m8-05 strategy is still synchronous. If a future cycle wraps it in `spawn_blocking`, that's m9+.

## Out of scope (explicit non-goals)

- **R4 (bundle-as-blob → side table for events).** Deferred from m8-04 explicitly. m9+ scope. m8-05 does not touch the bundle storage format.
- **Persisting original `target_hypothesis` in the bundle.** Deferred from m8-04 (R-hypothesis-reconstruction-fidelity). m9+ scope.
- **`chronos test run` live-probe plumbing.** Deferred from m8-04. m9+ scope.
- **Cross-milestone plumbing (e.g. M9 live probes calling M8 shrinker).** M9 scope.
- **New test verb coverage for Python / JVM / JS.** m8-05 inherits the m8-04 scope (Go + Rust only via the `chronos test replay` path).

## Cross-references

- `docs/milestones/m8-counterexample-shrinking-scoping.md` — M8 master plan, this doc is the m8-05 cycle scoping.
- `docs/milestones/m8-01-counterexample-foundation-scoping.md`..`m8-04-chronos-cli-engine-events-saved-rework-scoping.md` — preceding execute cycle scopings.
- `docs/milestones/m8-04-chronos-cli-engine-events-saved-rework-merge.md` § "Out-of-scope (deferred)" — source of the four m8-05 work items.
- `docs/milestones/m7-close-report.md` — M7 close report; structural precedent for `M8-CLOSE.md`.
- `docs/chronos-agentic-reconstruction/docs/roadmap/MILESTONE_ACCEPTANCE.md` § M8 line 78 — acceptance criterion source.

---

— Submitted 2026-09-11. Awaits m8-05 execute cycle kickoff.
