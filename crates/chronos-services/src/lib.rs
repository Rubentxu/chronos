//! Chronos services layer — extracted business logic from chronos-mcp.
//!
//! Services are plain Rust structs that can be called from any RPC layer
//! (today's rmcp, tomorrow's REST, etc.).
//!
//! ## Module index (post-M7-05 close, 2026-09-11)
//!
//! 20 algorithm modules (one per service), each owning the algorithm for
//! one or more MCP tools. The MCP wrappers at `crates/chronos-mcp/src/server.rs`
//! only parse params, build a `*Context<'_>`, dispatch to the service, and
//! map `ServiceError` back to MCP error text.
//!
//! | Module | LoC | Owns |
//! |---|---|---|
//! | [`analysis`] | 396 | `mutation_lens`, `causal_slice` (M3) |
//! | [`browser_probe`] | 273 | `browser_probe_start/stop/drain` |
//! | [`debug_read`] | 606 | `get_event`, memory read, register read, `get_memory`, `get_registers`, `analyze_memory`, `evaluate_expression`, `forensic_audit` — **3 callers now route via [`state_query`] dispatcher** |
//! | [`debug_trace`] | 596 | `execution_summary`, `get_call_stack`, `search_events`, `state_diff`, `debug_call_graph` — **2 callers (`query_events` + `get_event`) now route via [`events_read`] dispatcher (m7-01)** |
//! | [`debug_trace_specialized`] | 649 | `debug_query_by_kind`, `debug_locals`, `find_variable_origin`, `find_crash`, `inspect_causality`, `detect_races`, `expand_hotspot`, `get_saliency_scores` — **6 callers now route via [`execution_query`] dispatcher** |
//! | [`diff`] | 405 | `performance_regression_audit`, `compare_sessions` |
//! | [`events_read`] | 290 | **M7 (m7-01) dispatcher.** `events_read` v2 (mode: query / by_id). Cursor-based paginated read with filters (event_types, thread, time range, function_pattern, limit, cursor). Cursor encoding matches the existing `probe_drain` cursor. `gap_summary` always None in m7-01 (QueryEngine does not yet expose gap info). |
//! | [`execution_query`] | 331 | **M6 (m6-03) dispatcher.** `execution_query` v2 (kind: call_stack / execution_summary / call_graph / race_detect / hotspot / saliency). |
//! | [`hypothesis_test`] | 1023 | **M6 (m6-04) net-new.** `hypothesis_test` v2 (kind: invariant / existence / call_path). No v1 shim. |
//! | [`observe`] | 715 | **M7 (m7-02) dispatcher.** `observe` v2 (verb: create / list / delete / query; update rejected with `Unsupported`). Subsumes the v1 `tripwire_create`/`tripwire_list`/`tripwire_delete`/`tripwire_query`/`probe_inject` tools (now deprecated MCP shims). |
//! | [`probe`] | 588 | `probe_start`, `probe_stop`, `probe_drain` |
//! | [`query_service`] | 124 | `query`, `list_threads` |
//! | [`session_compare`] | ~384 | **M7 (m7-03) dispatcher.** `session_compare` v2 (kind: divergence / regression). Subsumes the v1 `compare_sessions` (kind=divergence) and `performance_regression_audit` (kind=regression) tools (now deprecated MCP shims). |
//! | [`session_explain`] | ~717 | **M7 (m7-03) net-new.** `session_explain` v2 (kind: facts / derived / inferred / hypothesis). No v1 shim. |
//! | [`session_export`] | 681 | **M6 (m6-05) net-new.** `session_export` v2 (format: json / otlp_json; zip_json reserved for m7+). Atomic tmp+rename write. `properties_snapshot` always empty in m6-05 (see `docs/milestones/m6-05-session-export.md`). |
//! | [`session_lifecycle`] | ~739 | **M7 (m7-05) dispatcher.** `session_start` v2 (action: spawn / load / attach), `session_stop` v2 (with `seal_tail` + `drain_subscriptions` defaults), `capabilities` v2 (target + session). v1 tools `probe_start` + `probe_stop` are deprecated MCP shims that route through `ChronosSessionLifecycleService::start{action=spawn}` / `stop`. `attach` returns `Unsupported` (m7+). `session_stop{drain_subscriptions=true}` chains to `observe{verb=list}` (destructive). |
//! | [`sessions`] | 671 | session CRUD: `save/load/list/delete/drop` |
//! | [`state_query`] | 328 | **M6 (m6-02) dispatcher.** `state_query` v2 (kind: register_diff / memory_read / register_snapshot / memory_analysis / expression_eval). |
//! | [`trace_slice`] | 377 | **M6 (m6-01) dispatcher.** `trace_slice` v2 (kind: variable_origin / crash / causality / memory_audit). |
//! | [`tripwires`] | 678 | `tripwire_*` (algorithm only; v1 shims dispatch via [`observe`] in m7-02) |
//!
//! Supporting modules: [`error`] (the `ServiceError` enum), [`output`]
//! (all DTOs), and the module index you are reading now (`lib`).
//! Total: **21 files, ~11,500 LoC** (algorithm + DTOs + supporting).
//! Exact post-m7-05 numbers will be recorded in the next close cycle.
//!
//! ## M7 (sub-cycle) progress
//!
//! - m7-01: added [`events_read`] dispatcher. v1 tools `query_events` and
//!   `get_event` are now deprecated MCP shims that route through
//!   `ChronosEventsReadService::read`. The v2 surface adds the spec-line-60
//!   fields `next_cursor`, `completeness`, `gap_summary`, and `provenance`
//!   to the response. See `docs/milestones/m7-01-events-read-merge.md`.
//! - m7-02: added [`observe`] dispatcher. v1 tools `tripwire_create`,
//!   `tripwire_list`, `tripwire_delete`, `tripwire_query`, and `probe_inject`
//!   are now deprecated MCP shims that route through
//!   `ChronosObserveService::observe`. The v2 surface adds the spec-line-14
//!   fields `action`, `retention`, `requested_evidence`, `scope`, `cursor`,
//!   and `provenance` to the request and response. See
//!   `docs/milestones/m7-02-observability-merge.md`.
//! - m7-03: added [`session_compare`] dispatcher and [`session_explain`] net-new.
//!   v1 tools `compare_sessions` and `performance_regression_audit` are now
//!   deprecated MCP shims that route through `ChronosSessionCompareService::compare`
//!   (with `kind=divergence` and `kind=regression` respectively). The `session_explain`
//!   surface adds `kind: facts / derived / inferred / hypothesis` and returns the
//!   m6-04-style `HypothesisTestPlan::CrashInvariant` and a new
//!   `DominantFunctionCallPath` plan. See `docs/milestones/m7-03-session-compare-explain-merge.md`.
//! - m7-04: foundation-only cycle. Added v2 DTOs
//!   (`SessionStartInput/Output`, `SessionStopInput/Output`,
//!   `CapabilitiesInput/Output`, `StaticCapabilities`,
//!   `DynamicCapabilities`, `CapabilitySnapshot`,
//!   `SessionLifecycleProvenance`) and extended `SessionMetadata`
//!   with `tail_sealed` + `sealed_at`. No dispatcher / wrappers /
//!   shims — deferred to m7-05 (see `docs/milestones/m7-04-session-start-stop-capabilities-merge.md` § "Revision note").
//! - m7-05: added [`session_lifecycle`] dispatcher. `session_start` v2
//!   (action: spawn / load / attach), `session_stop` v2 (with
//!   `seal_tail` + `drain_subscriptions` defaults), `capabilities` v2
//!   (target + session_id). v1 tools `probe_start` + `probe_stop` are
//!   deprecated MCP shims that route through the new dispatcher.
//!   `attach` returns `Unsupported` (m7+).
//!
//! ## Next (M7+ sub-cycle)
//!
//! See `docs/milestones/m6-close-report.md` §6 + `docs/milestones/m7-events-read-scoping.md`
//! for the deferred worklist: m7-06 (v1 sunset bookkeeping + M7 close),
//! deprecation sunset sweep (after 2027-09-11).

pub mod analysis;
pub mod browser_probe;
pub mod counterexample;
pub mod debug_read;
pub mod debug_trace;
pub mod debug_trace_specialized;
pub mod diff;
pub mod error;
pub mod events_read;
pub mod execution_query;
pub mod hypothesis_test;
pub mod observe;
pub mod output;
pub mod probe;
pub mod query_service;
pub mod session_compare;
pub mod session_explain;
pub mod session_export;
pub mod session_lifecycle;
pub mod sessions;
pub mod state_query;
pub mod trace_slice;
pub mod tripwires;
