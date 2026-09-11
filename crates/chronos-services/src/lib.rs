//! Chronos services layer — extracted business logic from chronos-mcp.
//!
//! Services are plain Rust structs that can be called from any RPC layer
//! (today's rmcp, tomorrow's REST, etc.).
//!
//! ## Module index (post-M6 close, 2026-09-11)
//!
//! 15 algorithm modules (one per service), each owning the algorithm for
//! one or more MCP tools. The MCP wrappers at `crates/chronos-mcp/src/server.rs`
//! only parse params, build a `*Context<'_>`, dispatch to the service, and
//! map `ServiceError` back to MCP error text.
//!
//! | Module | LoC | Owns |
//! |---|---|---|
//! | [`analysis`] | 396 | `mutation_lens`, `causal_slice` (M3) |
//! | [`browser_probe`] | 273 | `browser_probe_start/stop/drain` |
//! | [`debug_read`] | 606 | `get_event`, memory read, register read, `get_memory`, `get_registers`, `analyze_memory`, `evaluate_expression`, `forensic_audit` — **3 callers now route via [`state_query`] dispatcher** |
//! | [`debug_trace`] | 596 | `execution_summary`, `get_call_stack`, `search_events`, `events_read`, `state_diff`, `debug_call_graph` — **4 callers now route via [`trace_slice`] dispatcher** |
//! | [`debug_trace_specialized`] | 649 | `debug_query_by_kind`, `debug_locals`, `find_variable_origin`, `find_crash`, `inspect_causality`, `detect_races`, `expand_hotspot`, `get_saliency_scores` — **6 callers now route via [`execution_query`] dispatcher** |
//! | [`diff`] | 405 | `performance_regression_audit`, `compare_sessions` |
//! | [`execution_query`] | 331 | **M6 (m6-03) dispatcher.** `execution_query` v2 (kind: call_stack / execution_summary / call_graph / race_detect / hotspot / saliency). |
//! | [`hypothesis_test`] | 1023 | **M6 (m6-04) net-new.** `hypothesis_test` v2 (kind: invariant / existence / call_path). No v1 shim. |
//! | [`probe`] | 588 | `probe_start`, `probe_stop`, `probe_drain` |
//! | [`query_service`] | 124 | `query`, `list_threads` |
//! | [`session_export`] | 681 | **M6 (m6-05) net-new.** `session_export` v2 (format: json / otlp_json; zip_json reserved for m7+). Atomic tmp+rename write. `properties_snapshot` always empty in m6-05 (see `docs/milestones/m6-05-session-export.md`). |
//! | [`sessions`] | 671 | session CRUD: `save/load/list/delete/drop` |
//! | [`state_query`] | 328 | **M6 (m6-02) dispatcher.** `state_query` v2 (kind: register_diff / memory_read / register_snapshot / memory_analysis / expression_eval). |
//! | [`trace_slice`] | 377 | **M6 (m6-01) dispatcher.** `trace_slice` v2 (kind: variable_origin / crash / causality / memory_audit). |
//! | [`tripwires`] | 678 | `tripwire_*` |
//!
//! Supporting modules: [`error`] (the `ServiceError` enum), [`output`]
//! (all DTOs), and the module index you are reading now (`lib`).
//! Total: **18 files, 9,503 LoC** (algorithm + DTOs + supporting).
//!
//! ## M6 (sub-cycle) progress
//!
//! Closed 2026-09-11 — see `docs/milestones/m6-close-report.md`.
//!
//! - m6-01: added [`trace_slice`] dispatcher (m6-01). v1 tools
//!   `debug_find_variable_origin`, `debug_find_crash`, `inspect_causality`,
//!   `forensic_memory_audit` are now deprecated MCP shims that route through
//!   `ChronosTraceSliceService::slice`.
//! - m6-02: added [`state_query`] dispatcher. v1 tools `debug_get_memory`,
//!   `debug_get_registers`, `debug_analyze_memory`, `evaluate_expression` are
//!   now deprecated MCP shims that route through
//!   `ChronosStateQueryService::query`.
//! - m6-03: added [`execution_query`] dispatcher. v1 tools `get_call_stack`,
//!   `get_execution_summary`, `debug_call_graph`, `debug_detect_races`,
//!   `debug_expand_hotspot`, `debug_get_saliency_scores` are now deprecated
//!   MCP shims that route through `ChronosExecutionQueryService::query`.
//! - m6-04: added [`hypothesis_test`] dispatcher. **Net-new v2 tool**
//!   (no v1 shim). Three typed shapes: `invariant` (Pass if every observation
//!   satisfies the comparator; never false-PASS), `existence` (Pass if >=1
//!   matching event, else Violation), `call_path` (Pass if callee reachable
//!   from caller in the call graph, else Violation). All three return
//!   tri-state [`HypothesisVerdict`](output::HypothesisVerdict): Pass /
//!   Violation / Unsupported.
//! - m6-05: added [`session_export`] dispatcher. **Net-new v2 tool**
//!   (no v1 shim). Exports a session bundle (metadata + trace events +
//!   properties snapshot) to disk in `json` or `otlp_json` format;
//!   `zip_json` is reserved for m7+ and is rejected by the dispatcher.
//!   Atomic tmp+rename write guarantees no partial files at the final
//!   path. **Known limitation:** the `properties_snapshot` field is
//!   always empty in m6-05 because the `QueryEngine` API does not
//!   currently expose a property-table view (properties are evaluated
//!   on-demand by name). The DTO and JSON schema are final so m7+ can
//!   fill the field without breaking consumers.
//!
//! ## Next (M7+ sub-cycle)
//!
//! See `docs/milestones/m6-close-report.md` §6 for the deferred worklist:
//! `events_read` merge, `observe` merge, `session_compare` /
//! `session_explain` split, `session_start` / `session_stop` /
//! `capabilities` v2 surface, deprecation sunset sweep (after 2027-09-11).

pub mod analysis;
pub mod browser_probe;
pub mod debug_read;
pub mod debug_trace;
pub mod debug_trace_specialized;
pub mod diff;
pub mod error;
pub mod execution_query;
pub mod hypothesis_test;
pub mod output;
pub mod probe;
pub mod query_service;
pub mod session_export;
pub mod sessions;
pub mod state_query;
pub mod trace_slice;
pub mod tripwires;
