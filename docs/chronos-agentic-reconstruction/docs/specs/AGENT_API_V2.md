# Specification: Agent API v2

## Goal

Reduce the MCP surface to a small set of orthogonal primitives. The agent composes workflows; Rust implements trustworthy capabilities.

Target order of magnitude: **8–12 public tools**.

## Proposed tools

- `session_start` — start/attach and return `SessionId` + capability snapshot.
- `session_stop` — gracefully end producers and seal tail.
- `capabilities` — available evidence mechanisms for target/session.
- `observe` — create/change typed observation/subscription/instrumentation request.
- `events_read` — cursor-based, non-destructive evidence read.
- `execution_query` — invocation/call/performance projection queries.
- `state_query` — state transition/value evidence queries.
- `hypothesis_test` — plan/run evidence collection for a hypothesis, without hiding raw support.
- `trace_slice` — causal evidence around a target.
- `session_compare` — semantic comparison.
- `session_explain` — structured facts/derived/inferred/hypothesis bundle.
- `session_export` — selected evidence/projections, optionally OTLP-compatible.

## Types, not magic strings

Tool schemas use enumerations/structured filters generated from domain types. Invalid event names produce validation errors.

## Unify subscriptions, tripwires and properties

Use one conceptual model:

```text
Subscription
  scope
  condition
  requested evidence
  action
  retention
  consumer/cursor
```

Tripwire = condition/action subscription. Property = evaluative subscription/projection.

## Deprecated public concepts

Deprecate as primary workflow primitives:

- breakpoint create/step;
- watchpoint stepping loops;
- generic expression evaluation as main workflow;
- monolithic orchestrator tools;
- IDE-button emulation tools.

Internal backends can still use those mechanisms.

## MCP architecture

```text
DTO validation -> application service -> domain result -> MCP response
```

No large capture/query algorithms in `server.rs`.

## Agent ergonomics

Responses include where relevant: next cursor, completeness, gap summary, provenance, capability limitations and stable IDs for follow-up calls.
