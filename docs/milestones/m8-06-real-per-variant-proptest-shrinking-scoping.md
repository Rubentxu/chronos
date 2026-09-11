# m8-06 — real per-variant proptest shrinking (closes m8-05 R3)

**Branch:** `feat/m8-06-real-shrinkers`
**Cycle:** M8 §7, sixth execute cycle of the M8 multi-cycle plan. **M8-debt-closing** — not a new milestone.
**Predecessor doc:** `docs/milestones/M8-CLOSE.md` §7 ("m9+ handoff") and `docs/milestones/m8-05-proptest-shrinking-pagination-events-tool-m8-close-scoping.md` §B1. m8-01..m8-05 are all FF-merged to main.
**Status:** SCOPING — 2026-09-11

## Why this cycle

m8-05 shipped the **scaffolding** for real proptest shrinking:
- `build_strategy_for(target) -> SBoxedStrategy<HypothesisInput>` dispatches per `target.kind`.
- `drive_strategy<F, Fut>` runs proptest's `ValueTree` manually from async land (rebuild-each-iter work-around for `dyn ValueTree: !Send`, m8-05 R7).
- `SBoxedStrategy` chosen over `BoxedStrategy` for Send-compliance (m8-05 R5).

But the three per-variant helpers (`constant_strategy`, `existence_predicate_strategy`, `call_path_strategy`) all return `Just(base)` — i.e., no actual shrinking. The m8-05 close report documented this as **m8-05 R3** with an honest disclosure: "future cycle can swap in real shrinkers without changing any signature."

m8-06 is that future cycle. It does **not** add new public API surface, **not** change the wire envelope, **not** touch the store, **not** add new MCP tools. It replaces the bodies of the three helpers with real `Strategy<T>` impls that produce smaller variants of the captured hypothesis. `rounds_used` then reflects the **actual** proptest iteration count, not the `Just(base)`-forced 2.

This is the smallest possible cycle that closes the M8 acceptance criterion: **"A generated failing input shrinks while preserving the same property violation."** Today the violation is preserved (no breakage), but there is no shrinking (acceptance criterion not met). After m8-06, the criterion is met for at least `Number` and `Text` constant targets; `Bool` is degenerate (only 2 values) and is documented.

The M8 backlog reduces to:
- **m8-04 R-hypothesis-reconstruction-fidelity** (target_hypothesis persistence for byte-faithful `chronos test replay`) — still m9+.
- **Bundle-as-blob → side table** (m8-04 R4) — still m9+.
- **Single-call `session_stop`** (m7-07) — still m7+.

None of these is touched by m8-06.

## What's already in place (foundations m8-06 builds on)

- **`build_strategy_for`** (m8-05, `crates/chronos-services/src/counterexample.rs`): the dispatcher. Splices session_id + variant-fixed fields into the per-variant helper's output. m8-06 does NOT touch this function.
- **`drive_strategy<F, Fut>`** (m8-05): the async loop. Uses rebuild-each-iter (R7) and probes the fresh tree's `simplify()`. m8-06 does NOT touch this function either.
- **`SBoxedStrategy` Send-compliance** (m8-05 R5): the wire contract is fixed. New strategies must satisfy `HypothesisInput: Send + Sync + 'static` (m8-05 R4).
- **Per-variant helpers** (`constant_strategy`, `existence_predicate_strategy`, `call_path_strategy`): the three sites to replace.

## Architectural decisions

### D1 — `PropertyValue::Number` shrinks toward 0.0 via binary search

`PropertyValue::Number(f64)` is the variant with the most obvious shrinker. We use `proptest::num::f64::BinarySearch` semantics: starting from `base`, walk toward 0.0 by halving the distance, then by small additive steps for the last few ULPs.

- **Implementation**: hand-rolled `NumberShrinker { value: f64, target: f64 }` implementing `proptest::strategy::ValueTree<f64>`. `current()` returns `value`; `simplify()` picks `value = (value - target) / 2.0 + target`, returning `true` if it changed. `complicate()` is not used (we never re-expand).
- **Convergence**: in <= 64 rounds (log2 of 2^52 ULP precision × 4 for the additive fallback), `value` reaches 0.0 to within rounding error. The loop in `drive_strategy` already bounds `rounds_used` via the caller-supplied `max_rounds` (default 100).
- **Why hand-rolled instead of `proptest::num::f64::BinarySearch`**: proptest does ship `proptest::num::f64::POSITIVE | NEGATIVE | ANY`, but their `simplify()` walks toward the *bounds* of the strategy, not toward 0. We want a known anchor (0.0) for the M8 acceptance criterion ("shrinks while preserving the violation"). Hand-rolling is ~40 lines.

**Disclosure (m8-06 R1)**: f64 has 2^52 ULPs between adjacent integers; we converge to 0.0 within rounding error, NOT to exact 0.0. A test asserts `value.abs() < f64::EPSILON`. If the violation depends on exact-zero, the shrink will not find it.

### D2 — `PropertyValue::Text` shrinks toward `""` via character-level deletion

`PropertyValue::Text(String)` shrinks by deleting characters from the start, then the end, then alternating interior positions. This is the same algorithm proptest's `String` strategy uses internally for the `.*` regex pattern.

- **Implementation**: hand-rolled `TextShrinker { value: String }`. `current()` returns the string; `simplify()` returns a smaller string by removing one character (round-robin: start, end, middle, then second-to-start, etc.). Once `value.len() == 0`, `simplify()` returns `false`.
- **Convergence**: `O(len)` rounds. For a 256-character string, 256 rounds. The loop bounds `rounds_used` to `max_rounds` (100 by default — see D4). For strings > 100 chars, the strategy will not fully converge; this is acceptable because the M8 acceptance criterion only requires "shrinks while preserving", not "shrinks to minimum".

**Disclosure (m8-06 R2)**: the lexicographic character-level shrinker is deterministic but does NOT do "drop prefix" / "drop suffix" as separate steps (proptest does). This is fine — we just need any monotonic shrink, not proptest's exact algorithm.

### D3 — `PropertyValue::Bool` stays as `Just(base)`

`Bool` has only two values. The proptest semantics for `bool` are "produce one of `true`, `false`" with no meaningful shrink. The current `Just(base.clone())` is correct.

**Disclosure (m8-06 R3)**: `PropertyValue::Bool` is intentionally NOT shrunk. If the operator wants to test both `true` and `false` cases, they should issue two shrink calls. This is the same discipline as the comparison-direction disclosure in m8-05 ("operators who want to compare across comparison directions should issue a separate shrink call with a different target").

### D4 — `ExistencePredicate` shrinks the payload only; the variant is fixed

`ExistencePredicate` is a sum type with at least 4 variants (per the m6-04 dispatcher): `EventTypeEquals { event_type }`, `ThreadEquals { thread_id }`, `PropertyKeyEquals { target }`, and possibly more. We shrink the **payload** (string → `""`, u64 → 0) but keep the variant constant — we don't switch from `EventTypeEquals` to `ThreadEquals`, because that's a hypothesis-shape change, not a payload change.

- **Implementation**: per-variant helper functions in `existence_predicate_strategy` (replacing the m8-05 `Just(base)`). Each returns an `SBoxedStrategy<ExistencePredicate>` that varies only the payload field.
- **Convergence**: same as D2 for strings (O(len)) and D1 for u64 (O(log ULP)).

**Disclosure (m8-06 R4)**: we do NOT try to enumerate variants. If the operator wants to test `EventTypeEquals` vs `ThreadEquals` for the same session, they issue two shrink calls with different `target.predicate` values. This matches the m8-05 R3 variant-comparison discipline.

### D5 — `CallPath { caller, callee, max_depth }` shrinks each field independently

`CallPath` is the only variant where all three fields (caller, callee, max_depth) are part of the shrinking surface. We shrink each independently, in lockstep per round:

- `caller`: shrinks toward `""` (D2 algorithm).
- `callee`: shrinks toward `""` (D2 algorithm).
- `max_depth`: shrinks toward `Some(1)` then `None`.

The pair `(caller, callee)` is shrunk **simultaneously** — each round deletes one character from one of the two strings (round-robin). This is faster than shrinking each to `""` independently because most call-path violations depend on the *relationship* between the two strings, not their absolute lengths.

- **Implementation**: hand-rolled `CallPathShrinker { caller, callee, max_depth, round: usize }`. One round = one character deletion from one field. `simplify()` returns true until all three fields are at their minima.

**Disclosure (m8-06 R5)**: if `max_depth` is `None`, the strategy treats it as `usize::MAX` for shrinking purposes and stops at `None` once reached. This is the only place where a `None → None` "shrinker" is correct; everywhere else, shrinking toward `None` would be a bug.

### D6 — `max_rounds` cap preserved; new default 64

The m8-05 `drive_strategy` already accepts a `max_rounds` argument. m8-06 leaves the default at the m8-05 value (100) but documents that, for the new shrinkers, `rounds_used` typically lands in 8..64 for `Number`, 1..32 for `Text(<32 chars)`, and 1..8 for `CallPath`. `Bool` is always 2 (initial + 1 simplify-attempt that returns false).

The `max_rounds` cap is the **only** mechanism that prevents infinite shrinking loops. The cap is enforced inside `drive_strategy`, not in the per-variant helpers. m8-06 does NOT change this.

## Concrete API surface (no inventions)

The only public surface change is the implementation bodies of three free functions in `crates/chronos-services/src/counterexample.rs`:

```rust
// Before m8-06 (m8-05 R3):
fn constant_strategy(base: &PropertyValue) -> SBoxedStrategy<PropertyValue> {
    match base {
        PropertyValue::Number(_) => Just(base.clone()).sboxed(),
        PropertyValue::Text(_) => Just(base.clone()).sboxed(),
        PropertyValue::Bool(_) => Just(base.clone()).sboxed(),
    }
}
fn existence_predicate_strategy(base: &ExistencePredicate) -> SBoxedStrategy<ExistencePredicate> {
    Just(base.clone()).sboxed()
}
fn call_path_strategy(caller: &str, callee: &str, max_depth: Option<usize>) -> SBoxedStrategy<(String, String, Option<usize>)> {
    Just((caller.to_string(), callee.to_string(), max_depth)).sboxed()
}

// After m8-06:
fn constant_strategy(base: &PropertyValue) -> SBoxedStrategy<PropertyValue> {
    match base {
        PropertyValue::Number(n) => number_strategy(*n).sboxed(),
        PropertyValue::Text(s) => text_strategy(s).sboxed(),
        PropertyValue::Bool(_) => Just(base.clone()).sboxed(),  // D3
    }
}
fn existence_predicate_strategy(base: &ExistencePredicate) -> SBoxedStrategy<ExistencePredicate> {
    existence_predicate_variant_strategy(base)  // D4
}
fn call_path_strategy(caller: &str, callee: &str, max_depth: Option<usize>) -> SBoxedStrategy<(String, String, Option<usize>)> {
    call_path_shrinking_strategy(caller, callee, max_depth)  // D5
}
```

New helpers (private, file-scope):
- `number_strategy(n: f64) -> impl Strategy<Value = f64>` with `NumberShrinker` value tree.
- `text_strategy(s: &str) -> impl Strategy<Value = String>` with `TextShrinker` value tree.
- `existence_predicate_variant_strategy(base: &ExistencePredicate) -> SBoxedStrategy<ExistencePredicate>` (matches on variant, dispatches to payload-specific strategy).
- `call_path_shrinking_strategy(caller: &str, callee: &str, max_depth: Option<usize>) -> SBoxedStrategy<(String, String, Option<usize>)>` with `CallPathShrinker` value tree.

**No DTO changes. No wire envelope changes. No store changes. No MCP tool changes. No CLI changes.**

## Test plan

### T1 — Per-variant helper unit tests (services unit)

- `m8_06_number_strategy_shrinks_toward_zero`: assert that starting from `Number(100.0)`, after 8 rounds the value is `< 1.0`.
- `m8_06_number_strategy_converges_within_max_rounds`: assert that starting from `Number(1000.0)`, after 64 rounds the value is `< f64::EPSILON`.
- `m8_06_text_strategy_shrinks_toward_empty`: assert that starting from `Text("hello")`, after 5 rounds the string is one of `["", "ello", "hllo", "helo", "hell"]` (one char deleted).
- `m8_06_text_strategy_converges_to_empty`: assert that starting from `Text("hi")`, after 2 rounds the string is `""`.
- `m8_06_bool_strategy_returns_base`: assert `Bool(true)` strategy always returns `true`; same for `false`. (D3 disclosure.)
- `m8_06_existence_predicate_strategy_event_type_shrinks_string`: assert that starting from `EventTypeEquals { event_type: "OPEN" }`, the payload shrinks to `""`.
- `m8_06_existence_predicate_strategy_thread_shrinks_u64`: assert that starting from `ThreadEquals { thread_id: 1000 }`, the value shrinks toward 0.
- `m8_06_call_path_strategy_shrinks_caller_and_callee`: assert that `(caller="foo", callee="bar", max_depth=Some(3))` after a few rounds shrinks to shorter variants.
- `m8_06_call_path_strategy_max_depth_shrinks_to_none`: assert that `max_depth=Some(5)` eventually becomes `None`.

### T2 — `drive_strategy` integration with real shrinkers

- `m8_06_drive_strategy_with_number_shrinker_terminates`: assert that `drive_strategy` returns within `max_rounds` and the final `minimised` has `|constant - 0.0| < EPSILON`.
- `m8_06_drive_strategy_with_text_shrinker_terminates`: same for Text, assert `constant == ""`.
- `m8_06_drive_strategy_rounds_used_for_number_is_at_least_8`: assert `rounds_used >= 8` (the m8-05 "always 2" assertion is replaced).

### T3 — Sandbox smoke

- `ce10_shrink_number_target_rounds_used_at_least_8`: invoke `counterexample_shrink` with a `Number(1000.0)` constant target on `test_busyloop`. Assert `rounds_used >= 8` and `minimised.constant.unwrap()` is approximately `Number(0.0)` (within `EPSILON`).
- `ce11_shrink_text_target_rounds_used_at_least_2`: invoke with a `Text("OPEN_HTTP")` constant target. Assert `rounds_used >= 2` and `minimised.constant.unwrap()` is `Text("")` or a 1-char prefix.

`ce1..ce9` continue to pass (no envelope change). The "rounds_used == 2" assertions in ce1, ce5, ce7 are updated to "rounds_used >= 2".

## Risks and known limitations

- **m8-06 R1**: f64 shrinker converges to `0.0 ± EPSILON`, not exact `0.0`. Violations that depend on exact-zero will not be found.
- **m8-06 R2**: lexicographic character-level shrinker is deterministic but not byte-optimal. Acceptable for the acceptance criterion.
- **m8-06 R3**: `Bool` is degenerate; the operator must issue two shrink calls.
- **m8-06 R4**: `ExistencePredicate` variant is fixed; the operator must issue multiple shrink calls for variant enumeration.
- **m8-06 R5**: `max_depth=None` is the shrinker's terminal state.
- **m8-06 R6 (carried from m8-05)**: shrink requires populated engines map; empty map returns `SessionNotFound` (R6).
- **m8-06 R7 (carried from m8-05)**: `dyn ValueTree: !Send`; rebuild-each-iter is the work-around (R7).
- **m8-06 R8 (carried from m8-05)**: `HypothesisInput: Send + Sync + 'static` requirement (R4). All new strategy helpers satisfy this.
- **m8-06 R9 (NEW)**: `max_rounds` is the only mechanism preventing infinite loops. The cap is enforced inside `drive_strategy`, not in the per-variant helpers. A future cycle could add per-helper sanity checks (e.g., "if `value` didn't change in 3 rounds, terminate").

## Cross-references

- `docs/milestones/M8-CLOSE.md` §6 — m8-05 R3 disclosure being closed.
- `docs/milestones/m8-05-proptest-shrinking-pagination-events-tool-m8-close-scoping.md` §B1 — the original design (R3 honest disclosure).
- `docs/milestones/m8-counterexample-shrinking-scoping.md` — M8 master plan, §"Risks and known limitations" line 177 (the per-variant `Strategy` discovery).
- `docs/chronos-agentic-reconstruction/docs/roadmap/MILESTONE_ACCEPTANCE.md` §M8 line 80 — the acceptance criterion this cycle closes.
- `docs/chronos-agentic-reconstruction/docs/specs/RUNTIME_PROPERTIES_AND_SLICING.md` §"Counterexample shrinking" — spec subsection that motivates the per-variant dispatch.

---

— Submitted 2026-09-11. Awaits m8-06 execute cycle kickoff.
