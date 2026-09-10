# m6-04 — `hypothesis_test` tool spec

**Branch:** `feat/m6-04-hypothesis-test`
**Cycle:** M6 §4.2, item 1 (net-new capability)
**Precedence:** docs/milestones/m5-close-report.md §4.2; docs/.../specs/AGENT_API_V2.md line 18
**Status:** PROPOSED, 2026-09-10

## Why this cycle

The close-report §4.2 lists two **net-new** capabilities required by the v2
spec: `hypothesis_test` and `session_export`. m6-04 ships the first one.
m6-05 will ship `session_export`.

The close-report says these are "new algorithms in services, not MCP" — meaning
the algorithm does not exist yet. The natural home for the new code is a new
`chronos-services::hypothesis_test` module, following the dispatcher pattern
established in m6-01..m6-03 (single dispatcher + a single MCP wrapper named
`hypothesis_test`; no v1 tools to deprecate because there is no v1).

## Scope (this cycle)

- Add `chronos-services::hypothesis_test` module (~350-450 LoC, 16th service module).
- Add `HypothesisKind` enum (`Invariant` / `Existence` / `CallPath`).
- Add `HypothesisInput` DTO carrying one of:
  - `Invariant { scope: PropertyScope, comparison: ComparisonOp, value }`
  - `Existence { match_predicate, ... }`
  - `CallPath { caller, callee, ... }`
- Add `HypothesisOutput` envelope carrying:
  - `verdict`: PASS / VIOLATION / UNSUPPORTED (never false PASS, per property infra)
  - `support_event_ids: Vec<EventId>`
  - `counter_event_ids: Vec<EventId>`
  - `summary: string`
- Add `ChronosHypothesisTestService::test` dispatcher with unit tests.
- Add `hypothesis_test` v2 MCP tool wrapper.
- Honest disclosure: this is a new tool with **no v1 shim**. v1 names don't exist.

## Out of scope

- LLM integration (no LLM involvement; the algorithm is deterministic over the
  captured trace).
- Hypotheses expressed in arbitrary DSL strings. Only the 3 typed shapes above.
- m6-05 (`session_export`), m6-06 (`events_read` merge + sweep).

## Algorithm (the 3 shapes)

### Invariant

Reuses the scalar property infrastructure from
`chronos_domain::property::evaluate`. Takes a typed target
(`PropertyScope::EventCount / LatencyMs / PropertyValue`), a `ComparisonOp`,
and a value. Returns:

- `PASS` if every observed value satisfies the comparison
- `VIOLATION` if any observed value violates it; lists all violation event IDs
- `UNSUPPORTED` if the required observation wasn't captured

### Existence

Takes a predicate (a small enum of standard filter shapes: `event_type_eq`,
`thread_eq`, `address_in_range`, `property_key_eq`). Returns:

- `PASS` if at least one event matches
- `VIOLATION` if none match (the hypothesis "X exists" is false)
- `UNSUPPORTED` if the session has no events at all

### CallPath

Takes `caller: FunctionId` and `callee: FunctionId`. Walks the call graph
(already supported in m6-03's execution_query kind=CallGraph). Returns:

- `PASS` if `callee` is reachable from `caller` (or `caller == callee` for self)
- `VIOLATION` with the longest un-reachable path so far if not reachable
- `UNSUPPORTED` if the session has no call data

## Verification plan

| Gate | Expectation |
|---|---|
| T0 fmt+clippy | PASS |
| T1 `chronos-services --lib` | existing 130/130 + new dispatcher tests (~5-7) = 135-137 |
| T2 `chronos-services --tests` | existing 2/2 + 0 new integration tests |
| T3 workspace --lib × 25, with `--skip ptrace_tracer --skip capture_runner` | total window expected [770, 800] (with skip — same as m6-03 baseline) |
| T4-smoke | one or two sandbox tests that exercise `hypothesis_test` invocation; if none exists, content-test only with `e2e_connectivity` proving the wire survives |

## Honest disclosures

- This is a new dispatcher; no v1 shims.
- Schema is `HypothesisKind` with 3 variants (smaller than trace_slice's 4 or
  execution_query's 6 because no v1 fallback needed).
- LLM integration is **explicitly** out of scope per AGENT_API_V2.md
  ("without hiding raw support" — the raw event IDs are the answer).
