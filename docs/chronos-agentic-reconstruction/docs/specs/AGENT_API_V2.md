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

## Current surface (REC-C6 ratification, 2026-09-21)

The live MCP tool surface is **41 tools**, down from 63 at REC-C5 start
(REC-C5 deleted the 22 v1 aliases in C5.3.2, see regression test
`crates/chronos-mcp/tests/alias_deletion.rs`). The surface is split
into two groups:

**12 canonical v2 tools** (the orthogonal primitives defined in this
spec, plus `session_compare` / `session_explain` / `session_export`):

| Tool | Status | Owner |
|---|---|---|
| `session_start` | v2 canonical | REC-C5 |
| `session_stop` | v2 canonical | REC-C5 |
| `capabilities` | v2 canonical | REC-C5 |
| `observe` | v2 canonical | REC-C5 (absorbs `tripwire_create/list/query/delete` + `probe_inject`) |
| `events_read` | v2 canonical | REC-C5 (absorbs `query_events`, `get_event`) |
| `execution_query` | v2 canonical | REC-C5 (absorbs `get_call_stack`, `get_execution_summary`, `debug_call_graph`, `debug_detect_races`, `debug_expand_hotspot`, `debug_get_saliency_scores`) |
| `state_query` | v2 canonical | REC-C5 (absorbs `state_diff`, `debug_get_registers`, `debug_get_memory`, `debug_analyze_memory`, `evaluate_expression`) |
| `trace_slice` | v2 canonical | REC-C5 (absorbs `debug_find_crash`, `inspect_causality`, `forensic_memory_audit`, `debug_find_variable_origin`) |
| `hypothesis_test` | v2 canonical | REC-C5 |
| `session_compare` | v2 canonical | REC-C5 |
| `session_explain` | v2 canonical | REC-C5 |
| `session_export` | v2 canonical | REC-C5 |

**29 not-yet-converged neighbours**: working tools that predate the
v2 spec and have not yet been folded into a v2 canonical. Each
remains in `ALL_TOOL_NAMES` and is wired to the v2 dispatcher
infrastructure (chronos-mcp/src/server.rs router), but the v2
canonical tools above are the preferred entry points. The 29 names
are tracked in the `crates/chronos-mcp/src/server.rs::ALL_TOOL_NAMES`
constant and validated against the `#[tool]` router by the
`toolset_sync_check` test.

Future migration order (one cycle per neighbour group; not in REC-C6
scope):

1. The `probe_*` family — once a `probe` v2 canonical is designed.
2. The `browser_probe_*` family — once a `browser` v2 canonical is designed.
3. The `counterexample_*` family — once an `analysis` v2 canonical is designed.
4. The `sessions` CRUD family (`session_create`, `session_list`,
   `session_delete`, etc.) — absorbed by `session_start` /
   `session_stop` semantics.
5. The `mutation_lens` / `causal_slice` / `compare_sessions` family —
   folded into `hypothesis_test` or `session_compare`.
6. Misc — `get_variables`, `diff`, and any remaining standalone
   tools.

This future-migration order is documented here so each cycle that
folds one neighbour group into a v2 canonical has a pre-approved
direction; the order is not a release commitment.

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
