# M8-07 scoping — Persist original `target_hypothesis` for byte-faithful replay

**Cycle:** `m8-07-hypothesis-reconstruction-fidelity`
**Branch:** `feat/m8-07-hypothesis-reconstruction-fidelity`
**Base:** `148f009` (main at m8-06 close)
**Status:** PROPOSED — 2026-09-11

## §1 Problem statement

m8-04 R-hypothesis-reconstruction-fidelity (carried from the m8-04 close
report) discloses that `chronos test replay <bundle_id>` reconstructs a
`HypothesisInput` from the bundle's MINIMISED payload using synthetic
defaults:

```rust
// crates/chronos-cli/src/replay.rs:138-141
MinimisedPayload::Constant(v) => {
    input.scope = Some(HypothesisScope::PropertyValue);  // <-- synthetic default
    input.constant = Some(v.clone());
}
```

This is fine for the M8 acceptance scenario (re-running the SAME minimised
hypothesis against the SAME events must still produce a violation), but it
fails for two important replay use-cases:

1. **Diagnosing a "shrank past the violation" bug.** When the shrinker
   converges to a minimised payload that no longer violates (e.g., the
   `Ge/1000` → `Ge/0` binary search returned a non-violating min), the
   agent's only path back to "what was the original hypothesis?" is to
   re-derive it from the minimised payload + assumed defaults. The
   synthetic defaults in `reconstruct_hypothesis` then produce a DIFFERENT
   `HypothesisInput` from what the agent originally passed — so the
   `replay` output no longer matches the original `shrink` call.

2. **Non-default Invariant options.** For `Invariant` with
   `scope=EventCount, comparison=Ge, constant=Number(1000)`, the
   minimised payload is `Constant(Number(0.0))` — but the SCOPE and
   COMPARISON are lost. `replay` reconstructs as
   `scope=PropertyValue, comparison=None, constant=Number(0.0)` — which
   is a DIFFERENT hypothesis with a different evaluation rule.

The fix is to persist the original `target_hypothesis` (the user's wire
input to `counterexample_shrink`) in the bundle record alongside the
minimised payload. `replay` then reads the original hypothesis and uses it
verbatim, falling back to the synthetic-default reconstruction only when
the bundle predates m8-07 (no `target_hypothesis` field).

## §2 Goals & non-goals

### Goals

1. Add `target_hypothesis: Option<HypothesisInputWire>` field to
   `chronos-store::CounterexampleBundleRecord`.
2. Add `HypothesisInputWire` to `chronos-store` as a field-for-field
   mirror of `chronos_services::hypothesis_test::HypothesisInput`. This
   matches the R5 disclosure precedent (ExistencePredicateWire).
3. `shrink()` passes the original `target_hypothesis` to `save()` so it
   gets persisted on every shrink call.
4. `chronos-cli::replay::reconstruct_hypothesis` reads
   `bundle.target_hypothesis` first; falls back to the synthetic-default
   reconstruction if absent (pre-m8-07 bundles).
5. Sandbox ce12: shrink with `scope=EventCount, comparison=Ge,
   constant=Number(1000)`, replay, assert the reconstructed
   `HypothesisInput` matches the original (scope, comparison, constant
   all preserved).
6. Backward compatibility: bundles persisted pre-m8-07 have NO
   `target_hypothesis` field; serde's `#[serde(default)]` makes this a
   clean load. No migration script needed.

### Non-goals (deferred to m9+)

- **Persisting the literal JSON the user sent.** We persist the typed
  `HypothesisInput` shape; if the user passed extra unknown fields via
  a future wire schema, they won't round-trip. Adding a separate
  `target_hypothesis_raw_json: Option<String>` for that case is m9+.
- **Compacting the events table** (m8-04 R4). Out of scope.
- **Re-running shrink from replay.** `chronos test replay` re-tests
  the (minimised) hypothesis; it does NOT re-run shrink. The
  bundle-as-input is enough for the agent to decide what to do next.

## §3 Architectural decisions

### D1 — Wire mirror vs raw JSON

Following the R5 precedent (ExistencePredicateWire), we add a typed wire
mirror `HypothesisInputWire` in chronos-store rather than a raw JSON blob.
Pros:

* chronos-store stays the type authority for the bundle record schema;
  the wire mirror is checked at compile time.
* Round-tripping is deterministic — no JSON ordering / whitespace
  surprises.

Cons:

* Two types to keep in sync (mitigated by the test in §5).
* chronos-store grows by ~10 fields of trivial data. Acceptable.

### D2 — Field name and serde semantics

`target_hypothesis: Option<HypothesisInputWire>` with `#[serde(default)]`
on the field. Bundles persisted before m8-07 have no such field; on load,
`target_hypothesis = None`. The replay path uses `Option` and falls back
to the synthetic-default reconstruction.

### D3 — Replay fallback semantics

```rust
let target_hypothesis = bundle.target_hypothesis
    .as_ref()
    .map(|w| hypothesis_input_from_wire(w.clone()))
    .unwrap_or_else(|| reconstruct_synthetic_default(bundle)?);
```

If `target_hypothesis` is present, use it. Otherwise, use the
synthetic-default path (current m8-04 behaviour). This preserves
backward compatibility for the ~3 pre-m8-07 bundles in redb.

### D4 — Minimised payload stays

The `MinimisedPayload` field is NOT replaced by `target_hypothesis`. The
minimised payload IS the result of shrinking; `target_hypothesis` IS the
input. Both have independent value. For the M8 acceptance criterion
("the minimised payload still violates"), `replay` runs the MINIMISED
hypothesis against the same events (current behaviour); the
`target_hypothesis` is exposed via `chronos test replay --with-target`
flag (m9+). For m8-07, the only consumer of `target_hypothesis` is the
test suite; the CLI just preserves it for future use.

Wait — re-reading §4 of the existing replay.rs code, the CLI does
project `bundle.summary.bundle_id` etc into a flat report. The
`target_hypothesis` field will be visible in the bundle's redb row but
NOT in the CLI report output. That's fine — the CLI is for
acceptance-grade output; a future `chronos test replay --verbose` can
surface it.

### D5 — No new MCP tool surface

No new MCP tool is added in m8-07. The change is internal to the
bundle schema + replay path. Agents don't see a new tool; they only see
more accurate replay behaviour (which is observable indirectly via
existing tests).

## §4 Implementation sketch

### chronos-store (1 file, ~25 LoC)

`crates/chronos-store/src/counterexample_storage.rs`:

```rust
/// Wire mirror of chronos_services::hypothesis_test::HypothesisInput.
/// See chronos-services::counterexample::hypothesis_input_to_wire /
// hypothesis_input_from_wire for the conversion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct HypothesisInputWire {
    pub session_id: String,
    pub kind: String,                 // "invariant" | "existence" | "call_path"
    pub scope: Option<String>,        // "event_count" | "property_value" | "latency_ms"
    pub comparison: Option<String>,   // "Eq" | "Ne" | "Ge" | ...
    pub constant: Option<PropertyValue>,
    pub property_target: Option<String>,
    pub predicate: Option<ExistencePredicateWire>,
    pub caller: Option<String>,
    pub callee: Option<String>,
    pub max_depth: Option<u64>,
}

pub struct CounterexampleBundleRecord {
    pub summary: CounterexampleBundleSummary,
    pub events: Vec<TraceEvent>,
    pub minimised: Option<MinimisedPayload>,
    #[serde(default)]
    pub event_cas_hashes: Vec<ContentHash>,
    /// m8-07: original HypothesisInput the user passed to
    /// counterexample_shrink. None for bundles persisted before m8-07.
    #[serde(default)]
    pub target_hypothesis: Option<HypothesisInputWire>,
}
```

### chronos-services (1 file, ~70 LoC)

`crates/chronos-services/src/counterexample.rs`:

```rust
fn hypothesis_input_to_wire(h: &HypothesisInput) -> HypothesisInputWire {
    HypothesisInputWire {
        session_id: h.session_id.clone(),
        kind: hypothesis_kind_as_str(h.kind).to_string(),
        scope: h.scope.as_ref().map(hypothesis_scope_as_str).map(String::from),
        comparison: h.comparison.as_ref().map(comparison_op_as_str).map(String::from),
        constant: h.constant.clone(),
        property_target: h.property_target.clone(),
        predicate: h.predicate.as_ref().map(existence_predicate_to_wire),
        caller: h.caller.clone(),
        callee: h.callee.clone(),
        max_depth: h.max_depth.map(|d| d as u64),
    }
}

fn hypothesis_input_from_wire(w: HypothesisInputWire) -> HypothesisInput {
    HypothesisInput {
        session_id: w.session_id,
        kind: match w.kind.as_str() {
            "invariant" => HypothesisKind::Invariant,
            "existence" => HypothesisKind::Existence,
            "call_path" => HypothesisKind::CallPath,
            other => panic!("unknown hypothesis kind on disk: {other}"), // bounded by writer
        },
        scope: w.scope.as_deref().and_then(hypothesis_scope_from_str),
        comparison: w.comparison.as_deref().and_then(comparison_op_from_str),
        constant: w.constant,
        property_target: w.property_target,
        predicate: w.predicate.map(existence_predicate_from_wire),
        caller: w.caller,
        callee: w.callee,
        max_depth: w.max_depth.map(|d| d as usize),
    }
}
```

Update `save()` to accept a `target_hypothesis: &HypothesisInput` arg
(or have `shrink()` build the record directly). Update `shrink()` to
pass `target_hypothesis` through.

### chronos-cli (1 file, ~30 LoC)

`crates/chronos-cli/src/replay.rs`:

```rust
fn reconstruct_hypothesis(
    bundle: &CounterexampleBundleRecord,
    session_id: &str,
) -> Result<HypothesisInput> {
    if let Some(wire) = &bundle.target_hypothesis {
        return Ok(hypothesis_input_from_wire(wire.clone()));
    }
    // Pre-m8-07 fallback: reconstruct from the minimised payload.
    // ... existing logic ...
}
```

### Sandbox (1 new test)

`chronos-sandbox/tests/counterexample_tools.rs`:

```rust
/// CE12: shrink with non-default Invariant options, replay, assert
/// reconstructed hypothesis matches the original (m8-07).
#[tokio::test]
async fn ce12_replay_preserves_non_default_invariant_options() {
    // ... probe_start, counterexample_shrink with scope=EventCount,
    //     comparison=Ge, constant=Number(1000), then reload via
    //     counterexample_get, then assert bundle.target_hypothesis
    //     carries the same scope/comparison/constant.
}
```

## §5 Tests

### Unit (chronos-services lib)

* `hypothesis_input_to_wire_roundtrip` — convert a sample
  `HypothesisInput` to wire and back, assert equal.
* `hypothesis_input_to_wire_handles_all_kinds` — exercise Invariant,
  Existence, CallPath variants.

### Per-crate integration (chronos-store)

* `save_then_load_preserves_target_hypothesis` — store a bundle with
  `target_hypothesis = Some(...)`, load, assert `target_hypothesis`
  survives the round-trip.
* `load_legacy_bundle_target_hypothesis_is_none` — store a bundle
  without `target_hypothesis`, load, assert `target_hypothesis = None`.

### Sandbox (T4-smoke)

* `ce12_replay_preserves_non_default_invariant_options` — exercises
  the full shrink → save → replay path with non-default Invariant
  options.

## §6 Disclosures

### R1 (NEW) — HypothesisInputWire is a hand-maintained mirror

The wire mirror is duplicated from `chronos_services::hypothesis_test::HypothesisInput`.
Drift is possible if `HypothesisInput` evolves without updating the wire
mirror. Mitigated by the roundtrip test in §5.

### R2 (NEW) — Unknown future wire fields are dropped

If a future chronos-services version adds a field to `HypothesisInput`,
the wire mirror will silently drop it on persist. The `target_hypothesis`
field on the redb row is not versioned. Mitigated by the m9+ plan to
add `schema_version: u32` to `CounterexampleBundleSummary` (separate
followup).

### R3 (NEW) — Replay CLI does NOT expose target_hypothesis

The CLI's flat report (`chronos test replay <bundle_id>`) does not
surface the original `target_hypothesis`. Agents wanting it must read
the redb bundle directly (via the existing `counterexample_get` MCP
tool's underlying record). A `chronos test replay --verbose` flag is
m9+.

## §7 Tier & gate plan

This is **A-min** scope:
* T0: fmt + clippy
* T1: chronos-services lib
* T2: chronos-services + chronos-store + chronos-cli integration
* T4-smoke: ce12 + existing counterexample_tools (full file re-run to
  catch any envelope change)

No chronos-e2e (D bucket), no benches (E bucket).

## §8 Smoke subset chosen

Per AGENTS.md §2, the cycle touches the bundle schema (redb row layout)
+ replay path (CLI), so the smoke subset is:

- `counterexample_tools` (full file, 11 tests) — ce1..ce11 cover the
  wire envelope; ce12 (NEW) covers the target_hypothesis round-trip.

`e2e_connectivity` not re-run (no change to MCP startup plumbing).

## §9 What's NOT closed by m8-07

- **m8-04 R4 — bundle-as-blob → side table**: events still ride inside
  the bundle blob. m9+ scope.
- **m8-06 R4 — cross-variant existence predicate shrinking**: variant
  still fixed in ExistencePredicateShrinker. m9+ scope.
- **R-hypothesis-reconstruction-fidelity is now closed by m8-07.**
  (R1/R2/R3 are NEW m8-07 disclosures for the wire mirror schema.)
