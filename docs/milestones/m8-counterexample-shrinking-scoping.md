# M8 — Counterexample shrinking and test intelligence scoping

**Branch:** `feat/m8-counterexample-shrinking-scoping`
**Cycle:** M8 (counterexample shrinking + test intelligence) — first scope
**Precedence:** `docs/chronos-agentic-reconstruction/docs/roadmap/ROADMAP.md` line 179 (M8); `docs/chronos-agentic-reconstruction/docs/specs/RUNTIME_PROPERTIES_AND_SLICING.md` § "Counterexample shrinking"; `docs/milestones/m6-04-hypothesis-test.md` (closest structural precedent); `docs/chronos-agentic-reconstruction/docs/roadmap/MILESTONE_ACCEPTANCE.md` § M8 line 78

**Status:** SCOPING — 2026-09-11

> **Important naming note** (honest disclosure): the M7 close report
> (`docs/milestones/m7-close-report.md` § 7) listed an m8 handoff of
> "live probe performance + MCP hardening + distributed tracing". Those
> items do NOT match the roadmap M8 (counterexample shrinking). This
> scoping doc honors the **roadmap** M8 and re-books the m7-close handoff
> items as post-M8 backlog. See § "Naming collision" below.

## Why this cycle

The roadmap M8 is the **test intelligence** milestone. Its acceptance criterion (`MILESTONE_ACCEPTANCE.md` § M8) is precise: "A generated failing input shrinks while preserving the same property violation." M6 closed with the **runtime property** foundation (`hypothesis_test` v2 tool, dispatcher pattern established in `m6-04`). M8 builds on that foundation: given a property **VIOLATION** from `hypothesis_test`, M8 shrinks the violating input and persists the minimal failing input plus its minimal causal slice as durable evidence.

The roadmap M8 lists five work items:

1. proptest/Hypothesis integration contracts.
2. input/counterexample artifact.
3. rerun loop.
4. causal slice minimization.
5. `chronos test` experiment for Go/Rust.

This scoping doc proposes a concrete deliverable shape (modules, DTOs, dispatcher, MCP wrappers, CLI subcommand) and a multi-cycle execution plan. The doc is intentionally bounded: it does NOT spawn code in this cycle.

## What's already in place (foundations M8 builds on)

- **`chronos-services::hypothesis_test`** (m6-04, 1023 LoC): `ChronosHypothesisTestService::test(ctx, input) -> Result<HypothesisOutput>`. Dispatcher pattern established. `HypothesisOutput.verdict` already supports `VIOLATION`.
- **`chronos-services::trace_slice`** (m3-03): `trace_slice` v2 tool computes minimal causal slices backwards from a triggering event. M8 reuses this algorithm as the "causal slice minimization" step.
- **`chronos-services::session_export`** (m6-05): `ChronosSessionExportService` handles session-export persistence. M8 may reuse its `Workspace + (manifest, events, slice) triples` pattern.
- **`chronos-store::SessionStore`** (m0-04): redb-backed session save/load. M8 stores the minimal reproduction bundle under a new `counterexample_bundle` table (or as a session — see § Architectural decision A2).
- **`AGENT_API_V2.md`** `tools[hypothesis_test]` already in the v2 spec; M8's `counterexample_shrink` + `counterexample_get` tools join it.
- **`chronos-domain::property`**: typed property shapes (`Invariant { scope, comparison, value }`); M8 shrinks the inputs that violate these shapes via proptest-style strategies.

## Naming collision (m7-close handoff vs roadmap M8)

The m7-06 close report § 7 lists an m8 handoff of:

- `attach` domain API (m7+ followup, m7-09 candidate);
- Single-call `session_stop` (shipped in m7-07);
- Live probe performance (post-M7 backlog);
- Distributed tracing (m8+ soft book);
- MCP server hardening (post-M7 backlog).

These are real work items but they do **not** belong to roadmap M8 (counterexample shrinking). This scoping doc re-books them as:

- **`attach` domain API** → ticket `m7-09-attach` (m7+ scope, was the original m7-04 followup; not in M8). Targeted cycle.
- **Live probe performance** → ticket `m10.5-multi-process-fanout` (M10.5 = post-M10 milestone; tied to Execution Explorer timeline).
- **Distributed tracing** → ticket `m9.5-otel-export` (M9.5 = extension of M9 concurrency intelligence; depends on the OTel correlation work M6 already shipped).
- **MCP server hardening** → ticket `m11.5-mcp-observability` (post-M11; can be a smaller B-direct cycle once `chronos-mcp` gets an observability spike in `chrono_log`).

These re-books go in `sddk/bookmarks/` as standalone JSON pointers (no spec, just hints) after this scoping cycle. They are explicitly **not** part of M8's execute plan.

## Scope (this cycle)

This is the SCOPING cycle for M8. It defines:

### 1. `counterexample_*` v2 tool surface (M8-P1 through M8-P3)

Add three v2 tools to the `AGENT_API_V2.md` v2 surface:

- **`counterexample_shrink`** (`counterexample_shrink(ctx, input)`):
  - Input carries: `kind: HypothesisKind` (reuses existing enum from m6-04), `target_hypothesis: HypothesisInput`, `max_rounds: u32` (default 64, matches proptest's default), `seed: Option<u64>` (deterministic rerun).
  - Output: `{ shrunk_input: HypothesisInput, original_violation: PropertyVerdict, shrunk_violation: PropertyVerdict, rounds_used: u32, causal_slice: CausalSliceRef, bundle_id: String }`.
  - Behaviour: drives proptest-style shrinking (delta-first via proptest's `Shrink` trait) and reruns `ChronosHypothesisTestService::test` after each candidate. Stops when (a) violation persists AND round budget is exhausted, OR (b) violation disappears at this round (accept the parent round's input as the shrink boundary — mirrors proptest's behaviour for non-shrinkable violations).
- **`counterexample_get`** (`counterexample_get(bundle_id)`):
  - Read-only retrieval of a persisted bundle. Output: full bundle (input + causal slice + provenance + timestamps).
- **`counterexample_list`** (`counterexample_list(filter?)`):
  - Lists bundle IDs by property_kind + workspace_id + time window. Used by `chronos test` to drive the rerun loop.

The three tools are dispatcher-shaped: services-layer module `counterexample` (single dispatcher with 4 entrypoints: `shrink`, `get`, `list`, plus a shared `_persist` helper).

### 2. `chronos test` experiment (M8-P4)

Add a new workspace member `crates/chronos-cli/` with two subcommands today:

- `chronos test run <property.kdl> <target-binary>` — runs a single-property execution on a Go or Rust binary. Returns the hypothesis_test verdict + a `bundle_id` on VIOLATION.
- `chronos test replay <bundle_id>` — re-runs the bundle's stored minimal input through the dispatcher. Determinism guaranteed by the recorded `seed`.

The CLI is intentionally minimal (≤ 200 LoC) — it is a **driver**, not a new algorithm. All synthesis + shrinking happens in `chronos-services::counterexample`. The CLI is the UAT harness for `MILESTONE_ACCEPTANCE.md` § M8.

CLI args format: KDL for property description (already used by the rest of the project; see `docs/chronos-agentic-reconstruction/docs/specs/` for the property KDL grammar). The CLI does NOT extend KDL — it consumes the existing grammar.

### 3. Rerun loop contract (M8-P5)

The M8 acceptance criterion is "shrunken input **preserves** the property violation". The rerun loop is the substrate: after each shrink candidate, rerun `hypothesis_test` and check `verdict == VIOLATION` for the same `original_violation` semantic check. The loop terminates when the shrink can't go further OR the violation flips to PASS / UNSUPPORTED.

Three implementation notes:

- The `seed` field on `CounterexampleShrinkInput` is **mandatory** for determinism — proptest is seeded by the dispatcher (default seed = now-EpochMillis; passable for `chrono test replay`).
- The dispatcher's shrinker must **fail loud** on proptest panics (via `proptest::test_runner::TestRunner::run`) — proptest is wrapped, not reimplemented (per the roadmap line 187 directive: "Integrate rather than reimplement").
- Causal slice minimization happens after shrink — i.e., we first find the smallest input that still violates, then run `trace_slice` with the violation event ID to get the minimal evidence set. This is "shrink-then-slice", not "slice-then-shrink".

### 4. `CounterexampleBundle` artifact (M8-P6)

A bundle is a self-contained reproduction artifact (input + causal slice + provenance + seed + timestamps). Persisted in `chronos-store` under a new `counterexample_bundles` redb table (key: `bundle_id: String`, value: bincode-serialized `CounterexampleBundle`). The bundle ID is `format!("cb-{epoch_ms}-{rand4}")` (deterministic, sortable, unique). The bundle format is intentionally simple — JSON for inspection, bincode for storage efficiency.

## Architectural decisions (with rationale)

- **A1: proptest dependency.** Add `proptest = "1.5"` to `chronos-services`. Cargo workspace dependency. proptest is the de facto Rust shrinkage library, used by std-lib-style crates (serde, http, tokio). Adding it here is the project's first `dev-dep` in services; `proptest` is **not** used at runtime by other crates — only by `chronos-services::counterexample`. The `proptest` dep stays out of MCP and domain.
- **A2: bundles table vs session table.** Persist bundles in a new `counterexample_bundles` table, not as a session. Rationale: bundles have a much shorter lifecycle (a single shrinking session) than sessions (live → stopped → persisted → queried). Sessions are also keyed by `session_id` (a UUID); bundles by `bundle_id` (epoch-ms+rand). Different IDs, different lifecycles, different GC semantics. The M8 cycle adds the bundle table; m11+ may add a GC sweep if disk pressure becomes a concern.
- **A3: dispatcher module name.** `chronos_services::counterexample` (single module, three entrypoints: `shrink`, `get`, `list`). Follows the m6-04 `hypothesis_test` / m6-05 `session_export` precedent. Module size estimate: ~600-800 LoC + ~150-200 LoC unit tests.
- **A4: CLI as a thin driver.** `crates/chronos-cli/` is ≤ 200 LoC today; it is NOT a reimplementation of the dispatcher. It calls into `chronos-services` (services crate is in the workspace; CLI has it as a dep). The CLI can be expanded later (e.g., `chronos repl`, `chronos inspect`) — for M8 it's just the test driver.
- **A5: causal slice reuse from m3-03.** The "causal slice minimization" step (M8 work item 4) is not new code — it's a call into `chronos_services::trace_slice` with the violation's triggering event ID. M8 adds the glue (capture the violation event ID from `HypothesisOutput.violation_event_id` and pass it through). No new algorithm.
- **A6: 5 work items → 6 plan items.** The roadmap lists 5 work items. This scoping adds item 6 ("bundle artifact persistence") because it's the connective tissue the items need. The execute cycles may split or merge as it progresses.

## Concrete API surface (no inventions)

The dispatcher shape:

```rust
pub struct CounterexampleContext<'a> {
    pub store: &'a chronos_store::SessionStore,
    pub query_engine: &'a chronos_query::QueryEngine,
    pub session_id: String,
}

pub enum CounterexampleShrinkInput {
    Shrink {
        property_kind: HypothesisKind,
        target_hypothesis: HypothesisInput,
        max_rounds: u32,             // default 64
        seed: Option<u64>,           // deterministic rerun
    },
    Get { bundle_id: String },
    List {
        workspace_id: Option<String>,
        property_kind: Option<HypothesisKind>,
        since_ms: Option<u64>,
        until_ms: Option<u64>,
        limit: u32,
    },
}

pub enum CounterexampleOutput {
    Shrunk {
        bundle_id: String,
        original_violation: PropertyVerdict,
        shrunk_violation: PropertyVerdict,
        rounds_used: u32,
        causal_slice: CausalSliceRef,
        shrunk_input_summary: String,
    },
    Got { bundle: CounterexampleBundle },
    Listed { bundles: Vec<BundleSummary> },
}

impl ChronosCounterexampleService {
    pub fn shrink(&self, ctx: &CounterexampleContext<'_>, input: CounterexampleShrinkInput) -> Result<CounterexampleOutput, ServiceError> { /* proptest loop + trace_slice */ }
    pub fn get(&self, ctx: &CounterexampleContext<'_>, bundle_id: &str) -> Result<CounterexampleOutput, ServiceError> { /* redb read */ }
    pub fn list(&self, ctx: &CounterexampleContext<'_>, filter: ...) -> Result<CounterexampleOutput, ServiceError> { /* redb scan */ }
}
```

`CounterexampleBundle` is **not** in `output.rs` (it carries `Vec<TraceEvent>` like `SessionStopPersistence` m7-07 — internal to services). The MCP wrapper serialises a "bundle summary" (no events) to JSON; full bundles are read via `chronos-cli replay` or in-process callers.

`proptest::test_runner::TestRunner::run` is the engine; the dispatcher wraps it and converts `TestFailure::Property` (proptest error) into `PropertyVerdict::Violation`. `proptest::strategy::Strategy::boxed()` is used to wrap the typed `HypothesisInput` into a `proptest::ValueTree` for shrinking (this is the experimental bit — see § Risks).

## Test plan (planned, multi-cycle)

This is a multi-cycle milestone, not a single execute cycle. Each execute cycle will have its own scoping doc. The plan:

- **m8-01**: M8 foundation — services layer, `CounterexampleContext`, the three DTOs, `proptest` dep, 4-5 unit tests using the existing `hypothesis_test` mock. No MCP wrappers yet. Bounded (A-min). Smoke: unit tests only.
- **m8-02**: `proptest` shrink loop wiring. Includes the experimental conversion (`HypothesisInput → proptest::Strategy`). Unit tests + an integration test that drives a known buggy fixture through shrink and asserts the bundle persists. A-lite.
- **m8-03**: `counterexample_*` MCP wrappers + redb bundle table. Wrappers only; CLI not yet. T4 smoke on the same 3 sandbox suites as M7. A-lite.
- **m8-04**: `chronos test` CLI. Adds the workspace member. No new algorithms. CLI smoke (drives a Go + Rust fixture through shrink end-to-end). A-min.
- **m8-05 (close)**: M8 close report + final acceptance run. `MILESTONE_ACCEPTANCE.md` § M8 entry validated against a known-fixture failing input. A-lite.

The plan above reuses the existing fixture generation in `chronos-sandbox/programs/c/` if a known-failing fixture is needed. A new fixture (`test_property_violation.c`) may be added in m8-05 close if acceptance testing requires one.

## Risks and known limitations

- **`HypothesisInput → proptest::Strategy` conversion is experimental.** The M8 spec says "integrate rather than reimplement" but the actual conversion path is not obvious: `HypothesisInput` is an enum of typed shapes, not a flat `Vec<u8>`. The strategy adapter needs to dispatch on the variant and produce a `proptest::Strategy` for each. The first execute cycle (m8-02) may discover this requires a custom `Strategy` impl per variant — boundary case: `Existence { match_predicate }` shrinks the predicate complexity; `Invariant { scope, comparison, value }` shrinks the value range. Acceptable as long as we don't add proptest to chronos-domain (per A1).
- **No v1 deprecation.** Counterexample tools are net-new; no v1 names exist. The deprecation-bookkeeping overhead from M7 does not apply.
- **No MCP wrappers until m8-03.** m8-01/m8-02 work lives in services-only. This is intentional: services-first lets the algorithm stabilise before the wire contract.
- **Bundles are not GC'd.** A successful m8-05 close may warrant a followup to GC bundles older than N days (M11+ backlog).
- **CLI is Go/Rust only for M8.** Python / JVM / JS reuse the existing adapter layers but don't get new test verb coverage. M11 may extend — out of M8 scope.
- **`chronos-cli` is a brand-new crate.** First entry to the workspace in 2026. The module structure (single binary `chronos`) sets a precedent for `chronos repl` / `chronos inspect` etc.
- **Existing pre-existing flake (`chronos-native::ptrace_tracer::tests::test_launch_with_syscall_tracing`, `AGENTS.md` § 6.5)** still applies; M8 smoke excludes chronos-native lib tests.

## Cross-references

* `docs/chronos-agentic-reconstruction/docs/roadmap/ROADMAP.md` line 179 — M8 source.
* `docs/chronos-agentic-reconstruction/docs/roadmap/MILESTONE_ACCEPTANCE.md` § M8 line 78 — acceptance criterion: "A generated failing input shrinks while preserving the same property violation."
* `docs/chronos-agentic-reconstruction/docs/specs/RUNTIME_PROPERTIES_AND_SLICING.md` § "Counterexample shrinking" — spec subsection.
* `docs/milestones/m6-04-hypothesis-test.md` — closest existing structural precedent (single dispatcher + v2 tool wrapper).
* `docs/milestones/m3-runtime-properties-scoping.md` — `trace_slice` v2 dispatcher lives in this M3 scope; used for the causal slice minimization step.
* `docs/milestones/m6-05-session-export.md` — bundle persistence pattern reused for `CounterexampleBundle` storage.
* `docs/milestones/m7-06-session-lifecycle-sandbox-smoke-merge.md` § 7 "m8+ handoff" — *the naming-collision disclosure source*.
* `docs/milestones/m7-07-session-stop-single-call-merge.md` — `SessionStopPersistence` m7-07 pattern (service-internal enum, not a DTO) that M8 reuses for `CounterexampleBundle`.
* `docs/chronos-agentic-reconstruction/docs/specs/AGENT_API_V2.md` — v2 tool surface where `counterexample_*` joins `hypothesis_test`.

---

— Submitted 2026-09-11. Awaits M8 execute cycle kickoff.
