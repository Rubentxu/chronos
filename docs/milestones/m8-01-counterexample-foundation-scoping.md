# m8-01 — counterexample foundation scoping

**Branch:** `feat/m8-01-counterexample-foundation-scoping`
**Cycle:** M8 §1, first execute cycle of the M8 multi-cycle plan
**Precedence:** `docs/milestones/m8-counterexample-shrinking-scoping.md` (M8 scoping, parent); `docs/milestones/m6-04-hypothesis-test.md` (closest structural precedent); `docs/chronos-agentic-reconstruction/docs/specs/RUNTIME_PROPERTIES_AND_SLICING.md` § "Counterexample shrinking"
**Status:** SCOPING — 2026-09-11

## Why this cycle

The M8 scoping doc proposed a 5-cycle execution plan (m8-01 through m8-05). This is the first execute cycle: **foundation only**. It delivers (a) the `chronos-services::counterexample` module skeleton, (b) the three DTOs (input, output, internal-bundle), (c) the `proptest` workspace dependency wired only into the services crate, and (d) 4-5 unit tests. **No** MCP wrappers, **no** proptest shrink loop, **no** redb bundle table — those belong to m8-02 and m8-03 respectively.

This cycle boundary follows the m6-04 pattern (m6-04-scoping → m6-04-execute is a single cycle that ships the foundation + service + tool + tests together; m8 splits that into 5 cycles because M8 is bigger: 4 new algorithms rather than 1).

## Scope (this cycle)

### 1. Add `proptest` workspace dep (workspace member `Cargo.toml`)

```toml
[workspace.dependencies]
proptest = "1.5"
```

The dep is exposed at workspace level so future m8-02 cycles can pin it where needed, but the only crate that consumes `proptest` in m8-01 is `chronos-services` (via `dev-dependencies` — see §2).

**Honest disclosure:** proptest as a `dev-dependency` in m8-01, *not* a runtime dep. We do **not** run proptest-style probabilistic shrinks inside the production dispatcher path (m8-02 changes that). m8-01 only uses proptest for unit-test synthesis (assertions that the dispatcher's types are proptest-compatible: e.g., implementing `Arbitrary` on `CounterexampleShrinkInput`).

### 2. `chronos-services::counterexample` module (~150-200 LoC, 18th service module)

File: `crates/chronos-services/src/counterexample.rs`.

#### 2a. `pub struct CounterexampleContext<'a>`

```rust
pub struct CounterexampleContext<'a> {
    pub store: &'a chronos_store::SessionStore,
    pub hypothesis_ctx: &'a HypothesisTestContext<'a>,
}
```

`CounterexampleContext` does NOT embed `QueryEngine` directly. The dispatcher borrows the `hypothesis_ctx` (which carries the engine inside it, see `m6-04`). This avoids dual ownership of the engine and matches the m7-05 `SessionLifecycleContext` precedent (borrow `ProbeContext` rather than reach into raw state).

#### 2b. `pub enum CounterexampleShrinkInput { ... }` (NOT a DTO)

Stays in `crates/chronos-services/src/counterexample.rs` at module scope (NOT in `output.rs`). Why: it carries `proptest::TestRunner` config + a raw `HypothesisInput` reference for the proptest adapter. m8-02 work.

```rust
pub enum CounterexampleShrinkInput {
    Shrink {
        property_kind: HypothesisKind,             // reuses m6-04 enum
        target_hypothesis: HypothesisInput,         // reuses m6-04 enum
        max_rounds: u32,                            // default 64 (proptest default)
        seed: Option<u64>,                          // deterministic rerun
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
```

#### 2c. `pub enum CounterexampleOutput { ... }` (NOT a DTO — m8-03 promotes to DTOs)

Service-internal enum. m8-03 introduces the `CounterexampleOutputDto` wrapper that omits the events vector for the wire. m8-01 keeps everything internal.

#### 2d. `pub enum CounterexampleBundle { ... }` (service-internal, like `SessionStopPersistence` m7-07)

Carries `Vec<TraceEvent>` for the causal slice. Will be persisted via redb in m8-03. In m8-01 it's defined but **not persisted** — only constructed in unit tests to verify the struct shape.

#### 2e. `pub struct ChronosCounterexampleService;` + entry points

Two entry points in m8-01:

- `pub fn get(ctx: &CounterexampleContext<'_>, bundle_id: &str) -> Result<CounterexampleOutput, ServiceError>` — m8-01 always returns `Err(ServiceError::LoadFailed("counterexample_bundles table not yet provisioned (m8-03)"))` until m8-03 ships the redb schema. This signature lets the MCP wrapper (m8-03) bind to it without a follow-up cycle change.
- `pub fn list(ctx, filter) -> Result<...>` — same pattern: returns `UnsupportedByRecordedEvidence` until m8-03.

**No `shrink` entry point in m8-01.** m8-01 is foundation-only; the `shrink` algorithm requires the proptest loop wiring (m8-02 risk §).

#### 2f. Wire into `lib.rs`

```rust
// existing 20th entry pattern:
pub mod counterexample;  // m8-01
```

### 3. DTOs added to `output.rs` (only the wire-shape ones)

m8-01 introduces the **wire-shape only** DTOs:

- `pub struct CounterexampleBundleSummaryDto { bundle_id: String, property_kind: HypothesisKind, workspace_id: String, created_at_ms: u64, rounds_used: u32 }`
- `pub struct CounterexampleListOutputDto { bundles: Vec<CounterexampleBundleSummaryDto>, next_cursor: Option<String> }`
- `pub struct CounterexampleGetOutputDto { bundle: CounterexampleBundleSummaryDto, has_full_bundle: bool }`

`has_full_bundle` is `true` when the bundle is in the redb table (m8-03+); `false` when only the summary is available (m8-01 default — never persisted yet, but the wrapper must produce *something*).

### 4. Unit tests (`#[cfg(test)] mod tests` private to the module, ~80 LoC)

5 tests, all sandbox-free:

1. `counterexample_context_is_send_when_inner_refs_are_send` — compile-time check via `fn assert_send<T: Send>(_: T) {}`. Catches accidental `!Send` misuses.
2. `counterexample_get_returns_load_failed_when_table_missing` — verifies the m8-01 stub message.
3. `counterexample_list_returns_unsupported_when_table_missing` — same.
4. `counterexample_input_default_max_rounds_is_64` — pins the proptest default.
5. `counterexample_bundle_summary_dto_excludes_events_vector_field` — `has_full_bundle == false` is the only legitimate m8-01 state.

### 5. `cargo fmt --check` + `cargo clippy -D warnings` clean

T0 gate. Same precedence as m7-01 through m7-07.

### 6. `cargo test --workspace --lib` passes with 5 new tests in `chronos_services::counterexample::tests`

T1-equivalent gate at the workspace level (no sandbox smoke in m8-01).

## Architectural decisions (with rationale)

- **B1: proptest as `dev-dependency` in m8-01.** The shrinker wiring is m8-02; m8-01 only needs the workspace dep + the type system to know `proptest::TestRunner` exists. We do NOT add proptest to chronos-domain or chronos-mcp (per A1 in the M8 scoping doc).
- **B2: `CounterexampleContext` borrows `HypothesisTestContext`, not `QueryEngine`.** Mirrors `SessionLifecycleContext` borrowing `ProbeContext` (m7-05). Avoids dual ownership and matches the project's "context bundles carry live state" pattern from M5.
- **B3: DTOs in `output.rs` are summary-only (`*SummaryDto`).** Bundles carry events; summaries don't. m8-03 will introduce the full-bundle wire DTO only when redb roundtrip is verified.
- **B4: `get` and `list` entry points exist in m8-01 but return stub errors.** m8-01 lays down the **signature**; m8-03 fills in the redb logic. This avoids a follow-up signature-change cycle when m8-03 ships — the MCP wrapper can bind directly in m8-03.
- **B5: No `shrink` entry point in m8-01.** The `proptest` Strategy adapter (B6) is non-trivial and explicit-scope for m8-02. Spawning it in m8-01 would conflate foundation with risk mitigation.
- **B6: `HypothesisInput → proptest::Strategy` adapter is deferred to m8-02.** m8-01 only ensures the types can co-exist in the same module (no import-cycle panic) and that proptest is available as a dep. The actual adapter impl lives in m8-02 (the shrink loop wiring cycle) where it's the loop's central wiring concern.
- **B7: No `chrono_cli` workspace member in m8-01.** That's m8-04. Adding it now would require creating a new crate just to host nothing — ant anti-pattern. m8-01's scope is **services crate only**.

## Concrete API surface (signatures only — implementations in m8-03 / m8-02)

```rust
// crates/chronos-services/src/counterexample.rs

#[derive(Debug, thiserror::Error)]
pub enum CounterexampleError { LoadFailed, Unsupported, ... }   // bridges ServiceError
                                                                 // variants already defined

pub struct CounterexampleContext<'a> { pub store: &'a SessionStore, pub hypothesis_ctx: &'a HypothesisTestContext<'a> }

pub enum CounterexampleShrinkInput { Shrink { ... }, Get { bundle_id: String }, List { ... } }
pub enum CounterexampleOutput { Got { bundle: CounterexampleBundleSummary }, Listed { summaries: Vec<CounterexampleBundleSummary>, next_cursor: Option<String> } }  // no Shrunk variant in m8-01
pub struct CounterexampleBundleSummary { bundle_id: String, property_kind: HypothesisKind, workspace_id: String, created_at_ms: u64, rounds_used: u32, has_full_bundle: bool }
pub enum CounterexampleBundle { /* m8-03 payload */ }

pub struct ChronosCounterexampleService;
impl ChronosCounterexampleService {
    pub fn get(...) -> Result<CounterexampleOutput, ServiceError> { /* stub */ }
    pub fn list(...) -> Result<CounterexampleOutput, ServiceError> { /* stub */ }
    // no shrink in m8-01
}

// crates/chronos-services/src/output.rs (additions)
pub struct CounterexampleBundleSummaryDto { ... }   // wire shape
pub struct CounterexampleListOutputDto { ... }
pub struct CounterexampleGetOutputDto { ... }
```

## Test plan (planned)

All sandbox-free, all unit-level (m8-01 has no MCP wrappers, no CLI):

- 5 unit tests in `crates/chronos-services/src/counterexample.rs::tests` (listed in §4).
- T1 lib equivalent: `cargo test --workspace --lib --exclude chronos-native --exclude chronos-sandbox --exclude chronos-e2e` — expect 798/798 + 4 ignored + 5 new (m8-01 net) → 803/803 + 4 ignored.
- T2 per-crate integration: `cargo test -p chronos-services -p chronos-store -p chronos-mcp --tests` — should still be 358/358 (m8-01 doesn't add integration tests; MCP wrappers come in m8-03).
- T0 fmt + clippy `-D warnings`: PASS.
- **No T4 sandbox smoke** (m8-01 doesn't touch probes / sessions; m8-03 does).

## Risks and known limitations

- **proptest adapter is experimental.** m8-01 only verifies the dep is wired correctly. The actual `HypothesisInput → proptest::Strategy` impl is m8-02's central concern. If that adapter proves impossible for `Existence { match_predicate }` (proptest can't shrink arbitrary predicates), the shrink loop will need fallback heuristics. Out of m8-01 scope.
- **`get` / `list` are stubs.** Callers that hit them in m8-01 get a `LoadFailed("counterexample_bundles table not yet provisioned (m8-03)")` error. This is intentional but means **m8-01 callers (any caller) cannot actually retrieve a bundle until m8-03 ships**. Acceptable for a foundation cycle; documented in the merge doc.
- **No v1 deprecation.** Counterexample tools are net-new; no v1 names exist.
- **No new `ServiceError` variants.** The stub returns existing `LoadFailed` and `UnsupportedByRecordedEvidence` variants (already defined in `chronos-services::error`).
- **No `chrono_cli` workspace member.** m8-04. Adding it now = noise.

## Cross-references

* `docs/milestones/m8-counterexample-shrinking-scoping.md` § "What's already in place" + § "Scope (this cycle)" + § "Architectural decisions" — parent doc; m8-01 ships items 1, 6 partially, and lays the foundation for items 2-5.
* `docs/milestones/m6-04-hypothesis-test.md` — closest structural precedent. Notes that `chronos-services::hypothesis_test` ships all 4 in one cycle (1 cycle ≈ 1023 LoC). M8 is split across 5 cycles to bound each step.
* `docs/chronos-agentic-reconstruction/docs/specs/RUNTIME_PROPERTIES_AND_SLICING.md` § "Counterexample shrinking" — spec subsection lines 54-62.
* `docs/chronos-agentic-reconstruction/docs/roadmap/ROADMAP.md` line 179 (M8) — work item 1 ("proptest/Hypothesis integration contracts") and work item 2 ("input/counterexample artifact") are the ones m8-01 concretely delivers as foundation; items 3-5 are m8-02..m8-04.
* `docs/chronos-agentic-reconstruction/docs/roadmap/MILESTONE_ACCEPTANCE.md` § M8 line 78 — acceptance criterion. m8-01 does NOT satisfy it (m8-05 will).

---

— Submitted 2026-09-11. Awaits m8-01 execute cycle kickoff.
