# m8-03 — MCP wrappers + redb bundle table + real Strategy impls scoping

**Branch:** `feat/m8-03-mcp-wrappers-redb-bundle-table-scoping`
**Cycle:** M8 §3, third execute cycle of the M8 multi-cycle plan
**Precedence:** `docs/milestones/m8-counterexample-shrinking-scoping.md` (M8 parent); `docs/milestones/m8-01-counterexample-foundation-scoping.md` + `-merge.md` (m8-01 sibling); `docs/milestones/m8-02-shrink-loop-wiring-scoping.md` + `-merge.md` (m8-02 sibling); `docs/chronos-agentic-reconstruction/docs/specs/RUNTIME_PROPERTIES_AND_SLICING.md` § "Counterexample shrinking"
**Status:** SCOPING — 2026-09-11

## Why this cycle

m8-01 + m8-02 shipped the foundation + shrink-loop wiring with stub strategies. m8-03 is the **largest m8 cycle** because it owns the 4 deliverables the M8 plan deferred:

1. **Real per-variant `proptest::Strategy` impls** (replaces m8-02's stubs).
2. **`counterexample_bundles` redb table** + SessionStore extensions (replaces m8-01's stub `get`/`list`).
3. **Three v2 MCP wrappers** (`counterexample_shrink`, `counterexample_get`, `counterexample_list`) + their DTOs.
4. **T4 sandbox smoke** against real fixtures (`test_busyloop`, `test_exit_immediate`) to verify end-to-end shrink.

This cycle does NOT add the `chrono_cli` workspace member (that's m8-04) and does NOT produce the M8 acceptance run (that's m8-05).

## Scope (this cycle)

### 1. Real per-variant `proptest::Strategy` impls (~120-200 LoC in `counterexample.rs`)

Replaces the m8-02 stubs with real adapters that mutate the variable input fields per `HypothesisKind`:

#### 1a. `PropertyValueStrategy` (for `Invariant`)

```rust
struct PropertyValueStrategy;
impl proptest::strategy::Strategy for PropertyValueStrategy {
    type Tree = proptest::num::f64::BinarySearch; // or hand-rolled
    type Value = chronos_domain::property::PropertyValue;
    fn new_tree(&self, runner: &mut proptest::test_runner::TestRunner) -> proptest::strategy::NewTree<Self> {
        // Sample from the variant matching target.constant (preserving type).
        // For Number: sample integer nearby, shrink toward 0.
        // For Text: sample shorter strings, shrink lexicographically toward "".
        // For Bool: alternate true/false.
    }
}
```

**Disclosure:** proptest 1.5's `Strategy` trait expects `new_tree` + `simplify` + `current_map_size`. Hand-rolled impl is ~80 LoC; we lean on proptest's built-in `any::<f64>()`/`"\\PC*"`. (`f64` shrinks via BinarySearch toward 0; strings shrink via `String::shrink` defaults.)

#### 1b. `ExistencePredicateStrategy` (for `Existence`)

Per-variant Strategy. The variant is **fixed** by the target — only the inner field mutates:
- `EventTypeEquals { event_type: String }` → strings.
- `ThreadEquals { thread_id: u64 }` → u64.
- `PropertyKeyEquals { target: String }` → strings.

```rust
fn strategy_for_existence(input: &HypothesisInput) -> Result<BoxedStrategy<ExistencePredicate>, ...>
```

#### 1c. `CallPathStrategy` (for `CallPath`)

Returns `BoxedStrategy<(String, String, Option<usize>)>`. caller + callei are bounded to a-zA-Z0-9_` alphabet, max 32 chars. `max_depth` shrinks toward 1.

**All strategies encode the constraint that the variant of `ExistencePredicate` / `HypothesisKind` / `ComparisonOp` cannot change** — only the variable-value field mutates. This is what "shrink preserves the property violation" means concretely: an Invariant with `Lt + Number(5)` shrinks `5` toward smaller values; we never flip the comparison to `Gt`.

#### 1d. Real shrink loop body in `ChronosCounterexampleService::shrink`

Replace the `_runner` placeholder with:

```rust
let mut strategy_value = None;
let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    runner.run(&strategy, |input| {
        // 1. Synthesise a fresh HypothesisInput by replacing the mutating field.
        let mut candidate = target_hypothesis.clone();
        apply_strategy(&mut candidate, input, property_kind);
        // 2. Re-evaluate against the captured trace.
        // 3. If verdict == Violation: Ok(()) — proptest considers this a "pass".
        //    If verdict == Pass: Err(TestCaseError::fail("verdict=pass"))
        //    If verdict == Unsupported: Err(TestCaseError::fail("unsupported"))
    })
}));
match result {
    Ok(Ok(_)) => /* violation found and tried to shrink */,
    Ok(Err(TestCaseError::Fail(why))) if why.reason == "verdict=pass" => /* NoViolation */,
    Ok(Err(TestCaseError::Fail(why))) if why.reason == "unsupported" => /* NoViolation */,
    Err(_) => /* rounds exhausted; return best_so_far */,
}
```

**Honest disclosure**: proptest::test_runner::run has a complex Result type. The m8-03 implementation will start with the simplest form (returns `Ok` when violation found; loop as long as proptest finds smaller inputs) and document trade-offs in the merge doc. Optimisation (early termination on round budget) is m8-05 close-time work.

### 2. `counterexample_bundles` redb table + SessionStore extensions (~150-200 LoC)

**File**: `crates/chronos-store/src/counterexample_storage.rs` (new — separate file from `storage.rs` for clarity, same module as `cas.rs`).

#### 2a. Table definition

```rust
const COUNTEREXAMPLE_BUNDLES: TableDefinition<&[u8], &[u8]>
    = TableDefinition::new("counterexample_bundles");
```

**Key**: `bundle_id: String` (bytes).
**Value**: bincode-serialised `CounterexampleBundleRecord` struct (defined locally in `counterexample_storage.rs`; not re-exported).

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CounterexampleBundleRecord {
    pub summary: chronos_services::counterexample::CounterexampleBundleSummary,
    pub events: Vec<TraceEvent>,
    pub minimised_constant: Option<PropertyValue>,
    pub minimised_predicate: Option<ExistencePredicate>,
    pub minimised_call_path: Option<(String, String, Option<usize>)>,
}
```

**Why a separate `counterexample_storage.rs`**: m8-01 disclosure "bundles have a distinct identity (different lifecycle + GC + persistence from sessions)". Bundles are kept independently, even though they happen to be stored via the same `SessionStore`. This matches the m8-01 R1 architecture.

#### 2b. New `SessionStore` methods (additive, no signature changes for existing methods)

```rust
pub fn save_counterexample_bundle(
    &self,
    bundle: CounterexampleBundleRecord,
) -> Result<(), StoreError>;
pub fn load_counterexample_bundle(
    &self,
    bundle_id: &str,
) -> Result<Option<CounterexampleBundleRecord>, StoreError>;
pub fn list_counterexample_bundles(
    &self,
    workspace_id: Option<&str>,
    property_kind: Option<HypothesisKind>,
    since_ms: Option<u64>,
    until_ms: Option<u64>,
    limit: u32,
) -> Result<Vec<CounterexampleBundleSummary>, StoreError>;
```

`list_*` returns `CounterexampleBundleSummary` (not the full record) — it's the wire-shape response. The wire DTO `CounterexampleListOutputDto` flattens this on the way out.

#### 2c. Replace m8-01 stubs in `ChronosCounterexampleService`:

```rust
pub fn get(ctx, bundle_id: &str) -> Result<CounterexampleOutput, ServiceError> {
    let record = ctx.store.load_counterexample_bundle(bundle_id).map_err(...)?;
    match record {
        Some(rec) => Ok(CounterexampleOutput::Got {
            summary: rec.summary.with_has_full_bundle(true),
        }),
        None => Err(ServiceError::LoadFailed(format!("bundle `{bundle_id}` not found"))),
    }
}

pub fn list(ctx, filter: CounterexampleListFilter) -> Result<CounterexampleOutput, ServiceError> {
    let summaries = ctx.store.list_counterexample_bundles(
        filter.workspace_id.as_deref(),
        filter.property_kind,
        filter.since_ms, filter.until_ms, filter.limit,
    ).map_err(...)?;
    Ok(CounterexampleOutput::Listed {
        summaries,
        next_cursor: None,  // m8-03 ships single-page; pagination is m8-05 close work
    })
}
```

**Disclosure**: `next_cursor: None` always in m8-03 — pagination is a real feature, not a stub; m8-05 close can introduce it as a small follow-up if acceptance tests need it. Cursor would use uuid::v7 monotonic ordering.

### 3. Three v2 MCP wrappers (~250-350 LoC, in `chronos-mcp/src/server.rs`)

#### 3a. New wire DTOs in `crates/chronos-services/src/output.rs` (additive, ~40 LoC)

```rust
/// Wraps a CounterexampleBundleSummaryDto + minimised payload +
/// optional full Vec<TraceEvent> for the counterexample_get wire output.
pub struct CounterexampleBundleDto { summary: CounterexampleBundleSummaryDto, minimised: serde_json::Value, events: Option<Vec<TraceEvent>> }

/// Param shape for `counterexample_shrink`. Reuses the typed enums from
/// m6-04 hypothesis_test (HypothesisKind / HypothesisConstant etc.) so MCP
/// clients get JSON-schema-friendly params.
pub struct CounterexampleShrinkParams {
    pub property_kind: HypothesisKind,
    pub target_hypothesis: HypothesisInputWireDto,
    pub max_rounds: Option<u32>,
    pub seed: Option<u64>,
}

/// Param shape for `counterexample_get`.
pub struct CounterexampleGetParams { pub bundle_id: String, pub include_events: bool }

/// Param shape for `counterexample_list`.
pub struct CounterexampleListParams {
    pub workspace_id: Option<String>,
    pub property_kind: Option<HypothesisKind>,
    pub since_ms: Option<u64>,
    pub until_ms: Option<u64>,
    pub limit: Option<u32>,
}
```

**Disclosure**: `include_events` is a wire-shape knob to keep `counterexample_get` payloads small (Vec<TraceEvent> can be 10s of MB). Default `false`; let the caller opt in for drill-down.

#### 3b. HypothesisInputWireDto

Currently `HypothesisInput` is internal to `chronos-services` (not exposed on the wire because it carries `Vec<PropertyValue>` enum + JsonSchema-derive complications). m8-03 introduces a wire mirror:

```rust
pub struct HypothesisInputWireDto {
    pub session_id: String,
    pub kind: HypothesisKind,
    pub scope: Option<HypothesisScope>,
    pub comparison: Option<String>, // JSON-friendly "lt"/"le"/...
    pub constant: Option<HypothesisConstant>, // already wire-friendly
    pub property_target: Option<String>,
    pub predicate: Option<ExistencePredicate>,
    pub caller: Option<String>,
    pub callee: Option<String>,
    pub max_depth: Option<u64>, // u64 for wire consistency; cast to usize in conversion
}
```

Wrapper fn converts `HypothesisInputWireDto → HypothesisInput` (and back). Returns `Err(ServiceError::InvalidInput)` for unparseable comparison strings.

#### 3c. Three `#[tool]` annotated handlers

```rust
#[tool(name = "counterexample_shrink", description = "...")]
async fn counterexample_shrink(&self, params: Parameters<CounterexampleShrinkParams>) -> Result<Json<...>, rmcp::ErrorData>

#[tool(name = "counterexample_get", description = "...")]
async fn counterexample_get(&self, params: Parameters<CounterexampleGetParams>) -> Result<Json<...>, rmcp::ErrorData>

#[tool(name = "counterexample_list", description = "...")]
async fn counterexample_list(&self, params: Parameters<CounterexampleListParams>) -> Result<Json<...>, rmcp::ErrorData>
```

The wrappers instantiate `CounterexampleContext` (m8-01 type, holds `&SessionStore` + `&HypothesisTestContext`), call the dispatcher, and flatten the service-internal output to a JSON-friendly wire shape.

### 4. Sandbox client surface + smoke tests (~200-300 LoC in `chronos-sandbox`)

#### 4a. Types + client methods (additive, mirrors m7-06's pattern)

File: `chronos-sandbox/src/client/types.rs` (~80 LoC):

```rust
pub struct CounterexampleShrinkRequest { ... }
pub struct CounterexampleShrinkResponse { ... }
pub struct CounterexampleBundleResponse { ... }    // summary + has_full_bundle
pub struct CounterexampleListResponse { ... }
```

File: `chronos-sandbox/src/client/tools.rs` (~120 LoC):

```rust
impl McpSession {
    pub async fn counterexample_shrink(&mut self, req: CounterexampleShrinkRequest) -> Result<CounterexampleShrinkResponse, McpSandboxError>;
    pub async fn counterexample_get(&mut self, bundle_id: &str, include_events: bool) -> Result<CounterexampleBundleResponse, McpSandboxError>;
    pub async fn counterexample_list(&mut self, params: CounterexampleListParams) -> Result<CounterexampleListResponse, McpSandboxError>;
}
```

#### 4b. New smoke test files (or extensions to existing files, TBD)

Following the m7-06/`probe_lifecycle` + `session_lifecycle` + `program_scenarios` pattern:

- **`chronos-sandbox/tests/counterexample_tools.rs`** (new file, ~150-250 LoC, 4-6 tests):
  1. `test_counterexample_shrink_invariance_violation_on_busyloop` — start `test_busyloop`, evaluate Invariant `event_count < 5`, expect `Violation`. Then shrink → assert counterexample bundle persists with `rounds_used >= 1` + `minimised_constant` is `Some(...)` with a smaller constant than the original.
  2. `test_counterexample_shrink_existence_violation_on_exit_immediate` — start `test_exit_immediate`, run `Existence { predicate: EventTypeEquals { event_type: "Syscall" } }`. Trace has syscall events from probe plumbing → expect `Pass`. Then negate via a predicate that doesn't match → expect `Violation`. Shrink → assert `minimised_predicate` is `Some(...)` with a minimised predicate.
  3. `test_counterexample_get_returns_bundle_with_has_full_bundle_true` — run a shrink, persist the bundle, call `counterexample_get(bundle_id, include_events=false)`. Assert `summary.has_full_bundle == true` and `events == None`.
  4. `test_counterexample_list_filtered_by_property_kind` — run 2-3 shrinks across all 3 property kinds (Invariant, Existence, CallPath), call `counterexample_list(property_kind=Some(Invariant))`, assert at least 1 result.
  5. `test_counterexample_get_returns_load_failed_for_unknown_bundle` — call `counterexample_get("nonexistent", include_events=false)`, expect `LoadFailed`.
  6. `test_counterexample_shrink_no_violation_returns_unsupported` — start a fixture, run a hypothesis that already passes, expect `Unsupported` from `counterexample_shrink`.

- **Optional extensions** to existing files (1-2 tests in `e2e_connectivity.rs` to confirm counterexample tools are registered).

**Real fixtures**: `test_busyloop`, `test_exit_immediate`, `test_threads` from `chronos-sandbox/build.rs` (NOT `test_echo` — doesn't exist).

### 5. Gates

- **T0** (fmt + clippy `-D warnings`): PASS, 0 warnings.
- **T1** (lib unit, exluding chronos-native/sandbox/e2e): expect 815/815 + 0 ignored (was 811 in m8-02; +4-6 net = new Strategy tests + counterexample_storage tests + 3 wrapper param tests).
- **T2** (per-crate integration): expect +6+ from new tests across services + store.
- **T4 sandbox smoke**: 4-6 new tests in `counterexample_tools.rs`. Run with pre-built `chronos-mcp` binary at `/var/home/rubentxu/cargo-targets/debug/chronos-mcp`, `CHRONOS_MCP_PATH` exported, `--test-threads=1`.

## Architectural decisions (with rationale)

- **B1: `CounterexampleBundleRecord` lives in `chronos-store`, not `chronos-services`.** (C1 risk). Justification: the store crate is the persistence boundary. The record's wire-shape is `chronos-services`-defined; storage layout is store-defined. Same pattern as `session_events` table.
- **B2: `include_events: bool` on `counterexample_get`.** (Performance). Bundles can carry 10s of MB of events. Wire shape keeps the default response small; opt-in for drill-down. Mirrors `cargo's --no-deps --all-features` pattern (opt-in for verbose output).
- **B3: `HypothesisInputWireDto` is a wire mirror, not a reuse.** (JsonSchema compatibility). Domain `HypothesisInput` lacks `JsonSchema` derives; mirror carries a string-based `comparison` field instead of an enum mirror.
- **B4: `next_cursor: None` always in m8-03.** (Scope). Pagination is real feature work; defer to m8-05 close. The MCP wrapper falls back to single-page list.
- **B5: Real `proptest::Strategy` impls replace m8-02 stubs.** (B6 from m8-02). The scope moves from "wiring + dispatch" to "the algorithm itself". Bigger risk surface; documented per-variant.
- **B6: `EventTypeEquals` shrink direction is naive (proptest default string shrink).** (R3 from m8-02 doc). Acceptable for m8-03 baseline. Smarter "shrink within observed alphabet" is m8-05 close work.
- **B7: `counterexample_shrink` runs synchronously inside the MCP handler.** (Latency disclosure). proptest::TestRunner::run is blocking; we may wrap it in `tokio::task::spawn_blocking` to avoid blocking the async runtime. ~40 LoC. Pinned in this scope.
- **B8: `chrono_cli` workspace member NOT added.** (A-lite cut-off). Stays m8-04.

## Concrete API surface

```rust
// crates/chronos-store/src/counterexample_storage.rs (new file, ~200 LoC)
const COUNTEREXAMPLE_BUNDLES: TableDefinition<&[u8], &[u8]> = TableDefinition::new("counterexample_bundles");
pub struct CounterexampleBundleRecord { pub summary: ...CounterexampleBundleSummary, pub events: Vec<TraceEvent>, pub minimised_constant: ..., minimised_predicate: ..., minimised_call_path: ... }
impl SessionStore {
    pub fn save_counterexample_bundle(&self, bundle: CounterexampleBundleRecord) -> Result<(), StoreError>;
    pub fn load_counterexample_bundle(&self, bundle_id: &str) -> Result<Option<CounterexampleBundleRecord>, StoreError>;
    pub fn list_counterexample_bundles(&self, workspace_id: Option<&str>, property_kind: Option<HypothesisKind>, since_ms: Option<u64>, until_ms: Option<u64>, limit: u32) -> Result<Vec<CounterexampleBundleSummary>, StoreError>;
}

// crates/chronos-services/src/counterexample.rs (extend m8-02)
struct PropertyValueStrategy; // hand-rolled Strategy impl
fn strategy_for_existence(input: &HypothesisInput) -> Result<BoxedStrategy<ExistencePredicate>, ServiceError>;
fn strategy_for_call_path(input: &HypothesisInput) -> Result<BoxedStrategy<(String, String, Option<usize>)>, ServiceError>;
async fn shrink_invariant(...) // REAL impl, replaces stub
async fn shrink_existence(...) // REAL impl, replaces stub
async fn shrink_call_path(...) // REAL impl, replaces stub
pub fn save_counterexample(&self, ...) // new wrapper around SessionStore::save_counterexample_bundle

// crates/chronos-services/src/output.rs (additive DTOs)
pub struct CounterexampleBundleDto { summary, minimised: serde_json::Value, events: Option<Vec<TraceEvent>> }
pub struct HypothesisInputWireDto { ... } // wire mirror
pub struct CounterexampleShrinkParams { ... }
pub struct CounterexampleGetParams { ... }
pub struct CounterexampleListParams { ... }

// crates/chronos-mcp/src/server.rs (3 new #[tool] handlers)
#[tool(name = "counterexample_shrink", description = "Run a shrink loop on a known-failing hypothesis...")]
async fn counterexample_shrink(&self, params: Parameters<CounterexampleShrinkParams>) -> Result<...>

#[tool(name = "counterexample_get", description = "Retrieve a persisted counterexample bundle by id...")]
async fn counterexample_get(&self, params: Parameters<CounterexampleGetParams>) -> Result<...>

#[tool(name = "counterexample_list", description = "List persisted counterexample bundles with optional filters...")]
async fn counterexample_list(&self, params: Parameters<CounterexampleListParams>) -> Result<...>

// chronos-sandbox/src/client/types.rs (new DTOs)
pub struct CounterexampleShrinkResponse { pub bundle_id: String, pub rounds_used: u32, pub has_full_bundle: bool }
pub struct CounterexampleBundleResponse { pub summary: CounterexampleBundleSummaryDto, pub events: Option<Vec<TraceEvent>> }
pub struct CounterexampleListResponse { pub bundles: Vec<CounterexampleBundleSummaryDto>, pub has_more: bool }

// chronos-sandbox/src/client/tools.rs (3 new methods on McpSession)
pub async fn counterexample_shrink(&mut self, ...) -> Result<...>
pub async fn counterexample_get(&mut self, bundle_id: &str, include_events: bool) -> Result<...>
pub async fn counterexample_list(&mut self, ...) -> Result<...>

// chronos-sandbox/tests/counterexample_tools.rs (new, ~200 LoC, 4-6 tests)
test_counterexample_shrink_invariance_violation_on_busyloop
test_counterexample_shrink_existence_violation_on_exit_immediate
test_counterexample_get_returns_bundle_with_has_full_bundle_true
test_counterexample_list_filtered_by_property_kind
test_counterexample_get_returns_load_failed_for_unknown_bundle
test_counterexample_shrink_no_violation_returns_unsupported
```

## Test plan (planned)

- **4-6 new unit tests** for the per-variant Strategy impls (`strategy_for_invariant_preserves_comparison`, etc.) + 2-3 tests for `counterexample_storage` (save/load roundtrip, list filter).
- **T1**: expect 815/815 (was 811 in m8-02; +4-6 net).
- **T2**: services + store + mcp integration tests, expect +6+ net.
- **T4 sandbox smoke**: 4-6 new tests in `counterexample_tools.rs`, run with pre-built `chronos-mcp` binary + `--test-threads=1`.

**This is the FIRST cycle where T4 sandbox smoke is mandatory for M8** — the wire path is the only way to verify `counterexample_shrink → bundle persist → counterexample_get` round-trip end-to-end.

## Risks and known limitations

- **R1 (highest): real `proptest::Strategy` impls per-variant is the experimental risk explicitly flagged in the M8 parent doc (C0).** Mitigation: the per-variant split isolates blast radius (B5). If Invariant Strategy breaks, Existence / CallPath stay working.
- **R2: `proptest::test_runner::run` is blocking** — wrap in `spawn_blocking` (B7) to avoid stalling the MCP async runtime.
- **R3: `next_cursor: None` for m8-03** — list result is single-page. Documented; m8-05 adds pagination if acceptance needs it.
- **R4: bundling `Vec<TraceEvent>` as a single bincode blob is inefficient for large traces.** The redb `counterexample_bundles` table stores one big blob per bundle; this is fine for m8-03 (bundles are typically small — the shrunk subset). A future cycle can split events into CAS hashes the way `session_events` does.
- **R5: `HypothesisInputWireDto` adds a conversion round-trip per `counterexample_shrink` call.** Tiny overhead; same pattern as `HypothesisConstant` in m6-04.
- **R6: Sandbox smoke may take 30-60s** depending on `test_busyloop` capture duration. Within T4-smoke budget.

## Cross-references

* `docs/milestones/m8-counterexample-shrinking-scoping.md` § "Multi-cycle plan" — parent doc; this cycle ships items 3-5.
* `docs/milestones/m8-01-counterexample-foundation-scoping.md` + `-merge.md` — m8-01 sibling (stub entry points + wire DTOs).
* `docs/milestones/m8-02-shrink-loop-wiring-scoping.md` + `-merge.md` — m8-02 sibling (signature + dispatch + STUB strategies).
* `docs/milestones/m6-04-hypothesis-test.md` — closest architectural precedent (HypothesisConstant wire mirror, JsonSchema strategy, dispatcher pattern).
* `docs/milestones/m7-06-session-lifecycle-sandbox-smoke-merge.md` — sandbox client surface pattern (sandbox_tools.rs + sandbox client/types.rs + client/tools.rs).
* `docs/chronos-agentic-reconstruction/docs/specs/RUNTIME_PROPERTIES_AND_SLICING.md` § "Counterexample shrinking" — spec subsections 54-62.
* `docs/chronos-agentic-reconstruction/docs/roadmap/ROADMAP.md` line 179 — M8 work items 3-5.
* `docs/chronos-agentic-reconstruction/docs/roadmap/MILESTONE_ACCEPTANCE.md` § M8 line 78 — end-to-end acceptance criterion. m8-05 satisfies this; m8-03 satisfies "first end-to-end test" only.

---

— Submitted 2026-09-11. Awaits m8-03 execute cycle kickoff.
