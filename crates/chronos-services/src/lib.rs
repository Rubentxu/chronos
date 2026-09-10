//! Chronos services layer — extracted business logic from chronos-mcp.
//!
//! Services are plain Rust structs that can be called from any RPC layer
//! (today's rmcp, tomorrow's REST, etc.).
//!
//! ## Module index (post-M5 close, 2026-09-10)
//!
//! 12 service modules, each owning the algorithm for one or more MCP
//! tools. The MCP wrappers at `crates/chronos-mcp/src/server.rs` only
//! parse params, build a `*Context<'_>`, dispatch to the service, and
//! map `ServiceError` back to MCP error text.
//!
//! | Module | LoC | Owns |
//! |---|---|---|
//! | [`analysis`] | ~390 | `mutation_lens`, `causal_slice` (M3) |
//! | [`browser_probe`] | ~270 | `browser_probe_start/stop/drain` |
//! | [`debug_read`] | ~600 | `get_event`, memory read, register read, `state_diff` |
//! | [`debug_trace`] | ~600 | `execution_summary`, `get_call_stack`, `search_events`, `events_read` |
//! | [`debug_trace_specialized`] | ~650 | `debug_call_graph`, `debug_query_by_kind`, `debug_locals` |
//! | [`diff`] | ~400 | `performance_regression_audit`, `compare_sessions` |
//! | [`probe`] | ~590 | `probe_start`, `probe_stop`, `probe_drain` |
//! | [`query_service`] | ~120 | `query`, `list_threads` |
//! | [`sessions`] | ~670 | session CRUD: `save/load/list/delete/drop` |
//! | [`tripwires`] | ~680 | `tripwire_*` |
//!
//! Supporting modules: [`error`] (the `ServiceError` enum), [`output`]
//! (all DTOs), and the 12 algorithm modules above.
//!
//! M6 candidates (see `docs/milestones/m5-close-report.md`):
//! merge tools into `trace_slice` / `state_query` / `execution_query`,
//! add `hypothesis_test` + `session_export`, then deprecation shims.

pub mod analysis;
pub mod browser_probe;
pub mod debug_read;
pub mod debug_trace;
pub mod debug_trace_specialized;
pub mod diff;
pub mod error;
pub mod output;
pub mod probe;
pub mod query_service;
pub mod sessions;
pub mod tripwires;
