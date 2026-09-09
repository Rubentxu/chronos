# M3 — Runtime Properties, Mutation Lens, Causal Slice (Scoping)

**Cycle**: `p-3416cfb8288f8964/m3-scoping-explore` (B-direct, scoping)
**Path**: B-direct (documentation cycle — no production code changes)
**Author**: orchestrator
**Date**: 2026-09-09T16:13Z
**Base**: `5507f2c` (main, post M5-m5-01)

---

## Goal

Per roadmap `docs/chronos-agentic-reconstruction/docs/roadmap/ROADMAP.md`
(M3 section): *"Move from event browsing to bug evidence."*

Deliverables per roadmap:
- declarative property model
- selected state transition records
- PropertyProjection
- violation evidence bundle
- conservative backward causal slice
- historical property evaluation with explicit unsupported result
- **UAT**: negative order total caused by known discount computation.

## State of M3 in chronos (verified 2026-09-09)

Seven sub-cycles merged to main as tagged releases:

| Tag | Topic | Files added |
|---|---|---|
| `m3-01-property-domain-model.0` | `Property`, `PropertyId`, `PropertyValue`, `ComparisonOp`, `InvariantCheck` | `crates/chronos-domain/src/property.rs` (initial ~200 LoC) |
| `m3-02-sequence-delta-evaluator.0` | sequence evaluator | extension to `property.rs` |
| `m3-03-state-transition-evidence.0` | `StateTransition`, `MutationActor` | `property.rs:133+144` |
| `m3-04-property-violation-bundle.0` | `PropertyViolation` | `property.rs:177` |
| `m3-05-property-dsl-text.0` | declarative DSL round-trip | extension to `property.rs` |
| `m3-06-string-aggregation-dsl.0` | `contains()`, `matches()` predicates | extension to `property.rs` |
| `m3-07-causal-slice.0` | backward causal slice | `causal_slice.rs` (144 LoC) |

`crates/chronos-domain/src/property.rs` is now **1,200 LoC**.

## What's done vs what's missing

### ✅ Done (verified by grep)

- `Property` domain model (`property.rs:90`)
- `PropertyOutcome` = `Pass | Violation{before, after, message} | UnsupportedByRecordedEvidence{reason}` (`property.rs:103`) — never returns false `Pass` when evidence is missing
- `PropertySequenceOutcome` (`property.rs:117`) — sequence-level with index, before, after
- `StateTransition` + `MutationActor` (`property.rs:133,144`)
- `PropertyViolation` bundle (`property.rs:177`)
- DSL predicates: `== != < <= > >= changed() unchanged() exists() contains() matches() count() delta() before() after() eventually() never() until()`
- Conservative backward causal slice over `EvidenceNode` + `CausalEdge` (`causal_slice.rs:1-144`) — returns `included` + `missing` (gaps never silently dropped)
- Re-exports in `crates/chronos-domain/src/lib.rs` (per the m3-07 commit message)

### ❌ Missing or incomplete

1. **PropertyProjection** — `grep -rn "PropertyProjection" crates/` returns **zero
   hits**. The roadmap calls this out explicitly: *"PropertyProjection"*.
   This is the read-side projection that materializes a property's
   outcomes over a session/event stream.

2. **Mutation Lens as a queryable surface** — `MutationActor` and
   `StateTransition` exist as types, but there is no service or tool
   that surfaces them via MCP. No `mutation_lens` tool, no
   `MutationLensProjection`.

3. **Historical property evaluation with explicit unsupported result**
   — partially covered by `PropertyOutcome::UnsupportedByRecordedEvidence`,
   but there is no concrete evaluator that walks a session's
   `TraceEvent` stream and runs the DSL. The `m3-02` sequence
   evaluator exists but is not wired to MCP.

4. **UAT: negative order total caused by known discount computation**
   — no integration test found for this. It needs:
   - a tiny program with `Order::apply_discount` that produces a negative total
   - a fixture that captures the bug via chronos-native probe
   - a property `order_total_non_negative` declared via the DSL
   - an evaluation that produces a `PropertyViolation` bundle with
     `before`, `after`, `invocation`, `causal_predecessors`

5. **MCP exposure** — none of M3's data types surface via
   `chronos-mcp::server`. No `evaluate_property`, `mutation_lens`,
   `causal_slice` tool.

## Recommended next cycle(s) (after this scoping)

M3 is more advanced than M5 was at scoping time (M5 had only a
vault spec). Concrete remaining work for closing M3:

1. **`m3-08-property-projection`** (A-min, ~300 LoC): build
   `PropertyProjection` in `crates/chronos-query` that walks a
   `QueryEngine`'s events and emits `PropertyOutcome` per declared
   property. Includes `PropertyProjection::run(session_id, property,
   &engine) -> PropertyOutcome`.

2. **`m3-09-mutation-lens-tools`** (A-min, ~400 LoC): add
   `mutation_lens` tool to `chronos-mcp::server` that exposes
   `StateTransition` records. No service extraction needed yet
   (M5 work); inline in server.rs is fine for now.

3. **`m3-10-causal-slice-tool`** (A-min, ~300 LoC): add
   `causal_slice` tool that takes a sink event and returns the
   `CausalSlice` from `chronos_domain::causal_slice`. Builds the
   `EvidenceNode` graph on the fly from `TraceEvent` records.

4. **`m3-11-m3-uat-order-total`** (A-min, ~400 LoC): implement the
   UAT gate. A small Rust fixture binary that produces a negative
   order total via a known buggy `apply_discount`, captured via
   `chronos-native`. A property declared in DSL, run via
   `PropertyProjection`, asserts the violation is detected with
   full evidence bundle. Closes the M3 milestone.

5. **`m3-12-milestone-close`** (B-direct): write
   `~/.sddk-knowledge/p-3416cfb8288f8964/milestones/m3-runtime-properties-mutation-lens-causal-slice.md` mirroring the M2 milestone
   file.

**Total**: ~1,400 LoC across 4 A-min + 1 B-direct, ~3-5 working days.

## Risks

1. **PropertyProjection semantics ambiguity** — the roadmap does
   not specify whether `PropertyProjection` is incremental or
   recomputed from scratch. Recommend: recomputed (matches the
   M1 ExecutionLog vertical-slice pattern: append-only source,
   projections are derivable).

2. **Mutation Lens data source** — `StateTransition` records are
   populated by the semantic resolver. Need to verify that the
   resolver currently emits `StateTransition` events; if not, the
   Mutation Lens projection will be empty.

3. **DSL parser depth** — `m3-05` and `m3-06` added DSL primitives
   but the parser may have limitations (e.g., no nested temporal
   operators). The UAT property must stay within parser capabilities.

4. **Causal slice graph build cost** — building the
   `EvidenceNode`/`EvidenceEdge` graph from a long trace may be
   expensive. Mitigate by materializing lazily, only on tool call.

## Scope for THIS cycle (m3-scoping-explore)

This cycle ships ONLY this document. No production code changes.
No branch push.

## References

- Vault spec: `~/.sddk-knowledge/p-3416cfb8288f8964/specs/RUNTIME_PROPERTIES_AND_SLICING.md`
- Roadmap: `~/Proyectos/rust/chronos/docs/chronos-agentic-reconstruction/docs/roadmap/ROADMAP.md` (M3 section)
- Current code: `crates/chronos-domain/src/property.rs` (1,200 LoC) + `causal_slice.rs` (144 LoC)
- Existing tags: `m3-01` through `m3-07` on main
