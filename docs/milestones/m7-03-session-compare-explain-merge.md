# m7-03 — `session_compare` + `session_explain` merge

**Branch:** `feat/m7-03-session-compare-explain-merge`
**Cycle:** M7 (v2-spec sub-cycle), third deliverable
**Precedence:** `docs/milestones/m7-03-session-compare-explain-scoping.md`;
`docs/milestones/m7-events-read-scoping.md`; v2 spec
`AGENT_API_V2.md` lines 20–21
**Status:** PROPOSED, 2026-09-11

## Why this cycle

The v2 spec calls for two tools:

* `session_compare` — semantic comparison (per spec line 20)
* `session_explain` — structured facts/derived/inferred/hypothesis
  bundle (per spec line 21)

The current v1 tools cover the comparison side only:

* `compare_sessions` (v1) — BLAKE3 hash set-diff + similarity_pct +
  LLM-readable summary. Today at `crates/chronos-mcp/src/server.rs:4045`,
  backed by `chronos_services::diff::ChronosDiffService::compare_sessions`.
* `performance_regression_audit` (v1) — per-function call-count
  regression detection between two sessions. Today at
  `crates/chronos-mcp/src/server.rs:4011`, backed by
  `chronos_services::diff::ChronosDiffService::performance_regression_audit`.

Both v1 tools are two-session comparators. m7-03:

1. **Folds the two v1 comparators behind a single v2 endpoint**
   (`session_compare`) with a `kind: divergence | regression`
   discriminator. Both v1 names are preserved as deprecated shims.
2. **Adds a net-new v2 tool** `session_explain` with a four-tier
   bundle (facts / derived / inferred / hypothesis). No v1 analogue
   exists; this is net-new per v2 spec line 21.

This is the third M7 deliverable (after m7-01 events_read, m7-02
observe). After m7-03 ships, the M7 backlog has m7-04 (session_start
+ session_stop + capabilities), m7-05 (deprecation sunset, calendared
for 2027-09-11), and m7-06 (M7 close) remaining.

## Scope (this cycle)

- Add `chronos-services::session_compare` module (~280 LoC, 18th
  service module).
- Add `SessionCompareKind` enum (`Divergence` / `Regression`) — the
  v2 dispatcher discriminator.
- Add `SessionCompareInput` DTO carrying the kind + per-kind args.
- Add `SessionCompareOutput` envelope with `DivergenceResult` /
  `RegressionResult` variants matching the v1 `compare_sessions` /
  `performance_regression_audit` output shapes plus `provenance`.
- Add `ChronosSessionCompareService::compare` dispatcher with unit
  tests.
- Add `session_compare` v2 MCP tool wrapper.
- Add `chronos-services::session_explain` module (~220 LoC, 19th
  service module).
- Add `SessionExplainKind` enum (`Facts` / `Derived` / `Inferred` /
  `Hypothesis`).
- Add `SessionExplainInput` DTO + per-kind args.
- Add `SessionExplainOutput` envelope with `FactsBundle` /
  `DerivedBundle` / `InferredBundle` / `HypothesisBundle` variants.
- Add `ChronosSessionExplainService::explain` dispatcher with unit
  tests.
- Add `session_explain` v2 MCP tool wrapper.
- Convert v1 `compare_sessions` and `performance_regression_audit`
  to deprecated shims that route through `ChronosSessionCompareService`.
- Update `chronos_services::lib.rs` module index (16 → 18 algorithm
  modules after m7-02; 18 → 20 after m7-03).

## Out of scope

- `observe` already shipped (m7-02). No m7-03 changes there.
- `events_read` already shipped (m7-01). No m7-03 changes there.
- `session_start` / `session_stop` / `capabilities` surface
  (m7-04).
- Deprecation sunset sweep (m7-05).
- M7 close (m7-06).
- `hypothesis_test` execution (m6-04 already provides this; m7-03
  only *plans* a hypothesis via `session_explain{kind=hypothesis}`,
  does not execute it).
- Richer provenance (lineage graph, staleness windows) — m7+.
- Inferences beyond a small starter set (crash / I/O-heavy /
  CPU-bound / single-threaded) — m7+.

## Algorithm

### `session_compare{kind=divergence}`

Wraps the existing
`chronos_services::diff::ChronosDiffService::compare_sessions` algorithm:

* Input: `{ session_a, session_b }` (same as v1 `CompareSessionsInput`).
* Algorithm: `chronos_store::TraceDiff::compare` (BLAKE3 hash-based
  set diff, similarity_pct, address normalization with feature flag).
* Output: `{ session_a_id, session_b_id, only_in_a_count,
  only_in_b_count, total_a, total_b, common_count, similarity_pct,
  timing_delta_ms, summary }` + `provenance`.

### `session_compare{kind=regression}`

Wraps the existing
`chronos_services::diff::ChronosDiffService::performance_regression_audit`
algorithm:

* Input: `{ baseline_session_id, target_session_id, top_n }` (same
  as v1 `PerformanceRegressionAuditInput`).
* Algorithm: per-function call-count map for each session's
  `execution_summary().top_functions`; classify
  `delta > +50% => regression`, `delta < -50% => improvement`.
* Output: `{ baseline_session_id, target_session_id, regressions[],
  improvements[], summary }` + `provenance`.

### `session_explain{kind=facts}`

Returns the direct observations from the session:

* Input: `{ session_id }`.
* Algorithm: `chronos_query::QueryEngine` reads + filters
  `chronos_store::SessionStore`. For m7-03 the bundle carries
  `{ total_events, distinct_event_types, distinct_functions,
  duration_ms, thread_count, crash_detected, signal_delivered }`.
* Output: `FactsBundle { total_events, distinct_event_types,
  distinct_functions, duration_ms, thread_count, crash_detected,
  signal_delivered }` + `provenance`.

### `session_explain{kind=derived}`

Returns projections computed from facts:

* Input: `{ session_id }`.
* Algorithm: reuses `QueryEngine::execution_summary` + hotspot
  projection from m6-03 (`execution_query`).
* Output: `DerivedBundle { hotspots[], call_graph_summary,
  regressions_vs_none }` — `regressions_vs_none` is a typed
  `None` for m7-03 (regression detection requires a baseline; see
  `session_compare{kind=regression}`). `provenance` included.

### `session_explain{kind=inferred}`

Best-effort characterisations:

* Input: `{ session_id }`.
* Algorithm: small set of typed inferences:
  - `CrashDetected` — if `facts.crash_detected == true`.
  - `IoHeavy` — if event-type distribution shows > 50% of events
    are syscall_enter/exit for IO syscalls (`read`, `write`,
    `recv`, `send`, `accept`, `connect`).
  - `CpuBound` — if `hotspots` is dominated by one function with
    > 70% of total call counts.
  - `SingleThreaded` — if `facts.thread_count == 1`.
  - `Unknown` — when no characterisation applies.
* Output: `InferredBundle { inferences: Vec<InferredTag> }` +
  `provenance`.

### `session_explain{kind=hypothesis}`

Returns a typed `HypothesisTestPlan` that the agent can execute via
the m6-04 `hypothesis_test` tool:

* Input: `{ session_id, kind: HypothesisTestKind }` where
  `HypothesisTestKind ∈ { CrashInvariant, DominantFunctionCallPath }`.
  (Starter set; agent-driven composition is m7+.)
* Output: `HypothesisBundle { plan: HypothesisTestPlan, hint: String }` +
  `provenance`.

The plan is *not* executed by `session_explain`. The agent invokes
`hypothesis_test` separately with the typed `plan` to receive a
verdict. This is a clean composition with m6-04 — neither tool
duplicates the other's responsibility.

## DTOs (new in `output.rs`)

```text
SessionCompareKind        { Divergence, Regression }
SessionCompareInput       { kind, session_a, session_b,
                             baseline_session_id, target_session_id,
                             top_n }
SessionCompareOutput      { Divergence(CompareSessionsResult + provenance),
                             Regression(RegressionAuditResult + provenance) }
CompareSessionsResult     (unchanged from v1)
RegressionAuditResult     (unchanged from v1)
SessionCompareProvenance  { engine_version, source }

SessionExplainKind        { Facts, Derived, Inferred, Hypothesis }
SessionExplainInput       { kind, session_id, hypothesis_kind }
SessionExplainOutput      { Facts(FactsBundle + provenance),
                             Derived(DerivedBundle + provenance),
                             Inferred(InferredBundle + provenance),
                             Hypothesis(HypothesisBundle + provenance) }
FactsBundle               { total_events, distinct_event_types,
                             distinct_functions, duration_ms,
                             thread_count, crash_detected,
                             signal_delivered }
DerivedBundle             { hotspots, call_graph_summary,
                             regressions_vs_none: None }
InferredBundle            { inferences: Vec<InferredTag> }
InferredTag               { CrashDetected, IoHeavy, CpuBound,
                             SingleThreaded, Unknown }
HypothesisBundle          { plan: HypothesisTestPlan, hint: String }
HypothesisTestPlan        { session_id, kind: HypothesisTestKind,
                             condition: HypothesisCondition }
SessionExplainProvenance  { engine_version, source }
```

## v1 shim JSON shape preservation

| v1 tool | v2 mapping | Preserved shape |
|---|---|---|
| `compare_sessions` | `session_compare{kind=divergence}` | `{session_a_id, session_b_id, only_in_a_count, only_in_b_count, total_a, total_b, common_count, similarity_pct, timing_delta_ms, summary}` |
| `performance_regression_audit` | `session_compare{kind=regression}` | `{baseline_session_id, target_session_id, regressions[], improvements[], summary}` |

The shims drop the v2 `provenance` field (added on top) but keep
all v1 keys identical so existing callers don't break.

## File changes (planned)

* `crates/chronos-services/src/output.rs` (+180 LoC: 13 new types)
* `crates/chronos-services/src/error.rs` (no new variants —
  reuse `SessionNotFound`)
* `crates/chronos-services/src/session_compare.rs` (new, 280 LoC:
  `SessionCompareContext<'_>`, `SessionCompareInput`,
  `ChronosSessionCompareService::compare`, 8 unit tests)
* `crates/chronos-services/src/session_explain.rs` (new, 220 LoC:
  `SessionExplainContext<'_>`, `SessionExplainInput`,
  `ChronosSessionExplainService::explain`, 7 unit tests)
* `crates/chronos-services/src/lib.rs` (+24 LoC: register both
  modules, update module index 17 → 19 algorithm modules after
  m7-02 — but m7-03 adds 2 more → 19, not 20; the doc comment will
  be adjusted accordingly)
* `crates/chronos-mcp/src/server.rs` (+280/−80 net: `SessionCompareParams`,
  `SessionExplainParams`, `session_compare` tool wrapper, `session_explain`
  tool wrapper, 2 v1 shim conversions)

## Test plan (planned)

`crates/chronos-services/src/session_compare.rs`:

1. `compare_divergence_returns_divergence_result`
2. `compare_regression_returns_regression_result`
3. `compare_divergence_session_not_found`
4. `compare_regression_session_not_found`
5. `compare_unknown_kind_returns_invalid_input`
6. `compare_divergence_preserves_summary_string`
7. `compare_regression_classifies_threshold`
8. `compare_provenance_present_on_both_kinds`

`crates/chronos-services/src/session_explain.rs`:

1. `explain_facts_returns_facts_bundle`
2. `explain_derived_returns_derived_bundle`
3. `explain_inferred_crash_detected_when_facts_crash_true`
4. `explain_inferred_unknown_when_no_characterisation`
5. `explain_hypothesis_returns_plan_not_execution`
6. `explain_session_not_found`
7. `explain_provenance_present_on_all_kinds`

`chronos-sandbox/tests/diff_tools.rs` (extend existing):

1. `test_session_compare_divergence_via_v2`
2. `test_session_compare_regression_via_v2`
3. `test_session_explain_facts_via_v2`
4. `test_session_explain_inferred_via_v2`
5. `test_compare_sessions_v1_shim_preserves_shape` (smoke test that
   the deprecated shim still works for any v1 callers)
6. `test_performance_regression_audit_v1_shim_preserves_shape`

`chronos-sandbox/tests/analytics_tools.rs` (cross-cutting):

1. `test_session_explain_facts_after_probe_stop` (proves the
   `facts` kind works against a real session lifecycle)

Smoke subset chosen: `diff_tools.rs` + `analytics_tools.rs` =
2 suites. m7-03 does not touch probe lifecycle / eBPF / startup
(observe shim path), so `program_scenarios.rs` and
`e2e_connectivity.rs` are not required for T4-smoke.

## Tier plan (planned)

| Tier | Command | Why |
|---|---|---|
| T0 | `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings` | Always |
| T1 | `cargo test --workspace --lib --no-fail-fast` | Always |
| T4-smoke | `cargo test -p chronos-sandbox --test diff_tools --test analytics_tools -- --test-threads=1` | diff_tools = direct hit; analytics_tools = cross-cutting net-new facts path |

T2 (per-crate integration) skipped — chronos-services does not have
`tests/` and chronos-mcp/tests/* do not cover session_compare or
session_explain yet. T3 (full workspace) is part of T1 + T4
combined. T5 (full sandbox) is out of scope for an A-min cycle;
scheduled for m7-06 close.

## Roll-out

After T0+T1+T4 pass:

1. `git checkout main && git pull --ff-only`
2. `git merge --ff-only feat/m7-03-session-compare-explain-merge`
3. `git push origin main`
4. `git tag -a m7-03-session-compare-explain-merge.0 -m ...`
5. `git push origin m7-03-session-compare-explain-merge.0`

The apply-checkpoint will land as
`sddk/changes/m7-03-session-compare-explain-merge/apply-checkpoint.json`.

## Open questions for execute cycle

1. **Inferences threshold values.** The IoHeavy / CpuBound /
   SingleThreaded thresholds (50% / 70% / ==1) are reasonable
   defaults but should be reviewed against real session corpora
   before merge. If they trigger too often, m7+ can move them to
   input parameters.
2. **`facts` schema completeness.** The 7-field `FactsBundle` is
   the m7-03 minimum. Adding `signal_received`, `exit_status`,
   `peak_thread_count` would broaden coverage; deferred to m7+
   unless execute-cycle evidence shows they are load-bearing.
3. **`HypothesisTestPlan.condition` shape.** The plan carries a
   typed `HypothesisCondition` matching the m6-04 surface. m7-03
   only plans `CrashInvariant` and `DominantFunctionCallPath`; the
   `Existence` variant from m6-04 is not plannable from
   `session_explain` (the agent supplies the predicate at the
   point of invocation).

## Cross-references

* `docs/milestones/m7-03-session-compare-explain-scoping.md` —
  scoping rationale (parent doc).
* `docs/milestones/m7-01-events-read-merge.md` — closest
  dispatcher precedent (m7-01 A-min + 2 v1 shims).
* `docs/milestones/m7-02-observability-merge.md` — second
  precedent (m7-02 A-full + 5 v1 shims).
* `docs/milestones/m6-04-hypothesis-test.md` — `hypothesis_test`
  dispatcher that the `session_explain{kind=hypothesis}` plan
  composes against.
* `docs/milestones/m6-03-execution-query.md` — `execution_query`
  dispatcher that supplies the `hotspots` projection for
  `session_explain{kind=derived}`.
* `docs/chronos-agentic-reconstruction/docs/specs/AGENT_API_V2.md`
  lines 20–21 (session_compare + session_explain).

---

— Submitted 2026-09-11. Awaits m7-03 execute cycle kickoff.
