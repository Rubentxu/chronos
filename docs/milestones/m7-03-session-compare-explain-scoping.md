# M7-03 — `session_compare` + `session_explain` split (Scoping)

**Cycle:** `feat/m7-03-session-compare-explain-scoping` (B-direct, scoping)
**Path:** B-direct (documentation cycle — no production code changes)
**Author:** orchestrator
**Date:** 2026-09-11
**Status:** scoping proposal — **OPEN**, awaits m7-03 execution kickoff

> **Naming.** "M7" here refers to the v2-spec sub-cycle that follows the
> M6 sub-cycle (closed 2026-09-11). It is **not** the same as the
> reconstruction-roadmap M7 (*Differential execution v2*).

---

## Why this cycle

The M7 scoping doc (`docs/milestones/m7-events-read-scoping.md`
§Proposed M7 cycle split) classified m7-03 as:

> `m7-03` (`session_compare` + `session_explain` — split out of
> `chronos_services::diff`). Smaller; mostly a rename + parameter
> restructuring.

m7-02 just shipped (tag `m7-02-observability-merge.0`). With
`observe` out of the way, m7-03 is the next-pending cycle on the M7
backlog. The M7 split table says "The exact cycle split will be set
at M7 kickoff based on empirical evidence gathered during
m7-01..m7-03", so this scoping cycle **re-validates** the m7-03
shape against the actual diff.rs code and the v2 spec now that
m7-01 + m7-02 have landed.

This is that scoping pass. It does **not** refactor any code. It
produces a scoped proposal that the downstream m7-03 execution cycle
can execute against, and ships the **m7-03 cycle spec** for the
deliverable.

---

## Current diff/compare tool surface (M7-03 entry)

Two v1 MCP tools share the diff surface today, both backed by
`chronos_services::diff`:

* `compare_sessions` — semantic divergence comparison (BLAKE3 hash
  set-diff + similarity_pct + LLM-readable `summary`). At
  `crates/chronos-mcp/src/server.rs:4045` (~35 LoC of MCP body +
  exhaustive `ServiceError` arms + JSON envelope). Calls
  `ChronosDiffService::compare_sessions`.
* `performance_regression_audit` — per-function call-count regression
  detection between two sessions (top-N, delta > +50% regression,
  delta < -50% improvement). At
  `crates/chronos-mcp/src/server.rs:4011` (~30 LoC). Calls
  `ChronosDiffService::performance_regression_audit`.

Together: **~65 LoC of MCP glue** plus the underlying
`chronos_services::diff` (405 LoC: `DiffContext<'_>`,
`CompareSessionsInput`, `PerformanceRegressionAuditInput`,
`ChronosDiffService` with two methods).

The service layer is mature. `TraceDiff::compare` (in
`chronos_store`) already implements the BLAKE3 hash-based set diff;
`diff.rs` owns the per-function regression audit and the
LLM-readable summary formatter.

**No v1 `session_compare` or `session_explain` tool exists today.**
The v2 names are net-new from the v2 spec's perspective; the v1
tools `compare_sessions` and `performance_regression_audit` are the
closest matches but have **non-trivially different shapes** from the
v2 spec target.

---

## v2 spec target

From `docs/chronos-agentic-reconstruction/docs/specs/AGENT_API_V2.md`
lines 20–21:

> - `session_compare` — semantic comparison.
> - `session_explain` — structured facts/derived/inferred/hypothesis bundle.

The two tools have distinct semantic intents:

* **`session_compare`** is a two-session comparison — *"are these two
  runs semantically the same?"* — and folds what the v1 surface
  calls `compare_sessions` (BLAKE3 set diff + similarity_pct) and
  `performance_regression_audit` (per-function call-count regression
  detection) behind a single v2 endpoint with a `kind` discriminator.
* **`session_explain`** is a one-session structured bundle — *"what
  does this session show?"* — and is **net-new**: no v1 tool backs
  it. The bundle carries four tiers:
  - `facts` — direct observations from the trace (events, durations,
    function-call counts);
  - `derived` — projections computed from facts (hotspots,
    regressions vs a baseline if one is supplied);
  - `inferred` — higher-level characterisations (e.g. "this session
    crashed at iteration N", "this session is dominated by I/O
    waits");
  - `hypothesis` — typed `hypothesis_test` invocations the agent can
    execute against the session.

The v2 spec ("Types, not magic strings" + "Agent ergonomics" sections)
implies both tools use typed enums for discriminator values and
include the v2 ergonomics fields (`next_cursor`, `completeness`,
`gap_summary`, `provenance`) on responses where pagination / coverage
are meaningful. For m7-03 these are minimal: `session_compare` is
bounded by the two-session set, and `session_explain` returns a
finite bundle so neither needs cursor pagination in this cycle.

---

## Scope decisions locked in this cycle

The full spec lives at `docs/milestones/m7-03-session-compare-explain-merge.md`.
Key scope decisions:

| Decision | Choice | Why |
|---|---|---|
| Unify v1 `compare_sessions` + `performance_regression_audit` under one v2 tool? | **Yes** — `session_compare` with `kind: "divergence" \| "regression"` | Both tools compare two sessions; the kind discriminator avoids two near-identical endpoints |
| Introduce net-new `session_explain` in m7-03? | **Yes** — net-new (no v1 shim) | v2 spec requires the four-tier facts/derived/inferred/hypothesis bundle; no v1 analogue exists |
| `session_explain` data shape? | `kind: "facts" \| "derived" \| "inferred" \| "hypothesis"` discriminator + per-kind output | Mirrors the v1-style nested envelope pattern from `m6-04` hypothesis_test |
| Cursor pagination on `session_compare`? | **No** (bounded by 2 sessions) | Result set is O(events_in_a + events_in_b); no cursor needed in m7-03 |
| Cursor pagination on `session_explain`? | **No** (bundle is finite per-session) | The four-tier bundle is bounded by the session's persisted events |
| Provenance on `session_compare`? | **Yes** (`provenance: SessionCompareProvenance`) | v2 spec line 79 "include provenance" + agent ergonomics |
| Provenance on `session_explain`? | **Yes** (`provenance: SessionExplainProvenance`) | Same |
| v1 shim `compare_sessions`? | **Yes** — preserved as deprecated shim mapping `kind: "divergence"` | Sunset stays at 2027-09-11 (m6-close-report §4) |
| v1 shim `performance_regression_audit`? | **Yes** — preserved as deprecated shim mapping `kind: "regression"` | Same |
| `hypothesis` kind of `session_explain`? | **Returns a plan** (typed `HypothesisTestKind` + `condition`) — does **not** execute | m6-04 already provides `hypothesis_test` for execution; m7-03 only plans |
| `inferred` kind of `session_explain`? | Best-effort, typed enums; `Unknown` when the inference can't be made | Matches the m6 cycle's "typed enum, never magic strings" stance |

These scope decisions keep m7-03 a focused A-min path (one net-new
module + one v2 tool split + one net-new v2 tool + 2 v1 shims).
The four-tier `session_explain` bundle is the largest semantic
addition; everything else is mechanical shim conversion + dispatcher
restructuring.

---

## Why A-min path

m7-01 + m7-02 each took A-min and A-full paths respectively. m7-03
sits between them:

1. **2 v1 tools fold into 1 v2 tool** (`compare_sessions` +
   `performance_regression_audit` → `session_compare`).
2. **One net-new v2 tool** (`session_explain`) with a new
   discriminator (kind ∈ {facts, derived, inferred, hypothesis}).
3. **No architectural fork**: the algorithm already lives in
   `chronos_services::diff`; the `facts` + `derived` kinds of
   `session_explain` reuse existing per-session projections from
   `chronos_query::QueryEngine::execution_summary`.
4. **`inferred` is best-effort** with a typed `Unknown` fallback —
   no domain-layer change required.
5. **`hypothesis` kind returns a plan, not an execution result** —
   `hypothesis_test` already covers execution via m6-04.

Estimated LoC delta: **+550 / −120 = net +430**, comparable to
m7-01 in volume (385 + DTOs).

---

## Execution shape (planned)

| Commit | Subject | Files |
|---|---|---|
| 1 | DTOs in `output.rs` | output.rs (+180 LoC) |
| 2 | Dispatcher + 8 tests in `session_compare.rs` | session_compare.rs (new, +280 LoC) |
| 3 | `SessionCompareParams` + `session_compare` tool wrapper | server.rs (+80 LoC) |
| 4 | Net-new `SessionExplainParams` + `session_explain` tool wrapper | server.rs (+100 LoC) |
| 5 | Shim: `compare_sessions` → `session_compare{kind=divergence}` | server.rs (−20 LoC) |
| 6 | Shim: `performance_regression_audit` → `session_compare{kind=regression}` | server.rs (−20 LoC) |
| 7 | `lib.rs` module index + T0/T1 gate | lib.rs (+5 LoC) |

Then T4 sandbox smoke (2 representative suites — see
`docs/chronos-agentic-reconstruction/docs/specs/AGENT_API_V2.md`
test plan in the spec) before FF-merge. `diff_tools.rs` is the
direct hit; one cross-cutting suite (e.g. `analytics_tools`) for
the net-new `session_explain` path.

---

## Risks and unknowns

* **`session_explain` `inferred` quality.** "This session is dominated
  by I/O waits" is a characterisation that depends on heuristics
  over `execution_summary` + event-type distributions. m7-03 ships
  with a small set of typed inferences (crash detection, I/O-heavy,
  CPU-bound, single-threaded); broader inference coverage is m7+.
* **`hypothesis` plan shape.** The plan mirrors `chronos_services`
  `HypothesisTestKind` (Invariant / Existence / CallPath from
  m6-04). m7-03 picks a small set of "starter" hypotheses
  (e.g. "this session crashed", "the dominant function appears N
  times"). Agent-driven hypothesis composition is m7+.
* **No v1 `session_explain` shim.** Because no v1 tool exists with
  the same shape, the v2 tool is net-new with no deprecation
  migration window. This deviates from the M6 standing policy
  ("preserve v1 names as deprecated shims") but is justified
  because there is nothing to preserve.
* **Provenance coverage.** Both `session_compare` and
  `session_explain` need `provenance` to satisfy the v2 spec agent
  ergonomics line. m7-03 carries minimal provenance (engine version
  + a short `source` tag); richer provenance (lineage graph,
  staleness windows) is m7+.
* **Sandbox smoke coverage.** `chronos-sandbox/tests/diff_tools.rs`
  is the direct integration target for the shim conversions. m7-03
  will add a `session_compare` + `session_explain` block to that
  suite if one doesn't exist; otherwise it will ride the existing
  test rig.
* **v1 sunset drift.** No new v1 names added; existing `compare_sessions`
  + `performance_regression_audit` retain the 2027-09-11 sunset.

---

## Exit criteria for this scoping cycle

1. `docs/milestones/m7-03-session-compare-explain-scoping.md` (this
   file) lands on `main` via FF-merge.
2. `docs/milestones/m7-03-session-compare-explain-merge.md` (the
   cycle spec for the deliverable) lands on `main` via FF-merge.
3. `docs/ROADMAP.md` is unchanged — M7 candidates are already listed
   in the M6 close commit. The scoping cycle is documentation-only.
4. `cargo fmt --all -- --check` exits 0.
5. `cargo clippy --workspace --all-targets -- -D warnings` exits 0.

No source code is touched in this scoping cycle. The m7-03 execution
cycle that this scoping seeds is a separate A-min execution cycle.

---

## Cross-references

* `docs/milestones/m6-close-report.md` §6 — M7 candidate list.
* `docs/milestones/m7-events-read-scoping.md` — M7 split + sequencing.
* `docs/milestones/m7-01-events-read-merge.md` — closest dispatcher
  precedent (m7-01 dispatcher + 2 shims, A-min).
* `docs/milestones/m7-02-observability-merge.md` — second precedent
  (m7-02 dispatcher + 5 shims, A-full).
* `docs/milestones/m6-04-hypothesis-test.md` — `hypothesis_test`
  dispatcher that the `hypothesis` plan kind of `session_explain`
  composes against.
* `docs/chronos-agentic-reconstruction/docs/specs/AGENT_API_V2.md`
  lines 20–21 (session_compare + session_explain) + line 79 (types,
  not magic strings) + line 84+ (agent ergonomics — provenance,
  capability limitations, stable IDs).

---

— Submitted 2026-09-11. Awaits m7-03 execution kickoff.
