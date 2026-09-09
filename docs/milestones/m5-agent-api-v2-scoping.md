# M5 — Agent API v2 (Scoping)

**Cycle**: `p-3416cfb8288f8964/m5-agent-api-v2-explore` (B-direct, scoping)
**Path**: B-direct (documentation cycle — no production code changes)
**Author**: orchestrator
**Date**: 2026-09-09T14:23Z
**Status**: scoping proposal

---

## Why this cycle

The vault spec `AGENT_API_V2.md` (migrated 2026-09-07) defines an
explicit exit criterion for M5: **"no new application algorithm
belongs directly in `chronos-mcp::server`"**. The current
implementation has drifted significantly from that target:

- `crates/chronos-mcp/src/server.rs`: **5,341 lines**, **42 `#[tool(...)]`
  registrations**, **98 `async fn` definitions**.
- Many tools embed application logic (causality traversal, race
  detection, memory audits, saliency scoring) that should live in
  dedicated service crates.
- No service-layer extraction: `QueryService`, `ProbeService`,
  `PropertyService`, `DiffService` etc. either don't exist or are
  tangled into the server's `Arc<Self>` state.

This cycle does **not** refactor the server. It produces a scoped
proposal that downstream M5 cycles can execute against.

## Current tool surface (42 tools, verified 2026-09-09)

Counted via `grep -cE "    #\[tool\(" crates/chronos-mcp/src/server.rs`.

### Category A — Query (8 tools)

| Tool | Maps to v2 |
|---|---|
| `query_events` | `events_read` |
| `get_event` | `events_read` (by-id mode) |
| `get_call_stack` | `execution_query` (call-stack projection) |
| `get_execution_summary` | `execution_query` (summary) |
| `state_diff` | `state_query` |
| `list_threads` | `events_read` (filter mode) |
| `debug_call_graph` | `execution_query` (call-graph projection) |
| `inspect_causality` | `trace_slice` (causal projection) |

### Category B — Debug / Probe (15 tools)

| Tool | Maps to v2 |
|---|---|
| `debug_find_variable_origin` | `trace_slice` |
| `debug_find_crash` | `session_explain` (anomaly mode) |
| `debug_detect_races` | `state_query` (race heuristic) |
| `debug_expand_hotspot` | `execution_query` (hotspot expansion) |
| `debug_get_saliency_scores` | `execution_query` (saliency) |
| `debug_get_variables` | `execution_query` (locals) |
| `debug_get_memory` | `state_query` (memory read) |
| `debug_get_registers` | `state_query` (registers) |
| `debug_diff` | `session_compare` |
| `debug_analyze_memory` | `state_query` (memory access window) |
| `forensic_memory_audit` | `trace_slice` (memory lineage) |
| `evaluate_expression` | `state_query` (evaluated expression) |
| `probe_start` | `observe` |
| `probe_stop` | `session_stop` |
| `probe_drain` | `events_read` (live snapshot) |

### Category C — Session lifecycle (5 tools)

| Tool | Maps to v2 |
|---|---|
| `save_session` | `session_start` (durability) |
| `load_session` | `session_start` (restore) |
| `list_sessions` | `capabilities` (sessions listing) |
| `delete_session` | `session_stop` (with cleanup) |
| `drop_session` | `session_stop` (in-memory only) |

### Category D — Durable query path (1 tool)

| Tool | Maps to v2 |
|---|---|
| `probe_drain_log` | `events_read` (durable backend) |

### Category E — Browser debugging (3 tools)

| Tool | Maps to v2 |
|---|---|
| `browser_probe_start` | `observe` (browser target) |
| `browser_probe_stop` | `session_stop` (browser) |
| `browser_probe_drain` | `events_read` (browser) |

### Internal / non-`#[tool]` helpers (~10)

`build_and_store_engine`, `set_active_session`, `cleanup_session_memory`,
`auto_compaction_daemon`, `run_one_compaction_round`, etc. These are
already services-in-disguise; M5 should move them to dedicated
crates.

## Target surface (12 tools per `AGENT_API_V2.md`)

| # | v2 tool | Replaces (n) | Status in chronos |
|---|---|---|---|
| 1 | `session_start` | 4 (save/load + probe_start + browser_probe_start) | new |
| 2 | `session_stop` | 3 (drop + probe_stop + browser_probe_stop) | new |
| 3 | `capabilities` | 1 (list_sessions + session metadata) | new |
| 4 | `observe` | 1 (probe_start) + 1 (browser_probe_start) | new orthogonal primitive |
| 5 | `events_read` | 6 (query + get_event + list_threads + probe_drain + browser_probe_drain + probe_drain_log) | superset with cursor modes |
| 6 | `execution_query` | 4 (summary + call_stack + call_graph + hotspot + saliency) | project parameter |
| 7 | `state_query` | 4 (state_diff + memory + registers + races) | kind parameter |
| 8 | `hypothesis_test` | 0 (new capability) | net-new |
| 9 | `trace_slice` | 3 (causality + variable_origin + memory_audit) | slice_kind parameter |
| 10 | `session_compare` | 2 (debug_diff + state_diff) | scope parameter |
| 11 | `session_explain` | 2 (crash + heuristics) | mode parameter |
| 12 | `session_export` | 0 (new capability, OTLP-compatible) | net-new |

Net reduction: **42 → 12** (71%), with **2 net-new capabilities**
(`hypothesis_test`, `session_export`).

## Service-layer extraction (M5 architecture)

Per spec: "Exit: no new application algorithm belongs directly in
`chronos-mcp::server`."

Proposed crates/services (each is its own Rust crate or `pub mod`
inside an existing one):

```
crates/chronos-services/   # NEW crate: thin orchestration layer
├── src/session_service.rs   # SessionService
├── src/probe_service.rs     # ProbeService
├── src/property_service.rs  # PropertyService
├── src/query_service.rs     # QueryService
├── src/diff_service.rs      # DiffService
└── src/lib.rs
```

Each service is a plain Rust struct (not generic over MCP types) with
explicit input/output types. The `chronos-mcp::server` becomes a
**router**: it parses MCP requests, calls into the right service, and
serializes the response. No business logic remains in `server.rs`.

## Implementation strategy (downstream M5 cycles)

Because this is a large architectural refactor, **do not attempt it
in a single cycle**. Recommended cycle sequence:

1. **`m5-01-services-skeleton`** (A-min, ~300 LoC): create
   `chronos-services` crate with empty service structs + trait
   interfaces; wire one service into `server.rs` as a proof-of-concept
   (read-only, additive).
2. **`m5-02-session-service-extract`** (A-min, ~400 LoC): move all
   session CRUD + `save/load/list/delete/drop` into `SessionService`.
   Server routes 4 tools through it.
3. **`m5-03-query-service-extract`** (A-min, ~500 LoC): move event
   read paths (query, get_event, list_threads) into `QueryService`.
4. **`m5-04-probe-service-extract`** (A-min, ~500 LoC): move
   probe_start / probe_stop / probe_drain / browser_* into
   `ProbeService`.
5. **`m5-05-execution-query-merge`** (A-min, ~400 LoC): merge
   `get_execution_summary` + `get_call_stack` + `debug_call_graph` +
   hotspot + saliency into a single `execution_query` tool with a
   `projection` parameter.
6. **`m5-06-state-query-merge`** (A-min, ~400 LoC): merge
   `state_diff` + memory + registers + races into `state_query`.
7. **`m5-07-trace-slice-merge`** (A-min, ~300 LoC): merge causality
   + variable_origin + memory_audit into `trace_slice`.
8. **`m5-08-hypothesis-test`** (A-min, ~600 LoC): introduce the
   net-new capability (proptest-style, see vault spec
   `RUNTIME_PROPERTIES_AND_SLICING.md`).
9. **`m5-09-session-export`** (A-min, ~400 LoC): introduce
   OTLP-compatible export.
10. **`m5-10-deprecation-shims`** (A-min, ~200 LoC): keep the
    legacy 42 tools as thin shims that delegate to the new 12,
    marked deprecated. Remove in a later milestone once tooling
    has migrated.

**Total**: ~4,000 LoC across 10 A-min cycles, ~6-10 weeks calendar
time assuming 1 cycle/day. Each cycle keeps T0+T2+T4-smoke green.

## Risks

1. **Backward compatibility** — agents and external tooling may rely
   on the exact 42-tool surface. Mitigated by `m5-10-deprecation-shims`
   which keep the old names working as thin forwarders.
2. **Performance regression** — moving logic into separate functions
   may cross module boundaries; profile-driven optimization may be
   needed.
3. **Capability negotiation** — the new `capabilities` tool must
   accurately describe what each session can do, including the
   language backends available. This is a new surface; tests needed.
4. **State management** — `server.rs` holds the `Arc<Self>` and a
   lot of cached state (sessions, engines, ring buffers). Extracting
   services means moving that state. Refactoring this without
   dropping in-flight probes is non-trivial.
5. **Tool count optimism** — the vault spec's "8-12" may not be
   achievable if some genuinely distinct capabilities cannot be
   parameterized cleanly (e.g. browser debugging vs native probe
   may need separate `observe` calls rather than a unified one).
   Track actual tool count at each cycle; if it stays above 12,
   revisit the v2 design.

## Scope for THIS cycle (m5-agent-api-v2-explore)

This cycle ships ONLY this document. No code changes. No refactor.
The next cycle (`m5-01-services-skeleton`) is the first concrete step.

Cycle artifacts produced:
- `exploration-report.md` (this file) — scoping for downstream cycles
- `implementation-receipt.json` — points at this doc
- `verification-report.md` — confirms no code changed

---

## References

- Vault spec: `~/.sddk-knowledge/p-3416cfb8288f8964/specs/AGENT_API_V2.md`
- Vault spec (related): `~/.sddk-knowledge/p-3416cfb8288f8964/specs/RUNTIME_PROPERTIES_AND_SLICING.md`
- Roadmap: `~/Proyectos/rust/chronos/docs/chronos-agentic-reconstruction/docs/roadmap/ROADMAP.md` (M5 section)
- Current code: `crates/chronos-mcp/src/server.rs` (5,341 LoC, 42 `#[tool]` registrations)
