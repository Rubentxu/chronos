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

## Deprecated aliases and sunset policy (REC-C5)

The 22 v1 aliases below are deprecated shims over the v2 dispatchers. Every
alias description in `crates/chronos-mcp/src/server.rs` carries its explicit
replacement mapping. **Sunset policy: the aliases are removed at the next
minor release** (cycle REC-C5, slice C5.3 deletes the handlers and the legacy
wire shims behind them). No alias is left without a replacement; none of the
22 required a "no v2 replacement" marker.

| Deprecated alias | v2 replacement |
|---|---|
| `query_events` | `events_read` with `mode=query` |
| `get_event` | `events_read` with `mode=by_id` |
| `get_call_stack` | `execution_query` with `kind=call_stack` |
| `get_execution_summary` | `execution_query` with `kind=execution_summary` |
| `debug_call_graph` | `execution_query` with `kind=call_graph` |
| `debug_detect_races` | `execution_query` with `kind=race_detect` |
| `debug_expand_hotspot` | `execution_query` with `kind=hotspot` |
| `debug_get_saliency_scores` | `execution_query` with `kind=saliency` |
| `state_diff` | `state_query` with `kind=register_diff` |
| `evaluate_expression` | `state_query` with `kind=expression_eval` |
| `debug_get_memory` | `state_query` with `kind=memory_read` |
| `debug_get_registers` | `state_query` with `kind=register_snapshot` |
| `debug_analyze_memory` | `state_query` with `kind=memory_analysis` |
| `debug_find_variable_origin` | `trace_slice` with `slice_kind=variable_origin` |
| `debug_find_crash` | `trace_slice` with `slice_kind=crash` |
| `inspect_causality` | `trace_slice` with `slice_kind=causality` |
| `forensic_memory_audit` | `trace_slice` with `slice_kind=memory_audit` |
| `tripwire_create` | `observe` with `verb=create`, `condition.kind=tripwire` |
| `tripwire_list` | `observe` with `verb=list` |
| `tripwire_query` | `observe` with `verb=query` |
| `tripwire_delete` | `observe` with `verb=delete` + `subscription_id` |
| `probe_inject` | `observe` with `verb=create`, `condition.kind=uprobe`, `scope=session{session_id}` |

Consumers should migrate before the next minor: after removal, calls to these
names fail with an unknown-tool error. The v2 responses flatten the v1 payload
plus a discriminator tag (`mode`/`kind`) and, where applicable, the v2
ergonomics fields (`next_cursor`, `completeness`, `provenance`); consumers
relying on v1-only envelope fields (e.g. `query_events`' `not_found`/`reason`)
must derive them from the v2 payload (empty `events` page).

## Agent ergonomics

Responses include where relevant: next cursor, completeness, gap summary, provenance, capability limitations and stable IDs for follow-up calls.
