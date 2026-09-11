# m8-02 — shrink loop wiring scoping

**Branch:** `feat/m8-02-shrink-loop-scoping`
**Cycle:** M8 §2, second execute cycle of the M8 multi-cycle plan
**Precedence:** `docs/milestones/m8-counterexample-shrinking-scoping.md` (M8 parent); `docs/milestones/m8-01-counterexample-foundation-scoping.md` + `-merge.md` (m8-01 sibling); `docs/chronos-agentic-reconstruction/docs/specs/RUNTIME_PROPERTIES_AND_SLICING.md` § "Counterexample shrinking"
**Status:** SCOPING — 2026-09-11

## Why this cycle

m8-01 shipped the foundation (signatures + stubs + DTOs + 8 unit tests). m8-02 ships the **second piece** of the 5-piece M8 plan: the shrink loop + `HypothesisInput → proptest::Strategy` adapter. This is the **experimental-risk cycle** that the M8 scoping doc explicitly carved out — it owns the central wiring concern that the rest of M8 inherits.

This cycle does NOT touch MCP wrappers, redb persistence, the `chrono_cli` workspace member, or session/probe state. Those belong to m8-03, m8-03, m8-04, and (independently) m7-09 respectively.

## Scope (this cycle)

### 1. Promote `proptest` from dev-dep to runtime dep on `chronos-services`

```toml
# crates/chronos-services/Cargo.toml
[dependencies]
proptest = { workspace = true }
# [dev-dependencies]: keep the line for unit tests
```

**Disclosure:** This makes `proptest` an actual runtime dep (so `proptest::TestRunner`, `proptest::Strategy`, etc. are reachable from the dispatched `shrink` code). Dev-dep stays so unit tests don't pay a version-pin duplication.

### 2. Implement `ChronosCounterexampleService::shrink` (~200-280 LoC)

File: `crates/chronos-services/src/counterexample.rs` (extend m8-01's stubs).

**Signature** (additive on top of m8-01's `get`/`list`):

```rust
pub async fn shrink(
    ctx: &CounterexampleContext<'_>,
    input: CounterexampleShrinkInput,
) -> Result<CounterexampleOutput, ServiceError> {
    // ...
}
```

`async` because the underlying `hypothesis_test::test` is async (m6-04 precedent; the engines map is behind a `TokioMutex`).

**Body shape**:

```
1. Match input: only the Shrink { property_kind, target_hypothesis, max_rounds, seed } variant is handled.
   Get/List variants are caught at compile time by Rust's pattern matching; this is enforced because
   the entry point only accepts CounterexampleShrinkInput for shrink().
2. Build proptest::TestRunner config from (max_rounds, seed); pin default limit at 64 rounds.
3. Build the proptest::Strategy for the failing input via the adapter (#3 below).
4. Run runner.run(&strategy, |input| {
       // 4a. Synthesise a fresh HypothesisInput from `input` (the proptest sample).
       // 4b. Call ChronosHypothesisTestService::test(ctx.hypothesis_ctx, h).await? -- this
       //     re-evaluates the synthesised hypothesis against the **same captured trace**.
       // 4c. If verdict == Violation, Ok(()) — the test passed (we found a violation).
       //     If verdict == Pass, Err(TestCaseError::fail("verdict=Pass")) — wrong shape.
       //     If verdict == Unsupported, Err(TestCaseError::fail("unsupported")) —
       //     per spec, Unsupported cannot be collapsed to Pass.
   })
5. Convert the proptest failure back into a CounterexampleBundle with:
   - property_kind
   - workspace_id
   - created_at_ms = now_unix_ms()
   - rounds_used = actual_rounds_consumed
   - has_full_bundle = false (m8-03 will flip when redb is provisioned)
   - TraceEvent slice = the causal slice kept by the test runner, narrowed
     to the events that violate the hypothesis (re-uses ChronosTraceSliceService::slice,
     the m3 algorithm)
6. Wrap in CounterexampleOutput::Got (or add a new Shrunk variant — see #5 below).
```

**Honest disclosure**: shrinking re-runs the hypothesis evaluation against the **same captured trace** — proptest mutates *the input*, not the evidence. This is exactly what `RUNTIME_PROPERTIES_AND_SLICING.md` § "Counterexample shrinking" requires ("preserve the same property violation" while shrinking the input that triggers it).

### 3. `HypothesisInput → proptest::Strategy` adapter (~120-160 LoC, in `counterexample.rs`)

**Private module fn** (not exported). This is the experimental-risk adapter that the M8 doc flagged.

**Per-variant design**:

| HypothesisKind | What's mutating | Strategy |
|---|---|---|
| `Invariant` | `constant: PropertyValue` (scalar) + comparison op (fixed) + scope (fixed) | `PropertyValueStrategy::arbitrary()` over the same `PropertyValue` enum. Shrinks toward 0 (numeric) / "" (string) / `false` (bool). |
| `Existence` | `predicate: ExistencePredicate::EventTypeEquals { event_type }` or `ThreadEquals { thread_id }` or `PropertyKeyEquals { target }` | Custom: `ExistencePredicateStrategy` knows the variant set; per-variant `arbitrary()` + `shrink()` for EventTypeEquals (string shrinks lexicographically) and PropertyKeyEquals (same). ThreadEquals is u64 — already covered by proptest's integer strategy. |
| `CallPath` | `caller: String`, `callee: String`, optional `max_depth: usize` | Custom: `("a-zA-Z0-9_")`-bounded `StringStrategy` for caller+callee, length capped at 32 chars; max_depth shrinks toward 1. |

**Why per-variant custom instead of blanket `Arbitrary`**: proptest's blanket `Arbitrary` on an enum with named fields (like `ExistencePredicate`) is difficult to derive and doesn't shrink well. Hand-rolled per-variant gives us control of the shrink direction (alphabet shrink for strings, monotonic shrink for scalars).

**Known limitation (B3 risk in M8 doc)**: `ExistencePredicate::EventTypeEquals { event_type }` shrink will only shrink within the captured event-type vocabulary. If the violation requires an event type that the trace never sees, the shrink won't find it — it will report `RoundsExhausted`. Caller handles this gracefully (treat as "no smaller counterexample found").

### 4. New service-internal enum: `CounterexampleRunError` (NOT in `output.rs`)

Why a new enum: `shrink` can fail in three ways that **need distinct callers responses**:

```rust
pub enum CounterexampleRunError {
    /// The agent supplied a `target_hypothesis` that already evaluates to Pass or
    /// Unsupported on the captured trace. There's nothing to shrink — the input
    /// hypothesis is not a counterexample.
    NoViolation { reason: String },
    /// proptest exhausted `max_rounds` rounds without converging to a smaller input.
    /// The best candidate so far is preserved in `best_so_far: Option<CounterexampleBundle>`.
    RoundsExhausted { best_so_far: Option<CounterexampleBundle> },
    /// proptest + the adapter could not even produce a candidate input (e.g.,
    /// internal compile failure of the Strategy). Treated as a hard error.
    AdapterFailed(String),
}

impl From<CounterexampleRunError> for ServiceError {
    fn from(e: CounterexampleRunError) -> Self {
        match e {
            CounterexampleRunError::NoViolation { reason } => ServiceError::Unsupported(reason),
            CounterexampleRunError::RoundsExhausted { .. } => ServiceError::Unsupported("shrink rounds exhausted".to_string()),
            CounterexampleRunError::AdapterFailed(s) => ServiceError::EvalError(s),
        }
    }
}
```

This bridges cleanly to the existing `ServiceError` variants without adding new ones (per the project's "reuse ServiceError::Unsupported for m7+ stubs" rule).

### 5. Wire-shape decision: `CounterexampleOutput::Got` vs new `Shrunk` variant

**Decision: extend `CounterexampleOutput` with a new `Shrunk` variant.** This is the minimal-blast-radius choice:

```rust
pub enum CounterexampleOutput {
    Got { summary: CounterexampleBundleSummary },          // m8-01
    Listed { summaries: Vec<CounterexampleBundleSummary>, next_cursor: Option<String> }, // m8-01
    Shrunk {                                                   // m8-02
        bundle: CounterexampleBundleSummary,
        rounds_used: u32,
        minimised_constant: Option<PropertyValue>,
        minimised_predicate: Option<ExistencePredicate>,
        minimised_call_path: Option<(String, String, Option<usize>)>,
    },
}
```

**Disclosure**: at most one of `minimised_constant` / `minimised_predicate` / `minimised_call_path` is `Some`, dictated by `bundle.property_kind`. The m8-03 MCP wrapper will collapse this to a single `minimised: serde_json::Value` field on the wire so the discriminant (which variant won) is captured in wire shape. m8-02 keeps the typed enum because internal callers want the typed view.

### 6. Unit tests (6-8 new, sandbox-free)

All in `#[cfg(test)] mod tests` in `counterexample.rs`:

1. `shrink_promotes_max_rounds_default_64` — pinned config.
2. `shrink_returns_no_violation_when_target_hypothesis_already_passes` — uses a known-Pass hypothesis (e.g., `event_count > 0` on a captured trace).
3. `shrink_returns_no_violation_when_target_hypothesis_unsupported` — uses `scope=property_value` with no recorded target.
4. `shrink_returns_shrunk_variant_on_violation_after_round` — **the central m8-02 win**: build a hypothesis with `event_count == 4` against an event-busy trace, run shrink, verify `bundle.has_full_bundle == false` AND `bundle.rounds_used >= 1`. Stub TestRunner with a canned input that simplifies to event_count=2 over shrink steps.
5. `invariant_strategy_shrinks_arithmetic_constant_toward_zero` — directly test the `PropertyValueStrategy`.
6. `existence_predicate_strategy_shrinks_event_type_to_empty` — same for Existence.
7. `call_path_strategy_shrinks_caller_alphabetically` — same for CallPath.
8. `rounds_exhausted_returns_unsupported` — set max_rounds=0 (not real proptest run; test the early-exit branch).

Tests 4-7 use **synthesised `HypothesisInput`s against a synthetic event buffer** (Vec<TraceEvent>) created inline — they don't need real session engines, but they may need access to `HypothesisTestContext`. If that's not feasible (TokioMutex construction in tests), m8-02 will gate those tests on `#[ignore]` and let the m8-03 sandbox smoke exercise them. **Decision: try inline first; if TokioMutex construction is heavyweight, gate with `#[ignore]` and document in the merge doc.**

### 7. `proptest` strategy crate-feature audit

`proptest = "1.5"` brings in `bit-set`, `bit-vec`, `rand`, `rusty-fork`, `tempfile`, `regex-syntax`, `timeout` (default features). m8-02 will **not** need `proptest-macro` (we hand-roll strategies, not `proptest!{}` blocks). Default features are fine.

Compile-time cost: ~6-8s longer `cargo check -p chronos-services` (per m6-04 baseline). Acceptable.

### 8. `cargo fmt --check` + `cargo clippy -D warnings` + `cargo test --lib -p chronos-services`

T0 + T1-equivalent gates at the services crate level. No sandbox smoke in m8-02 (no MCP wrappers, no integration tests).

## Architectural decisions (with rationale)

- **B1: Promote proptest to runtime dep** (C1 risk). Justification: the shrink loop *runs* at session-tool invocation time, not at cargo test time. Dev-dep would force `cargo test --features runtime-proptest` or similar — not a feasible API. Runtime dep on `chronos-services` only (not workspace-wide), per A1 in the M8 scoping doc.
- **B2: Adapter is per-variant hand-rolled.** (C0 risk in M8 doc). Justification: blanket `Arbitrary` on the `HypothesisInput` enum is brittle — `#[derive(Arbitrary)]` does not exist in proptest 1.5; manual hand-roll is the maintainable path. Per-variant strategies give us control of shrink direction.
- **B3: Re-evaluate hypothesis against the same captured trace.** (Spec compliance). Per `RUNTIME_PROPERTIES_AND_SLICING.md`, shrink preserves the violation property while shrinking the input that triggers it. The captured trace is invariant across rounds.
- **B4: New `CounterexampleOutput::Shrunk` variant**, not a free-form `serde_json::Value` inside `Got`. Justification: keeps internal callers type-safe, m8-03 promotes to wire DTO with a flattened shape.
- **B5: Reject `target_hypothesis` that already evaluates to Pass / Unsupported.** Returns `NoViolation → ServiceError::Unsupported`. Rationale: a hypothesis that already passes has no counterexample to find — erroring early saves a useless shrink budget.
- **B6: `best_so_far: Option<CounterexampleBundle>` preserved on `RoundsExhausted`.** (Spec compliance). Even if shrink doesn't fully converge, the smallest input that DID violate is returned. Lets the agent compare against the original.
- **B7: No new `ServiceError` variants.** Reuse `Unsupported` for NoViolation + RoundsExhausted (semantically: "no smaller counterexample found"); `EvalError` for adapter-internal failures (preserves the existing variant's contract: "evaluation of a hypothesis failed").
- **B8: `chrono_cli` workspace member NOT added in m8-02.** Stays m8-04.

## Concrete API surface

```rust
// crates/chronos-services/src/counterexample.rs (additions on top of m8-01)

pub enum CounterexampleOutput {
    Got { summary: CounterexampleBundleSummary },                                  // m8-01
    Listed { summaries: Vec<CounterexampleBundleSummary>, next_cursor: Option<String> }, // m8-01
    Shrunk {                                                                          // m8-02
        bundle: CounterexampleBundleSummary,
        rounds_used: u32,
        minimised_constant: Option<PropertyValue>,
        minimised_predicate: Option<ExistencePredicate>,
        minimised_call_path: Option<(String, String, Option<usize>)>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CounterexampleRunError { NoViolation { reason }, RoundsExhausted { best_so_far: Option<CounterexampleBundle> }, AdapterFailed(String) }

pub struct ShrinkConfig { pub max_rounds: u32, pub seed: Option<u64>, /* future: timeout_ms */ }
impl Default for ShrinkConfig { ... }
impl From<ShrinkConfig> for proptest::test_runner::Config { ... }

impl ChronosCounterexampleService {
    pub async fn shrink(
        ctx: &CounterexampleContext<'_>,
        input: CounterexampleShrinkInput,
    ) -> Result<CounterexampleOutput, ServiceError>
}

// private:
fn strategy_for_invariant(input: &HypothesisInput) -> impl Strategy<Value = PropertyValue> { ... }
fn strategy_for_existence(input: &HypothesisInput) -> impl Strategy<Value = ExistencePredicate> { ... }
fn strategy_for_call_path(input: &HypothesisInput) -> impl Strategy<Value = (String, String, Option<usize>)> { ... }
```

## Test plan (planned)

T1-equivalent at the workspace level (no T4 sandbox smoke in m8-02):

- 6-8 new tests in `chrono_services::counterexample::tests` (listed in §6).
- T1 lib: `cargo test --workspace --lib --no-fail-fast --exclude chronos-native --exclude chronos-sandbox --exclude chronos-e2e` — expect 814/814 + 0 ignored (was 806 before; +6-8 net).
- T2 per-crate integration: `cargo test -p chronos-services --tests --no-fail-fast` — expect +6-8 net (currently no integration tests in counterexample).
- T0: PASS.

**No T4 sandbox smoke** in m8-02. m8-03 brings the first MCP wrapper, m8-03 also brings the first redb roundtrip (which is where shrink's persisted output gets exercised). m8-02 is "the algorithm works against an in-memory trace", not "the algorithm works against a real session".

## Risks and known limitations

- **R1: `HypothesisInput → Strategy` adapter is the highest-risk item in M8.** This is exactly what the M8 doc flagged as C0. Per-variant hand-rolled strategies cap the blast radius if one variant breaks (e.g., Existence shrinking may degrade to "no useful shrinkage" without taking the Invariant shrink down with it). Mitigation: tests 5-7 exercise each variant in isolation.
- **R2: `proptest::TestRunner::run` is synchronous-ish.** proptest returns its own async-style result (`TestRunner::run(&strategy, fn)`). The fn captures the HypothesisTestContext by reference; the runner is short-lived. We don't need async `await` inside the runner — the await is at `shrink()`'s outer fn level.
- **R3: `ExistencePredicate::EventTypeEquals` shrink direction is naive** (proptest's default string shrink — shrinks to shorter strings, then lexicographically smaller). If the trace sees only e.g. `Syscall` events, the shrink will converge to `""`, then to alphabet-shrink within `Syscall`. Acceptable for an m8-02 baseline; m8-05 close can refine to "shrink within the observed alphabet" if needed.
- **R4: Hand-rolled strategies per-variant means we reimplement `Arbitrary` for `PropertyValue`.** PropertyValue is a 3-variant scalar enum; ~30 LoC. Acceptable.
- **R5: Tests 4-7 may need `#[ignore]` if `HypothesisTestContext` is hard to construct in a unit test.** Fallback is documented in the merge doc.

## Cross-references

* `docs/milestones/m8-counterexample-shrinking-scoping.md` § "Architectural decisions + multi-cycle plan" — parent doc; this cycle ships item 2/5.
* `docs/milestones/m8-01-counterexample-foundation-scoping.md` + `-merge.md` — m8-01 sibling, signature-only foundation.
* `docs/milestones/m6-04-hypothesis-test.md` — closest architectural precedent. `chronos_services::hypothesis_test::eval_invariant` etc. are the algorithms the shrink loop re-runs.
* `docs/chronos-agentic-reconstruction/docs/specs/RUNTIME_PROPERTIES_AND_SLICING.md` § "Counterexample shrinking" — spec subsection lines 54-62. Specifically: "preserve the same property violation" while shrinking the input that triggers it (m8-02 §2 step 4c).
* `docs/chronos-agentic-reconstruction/docs/roadmap/ROADMAP.md` line 179 — M8 work item 2 ("input/counterexample artifact") + work item 3 ("stabilised shrinking").
* `docs/chronos-agentic-reconstruction/docs/roadmap/MILESTONE_ACCEPTANCE.md` § M8 line 78 — end-to-end acceptance criterion (m8-05; m8-02 partial).

---

— Submitted 2026-09-11. Awaits m8-02 execute cycle kickoff.
