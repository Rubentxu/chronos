//! Tool parameter types, toolset constants and pure helpers for the MCP server.
//!
//! Extracted verbatim from `server.rs`. Everything in this module is a data
//! type, a constant, or a pure function: it holds no `ChronosServer` state and
//! no handle, so it can be reasoned about without the handler surface.
//!
//! The `impl ChronosServer` blocks — the parts that do touch server state —
//! stay in `server.rs`, which re-exports the public items from here.

use chronos_domain::trace::event::EventType;
use chronos_domain::tripwire::TripwireCondition;
use chronos_services::output::{EventsReadKind, ExecutionQueryKind, StateQueryKind};
use schemars::JsonSchema;
use serde::Deserialize;

// ============================================================================
// Toolset filtering constants (MS-CAP-DISCOVERY / REQ-CAP-004, REQ-CAP-005)
// ============================================================================

/// Tools available in the `native` toolset profile.
/// These are the core tools that do not require a language runtime or browser.
/// Kept ≤ 25 as required by REQ-CAP-005.
pub const NATIVE_TOOL_NAMES: &[&str] = &[
    // Session lifecycle (5)
    "session_start",
    "session_stop",
    "session_snapshot",
    "session_export",
    "session_compare",
    // Storage (5)
    "save_session",
    "load_session",
    "list_sessions",
    "delete_session",
    "drop_session",
    // Probe lifecycle (6)
    "probe_start",
    "probe_stop",
    "probe_drain",
    "probe_drain_log",
    "probe_compaction_metrics",
    "probe_status",
    // Analysis / core read tools (7)
    "events_read",
    "execution_query",
    "state_query",
    "observe",
    "capabilities",
    "list_threads",
    "session_explain",
];

/// Tools available in the `ebpf` toolset profile.
pub const EBPF_TOOL_NAMES: &[&str] = &[
    "session_start",
    "session_stop",
    "session_snapshot",
    "session_export",
    "session_compare",
    "session_explain",
    "save_session",
    "load_session",
    "list_sessions",
    "delete_session",
    "drop_session",
    "probe_start",
    "probe_stop",
    "probe_drain",
    "probe_drain_log",
    "probe_compaction_metrics",
    "probe_status",
    "events_read",
    "execution_query",
    "state_query",
    "observe",
    "capabilities",
    "list_threads",
    "compare_sessions",
    "causal_slice",
    "trace_slice",
    "mutation_lens",
    "hypothesis_test",
    "performance_regression_audit",
    "counterexample_shrink",
    "counterexample_get",
    "counterexample_list",
    "counterexample_events_count",
    "counterexample_bundle_events",
];

/// Tools available in the `python` toolset profile.
pub const PYTHON_TOOL_NAMES: &[&str] = &[
    "session_start",
    "session_stop",
    "session_snapshot",
    "session_export",
    "session_compare",
    "session_explain",
    "save_session",
    "load_session",
    "list_sessions",
    "delete_session",
    "drop_session",
    "probe_start",
    "probe_stop",
    "probe_drain",
    "probe_drain_log",
    "probe_compaction_metrics",
    "probe_status",
    "events_read",
    "execution_query",
    "state_query",
    "observe",
    "capabilities",
    "list_threads",
    "compare_sessions",
    "causal_slice",
    "trace_slice",
    "mutation_lens",
    "hypothesis_test",
    "performance_regression_audit",
    "debug_get_variables",
    "counterexample_shrink",
    "counterexample_get",
    "counterexample_list",
    "counterexample_events_count",
    "counterexample_bundle_events",
];

/// Tools available in the `java` toolset profile.
pub const JAVA_TOOL_NAMES: &[&str] = &[
    "session_start",
    "session_stop",
    "session_snapshot",
    "session_export",
    "session_compare",
    "session_explain",
    "save_session",
    "load_session",
    "list_sessions",
    "delete_session",
    "drop_session",
    "probe_start",
    "probe_stop",
    "probe_drain",
    "probe_drain_log",
    "probe_compaction_metrics",
    "probe_status",
    "events_read",
    "execution_query",
    "state_query",
    "observe",
    "capabilities",
    "list_threads",
    "compare_sessions",
    "causal_slice",
    "trace_slice",
    "mutation_lens",
    "hypothesis_test",
    "performance_regression_audit",
    "debug_get_variables",
    "counterexample_shrink",
    "counterexample_get",
    "counterexample_list",
    "counterexample_events_count",
    "counterexample_bundle_events",
];

/// Tools available in the `go` toolset profile.
pub const GO_TOOL_NAMES: &[&str] = &[
    "session_start",
    "session_stop",
    "session_snapshot",
    "session_export",
    "session_compare",
    "session_explain",
    "save_session",
    "load_session",
    "list_sessions",
    "delete_session",
    "drop_session",
    "probe_start",
    "probe_stop",
    "probe_drain",
    "probe_drain_log",
    "probe_compaction_metrics",
    "probe_status",
    "events_read",
    "execution_query",
    "state_query",
    "observe",
    "capabilities",
    "list_threads",
    "compare_sessions",
    "causal_slice",
    "trace_slice",
    "mutation_lens",
    "hypothesis_test",
    "performance_regression_audit",
    "debug_get_variables",
    "counterexample_shrink",
    "counterexample_get",
    "counterexample_list",
    "counterexample_events_count",
    "counterexample_bundle_events",
];

/// Tools available in the `js` (JavaScript) toolset profile.
pub const JS_TOOL_NAMES: &[&str] = &[
    "session_start",
    "session_stop",
    "session_snapshot",
    "session_export",
    "session_compare",
    "session_explain",
    "save_session",
    "load_session",
    "list_sessions",
    "delete_session",
    "drop_session",
    "probe_start",
    "probe_stop",
    "probe_drain",
    "probe_drain_log",
    "probe_compaction_metrics",
    "probe_status",
    "events_read",
    "execution_query",
    "state_query",
    "observe",
    "capabilities",
    "list_threads",
    "compare_sessions",
    "causal_slice",
    "trace_slice",
    "mutation_lens",
    "hypothesis_test",
    "performance_regression_audit",
    "debug_get_variables",
    "counterexample_shrink",
    "counterexample_get",
    "counterexample_list",
    "counterexample_events_count",
    "counterexample_bundle_events",
];

/// Minimal toolset: only session lifecycle + capabilities.
///
/// C5.3 (REC-C5): the entries `probe_inject` and `state_diff` previously
/// referenced here were deprecated MCP shims retired in C5.3.2. The minimal
/// toolset now matches what remains live on the router: probe lifecycle
/// (`probe_start/stop/drain/...`) and the dispatcher facade
/// (`events_read`/`execution_query`/`state_query`) — no v1 aliases.
pub const MINIMAL_TOOL_NAMES: &[&str] = &[
    "session_start",
    "session_stop",
    "session_snapshot",
    "session_export",
    "session_compare",
    "session_explain",
    "save_session",
    "load_session",
    "list_sessions",
    "delete_session",
    "drop_session",
    "probe_start",
    "probe_stop",
    "probe_drain",
    "probe_drain_log",
    "probe_compaction_metrics",
    "probe_status",
    "events_read",
    "execution_query",
    "state_query",
    "observe",
    "capabilities",
];

/// Complete list of all registered tool names (42 total).
/// Used by `build_tool_availability` to populate the full `tool_availability` map.
///
/// Authoritative source: the live `#[rmcp::tool_router]` registration on
/// `ChronosServer`. The const below mirrors the router's name list; the
/// `toolset_sync_check` test asserts the two stay in lockstep. If you
/// add or remove a `#[tool(name = …)]`, extend or trim this list
/// accordingly.
pub const ALL_TOOL_NAMES: &[&str] = &[
    "execution_query",
    "state_query",
    "list_threads",
    "debug_get_variables",
    "save_session",
    "load_session",
    "list_sessions",
    "delete_session",
    "drop_session",
    "debug_diff",
    "probe_start",
    "probe_stop",
    "session_start",
    "session_stop",
    "capabilities",
    "probe_drain",
    "probe_drain_log",
    "probe_compaction_metrics",
    "session_snapshot",
    "probe_status",
    "capture_session",
    "probe_advance",
    "probe_step",
    "browser_probe_start",
    "browser_probe_stop",
    "browser_probe_drain",
    "performance_regression_audit",
    "compare_sessions",
    "session_compare",
    "session_explain",
    "mutation_lens",
    "causal_slice",
    "hypothesis_test",
    "session_export",
    "trace_slice",
    "events_read",
    "execution_log_read",
    "observe",
    "counterexample_shrink",
    "counterexample_get",
    "counterexample_list",
    "counterexample_events_count",
    "counterexample_bundle_events",
];

/// Return the required capabilities for a given tool name.
/// Empty Vec means the tool requires no specific capability.
pub fn tool_required_capabilities(tool_name: &str) -> Vec<String> {
    match tool_name {
        // Language runtime required
        "evaluate_expression" => vec!["language/any".to_string()],
        "debug_get_variables" => vec!["language/any".to_string()],
        "debug_get_memory" => vec!["language/any".to_string()],
        "debug_get_registers" => vec!["language/any".to_string()],
        "debug_diff" => vec!["language/any".to_string()],
        "debug_analyze_memory" => vec!["language/any".to_string()],
        "forensic_memory_audit" => vec!["language/any".to_string()],
        // Browser required
        "browser_probe_start" => vec!["browser".to_string()],
        "browser_probe_stop" => vec!["browser".to_string()],
        "browser_probe_drain" => vec!["browser".to_string()],
        // All others are native / universal
        _ => vec![],
    }
}

// ============================================================================
// Tool parameter types
// ============================================================================

pub fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueryEventsParams {
    /// Session ID to query.
    pub session_id: String,
    /// Filter by event types (v1 string-typed shim; validated against all 21
    /// `EventType` variants via `EventType::from_snake_case`).
    pub event_types: Option<Vec<String>>,
    /// Filter by thread ID.
    pub thread_id: Option<u64>,
    /// Start timestamp in nanoseconds (inclusive).
    pub timestamp_start: Option<u64>,
    /// End timestamp in nanoseconds (exclusive).
    pub timestamp_end: Option<u64>,
    /// Filter by function name pattern (glob).
    pub function_pattern: Option<String>,
    /// Maximum events to return.
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Number of events to skip.
    #[serde(default)]
    pub offset: usize,
}

pub fn default_limit() -> usize {
    100
}

/// Default time-bucket width for `execution_log_read` mode=summarize.
///
/// 1 second, matching the `CHRONOS_EXEC_EXPLORER_VIRT_THRESHOLD`
/// default documented in ADR-0029 §2.3. It is a DEFAULT, not a claim:
/// callers can widen it, and the response echoes the width actually used.
pub fn default_bucket_size_ns() -> u64 {
    1_000_000_000
}

/// Parameters for the v2 `events_read` tool (m7-01).
///
/// `mode=query` is the paginated event-list read (supersedes v1
/// `query_events`); `mode=by_id` is the single-event lookup (supersedes
/// v1 `get_event`). The cursor field carries an opaque
/// `{total_pushed, snapshot_len}` payload — same encoding as the
/// `probe_drain` cursor at the MCP boundary.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct EventsReadParams {
    /// Session ID to read.
    pub session_id: String,
    /// Discriminator: `"query"` for paginated event list, `"by_id"` for
    /// single-event lookup.
    #[schemars(rename = "mode")]
    pub mode: EventsReadKind,
    /// Event ID (required when mode=by_id).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_id: Option<u64>,
    /// Filter by event types (mode=query only; typed snake_case enum, e.g.
    /// "function_entry"). Unknown names are rejected at JSON-RPC parse time
    /// with a typed schema error (MS-EVT-TYPED / ADR-0003).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_types: Option<Vec<EventType>>,
    /// Filter by thread ID (mode=query only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<u64>,
    /// Start timestamp in nanoseconds (inclusive; mode=query only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp_start: Option<u64>,
    /// End timestamp in nanoseconds (exclusive; mode=query only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp_end: Option<u64>,
    /// Filter by function name pattern (glob; mode=query only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function_pattern: Option<String>,
    /// Maximum events to return (mode=query only).
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Opaque cursor for the next page (`ecv1:<schema>:<len>:<session>:<seq>`,
    /// as returned in `next_cursor`). Omitted for the first page, which starts
    /// at seq#0 inclusive. A cursor minted for another session is refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

/// Parameters for the `execution_log_read` tool (M10 read path).
///
/// Read-path views over the SAME authoritative per-session
/// `ExecutionLog` that `events_read` consults. It is a distinct
/// discriminator on purpose: `events_read` returns paged evidence,
/// this returns the Execution Explorer's stream and aggregate views.
/// Keeping them separate stops an aggregate from being mistaken for
/// evidence (REC-C1 single-truth).
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
pub struct ExecutionLogReadParams {
    /// Session whose log to read.
    pub session_id: String,
    /// Discriminator: `poll` | `summarize` | `rollup` | `causality`.
    #[schemars(rename = "mode")]
    pub mode: ExecutionLogReadKind,
    /// Bucket width in nanoseconds (mode=summarize only; must be > 0).
    #[serde(default = "default_bucket_size_ns")]
    pub bucket_size_ns: u64,
    /// Maximum events per poll (mode=poll only).
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Opaque cursor (`ecv1:…`), as returned in `next_cursor`. Omitted
    /// starts at seq#0. A cursor minted for another session is refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

/// Which read-path view an agent is asking for.
///
/// Kept separate from `EventsReadKind` on purpose: `events_read` is the
/// paged evidence reader, this is the Execution Explorer's aggregate and
/// stream view. Both read the same authoritative log; conflating them
/// would make the explorer's aggregates look like evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionLogReadKind {
    /// Bounded live poll of new events past the cursor.
    Poll,
    /// Time-bucketed summary of the whole log.
    Summarize,
    /// Per-invocation (per-thread) rollup of the whole log.
    Rollup,
    /// Causality availability for this session.
    Causality,
}

/// Parameters for the v2 `observe` tool (m7-02).
///
/// The unified v2 entry-point that supersedes 5 v1 tools:
/// `tripwire_create`, `tripwire_list`, `tripwire_delete`, `tripwire_query`,
/// and `probe_inject`. The `verb` discriminator selects the operation:
///   - `create`  → register a subscription (tripwire or uprobe)
///   - `list`    → enumerate subscriptions + drain fired events (destructive)
///   - `query`   → non-destructive subscription snapshot
///   - `delete`  → unregister a subscription
///   - `update`  → rejected with `unsupported` in m7-02 (deferred to m7+)
///
/// See `docs/milestones/m7-02-observability-merge.md` for the full spec.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ObserveParams {
    /// Discriminator: `"create" | "list" | "update" | "delete" | "query"`.
    #[schemars(rename = "verb")]
    pub verb: chronos_services::output::ObserveVerb,
    /// Subscription ID (required for `verb=delete`).
    /// String on the wire; parsed into the typed
    /// [`chronos_domain::SubscriptionId`] at the boundary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription_id: Option<chronos_domain::SubscriptionId>,
    /// Subscription body — discriminator + payload. Required for
    /// `verb=create`. Shape:
    /// `{kind: "tripwire", condition: {...}, label: "..."}` for tripwires,
    /// `{kind: "uprobe", binary_path: "...", symbol_name: "...", pid: <u32>, label: "..."}`
    /// for uprobe-injection conditions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<ObserveConditionWire>,
    /// What to do when the subscription fires (`"record" | "notify" |
    /// "inject_uprobe"`). Defaults to `"record"`. `inject_uprobe` is
    /// parsed but is a no-op in m7-02.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<chronos_services::output::ObserveAction>,
    /// Retention policy (`"drained" | "retained_until_session_end" |
    /// "permanent"`). Defaults to `"drained"` (matches v1 `tripwire_list`).
    /// `permanent` is rejected with `unsupported` in m7-02.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retention: Option<chronos_services::output::ObserveRetention>,
    /// Requested evidence kind (`{kind: "event_types", event_types: [...]}`
    /// or `{kind: "properties", ...}`). `properties` is rejected in m7-02.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_evidence: Option<ObserveRequestedEvidenceWire>,
    /// Scope (`{scope: "session", session_id: "..."}` or `{scope: "global"}`).
    /// Required for `verb=create` + `condition.kind=uprobe`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<ObserveScopeWire>,
    /// Optional opaque cursor for `verb=list` (`ecv1:<schema>:<len>:<session>:<seq>`).
    ///
    /// REC-C2.1.4b: firings are paged out of the ExecutionLog, so the cursor is
    /// an `EventsCursorV1` token (session + next EventSeq), not the legacy bus
    /// cursor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    /// Optional human-readable label (alternative to `condition.label`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// Wire-shape wrapper for the v2 `observe` `condition` body. The MCP
/// layer parses this into the dispatcher's typed
/// [`ObserveCondition`](chronos_services::output::ObserveCondition) (which
/// carries a parsed `TripwireCondition`, not a raw JSON value).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ObserveConditionWire {
    /// Tripwire-style condition.
    Tripwire {
        /// The v1 JSON shape: `{"type": "event_type", "event_types": [...]}` etc.
        /// Re-parsed via [`TripwireConditionType::into_condition`].
        condition: serde_json::Value,
        /// Optional human-readable label.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
    },
    /// Uprobe-injection condition.
    Uprobe {
        /// Path to the binary or shared library.
        binary_path: String,
        /// Symbol name to attach the uprobe to.
        symbol_name: String,
        /// Optional PID override.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pid: Option<u32>,
        /// Optional human-readable label.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
    },
}

/// Wire-shape wrapper for the v2 `observe` `requested_evidence` body.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ObserveRequestedEvidenceWire {
    /// Capture events matching the given event-types filter.
    EventTypes {
        /// Event-type name list (e.g. `["function_entry", "exception"]`).
        event_types: Vec<String>,
    },
    /// Reserved; rejected with `unsupported` in m7-02.
    Properties {
        /// Property names to project (deferred to m7+).
        names: Vec<String>,
    },
}

/// Wire-shape wrapper for the v2 `observe` `scope` body.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(tag = "scope", rename_all = "snake_case")]
pub enum ObserveScopeWire {
    /// Subscription attached to a specific session id.
    Session {
        /// Session id (required when `scope=session`).
        session_id: String,
    },
    /// Subscription applies to every live session.
    Global,
}

/// Default BFS expansion depth for `hypothesis_test` kind=call_path.
/// Matches the `debug_call_graph` v1 default (see `chronos_services::debug_trace`).
fn default_max_depth() -> usize {
    10
}

/// Parse the JSON-friendly `scope` string from `HypothesisTestParams`
/// into the typed [`chronos_services::output::HypothesisScope`]. Returns
/// `None` for unknown values — the dispatcher then uses its
/// `EventCount` default; for `latency_ms` the dispatcher returns
/// `Unsupported` with the documented reason.
pub fn parse_hypothesis_scope(s: &str) -> Option<chronos_services::output::HypothesisScope> {
    use chronos_services::output::HypothesisScope;
    match s {
        "event_count" => Some(HypothesisScope::EventCount),
        "latency_ms" => Some(HypothesisScope::LatencyMs),
        "property_value" => Some(HypothesisScope::PropertyValue),
        _ => None,
    }
}

/// Parse the JSON-friendly `comparison` string from `HypothesisTestParams`
/// into the typed [`chronos_services::property::ComparisonOp`]. Returns
/// `None` for unknown values.
pub fn parse_comparison_op(s: &str) -> Option<chronos_services::output::ComparisonOp> {
    use chronos_services::output::ComparisonOp;
    match s {
        "lt" => Some(ComparisonOp::Lt),
        "le" => Some(ComparisonOp::Le),
        "gt" => Some(ComparisonOp::Gt),
        "ge" => Some(ComparisonOp::Ge),
        "eq" => Some(ComparisonOp::Eq),
        "ne" => Some(ComparisonOp::Ne),
        _ => None,
    }
}

/// Parse the JSON-friendly `format` string from `SessionExportParams`
/// into the typed [`chronos_services::output::ExportFormat`]. Returns
/// `None` for unknown values — the dispatcher then returns
/// `ServiceError::InvalidExportParameter` with a clear reason. The
/// `zip_json` variant is parsed but rejected by the dispatcher itself
/// (m7+ scope).
pub fn parse_export_format(s: &str) -> Option<chronos_services::output::ExportFormat> {
    use chronos_services::output::ExportFormat;
    match s {
        "json" => Some(ExportFormat::Json),
        "otlp_json" => Some(ExportFormat::OtlpJson),
        "zip_json" => Some(ExportFormat::ZipJson),
        _ => None,
    }
}

/// Parse a `session_compare{kind=...}` discriminator string.
/// Returns an error if the caller asked for anything other than the two
/// documented kinds (divergence / regression).
pub fn parse_session_compare_kind(
    s: &str,
) -> Result<chronos_services::output::SessionCompareKind, rmcp::ErrorData> {
    use chronos_services::output::SessionCompareKind;
    match s {
        "divergence" => Ok(SessionCompareKind::Divergence),
        "regression" => Ok(SessionCompareKind::Regression),
        other => Err(invalid_kind_error(other, &["divergence", "regression"])),
    }
}

/// Parse a `session_explain{kind=...}` discriminator string.
/// Returns an error for anything outside the four documented kinds
/// (facts / derived / inferred / hypothesis).
pub fn parse_session_explain_kind(
    s: &str,
) -> Result<chronos_services::output::SessionExplainKind, rmcp::ErrorData> {
    use chronos_services::output::SessionExplainKind;
    match s {
        "facts" => Ok(SessionExplainKind::Facts),
        "derived" => Ok(SessionExplainKind::Derived),
        "inferred" => Ok(SessionExplainKind::Inferred),
        "hypothesis" => Ok(SessionExplainKind::Hypothesis),
        other => Err(invalid_kind_error(
            other,
            &["facts", "derived", "inferred", "hypothesis"],
        )),
    }
}

/// Build a uniform "unknown discriminator" error so callers see the same
/// shape from every parser. The MCP server maps `ErrorData` straight to a
/// tool-error response; no further wrapping needed at the call site.
fn invalid_kind_error(other: &str, allowed: &[&str]) -> rmcp::ErrorData {
    use rmcp::model::ErrorData as McpError;
    McpError::invalid_request(
        format!("unknown kind '{}'; allowed: {}", other, allowed.join(", ")),
        None,
    )
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetEventParams {
    /// Session ID.
    pub session_id: String,
    /// Event ID to retrieve.
    pub event_id: u64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetCallStackParams {
    /// Session ID.
    pub session_id: String,
    /// Event ID at which to reconstruct the stack.
    pub event_id: u64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetExecutionSummaryParams {
    /// Session ID.
    pub session_id: String,
}

/// v2 `execution_query` tool params.
///
/// Discriminated by `kind`. Each kind has its own optional target
/// fields; the dispatcher validates required fields (only `event_id`
/// for kind=call_stack is strictly required) and applies v1 default
/// values for optional fields.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExecutionQueryParams {
    /// Session ID.
    pub session_id: String,
    /// Query kind discriminator.
    pub kind: ExecutionQueryKind,
    /// Event ID (CallStack; required).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<u64>,
    /// Max call-graph depth (CallGraph; default 10).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_depth: Option<usize>,
    /// Race-detection threshold ns (RaceDetect; default 100).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold_ns: Option<u64>,
    /// Top-N for hotspot (Hotspot; default 10).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_n: Option<usize>,
    /// Saliency limit (Saliency; default 20).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saliency_limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StateDiffParams {
    /// Session ID.
    pub session_id: String,
    /// First timestamp (nanoseconds).
    pub timestamp_a: u64,
    /// Second timestamp (nanoseconds).
    pub timestamp_b: u64,
}

/// v2 `state_query` tool params.
///
/// Discriminated by `kind`. Each kind requires specific fields; the
/// dispatcher validates and returns `ServiceError::InvalidInput` for
/// missing required fields.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct StateQueryParams {
    /// Session ID.
    pub session_id: String,
    /// Query kind discriminator.
    pub kind: StateQueryKind,
    /// First timestamp (RegisterDiff).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp_a: Option<u64>,
    /// Second timestamp (RegisterDiff).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp_b: Option<u64>,
    /// Event ID (RegisterSnapshot, ExpressionEval).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<u64>,
    /// Memory address (MemoryRead).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<u64>,
    /// Timestamp in nanoseconds (MemoryRead).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp_ns: Option<u64>,
    /// Memory range start (MemoryAnalysis).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_address: Option<u64>,
    /// Memory range end (MemoryAnalysis).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_address: Option<u64>,
    /// Window start in nanoseconds (MemoryAnalysis).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_ts: Option<u64>,
    /// Window end in nanoseconds (MemoryAnalysis).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_ts: Option<u64>,
    /// Arithmetic expression (ExpressionEval).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expression: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListThreadsParams {
    /// Session ID.
    pub session_id: String,
}

// ============================================================================
// M3 — Mutation Lens Tools
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MutationLensParams {
    /// Session ID.
    pub session_id: String,
    /// Optional target variable name filter (e.g. "Order.total"). None = all.
    pub target: Option<String>,
    /// Max transitions to return (default 100).
    pub limit: Option<usize>,
}

// ============================================================================
// M3 — Causal Slice Tools
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CausalSliceParams {
    /// Session ID.
    pub session_id: String,
    /// Sink event ID to compute the backward causal slice from.
    pub sink_event_id: u64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DebugCallGraphParams {
    /// Session ID.
    pub session_id: String,
    /// Maximum call depth to include (default 10).
    #[serde(default = "default_call_graph_depth")]
    pub max_depth: usize,
}

fn default_call_graph_depth() -> usize {
    10
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DebugFindVariableOriginParams {
    /// Session ID.
    pub session_id: String,
    /// Variable name to trace (exact match).
    pub variable_name: String,
    /// Maximum number of mutations to return.
    #[serde(default = "default_limit")]
    pub limit: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DebugFindCrashParams {
    /// Session ID.
    pub session_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DebugDetectRacesParams {
    /// Session ID.
    pub session_id: String,
    /// Race detection threshold in nanoseconds (default 100).
    #[serde(default = "default_race_threshold_ns")]
    pub threshold_ns: u64,
}

fn default_race_threshold_ns() -> u64 {
    100
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct InspectCausalityParams {
    /// Session ID.
    pub session_id: String,
    /// Memory address (decimal) to inspect causal history.
    pub address: u64,
    /// Maximum number of entries to return.
    #[serde(default = "default_limit")]
    pub limit: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct HypothesisTestParams {
    /// Session ID.
    pub session_id: String,
    /// Discriminator selecting which typed hypothesis shape to evaluate.
    /// - `invariant`: reuses the property comparator; requires
    ///   `scope + comparison + constant`. When `scope=property_value`,
    ///   also requires `property_target`.
    /// - `existence`: requires `predicate` (event_type, thread, or
    ///   property_key).
    /// - `call_path`: requires `caller + callee`.
    pub kind: chronos_services::output::HypothesisKind,
    /// What scalar target the Invariant shape observes. JSON-friendly
    /// string the wrapper parses (avoids JsonSchema derive on the domain
    /// enum). Allowed values: `"event_count"`, `"latency_ms"`,
    /// `"property_value"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    /// Comparison operator for the Invariant shape. JSON-friendly string;
    /// allowed values: `"lt"`, `"le"`, `"gt"`, `"ge"`, `"eq"`, `"ne"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comparison: Option<String>,
    /// Constant value for the Invariant shape. Wraps the domain
    /// `PropertyValue` so MCP clients can send it as JSON without
    /// triggering a JsonSchema derive on the domain type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constant: Option<chronos_services::output::HypothesisConstant>,
    /// Property target key (required when `scope=property_value`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub property_target: Option<String>,
    /// Predicate for the Existence shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predicate: Option<chronos_services::output::ExistencePredicate>,
    /// Caller function for the CallPath shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caller: Option<String>,
    /// Callee function for the CallPath shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub callee: Option<String>,
    /// Maximum BFS expansion depth for CallPath (default 10).
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TraceSliceParams {
    /// Session ID.
    pub session_id: String,
    /// Discriminator selecting which kind of slice to produce.
    /// - `variable_origin`: causal evidence around a named variable.
    /// - `crash`: call stack at the last fatal signal in the session.
    /// - `causality`: full reads + writes at a memory address.
    /// - `memory_audit`: writes at a memory address, with call stacks.
    pub slice_kind: chronos_services::output::TraceSliceKind,
    /// Variable name (required when slice_kind=variable_origin).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variable_name: Option<String>,
    /// Memory address (required when slice_kind=causality or memory_audit).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<u64>,
    /// Maximum number of entries to return.
    #[serde(default = "default_limit")]
    pub limit: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DebugExpandHotspotParams {
    /// Session ID.
    pub session_id: String,
    /// Maximum functions to include (default 10 = Hotspot level).
    #[serde(default = "default_hotspot_limit")]
    pub top_n: usize,
}

fn default_hotspot_limit() -> usize {
    10
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DebugGetSaliencyScoresParams {
    /// Session ID.
    pub session_id: String,
    /// Maximum functions to score (default 20).
    #[serde(default = "default_saliency_limit")]
    pub limit: usize,
}

fn default_saliency_limit() -> usize {
    20
}

// ============================================================================
// SF5 — Persistence Tools (T10–T14)
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SaveSessionParams {
    /// Session ID (existing in-memory session).
    pub session_id: String,
    /// Language/runtime.
    pub language: String,
    /// Target program path or name.
    pub target: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SessionExportParams {
    /// Session ID (existing in-memory session). The exporter reads the
    /// live engine for this session and serializes its bundle to disk.
    pub session_id: String,
    /// Language/runtime metadata tag. Same vocabulary as `save_session`.
    pub language: String,
    /// Target program path or name. Same vocabulary as `save_session`.
    pub target: String,
    /// Output format. JSON-friendly string the wrapper parses via
    /// `parse_export_format`. Allowed values:
    /// - `"json"` -- pretty-printed canonical `ExportBundle`.
    /// - `"otlp_json"` -- OpenTelemetry-compatible JSON wire format.
    /// - `"zip_json"` -- reserved for m7+; rejected by the dispatcher.
    pub format: String,
    /// Absolute or server-CWD-relative path where the bundle will be
    /// written. The dispatcher uses an atomic tmp + rename write, so
    /// intermediate state is never visible at this path.
    pub output_path: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LoadSessionParams {
    /// Session ID to load from persistent store.
    pub session_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteSessionParams {
    /// Session ID to delete.
    pub session_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DropSessionParams {
    /// Session ID to drop from memory (without touching persistent storage).
    pub session_id: String,
}

// ============================================================================
// SF7 — Phase 11 Missing Tools (T20–T24)
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DebugGetRegistersParams {
    /// Session ID.
    pub session_id: String,
    /// Event ID at which to get register values.
    pub event_id: u64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DebugDiffParams {
    /// Session ID.
    pub session_id: String,
    /// First event ID.
    pub event_id_a: u64,
    /// Second event ID.
    pub event_id_b: u64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DebugAnalyzeMemoryParams {
    /// Session ID.
    pub session_id: String,
    /// Start address (inclusive).
    pub start_address: u64,
    /// End address (inclusive).
    pub end_address: u64,
    /// Start timestamp in nanoseconds.
    pub start_ts: u64,
    /// End timestamp in nanoseconds.
    pub end_ts: u64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ForensicMemoryAuditParams {
    /// Session ID.
    pub session_id: String,
    /// Memory address to audit.
    pub address: u64,
    /// Maximum number of writes to return.
    #[serde(default = "default_limit")]
    pub limit: usize,
}

// ============================================================================
// SF8 — Tripwire Tools (T21–T23)
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TripwireCreateParams {
    /// Type of condition to watch.
    pub condition: TripwireConditionType,
    /// Optional human-readable label for this tripwire.
    pub label: Option<String>,
    /// Optional session id to scope this tripwire to. CIH-E: explicit scope
    /// is required by the canonical-evidence observe pipeline (resolve_session
    /// precedence: scope=session{id} wins, else active_session fallback,
    /// else NoActiveSession). Tripwires are global manager state but their
    /// `fire_count` and `fired_events` reads are scoped to the session id;
    /// a session-scoped tripwire keeps its identity across the session, but
    /// its firings are only visible to observers with the same scope.
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TripwireConditionType {
    /// Watch for specific event types.
    EventType {
        /// Event type names (e.g., "function_entry", "exception").
        event_types: Vec<String>,
    },
    /// Watch for function entry/exit by name pattern (glob).
    FunctionName {
        /// Glob pattern (e.g., "process_*", "UserService.*").
        pattern: String,
    },
    /// Watch for exceptions of a specific type.
    ExceptionType {
        /// Exception type substring to match (e.g., "ValueError", "NullPointerException").
        exc_type: String,
    },
    /// Watch for execution in a memory address range.
    MemoryAddress {
        /// Start address (inclusive).
        start: u64,
        /// End address (inclusive).
        end: u64,
    },
    /// Watch for specific syscall numbers.
    SyscallNumber {
        /// Syscall numbers (e.g., [1] for write, [2] for open).
        numbers: Vec<u64>,
    },
    /// Watch for access to a specific variable name.
    VariableName {
        /// Variable name to watch (exact match).
        name: String,
    },
    /// Watch for specific signals.
    Signal {
        /// Signal numbers (e.g., [11] for SIGSEGV, [9] for SIGKILL).
        numbers: Vec<i32>,
    },
}

impl TripwireConditionType {
    pub fn into_condition(self) -> Result<TripwireCondition, String> {
        match self {
            TripwireConditionType::EventType { event_types } => {
                let mut types = Vec::with_capacity(event_types.len());
                for s in &event_types {
                    match EventType::from_snake_case(s) {
                        Some(t) => types.push(t),
                        None => return Err(s.clone()),
                    }
                }
                Ok(TripwireCondition::EventType(types))
            }
            TripwireConditionType::FunctionName { pattern } => {
                Ok(TripwireCondition::FunctionName { pattern })
            }
            TripwireConditionType::ExceptionType { exc_type } => {
                Ok(TripwireCondition::ExceptionType { exc_type })
            }
            TripwireConditionType::MemoryAddress { start, end } => {
                Ok(TripwireCondition::MemoryAddress { start, end })
            }
            TripwireConditionType::SyscallNumber { numbers } => {
                Ok(TripwireCondition::SyscallNumber { numbers })
            }
            TripwireConditionType::VariableName { name } => {
                Ok(TripwireCondition::VariableName { name })
            }
            TripwireConditionType::Signal { numbers } => Ok(TripwireCondition::Signal { numbers }),
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TripwireDeleteParams {
    /// ID of the tripwire to delete.
    pub tripwire_id: String,
    /// Optional session id to scope the delete operation. CIH-E: same
    /// precedence as `TripwireCreateParams::session_id`. Without an explicit
    /// scope, the canonical-evidence observe pipeline falls back to
    /// `active_session` and reports `NoActiveSession` if no active session
    /// exists.
    pub session_id: Option<String>,
}

/// Parameters for `tripwire_list`. CIH-E: introduced to carry the explicit
/// `session_id` scope required by the canonical-evidence observe pipeline.
/// Replaces the previous `NoParams` placeholder so that callers can pass
/// `scope=session{session_id}` and the manager's read path does not need
/// an implicit `active_session` fallback.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TripwireListParams {
    /// Optional session id to scope the list to. CIH-E.
    pub session_id: Option<String>,
}

/// Parameters for `tripwire_query`. CIH-E: same rationale as
/// `TripwireListParams`. Non-destructive read; the same precedence rules
/// apply (scope wins, else active_session fallback, else NoActiveSession).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TripwireQueryParams {
    /// Optional session id to scope the query to. CIH-E.
    pub session_id: Option<String>,
}

// ============================================================================
// SF9 — Live Probe Tools (probe_start / probe_stop / probe_drain)
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProbeAdvanceParams {
    pub session_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProbeStepParams {
    pub session_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProbeStartParams {
    /// Path to the target binary.
    pub program: String,
    /// Command-line arguments for the target.
    #[serde(default)]
    pub args: Vec<String>,
    /// Whether to trace syscalls (default: true).
    #[serde(default = "default_true")]
    pub trace_syscalls: bool,
    /// Working directory for the target.
    pub cwd: Option<String>,
    /// Whether to capture real function frames (default: false).
    ///
    /// When `true`, `NativeProbeBackend` plants INT3 at the relocated
    /// function-entry addresses of the spawned binary and emits
    /// `FunctionEntry` events (with `invocation_id`,
    /// `parent_invocation_id`, `symbol_id`) onto the session-owned
    /// `SegmentedExecutionLog` v2 through the same producer seam as
    /// syscall/registers events. Requires symbols in the binary.
    #[serde(default)]
    pub track_function_frames: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CaptureSessionParams {
    /// Path to the target binary to spawn and capture.
    pub program: String,
    /// Command-line arguments for the target.
    #[serde(default)]
    pub args: Vec<String>,
    /// Whether to trace syscalls (default: true).
    #[serde(default = "default_true")]
    pub trace_syscalls: bool,
    /// Working directory for the target.
    #[serde(default)]
    pub cwd: Option<String>,
    /// Language metadata tag used when persisting the session
    /// (same vocabulary as `save_session`). Defaults to the
    /// server-side inference from the program path.
    #[serde(default)]
    pub language: Option<String>,
}

/// Map a language string (from JSON) to a `Language` enum variant.
/// Returns `None` if the string does not match a known variant.
pub fn parse_language(s: &str) -> Option<chronos_domain::trace::Language> {
    use chronos_domain::trace::Language;
    Some(match s {
        "c" => Language::C,
        "cpp" | "c++" => Language::Cpp,
        "rust" => Language::Rust,
        "java" => Language::Java,
        "kotlin" => Language::Kotlin,
        "scala" => Language::Scala,
        "python" => Language::Python,
        "javascript" | "js" => Language::JavaScript,
        "go" => Language::Go,
        "csharp" | "c#" => Language::CSharp,
        "ebpf" => Language::Ebpf,
        "wasm" | "webassembly" => Language::WebAssembly,
        "native" => Language::Native,
        "unknown" => Language::Unknown,
        _ => return None,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProbeStartAttachParams {
    /// Process ID to attach to.
    pub pid: u32,
    /// Whether to trace syscalls (default: true).
    #[serde(default = "default_true")]
    pub trace_syscalls: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProbeStopParams {
    /// Session ID returned by probe_start.
    pub session_id: String,
}

// ============================================================================
// SF9 — v2 Session Lifecycle Tools (m7-05)
// ============================================================================

/// Parameters for the v2 `session_start` tool.
///
/// `action` discriminates between `spawn` (start a new probe + return
/// session_id), `load` (load an existing session from the store), and
/// `attach` (currently a stub returning `Unsupported`).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SessionStartParams {
    /// Action: "spawn" | "load" | "attach".
    pub action: String,
    /// Required when `action=spawn`. Mirrors the v1 `ProbeStartParams`
    /// shape 1:1 so the v1 `probe_start` shim can route through this.
    #[serde(default)]
    pub spawn_fields: Option<SessionStartSpawnParamsDto>,
    /// Required when `action=load`. Session id to load from the store.
    #[serde(default)]
    pub session_id: Option<String>,
    /// Required when `action=attach`. PID to attach to (m7+).
    #[serde(default)]
    pub pid: Option<u32>,
    /// Reserved for `action=attach` with a path (m7+).
    #[serde(default)]
    pub path: Option<String>,
}

/// Spawn-fields for `SessionStartParams`. Mirrors v1 `ProbeStartParams`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SessionStartSpawnParamsDto {
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_true")]
    pub trace_syscalls: bool,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub track_function_frames: Option<bool>,
}

/// Parameters for the v2 `session_stop` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SessionStopParams {
    pub session_id: String,
    /// Mark the session metadata as sealed (default: true).
    #[serde(default = "default_true")]
    pub seal_tail: bool,
    /// Drain observe subscriptions before stopping (default: true).
    #[serde(default = "default_true")]
    pub drain_subscriptions: bool,
}

/// Parameters for the v2 `capabilities` tool.
///
/// At least one of `target` or `session_id` must be provided.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CapabilitiesParams {
    /// Static capabilities for a target program.
    #[serde(default)]
    pub target: Option<CapabilitiesTargetDto>,
    /// Dynamic capabilities for an existing session.
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CapabilitiesTargetDto {
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub language: Option<String>,
}

// ============================================================================
// End SF9 v2 params
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProbeDrainParams {
    /// Session ID returned by probe_start.
    pub session_id: String,
    /// Maximum events to return (default: 1000).
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Offset to skip events (default: 0).
    #[serde(default)]
    pub offset: usize,
    /// Canonical `ecv1:...` cursor returned by a previous `probe_drain` call.
    /// When set, the read resumes after the last record that call EXAMINED.
    ///
    /// The legacy ring cursor (`total_pushed`/`snapshot_len`) is not accepted:
    /// it lives in a different coordinate space and is never converted into an
    /// EventSeq.
    #[serde(default)]
    pub evidence_cursor: Option<String>,
}

/// m1-03: parameters for `probe_drain_log`. Cursor is a seq number
/// rather than the EventBus cursor DTO.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProbeDrainLogParams {
    pub session_id: String,
    /// Optional seq used as a strict lower bound. Records with
    /// seq strictly greater than `since` are returned.
    #[serde(default)]
    pub since: Option<u64>,
    #[serde(default = "default_log_limit")]
    pub limit: usize,
}

fn default_log_limit() -> usize {
    256
}

/// m1-07: parameters for `probe_compaction_metrics`. Just a
/// session_id — the response is a snapshot of three counters.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProbeCompactionMetricsParams {
    pub session_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SessionSnapshotParams {
    /// Session ID of a live probe (returned by probe_start).
    pub session_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProbeInjectParams {
    /// Session ID of an existing live probe session.
    pub session_id: String,
    /// Binary/library path to attach uprobe to (e.g., "/usr/lib/libfoo.so").
    pub binary_path: String,
    /// Function symbol name to attach uprobe to (e.g., "malloc", "handle_request").
    pub symbol_name: String,
    /// Optional: PID to attach to (if not the session's target).
    pub pid: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProbeStatusParams {
    /// Session ID of an existing live probe session.
    pub session_id: String,
}

// ============================================================================
// SF10 — Browser/WASM Probe Tools (T15–T16)
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BrowserProbeStartParams {
    /// URL to navigate to.
    pub url: String,
    /// Whether to run Chrome headless (default: true).
    #[serde(default = "default_true")]
    pub headless: bool,
    /// Path to Chrome binary (auto-detected if omitted).
    pub chrome_path: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BrowserProbeStopParams {
    /// Session ID returned by browser_probe_start.
    pub session_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BrowserProbeDrainParams {
    /// Session ID returned by browser_probe_start.
    pub session_id: String,
    /// Maximum events to return (default: 1000).
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Offset to skip events (default: 0).
    #[serde(default)]
    pub offset: usize,
}

// ============================================================================
// Super-tool: Maven-like Phase Orchestration
// ============================================================================

// ============================================================================
// Compare Sessions (Divergence Engine)
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CompareSessionsParams {
    /// First session ID.
    pub session_a: String,
    /// Second session ID.
    pub session_b: String,
}

// ============================================================================
// Performance Regression Audit
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PerformanceRegressionAuditParams {
    /// Baseline session ID.
    pub baseline_session_id: String,
    /// Target session ID to compare against baseline.
    pub target_session_id: String,
    /// Maximum number of top functions to compare (default: 20).
    pub top_n: Option<usize>,
}

// ============================================================================
// Session Compare (v2, m7-03 dispatcher)
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SessionCompareParams {
    /// Session comparison kind.
    /// - `divergence`: pairwise divergence of session_a vs session_b (functions only in A / only in B / common).
    /// - `regression`: top-function performance regression audit (baseline vs target).
    pub kind: String,
    /// First session ID (divergence: session_a; regression: baseline_session_id).
    pub session_a: String,
    /// Second session ID (divergence: session_b; regression: target_session_id).
    pub session_b: String,
    /// Maximum number of top functions to compare (regression only, default: 20).
    pub top_n: Option<usize>,
}

/// Response wire shape for the session-comparison family.
///
/// m9-74 (`FIND-M9-74-V1-SHIMS-RETURN-V2-ENVELOPE`): m7-03 rerouted the two
/// deprecated v1 tools through the v2 dispatcher and, with them, changed what
/// they return — from the flat v1 result to the tagged `SessionCompareOutput`
/// envelope. The tools kept their v1 names and their "v1 parameter names are
/// preserved" descriptions, so a v1 client (the sandbox harness is one) started
/// failing to parse a response that was still advertised as v1. `session_compare`
/// gets the envelope; the shims get back the flat result the v1 DTOs
/// (`CompareSessionsResult`, `PerformanceRegressionAuditResult`) were kept for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionCompareWire {
    /// Tagged `SessionCompareOutput` envelope (`session_compare`, v2).
    V2Envelope,
    /// Flat v1 result (`compare_sessions`, `performance_regression_audit`).
    V1Flat,
}

impl SessionCompareWire {
    /// Serialize a dispatcher result in this wire shape.
    pub fn to_value(
        self,
        out: chronos_services::output::SessionCompareOutput,
    ) -> Result<serde_json::Value, serde_json::Error> {
        use chronos_services::output::SessionCompareOutput;
        match self {
            SessionCompareWire::V2Envelope => serde_json::to_value(out),
            SessionCompareWire::V1Flat => match out {
                SessionCompareOutput::Divergence { result, .. } => serde_json::to_value(result),
                SessionCompareOutput::Regression { result, .. } => serde_json::to_value(result),
            },
        }
    }
}

// ============================================================================
// Session Explain (v2, m7-03 net-new)
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SessionExplainParams {
    /// Session ID to explain.
    pub session_id: String,
    /// Explanation kind:
    /// - `facts`: raw counts and metadata (no analysis).
    /// - `derived`: function hotspots, call-graph summary, syscall breakdown.
    /// - `inferred`: heuristic characterisations (IoHeavy / CpuBound / SingleThreaded / CrashDetected / Unknown).
    /// - `hypothesis`: typed hypothesis test plans (CrashInvariant / DominantFunctionCallPath).
    pub kind: String,
}

// ============================================================================
// SF6 — Inspection Tools (T4–T7)
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct EvaluateExpressionParams {
    /// Session ID.
    pub session_id: String,
    /// Event ID at which to evaluate the expression.
    pub event_id: u64,
    /// Arithmetic expression to evaluate (e.g., "x + y * 2").
    pub expression: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DebugGetVariablesParams {
    /// Session ID.
    pub session_id: String,
    /// Event ID at which to get variables.
    pub event_id: u64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DebugGetMemoryParams {
    /// Session ID.
    pub session_id: String,
    /// Memory address to read.
    pub address: u64,
    /// Timestamp in nanoseconds (will return most recent write at or before this time).
    pub timestamp_ns: u64,
}

/// The session store this process was configured to open could not be opened.
///
/// REC-C3.3.2: re-exported from `crate::composition::StoreOpenError` so
/// callers that imported `crate::server::StoreOpenError` keep working.
pub use crate::composition::StoreOpenError;

// ============================================================================
// Counterexample tool params (m8-03 MCP wrappers)
// ============================================================================

/// Params for `counterexample_shrink` (m8-03). Mirrors
/// `chronos_services::output::CounterexampleShrinkParamsDto` 1:1 but
/// re-declared here so the rmcp `Parameters<T>` derive can drive the
/// JSON-schema-driven argument generation without exposing the
/// services-internal HypothesisInput type on the wire verbatim.
#[derive(Debug, serde::Deserialize, JsonSchema)]
pub struct CounterexampleShrinkParams {
    pub property_kind: chronos_services::output::HypothesisKind,
    pub target_hypothesis: chronos_services::output::HypothesisInputWireDto,
    pub max_rounds: Option<u32>,
    pub seed: Option<u64>,
}

/// Params for `counterexample_get` (m8-03).
#[derive(Debug, serde::Deserialize, JsonSchema)]
pub struct CounterexampleGetParams {
    pub bundle_id: String,
}

/// Params for `counterexample_list` (m8-03; m8-05 added cursor).
#[derive(Debug, serde::Deserialize, JsonSchema)]
pub struct CounterexampleListParams {
    pub workspace_id: Option<String>,
    pub property_kind: Option<chronos_services::output::HypothesisKind>,
    pub since_ms: Option<u64>,
    pub until_ms: Option<u64>,
    pub limit: Option<u32>,
    /// m8-05 (B2): opaque pagination cursor. Pass the `next_cursor`
    /// value returned by the previous page's response. `None` (or
    /// omitted) means first page.
    pub cursor: Option<String>,
}

/// Params for `counterexample_events_count` (m8-05 B3).
#[derive(Debug, serde::Deserialize, JsonSchema)]
pub struct CounterexampleEventsCountParams {
    pub bundle_id: String,
}

/// Params for `counterexample_bundle_events` (m9-91 — closes m9-02-R4).
///
/// Reads the events stream of a counterexample bundle via the m9-02
/// v3 side-table (`crate::counterexample_storage::bundle_events_or_legacy`).
/// Supports full-stream reads (omit `limit`) and offset/limit pagination.
///
/// - `bundle_id`: target bundle. Missing bundle → tool-level error.
/// - `limit`: optional max events to return. `None` returns the entire
///   remaining stream starting at `offset`.
/// - `offset`: zero-based start index. `None` → start at 0.
#[derive(Debug, serde::Deserialize, JsonSchema)]
pub struct CounterexampleBundleEventsParams {
    pub bundle_id: String,
    /// Maximum events to return in this page. None means "all remaining".
    pub limit: Option<usize>,
    /// Zero-based starting index into the events stream. None means 0.
    pub offset: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every tool a profile lists must actually be registered by the router.
    ///
    /// C5.3.2 deleted 22 deprecated alias handlers from `server.rs`, but five of
    /// them (`probe_inject`, `state_diff`, `evaluate_expression`,
    /// `debug_detect_races`, `inspect_causality`) survived in the profile lists:
    /// 26 stale entries across six profiles. The alias-deletion test could not
    /// catch it because it only greps the router, and these lists live here.
    ///
    /// The runtime impact of the drift is nil — `build_tool_availability`
    /// iterates `ALL_TOOL_NAMES`, and `toolset_guard` is only reached from
    /// handlers these names no longer have. The cost is that the profile lists
    /// are the executable specification of what each toolset contains, and they
    /// were describing a toolset that does not exist. This test keeps them
    /// honest.
    #[test]
    fn profile_lists_only_name_registered_tools() {
        let registered: std::collections::HashSet<&str> = ALL_TOOL_NAMES.iter().copied().collect();

        for (profile, list) in [
            ("native", NATIVE_TOOL_NAMES),
            ("ebpf", EBPF_TOOL_NAMES),
            ("python", PYTHON_TOOL_NAMES),
            ("java", JAVA_TOOL_NAMES),
            ("go", GO_TOOL_NAMES),
            ("js", JS_TOOL_NAMES),
            ("minimal", MINIMAL_TOOL_NAMES),
        ] {
            let stale: Vec<&&str> = list
                .iter()
                .filter(|name| !registered.contains(**name))
                .collect();
            assert!(
                stale.is_empty(),
                "toolset profile '{profile}' lists tools the router does not \
                 register: {stale:?}. Remove them, or register the tool if it is \
                 meant to exist."
            );
        }
    }
}
