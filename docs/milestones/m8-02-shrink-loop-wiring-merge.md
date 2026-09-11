# m8-02 — shrink loop wiring merge doc

**Cycle:** M8 §2, second execute cycle of the M8 multi-cycle plan.
**Predecessor doc:** `docs/milestones/m8-02-shrink-loop-scoping.md` (PROPOSED, FF-merged before this cycle).
**Branch:** `feat/m8-02-shrink-loop-wiring` (FF-merged to `main` after this cycle).
**Status:** MERGED — 2026-09-11

## What shipped

### 1. `proptest` promoted to runtime dep on `chronos-services`

`crates/chronos-services/Cargo.toml`: added `proptest = { workspace = true }` to `[dependencies]` (kept the same in `[dev-dependencies]`). The dep is now reachable from the dispatched `shrink()` code rather than only from unit tests.

**Disclosure:** `proptest = "1.5"` brings in `bit-set`, `bit-vec`, `rand`, `rusty-fork`, `tempfile`, `regex-syntax`, `timeout` (default features). Compile-time cost: ~6-8s longer `cargo check -p chronos-services`. Acceptable per scoping doc §7.

### 2. `ShrinkConfig` + `From<ShrinkConfig> for proptest::test_runner::Config`

```rust
pub struct ShrinkConfig {
    pub max_rounds: u32,
    pub seed: Option<u64>,
}
impl Default for ShrinkConfig { /* max_rounds = DEFAULT_SHRINK_MAX_ROUNDS (64), seed = None */ }
impl From<ShrinkConfig> for proptest::test_runner::Config { /* cases = max(max_rounds, 1); rng_seed = Fixed(seed) or Random; rest via Default::default() */ }
```

The struct-update syntax in `From` satisfies the project's `-D warnings` policy on `clippy::field_reassign_with_default`. (`proptest::test_runner::Config` has ~30 fields; we only touch `cases` and `rng_seed`.)

**Disclosure:** proptest 1.5 exposes no separate `max_shrinks` knob — `cases` is the total sample budget (including shrinking). Per scoping doc §2 step 2 disclosure, the budget maps 1:1 because the shrink loop does not generate new failing inputs from scratch.

### 3. `CounterexampleOutput::Shrunk` variant

```rust
pub enum CounterexampleOutput {
    Got { summary: CounterexampleBundleSummary },                                                                  // m8-01
    Listed { summaries: Vec<CounterexampleBundleSummary>, next_cursor: Option<String> },                             // m8-01
    Shrunk {                                                                                                         // m8-02
        bundle: CounterexampleBundleSummary,
        rounds_used: u32,
        minimised_constant: Option<PropertyValue>,
        minimised_predicate: Option<ExistencePredicate>,
        minimised_call_path: Option<(String, String, Option<usize>)>,
    },
}
```

**Disclosure**: at most one of `minimised_constant` / `minimised_predicate` / `minimised_call_path` is `Some`, dictated by `bundle.property_kind`. m8-03 promotes to wire DTO with flattened shape.

### 4. `CounterexampleRunError` + `From<CounterexampleRunError> for ServiceError`

Three-variant service-internal error enum:

| Variant | Maps to | Rationale |
|---|---|---|
| `NoViolation { reason }` | `ServiceError::Unsupported(reason)` | target_hypothesis already passes/unsupported; nothing to shrink |
| `RoundsExhausted { best_so_far }` | `ServiceError::Unsupported("shrink rounds exhausted")` | proptest burned through `max_rounds` without further shrinking |
| `AdapterFailed(String)` | `ServiceError::EvalError(s)` | hard error — Strategy adapter itself failed |

No new `ServiceError` variants added.

### 5. `ChronosCounterexampleService::shrink` async entry point

`pub async fn shrink(ctx, input)`:

1. **Pattern-match input:** only the `Shrink { ... }` variant. Get/List dispatched here = `Err(EvalError)`.
2. **Pre-validate:** invoke `ChronosHypothesisTestService::test(ctx.hypothesis_ctx, target_hypothesis.clone())`. If verdict is `Pass`/`Unsupported`, return `Err(Unsupported(...))` early — no budget wasted on inputs that already pass.
3. **Build proptest config:** `ShrinkConfig { max_rounds: max(max_rounds, 1), seed }` → `proptest::test_runner::Config`.
4. **Build runner:** `proptest::test_runner::TestRunner::new(p_cfg)`.
5. **Dispatch to per-variant strategy:**
   - `HypothesisKind::Invariant → shrink_invariant(&runner, ctx, &target)`
   - `HypothesisKind::Existence → shrink_existence(...)`
   - `HypothesisKind::CallPath → shrink_call_path(...)`
6. **Synthesise the bundle** (in-memory only — `has_full_bundle = false`):
   - `bundle_id = uuid::Uuid::now_v7()`
   - `property_kind`, `workspace_id = "ws-default"` (placeholder; m8-03 derives from request)
   - `created_at_ms`, `rounds_used`
   - `has_full_bundle = false`
7. **Return** `CounterexampleOutput::Shrunk { ... }`.

### 6. Per-variant strategy stubs (m8-02 honest disclosure)

`shrink_invariant`, `shrink_existence`, `shrink_call_path` are **type-shape stubs** — they return the original minimised field unchanged with `rounds_used=1`. They establish the per-variant dispatch pattern but the actual `proptest::TestRunner::run(&strategy, closure)` shrink loop is **deferred to m8-03** scope for these reasons:

- Real captured-trace shrinking requires a live `QueryEngine` map. m8-02 unit tests cannot construct one without redb init.
- The per-variant `Strategy<Value = ...>` adapter needs an actual proptest channel test to verify; that's sandbox-smoke scope (m8-03).
- m8-02's progress is the wiring + signature + error handling + type invariants — the **algorithm runs in m8-03**.

This is captured in the apply-checkpoint `notes[].experimental_disclosures[]`.

### 7. Helper visibility fix (m8-01 carry-forward)

Lifted `now_unix_ms()` and `fresh_bundle_id()` out of `#[cfg(test)]` — m8-01 had them cfg-test-only because at that point no production caller existed. m8-02's `shrink` calls them; they're now production-public. Doc-string updated.

### 8. Unit tests (5 new, sandbox-free)

1. `m8_02_shrink_config_default_matches_pinned_max_rounds` — pins default 64.
2. `m8_02_shrink_config_to_proptest_enforces_case_ceiling` — pins 1:1 case mapping.
3. `m8_02_run_error_maps_to_documented_service_error_variants` — pins the B7 (no new ServiceError variants) decision.
4. `m8_02_shrunk_variant_carries_exactly_one_payload` — pins the "exactly one of three Option fields is Some" invariant.
5. `m8_02_shrink_input_variant_carries_documented_four_fields` — pins the input-variant shape for m8-03 wire DTO reuse.

## Test results

| Tier | Command | Result |
|---|---|---|
| **T0 — fmt + clippy** | `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings` | PASS, 0 warnings |
| **T1 — lib unit** | `cargo test --workspace --lib --no-fail-fast --exclude chronos-native --exclude chronos-sandbox --exclude chronos-e2e` | **811 passed, 0 failed** (was 806 before m8-02; +5 net = 5 new m8-02 tests) |
| **T2 — per-crate integration** | `cargo test -p chronos-services -p chronos-store -p chronos-mcp --tests --no-fail-fast` | All green (chronos-services 226 = 221 + 5 new) |
| **T4-smoke — sandbox** | n/a | **not required** — m8-02 doesn't touch probes/sessions/MCP wire. Sandbox binding lives in m8-03. |

No regressions. No ignored tests touched.

## Honest disclosures (carried forward to m8-03)

1. **The shrink strategies are stubs.** `shrink_invariant`, `shrink_existence`, `shrink_call_path` return the original input with `rounds_used=1`. The real per-variant `proptest::Strategy` impls + `proptest::TestRunner::run(&strategy, closure)` loop are **m8-03 scope**, on purpose — they need a live `QueryEngine` map to evaluate the synthesised hypothesis against, which only sandbox smoke provides end-to-end.
2. **m8-02 does NOT satisfy the M8 acceptance criterion** (`MILESTONE_ACCEPTANCE.md` § M8 line 78: "A generated failing input shrinks while preserving the same property violation."). That's m8-05 (or possibly m8-03, depending on how much real shrink we can land there).
3. **No wire DTO additions** to `output.rs` in m8-02. The `CounterexampleOutput::Shrunk` is service-internal; m8-03 introduces the corresponding `CounterexampleShrunkOutputDto` (with flattened `minimised: serde_json::Value`).
4. **`HypothesisKind`'s additive Serialize from m8-01 still applies** — the `Shrunk` variant doesn't re-touch it.
5. **m8-02 runtime dep on `proptest` is `chronos-services`-only**, not workspace-wide. Per A1 in the M8 parent doc.

## What did NOT ship (deferred, per scoping)

| Item | Cycle |
|---|---|
| Per-variant hand-rolled `Strategy<Value = PropertyValue>` for `Invariant` | **m8-03** |
| Per-variant hand-rolled `Strategy<Value = ExistencePredicate>` for `Existence` | **m8-03** |
| Per-variant hand-rolled `Strategy<Value = (String, String, Option<usize>)>` for `CallPath` | **m8-03** |
| `proptest::TestRunner::run(&strategy, closure)` loop with re-evaluation against the trace | **m8-03** |
| `counterexample_shrink` MCP wrapper | **m8-03** |
| `counterexample_get` MCP wrapper (replaces m8-01 stub) | **m8-03** |
| `counterexample_list` MCP wrapper (replaces m8-01 stub) | **m8-03** |
| `counterexample_bundles` redb table | **m8-03** |
| Full-bundle wire DTO `CounterexampleBundleDto` (with `Vec<TraceEvent>`) | **m8-03** |
| `crates/chronos-cli` workspace member | **m8-04** |
| M8 close report | **m8-05** |

## Files touched (in this cycle's commit)

| File | Change | Lines |
|---|---|---|
| `crates/chronos-services/Cargo.toml` | Promoted `proptest` to runtime dep | +1 |
| `crates/chronos-services/src/counterexample.rs` | Added `Shrunk` variant + `ShrinkConfig` + `From` impl + `CounterexampleRunError` + `shrink()` entry + 3 strategy stubs + 5 new tests; lifted helpers out of `cfg(test)` | +322 / −5 |
| `docs/milestones/m8-02-shrink-loop-wiring-merge.md` | **This document** | (this file) |
| `sddk/changes/m8-02-shrink-loop-wiring-merge/apply-checkpoint.json` | The apply-checkpoint | (new file) |

Total net additions: **~318 LoC** plus the merge doc + checkpoint.

## Commit + tag + release

- Commit: feature commit `feat(services): m8-02 shrink loop wiring (signature + stubs + per-variant dispatch)`.
- Chore commit: apply-checkpoint sync.
- Tag: `m8-02-shrink-loop-wiring.0`.
- Apply-checkpoint: `sddk/changes/m8-02-shrink-loop-wiring-merge/apply-checkpoint.json`.
- Push: `origin main` only.

## Pattern compliance with prior cycles

| Pattern | m7-07 + m8-01 precedent | m8-02 conformance |
|---|---|---|
| Doc first, code second | yes | yes (scoping doc FF-merged before this branch) |
| Stub + machine-readable error to avoid follow-up cycle signature change | yes | yes (strategy stubs return input unchanged with rounds_used=1) |
| Wire DTOs in `output.rs`, internal enums at module scope | yes | yes (`CounterexampleRunError` stays in counterexample.rs) |
| `ServiceError::Unsupported(String)` reused for new stubs | yes | yes (NoViolation + RoundsExhausted both → Unsupported) |
| `chrono-cli` workspace member not added if cycle doesn't need it | yes | yes (m8-04) |
| T0 + T1 + T2 gates pass on cycle close | yes | yes |
| Honest disclosure of "the algorithm runs later, this cycle shipped the shape" | yes | yes (this document § "Honest disclosures" #1) |

## Cross-references

* `docs/milestones/m8-counterexample-shrinking-scoping.md` — parent doc, multi-cycle plan.
* `docs/milestones/m8-02-shrink-loop-scoping.md` — cycle-specific scoping.
* `docs/milestones/m8-01-counterexample-foundation-scoping.md` + `-merge.md` — m8-01 sibling, signature-only foundation.
* `docs/chronos-agentic-reconstruction/docs/specs/RUNTIME_PROPERTIES_AND_SLICING.md` § "Counterexample shrinking" — spec subsections 54-62.
* `docs/chronos-agentic-reconstruction/docs/roadmap/MILESTONE_ACCEPTANCE.md` § M8 line 78 — end-to-end acceptance criterion (m8-05).

---

— Submitted 2026-09-11. Cycle closed on `main`.
