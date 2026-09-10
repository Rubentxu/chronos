//! Chronos services layer — extracted business logic from chronos-mcp.
//!
//! Services are plain Rust structs that can be called from any RPC layer
//! (today's rmcp, tomorrow's REST, etc.).
//!
//! ## Module index (post-M5 close, 2026-09-10; updated m6-01, 2026-09-10; updated m6-02, 2026-09-10)
//!
//! 14 service modules, each owning the algorithm for one or more MCP
//! tools. The MCP wrappers at `crates/chronos-mcp/src/server.rs` only
//! parse params, build a `*Context<'_>`, dispatch to the service, and
//! map `ServiceError` back to MCP error text.
//!
//! | Module | LoC | Owns |
//! |---|---|---|
//! | [`analysis`] | ~390 | `mutation_lens`, `causal_slice` (M3) |
//! | [`browser_probe`] | ~270 | `browser_probe_start/stop/drain` |
//! | [`debug_read`] | ~600 | `get_event`, memory read, register read, `state_diff`, `get_memory`, `get_registers`, `analyze_memory`, `evaluate_expression`, `forensic_audit` |
//! | [`debug_trace`] | ~600 | `execution_summary`, `get_call_stack`, `search_events`, `events_read` |
//! | [`debug_trace_specialized`] | ~650 | `debug_call_graph`, `debug_query_by_kind`, `debug_locals`, `find_variable_origin`, `find_crash`, `inspect_causality`, `detect_races`, `expand_hotspot`, `get_saliency_scores` |
//! | [`diff`] | ~400 | `performance_regression_audit`, `compare_sessions` |
//! | [`probe`] | ~590 | `probe_start`, `probe_stop`, `probe_drain` |
//! | [`query_service`] | ~120 | `query`, `list_threads` |
//! | [`sessions`] | ~670 | session CRUD: `save/load/list/delete/drop` |
//! | [`state_query`] | ~330 | **M6 dispatcher.** `state_query` v2 (kind: register_diff / memory_read / register_snapshot / memory_analysis / expression_eval). |
//! | [`trace_slice`] | ~400 | **M6 dispatcher.** `trace_slice` v2 (kind: variable_origin / crash / causality / memory_audit). |
//! | [`tripwires`] | ~680 | `tripwire_*` |
//!
//! Supporting modules: [`error`] (the `ServiceError` enum), [`output`]
//! (all DTOs), and the 14 algorithm modules above.
//!
//! M6 progress:
//! - m6-01: added [`trace_slice`] dispatcher. v1 tools `debug_find_variable_origin`,
//!   `debug_find_crash`, `inspect_causality`, `forensic_memory_audit` are now
//!   deprecated MCP shims that route through `ChronosTraceSliceService::slice`.
//! - m6-02: added [`state_query`] dispatcher. v1 tools `state_diff`,
//!   `debug_get_memory`, `debug_get_registers`, `debug_analyze_memory`,
//!   `evaluate_expression` are now deprecated MCP shims that route through
//!   `ChronosStateQueryService::query`.
//! - remaining: `execution_query` (m6-03), `hypothesis_test` (m6-04),
//!   `session_export` (m6-05), `events_read` merge + full deprecation sweep (m6-06).
//!   See `docs/milestones/m5-close-report.md`.

pub mod analysis;
pub mod browser_probe;
pub mod debug_read;
pub mod debug_trace;
pub mod debug_trace_specialized;
pub mod diff;
pub mod error;
pub mod execution_query;
pub mod output;
pub mod probe;
pub mod query_service;
pub mod sessions;
pub mod state_query;
pub mod trace_slice;
pub mod tripwires;
