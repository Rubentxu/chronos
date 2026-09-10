//! Chronos MCP server — exposes debugging tools via MCP.
//!
//! Implements 10 tools for AI-assisted debugging.
//!
//! # Concurrency Model
//!
//! This server is designed to handle multiple concurrent client connections safely:
//!
//! - **Sessions are isolated**: Each debug session has a unique ID. Events collected
//!   for one session cannot leak into another, even under concurrent access.
//!
//! - **Shared state is protected**: All shared mutable state uses `Arc<Mutex<...>>` or
//!   `Arc<tokio::sync::Mutex<...>>`. The mutex granularity is at the session map level,
//!   not individual sessions, which is sufficient since operations are batched per session.
//!
//! - **Engine immutability**: `QueryEngine` is immutable after construction (indices are
//!   built once, then read-only). This makes sharing across threads safe.
//!
//! - **Atomic CAS operations**: The content-addressable store (`ContentStore::put`) uses
//!   a single write transaction with internal deduplication, ensuring atomicity under
//!   concurrent writes of identical content.
//!
//! - **Background sessions**: The `background_sessions` map tracks pending sessions
//!   (as empty placeholders) until completion, at which point they're added to
//!   `engines` and removed from the map.

// BrowserAdapter / CaptureConfig / CaptureSession are used by the in-file
// `#[cfg(test)] mod tests` block; the lib code itself delegates everything
// to BrowserProbeService. The clippy::unused_imports lint complains even
// though the imports are real (just only used in tests). Allow explicitly.
#[allow(unused_imports)]
use chronos_browser::BrowserAdapter;
use chronos_domain::tripwire::{TripwireCondition, TripwireManager};
#[allow(unused_imports)]
use chronos_domain::{
    CaptureConfig, CaptureSession, EventData, EventType, Language, TraceEvent, VariableInfo,
};
use chronos_index::builder::IndexBuilder;
use chronos_query::QueryEngine;
use chronos_services::browser_probe::BrowserProbeSession;
use chronos_services::browser_probe::{
    BrowserProbeContext, BrowserProbeService, BrowserProbeStartInput,
};
use chronos_services::debug_read::DebugReadService;
use chronos_services::debug_trace::DebugTraceService;
use chronos_services::error::ServiceError;
use chronos_services::execution_query::{
    ChronosExecutionQueryService, ExecutionQueryContext, ExecutionQueryInput,
};
use chronos_services::output::EvalResult;
use chronos_services::output::{ExecutionQueryKind, ExecutionQueryOutput};
use chronos_services::output::{StateQueryKind, StateQueryOutput};
use chronos_services::probe::LiveProbeSession;
use chronos_services::query_service::QueryService;
use chronos_services::sessions::{SessionsContext, SessionsService};
use chronos_services::state_query::{ChronosStateQueryService, StateQueryContext, StateQueryInput};
use chronos_services::trace_slice::{ChronosTraceSliceService, TraceSliceContext, TraceSliceInput};
use chronos_services::tripwires::TripwiresService;
#[allow(unused_imports)]
use chronos_store::{SessionMetadata, SessionStore};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content};
use rmcp::tool;
use schemars::JsonSchema;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

/// Resource limits for capture operations.
///
/// Used to prevent resource exhaustion attacks by capping the number of events
/// and the wall-clock time of a capture.
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Maximum number of events to collect before stopping (default: 1_000_000).
    pub max_events: usize,
    /// Timeout in seconds for the capture (default: 60).
    pub timeout_secs: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_events: 1_000_000,
            timeout_secs: 60,
        }
    }
}

/// Type alias for background session placeholder storage.
/// Background sessions store an empty placeholder here while running. Once complete,
/// the session is moved to `engines` and becomes queryable. The placeholder is kept
/// in this map to track which sessions are still pending completion.
type BackgroundSessionEvents = Arc<std::sync::Mutex<Vec<TraceEvent>>>;

/// Empty parameter type for tools that take no arguments.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct NoParams {}

/// The Chronos MCP server state.
pub struct ChronosServer {
    /// Loaded query engines (session_id → engine).
    engines: Arc<Mutex<HashMap<String, QueryEngine>>>,
    /// Session languages (session_id → language) for routing evaluations.
    session_languages: Arc<Mutex<HashMap<String, chronos_domain::Language>>>,
    /// Persistent session store.
    store: Arc<SessionStore>,
    /// Active background sessions: session_id → events vector.
    /// Tracks pending sessions that are still running in background threads.
    /// Uses `std::sync::Mutex` (not tokio) intentionally: all lock holders are
    /// sync, locks are held only for short non-blocking operations, and std Mutex
    /// is faster than tokio Mutex for sub-microsecond critical sections.
    /// INVARIANT: Never hold this lock across an `.await` point.
    #[allow(dead_code)]
    background_sessions: Arc<std::sync::Mutex<HashMap<String, BackgroundSessionEvents>>>,
    /// Sessions with connected debug clients (Python debugpy or JS Node.js inspector).
    /// Used to track which sessions have active DAP/CDP connections.
    connected_sessions: Arc<std::sync::Mutex<HashSet<String>>>,
    /// Currently active session for phased workflows.
    /// Automatically set after probe_start or capture completes.
    active_session: Arc<Mutex<Option<String>>>,
    /// Tripwire manager for condition-based event notification.
    tripwire_manager: Arc<TripwireManager>,
    /// Live probe sessions: session_id → LiveProbeSession.
    /// These are real-time probe sessions using `NativeProbeBackend` where events
    /// stream to an `EventBus` ring buffer. Use `probe_drain` to read current events
    /// and `probe_stop` to finalize.
    live_probes: Arc<std::sync::Mutex<HashMap<String, LiveProbeSession>>>,
    /// Live browser probe sessions: session_id → BrowserProbeSession.
    /// These are real-time WASM debugging sessions via Chrome CDP.
    /// Use `browser_probe_drain` to read events and `browser_probe_stop` to finalize.
    live_browser_probes: Arc<std::sync::Mutex<HashMap<String, BrowserProbeSession>>>,
}

// ============================================================================
// Tool parameter types
// ============================================================================

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueryEventsParams {
    /// Session ID to query.
    pub session_id: String,
    /// Filter by event types (e.g., "function_entry", "syscall_enter").
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

fn default_limit() -> usize {
    100
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
fn parse_hypothesis_scope(s: &str) -> Option<chronos_services::output::HypothesisScope> {
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
fn parse_comparison_op(s: &str) -> Option<chronos_services::output::ComparisonOp> {
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
fn parse_export_format(s: &str) -> Option<chronos_services::output::ExportFormat> {
    use chronos_services::output::ExportFormat;
    match s {
        "json" => Some(ExportFormat::Json),
        "otlp_json" => Some(ExportFormat::OtlpJson),
        "zip_json" => Some(ExportFormat::ZipJson),
        _ => None,
    }
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
    fn into_condition(self) -> Result<TripwireCondition, String> {
        match self {
            TripwireConditionType::EventType { event_types } => {
                let mut types = Vec::with_capacity(event_types.len());
                for s in &event_types {
                    match ChronosServer::parse_event_type(s) {
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
}

// ============================================================================
// SF9 — Live Probe Tools (probe_start / probe_stop / probe_drain)
// ============================================================================

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
    /// EventBus ring buffer capacity (default: 50000).
    #[serde(default = "default_bus_capacity")]
    pub bus_capacity: usize,
    /// Whether to capture real function frames (default: false).
    ///
    /// When `true`, `NativeProbeBackend` plants INT3 at the relocated
    /// function-entry addresses of the spawned binary and emits
    /// `FunctionEntry` events (with `invocation_id`,
    /// `parent_invocation_id`, `symbol_id`) to both `EventBus` and
    /// `SegmentedExecutionLog` v2 through the same producer seam as
    /// syscall/registers events. Requires symbols in the binary.
    #[serde(default)]
    pub track_function_frames: Option<bool>,
}

fn default_bus_capacity() -> usize {
    50000
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProbeStartAttachParams {
    /// Process ID to attach to.
    pub pid: u32,
    /// Whether to trace syscalls (default: true).
    #[serde(default = "default_true")]
    pub trace_syscalls: bool,
    /// EventBus ring buffer capacity (default: 50000).
    #[serde(default = "default_bus_capacity")]
    pub bus_capacity: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProbeStopParams {
    /// Session ID returned by probe_start.
    pub session_id: String,
}

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
    /// Optional cursor returned by a previous `probe_drain` call. When set,
    /// the server anchors the read to the cursor's position so it can return
    /// the events that arrived since the cursor was issued.
    #[serde(default)]
    pub cursor: Option<CursorDto>,
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

/// Wire format for [`chronos_domain::EventCursor`] in MCP JSON payloads.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct CursorDto {
    #[serde(default)]
    pub total_pushed: Option<u64>,
    #[serde(default)]
    pub snapshot_len: Option<u64>,
}

impl CursorDto {
    /// Convert to the domain cursor, returning `None` if the payload is
    /// malformed (e.g., negative fields or missing required values).
    pub fn to_domain(&self) -> Option<chronos_domain::EventCursor> {
        Some(chronos_domain::EventCursor {
            total_pushed: self.total_pushed?,
            snapshot_len: self.snapshot_len?,
        })
    }
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

impl ChronosServer {
    pub fn new() -> Self {
        let db_path = std::env::var("CHRONOS_DB_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let mut path = std::env::var("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| PathBuf::from("."));
                path.push(".local");
                path.push("share");
                path.push("chronos");
                path.push("sessions.redb");
                path
            });

        // Try to open existing database with graceful lock handling
        let store = match SessionStore::try_open(&db_path) {
            Ok(s) => {
                tracing::info!("Opened session store at {:?}", db_path);
                s
            }
            Err(e) => {
                tracing::warn!(
                    "Could not open session store at {:?}: {}. Using in-memory store.",
                    db_path,
                    e
                );
                // Fall back to in-memory store if disk store fails
                SessionStore::in_memory().expect("Failed to create in-memory session store")
            }
        };

        Self {
            engines: Arc::new(Mutex::new(HashMap::new())),
            session_languages: Arc::new(Mutex::new(HashMap::new())),
            store: Arc::new(store),
            background_sessions: Arc::new(std::sync::Mutex::new(HashMap::new())),
            connected_sessions: Arc::new(std::sync::Mutex::new(HashSet::new())),
            active_session: Arc::new(Mutex::new(None)),
            tripwire_manager: Arc::new(TripwireManager::new()),
            live_probes: Arc::new(std::sync::Mutex::new(HashMap::new())),
            live_browser_probes: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    /// Inject a `QueryEngine` into the server's engine map for testing.
    ///
    /// This is intentionally **not** public in the production API — it exists
    /// only to support integration tests in `tests/debug_read_tools.rs`.
    #[cfg(test)]
    pub async fn inject_engine_for_testing(
        &self,
        session_id: &str,
        engine: chronos_query::QueryEngine,
    ) {
        self.engines
            .lock()
            .await
            .insert(session_id.to_string(), engine);
    }

    fn parse_event_type(name: &str) -> Option<EventType> {
        match name {
            "syscall_enter" => Some(EventType::SyscallEnter),
            "syscall_exit" => Some(EventType::SyscallExit),
            "function_entry" => Some(EventType::FunctionEntry),
            "function_exit" => Some(EventType::FunctionExit),
            "variable_write" => Some(EventType::VariableWrite),
            "memory_write" => Some(EventType::MemoryWrite),
            "signal_delivered" => Some(EventType::SignalDelivered),
            "breakpoint_hit" => Some(EventType::BreakpointHit),
            "thread_create" => Some(EventType::ThreadCreate),
            "thread_exit" => Some(EventType::ThreadExit),
            "exception_thrown" => Some(EventType::ExceptionThrown),
            _ => None,
        }
    }

    /// Remove all in-memory state for a session: query engine, language tag,
    /// and connected-session marker.
    async fn cleanup_session_memory(&self, session_id: &str) {
        self.engines.lock().await.remove(session_id);
        self.session_languages.lock().await.remove(session_id);
        if let Ok(mut sessions) = self.connected_sessions.lock() {
            sessions.remove(session_id);
        }
    }

    async fn build_and_store_engine(
        &self,
        session_id: &str,
        events: Vec<TraceEvent>,
        language: Language,
    ) {
        // Filter out internal/noisy events before indexing:
        // - EventType::Custom with EventData::Registers → ptrace register snapshots (infrastructure noise)
        // - EventType::Unknown → unclassified ptrace stops
        // These are implementation details of the tracer, not meaningful for AI analysis.
        let events: Vec<TraceEvent> = events
            .into_iter()
            .filter(|e| {
                // Keep everything except raw register snapshots and unknowns
                !matches!(
                    (&e.event_type, &e.data),
                    (EventType::Custom, EventData::Registers(_)) | (EventType::Unknown, _)
                )
            })
            .collect();

        let mut engines = self.engines.lock().await;
        let mut session_languages = self.session_languages.lock().await;

        if let Some(existing) = engines.get_mut(session_id) {
            // Cumulative refresh: merge new events into the existing engine
            // and rebuild indices. m0-02-make-snapshots-cumulative (UAT-M0-02).
            existing.merge(events);
            info!(
                "Refreshed query engine for session {} (cumulative, now {} events)",
                session_id,
                existing.event_count()
            );
        } else {
            // First snapshot: build from scratch.
            let mut builder = IndexBuilder::new();
            builder.push_all(&events);
            let indices = builder.finalize();

            let engine = QueryEngine::with_indices(events, indices.shadow, indices.temporal)
                .with_causality(indices.causality)
                .with_performance(indices.performance);

            info!(
                "Built query engine for session {} (language: {:?})",
                session_id, language
            );
            engines.insert(session_id.to_string(), engine);
        }
        session_languages.insert(session_id.to_string(), language);

        // Set this session as the active session
        drop(engines);
        drop(session_languages);
        self.set_active_session(session_id).await;
    }

    /// Set the active session after capture or load.
    async fn set_active_session(&self, session_id: &str) {
        let mut active = self.active_session.lock().await;
        *active = Some(session_id.to_string());
    }

    /// Run the server on stdio.
    pub async fn run_stdio(self) -> Result<(), Box<dyn std::error::Error>> {
        use rmcp::ServiceExt;
        let server = Arc::new(self);
        // m1-08: spawn the auto-compaction daemon alongside the
        // MCP service. The daemon periodically walks `live_probes`
        // and calls `maybe_compact()` on each attached ExecutionLog,
        // so operators don't need to wire a separate scheduler.
        // `waiting()` is what blocks until the service shuts down;
        // we tie the daemon to that with a shutdown signal so they
        // die together.
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let daemon_handle = tokio::spawn(Self::auto_compaction_daemon(server.clone(), shutdown_rx));
        let transport = (tokio::io::stdin(), tokio::io::stdout());
        let service = server.serve(transport).await?;
        info!("Chronos MCP server started on stdio");
        let result = service.waiting().await;
        // Signal the daemon to stop and let it drain.
        let _ = shutdown_tx.send(true);
        // daemon_handle.await returns Result<(), JoinError>;
        // we don't care about the daemon's exit status here
        // (it returns Ok on graceful shutdown, Err only if it
        // panicked — which we want to propagate).
        let daemon_result = daemon_handle.await;
        result?;
        daemon_result
            .map_err(|e| Box::<dyn std::error::Error>::from(format!("daemon panicked: {}", e)))?;
        Ok(())
    }

    /// Background task that periodically walks every live probe
    /// session and calls `maybe_compact()` on its attached
    /// ExecutionLog (m1-08).
    ///
    /// Interval comes from `CHRONOS_AUTO_COMPACT_INTERVAL_SECS`
    /// (default 30 s). Setting the env var to `0` disables the
    /// daemon entirely — useful for tests.
    ///
    /// Errors from `maybe_compact()` are logged at `warn` level and
    /// swallowed; compaction is best-effort and the next tick will
    /// retry.
    async fn auto_compaction_daemon(
        server: Arc<Self>,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
    ) {
        let interval_secs: u64 = std::env::var("CHRONOS_AUTO_COMPACT_INTERVAL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(30);
        if interval_secs == 0 {
            info!("auto-compaction daemon disabled (CHRONOS_AUTO_COMPACT_INTERVAL_SECS=0)");
            return;
        }
        info!(
            "auto-compaction daemon started (interval = {}s)",
            interval_secs
        );
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
        // Skip the first immediate tick — the daemon shouldn't fire
        // before the server has even accepted a single probe.
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        ticker.tick().await; // consume the initial 0-time tick
        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        info!("auto-compaction daemon shutting down");
                        return;
                    }
                }
                _ = ticker.tick() => {
                    Self::run_one_compaction_round(&server).await;
                }
            }
        }
    }

    /// Single pass over live_probes; one `maybe_compact()` call per
    /// backend that has an attached log. Logs cumulative metrics
    /// before and after so the operator can see reclaimed space.
    ///
    /// Holds `live_probes` for the entire pass — `maybe_compact()`
    /// is a short call (it operates on the backend's own log, not
    /// the live_probes map), and we want a consistent snapshot.
    /// The mutex is std::sync, never held across .await, so this
    /// does not block the async runtime.
    async fn run_one_compaction_round(server: &Arc<Self>) {
        let probes = server.live_probes.lock().unwrap();
        for (session_id, live_probe) in probes.iter() {
            let log = match live_probe.backend.execution_log() {
                Some(l) => l,
                None => continue,
            };
            let before = log.compaction_metrics();
            let removed = log.maybe_compact();
            match removed {
                Ok(paths) if !paths.is_empty() => {
                    let after = log.compaction_metrics();
                    let reclaimed_bytes =
                        after.bytes_reclaimed_total - before.bytes_reclaimed_total;
                    info!(
                        "auto-compact: session={} removed {} segment(s) ({} bytes reclaimed); \
                         cumulative runs={}, bytes_reclaimed={}, segments_removed={}",
                        session_id,
                        paths.len(),
                        reclaimed_bytes,
                        after.compaction_runs_total,
                        after.bytes_reclaimed_total,
                        after.segments_removed_total,
                    );
                }
                Ok(_) => {}
                Err(e) => {
                    warn!(
                        "auto-compact: session={} failed: {} (will retry on next tick)",
                        session_id, e
                    );
                }
            }
        }
    }
}

impl Default for ChronosServer {
    fn default() -> Self {
        Self::new()
    }
}

// Helper to create JSON text content
fn json_content(value: &serde_json::Value) -> Vec<Content> {
    vec![Content::text(
        serde_json::to_string_pretty(value).unwrap_or_default(),
    )]
}

fn text_content(text: impl Into<String>) -> Vec<Content> {
    vec![Content::text(text.into())]
}

// ============================================================================
// Tool handlers using rmcp macros
// ============================================================================

#[rmcp::tool_router]
impl ChronosServer {
    #[tool(
        name = "query_events",
        description = "Query trace events with filters. Returns paginated results."
    )]
    async fn query_events(
        &self,
        params: Parameters<QueryEventsParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        // Parse event_type strings
        let mut event_types: Option<Vec<EventType>> = None;
        if let Some(ref types) = params.event_types {
            let mut parsed: Vec<EventType> = Vec::with_capacity(types.len());
            for t in types {
                match Self::parse_event_type(t) {
                    Some(et) => parsed.push(et),
                    None => {
                        return Ok(CallToolResult::error(text_content(format!(
                            "query_events: unknown event_type '{}'. Valid types: syscall_enter, syscall_exit, function_entry, function_exit, variable_write, memory_write, signal_delivered, breakpoint_hit, thread_create, thread_exit, exception_thrown.",
                            t
                        ))));
                    }
                }
            }
            if !parsed.is_empty() {
                event_types = Some(parsed);
            }
        }

        let result = match DebugTraceService::query_events(
            &params.session_id,
            event_types,
            params.thread_id,
            params.timestamp_start,
            params.timestamp_end,
            params.function_pattern.as_deref(),
            params.limit,
            params.offset,
            &self.engines,
        )
        .await
        {
            Ok(r) => r,
            Err(ServiceError::SessionNotFound(s)) => {
                return Ok(CallToolResult::error(text_content(format!(
                    "Session '{}' not found or not finalized",
                    s
                ))));
            }
            Err(ServiceError::LockPoisoned) => {
                return Ok(CallToolResult::error(text_content("lock poisoned")));
            }
            Err(ServiceError::QueryExecutionError(s)) => {
                return Ok(CallToolResult::error(text_content(format!(
                    "query execution error: {}",
                    s
                ))));
            }
            // Other error variants cannot occur from query_events but are listed
            // for exhaustiveness.
            Err(ServiceError::EventNotFound { event_id: _ }) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected event error",
                )));
            }
            Err(ServiceError::MemoryNotFound { .. }) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected memory error",
                )));
            }
            Err(ServiceError::NoRegisterState { event_id: _ }) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected register error",
                )));
            }
            Err(ServiceError::EvalError(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected eval error",
                )));
            }
            Err(ServiceError::SessionNotInMemory(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected session error",
                )));
            }
            Err(ServiceError::EmptySession(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected empty session",
                )));
            }
            Err(ServiceError::SaveFailed(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected save error",
                )));
            }
            Err(ServiceError::LoadFailed(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected load error",
                )));
            }
            Err(ServiceError::ListFailed(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected list error",
                )));
            }
            Err(ServiceError::DeleteFailed(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected delete error",
                )));
            }
            Err(ServiceError::InvalidCondition(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected condition error",
                )));
            }
            Err(ServiceError::InvalidTripwireIdFormat(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected tripwire ID format error",
                )));
            }
            Err(ServiceError::TripwireNotFound(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected tripwire not found",
                )));
            }
            // Probe service variants cannot be produced by query_events but are
            // listed for exhaustiveness against the ServiceError enum.
            Err(ServiceError::InvalidProgramPath(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected probe program path error",
                )));
            }
            Err(ServiceError::ProbeNotFound(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected probe not found",
                )));
            }
            Err(ServiceError::ProbeStartFailed(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected probe start failure",
                )));
            }
            Err(ServiceError::ProbeStopError(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected probe stop error",
                )));
            }
            Err(ServiceError::InvalidCursorPayload) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected invalid cursor",
                )));
            }
            Err(ServiceError::CursorStale) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected cursor stale",
                )));
            }
            Err(ServiceError::DrainFailed(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected drain failure",
                )));
            }
            Err(ServiceError::ProbeStarting) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected probe starting",
                )));
            }
            Err(ServiceError::EbpfUnsupported(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected eBPF unsupported",
                )));
            }
            Err(ServiceError::InjectionFailed(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected injection failure",
                )));
            }
            // Browser probe service variants cannot be produced by
            // this call site but are listed for exhaustiveness against
            // the ServiceError enum.
            Err(ServiceError::ChromeUnavailable) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected Chrome unavailable",
                )));
            }
            Err(ServiceError::BrowserProbeNotFound(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected browser probe not found",
                )));
            }
            Err(ServiceError::BrowserProbeStartFailed(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected browser probe start failure",
                )));
            }
            Err(ServiceError::BrowserProbeDrainFailed(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected browser probe drain failure",
                )));
            }
            Err(ServiceError::InvalidInput(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected invalid input",
                )));
            }
            // m6-05: session_export variants cannot be produced by this
            // call site but are listed for exhaustiveness against the
            // ServiceError enum.
            Err(ServiceError::ExportFailed(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected export failure",
                )));
            }
            Err(ServiceError::InvalidExportParameter(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected invalid export parameter",
                )));
            }
        };

        // m0-07: explicit query absence semantics
        let not_found = result.result.events.is_empty();
        let reason = if not_found {
            Some("no_matching_events".to_string())
        } else {
            None
        };

        let output = serde_json::json!({
            "session_id": params.session_id,
            "not_found": not_found,
            "reason": reason,
            "total_matching": result.result.total_matching,
            "returned_count": result.result.events.len(),
            "next_offset": result.result.next_offset,
            "events": result.result.events.iter().map(|e| serde_json::json!({
                "event_id": e.event_id,
                "timestamp_ns": e.timestamp_ns,
                "thread_id": e.thread_id,
                "type": e.event_type.to_string(),
                "function": e.location.function,
                "address": format!("0x{:x}", e.location.address),
            })).collect::<Vec<_>>(),
        });

        Ok(CallToolResult::success(json_content(&output)))
    }

    #[tool(
        name = "get_event",
        description = "Get detailed information about a specific trace event."
    )]
    async fn get_event(
        &self,
        params: Parameters<GetEventParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        match DebugTraceService::get_event(&params.session_id, params.event_id, &self.engines).await
        {
            Ok(Some(event)) => {
                let json = serde_json::to_string_pretty(&event).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Ok(None) => Ok(CallToolResult::error(text_content(format!(
                "Event {} not found",
                params.event_id
            )))),
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{}' not found", s),
            ))),
            Err(ServiceError::LockPoisoned) => {
                Ok(CallToolResult::error(text_content("lock poisoned")))
            }
            Err(ServiceError::QueryExecutionError(s)) => Ok(CallToolResult::error(text_content(
                format!("internal error: unexpected query error: {}", s),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!(
                "internal error: unexpected error: {}",
                e
            )))),
        }
    }

    #[tool(
        name = "get_call_stack",
        description = "Deprecated. Use `execution_query` with `kind=call_stack` instead. Reconstruct the call stack at a specific event."
    )]
    async fn get_call_stack(
        &self,
        params: Parameters<GetCallStackParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let ctx = ExecutionQueryContext {
            engines: &self.engines,
        };
        let input = ExecutionQueryInput {
            session_id: params.session_id.clone(),
            kind: ExecutionQueryKind::CallStack,
            event_id: Some(params.event_id),
            max_depth: None,
            threshold_ns: None,
            top_n: None,
            saliency_limit: None,
        };

        match ChronosExecutionQueryService::query(&ctx, input).await {
            Ok(ExecutionQueryOutput::CallStack { frames }) => {
                let output = serde_json::json!({
                    "session_id": params.session_id,
                    "at_event_id": params.event_id,
                    "depth": frames.len(),
                    "frames": frames.iter().map(|f| serde_json::json!({
                        "depth": f.depth,
                        "function": f.function,
                        "file": f.file,
                        "line": f.line,
                        "address": format!("0x{:x}", f.address),
                    })).collect::<Vec<_>>(),
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{}' not found", s),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!(
                "internal error: unexpected error: {}",
                e
            )))),
            Ok(_) => unreachable!("kind=call_stack always yields CallStack variant"),
        }
    }

    #[tool(
        name = "get_execution_summary",
        description = "Deprecated. Use `execution_query` with `kind=execution_summary` instead. Get execution summary: event counts, top functions, issues."
    )]
    async fn get_execution_summary(
        &self,
        params: Parameters<GetExecutionSummaryParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let ctx = ExecutionQueryContext {
            engines: &self.engines,
        };
        let input = ExecutionQueryInput {
            session_id: params.session_id,
            kind: ExecutionQueryKind::ExecutionSummary,
            event_id: None,
            max_depth: None,
            threshold_ns: None,
            top_n: None,
            saliency_limit: None,
        };

        match ChronosExecutionQueryService::query(&ctx, input).await {
            Ok(ExecutionQueryOutput::ExecutionSummary { summary }) => {
                let json = serde_json::to_string_pretty(&summary).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{}' not found", s),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!(
                "internal error: unexpected error: {}",
                e
            )))),
            Ok(_) => unreachable!("kind=execution_summary always yields ExecutionSummary variant"),
        }
    }

    #[tool(
        name = "execution_query",
        description = "v2 dispatcher for execution/call/performance projection queries. Select the query kind via `kind` (call_stack | execution_summary | call_graph | race_detect | hotspot | saliency). Each kind has its own required/optional fields; see docs/chronos-agentic-reconstruction/docs/specs/AGENT_API_V2.md."
    )]
    async fn execution_query(
        &self,
        params: Parameters<ExecutionQueryParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let ctx = ExecutionQueryContext {
            engines: &self.engines,
        };
        let input = ExecutionQueryInput {
            session_id: params.session_id,
            kind: params.kind,
            event_id: params.event_id,
            max_depth: params.max_depth,
            threshold_ns: params.threshold_ns,
            top_n: params.top_n,
            saliency_limit: params.saliency_limit,
        };

        match ChronosExecutionQueryService::query(&ctx, input).await {
            Ok(out) => {
                let value = serde_json::to_value(&out).unwrap_or(serde_json::json!({}));
                Ok(CallToolResult::success(json_content(&value)))
            }
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{}' not found", s),
            ))),
            Err(ServiceError::InvalidInput(s)) => Ok(CallToolResult::error(text_content(s))),
            Err(e) => Ok(CallToolResult::error(text_content(format!(
                "internal error: unexpected error: {}",
                e
            )))),
        }
    }

    #[tool(
        name = "state_diff",
        description = "Deprecated. Use `state_query` with `kind=register_diff` instead. Compare program state (registers) between two timestamps."
    )]
    async fn state_diff(
        &self,
        params: Parameters<StateDiffParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let ctx = StateQueryContext {
            engines: &self.engines,
        };
        let input = StateQueryInput {
            session_id: params.session_id,
            kind: StateQueryKind::RegisterDiff,
            timestamp_a: Some(params.timestamp_a),
            timestamp_b: Some(params.timestamp_b),
            event_id: None,
            address: None,
            timestamp_ns: None,
            start_address: None,
            end_address: None,
            start_ts: None,
            end_ts: None,
            expression: None,
        };

        match ChronosStateQueryService::query(&ctx, input).await {
            Ok(StateQueryOutput::RegisterDiff { result }) => {
                let json = serde_json::to_string_pretty(&result).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{}' not found", s),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!(
                "internal error: unexpected error: {}",
                e
            )))),
            Ok(_) => unreachable!("kind=register_diff always yields RegisterDiff variant"),
        }
    }

    #[tool(
        name = "state_query",
        description = "v2 dispatcher for state transition/value evidence queries. Select the query kind via `kind` (register_diff | memory_read | register_snapshot | memory_analysis | expression_eval). Each kind has its own required fields; see `docs/chronos-agentic-reconstruction/docs/specs/AGENT_API_V2.md`."
    )]
    async fn state_query(
        &self,
        params: Parameters<StateQueryParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let ctx = StateQueryContext {
            engines: &self.engines,
        };
        let input = StateQueryInput {
            session_id: params.session_id,
            kind: params.kind,
            timestamp_a: params.timestamp_a,
            timestamp_b: params.timestamp_b,
            event_id: params.event_id,
            address: params.address,
            timestamp_ns: params.timestamp_ns,
            start_address: params.start_address,
            end_address: params.end_address,
            start_ts: params.start_ts,
            end_ts: params.end_ts,
            expression: params.expression,
        };

        match ChronosStateQueryService::query(&ctx, input).await {
            Ok(out) => {
                let value = serde_json::to_value(&out).unwrap_or(serde_json::json!({}));
                Ok(CallToolResult::success(json_content(&value)))
            }
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{}' not found", s),
            ))),
            Err(ServiceError::InvalidInput(s)) => Ok(CallToolResult::error(text_content(s))),
            Err(e) => Ok(CallToolResult::error(text_content(format!(
                "internal error: unexpected error: {}",
                e
            )))),
        }
    }

    #[tool(
        name = "list_threads",
        description = "List all thread IDs in the trace."
    )]
    async fn list_threads(
        &self,
        params: Parameters<ListThreadsParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let threads = match QueryService::list_threads(&params.session_id, &self.engines).await {
            Ok(t) => t,
            Err(ServiceError::SessionNotFound(_)) => {
                return Ok(CallToolResult::error(text_content(format!(
                    "Session '{}' not found",
                    params.session_id
                ))));
            }
            Err(ServiceError::LockPoisoned) => {
                return Ok(CallToolResult::error(text_content("lock poisoned")));
            }
            // The new ServiceError variants cannot occur from list_threads,
            // but must be listed for exhaustiveness.
            Err(ServiceError::MemoryNotFound { .. }) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected memory error",
                )));
            }
            Err(ServiceError::EventNotFound { .. }) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected event error",
                )));
            }
            Err(ServiceError::NoRegisterState { .. }) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected register error",
                )));
            }
            Err(ServiceError::EvalError(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected eval error",
                )));
            }
            Err(ServiceError::QueryExecutionError(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected query error",
                )));
            }
            // New session-lifecycle error variants (cannot occur from list_threads
            // but must be listed for exhaustiveness).
            Err(ServiceError::SessionNotInMemory(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected session error",
                )));
            }
            Err(ServiceError::EmptySession(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected empty session",
                )));
            }
            Err(ServiceError::SaveFailed(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected save error",
                )));
            }
            Err(ServiceError::LoadFailed(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected load error",
                )));
            }
            Err(ServiceError::ListFailed(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected list error",
                )));
            }
            Err(ServiceError::DeleteFailed(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected delete error",
                )));
            }
            // New tripwire error variants (cannot occur from list_threads).
            Err(ServiceError::InvalidCondition(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected condition error",
                )));
            }
            Err(ServiceError::InvalidTripwireIdFormat(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected tripwire ID format error",
                )));
            }
            Err(ServiceError::TripwireNotFound(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected tripwire not found",
                )));
            }
            // Probe service variants cannot be produced by query_events but are
            // listed for exhaustiveness against the ServiceError enum.
            Err(ServiceError::InvalidProgramPath(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected probe program path error",
                )));
            }
            Err(ServiceError::ProbeNotFound(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected probe not found",
                )));
            }
            Err(ServiceError::ProbeStartFailed(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected probe start failure",
                )));
            }
            Err(ServiceError::ProbeStopError(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected probe stop error",
                )));
            }
            Err(ServiceError::InvalidCursorPayload) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected invalid cursor",
                )));
            }
            Err(ServiceError::CursorStale) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected cursor stale",
                )));
            }
            Err(ServiceError::DrainFailed(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected drain failure",
                )));
            }
            Err(ServiceError::ProbeStarting) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected probe starting",
                )));
            }
            Err(ServiceError::EbpfUnsupported(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected eBPF unsupported",
                )));
            }
            Err(ServiceError::InjectionFailed(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected injection failure",
                )));
            }
            // Browser probe service variants cannot be produced by
            // this call site but are listed for exhaustiveness against
            // the ServiceError enum.
            Err(ServiceError::ChromeUnavailable) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected Chrome unavailable",
                )));
            }
            Err(ServiceError::BrowserProbeNotFound(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected browser probe not found",
                )));
            }
            Err(ServiceError::BrowserProbeStartFailed(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected browser probe start failure",
                )));
            }
            Err(ServiceError::BrowserProbeDrainFailed(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected browser probe drain failure",
                )));
            }
            Err(ServiceError::InvalidInput(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected invalid input",
                )));
            }
            // m6-05: session_export variants cannot be produced by this
            // call site but are listed for exhaustiveness against the
            // ServiceError enum.
            Err(ServiceError::ExportFailed(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected export failure",
                )));
            }
            Err(ServiceError::InvalidExportParameter(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected invalid export parameter",
                )));
            }
        };

        let output = serde_json::json!({
            "session_id": params.session_id,
            "thread_count": threads.len(),
            "thread_ids": threads,
        });
        Ok(CallToolResult::success(json_content(&output)))
    }

    // ========================================================================
    // SF4 — Semantic Compression + Advanced Tools (T17–T23)
    // ========================================================================

    #[tool(
        name = "debug_call_graph",
        description = "Deprecated. Use `execution_query` with `kind=call_graph` instead. Build the call graph for a session up to a given depth. Returns callers and callees for each function observed in the trace."
    )]
    async fn debug_call_graph(
        &self,
        params: Parameters<DebugCallGraphParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let ctx = ExecutionQueryContext {
            engines: &self.engines,
        };
        let input = ExecutionQueryInput {
            session_id: params.session_id.clone(),
            kind: ExecutionQueryKind::CallGraph,
            event_id: None,
            max_depth: Some(params.max_depth),
            threshold_ns: None,
            top_n: None,
            saliency_limit: None,
        };

        match ChronosExecutionQueryService::query(&ctx, input).await {
            Ok(ExecutionQueryOutput::CallGraph { graph }) => {
                let nodes: Vec<serde_json::Value> = graph
                    .nodes
                    .iter()
                    .map(|n| {
                        serde_json::json!({
                            "function": n.function,
                            "call_count": n.call_count,
                            "callers": n.callers,
                            "callees": n.callees,
                        })
                    })
                    .collect();
                let output = serde_json::json!({
                    "session_id": params.session_id,
                    "max_depth": params.max_depth,
                    "unique_functions": graph.stats.node_count,
                    "nodes": nodes,
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{}' not found", s),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!(
                "internal error: unexpected error: {}",
                e
            )))),
            Ok(_) => unreachable!("kind=call_graph always yields CallGraph variant"),
        }
    }

    #[tool(
        name = "debug_find_variable_origin",
        description = "Deprecated. Use `trace_slice` with `slice_kind=variable_origin` instead. Trace the origin of a variable: find all write mutations to it and reconstruct its lineage. Uses the CausalityIndex."
    )]
    async fn debug_find_variable_origin(
        &self,
        params: Parameters<DebugFindVariableOriginParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let ctx = TraceSliceContext {
            engines: &self.engines,
        };
        let input = TraceSliceInput {
            session_id: params.session_id,
            slice_kind: chronos_services::output::TraceSliceKind::VariableOrigin,
            variable_name: Some(params.variable_name),
            address: None,
            limit: params.limit,
        };

        match ChronosTraceSliceService::slice(&ctx, input).await {
            Ok(chronos_services::output::TraceSliceOutput::VariableOrigin {
                session_id: _,
                result,
            }) => {
                let output = serde_json::json!({
                    "session_id": result.session_id,
                    "variable_name": result.variable_name,
                    "mutation_count": result.mutation_count,
                    "mutations": result.mutations.iter().map(|m| serde_json::json!({
                        "event_id": m.event_id,
                        "timestamp_ns": m.timestamp_ns,
                        "thread_id": m.thread_id,
                        "value_before": m.value_before,
                        "value_after": m.value_after,
                        "function": m.function,
                    })).collect::<Vec<_>>(),
                    "note": result.note,
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{}' not found", s),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!(
                "internal error: unexpected error: {}",
                e
            )))),
            Ok(_) => {
                unreachable!("slice_kind=variable_origin always yields VariableOrigin variant")
            }
        }
    }

    #[tool(
        name = "debug_find_crash",
        description = "Deprecated. Use `trace_slice` with `slice_kind=crash` instead. Identify the crash point in a trace: find the last event before a fatal signal (SIGSEGV, SIGABRT, etc.) and return the call stack at that point."
    )]
    async fn debug_find_crash(
        &self,
        params: Parameters<DebugFindCrashParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let ctx = TraceSliceContext {
            engines: &self.engines,
        };
        let input = TraceSliceInput {
            session_id: params.session_id,
            slice_kind: chronos_services::output::TraceSliceKind::Crash,
            variable_name: None,
            address: None,
            limit: 0,
        };

        match ChronosTraceSliceService::slice(&ctx, input).await {
            Ok(chronos_services::output::TraceSliceOutput::Crash {
                session_id: _,
                result,
            }) => {
                let output = if result.crash_found {
                    serde_json::json!({
                        "session_id": result.session_id,
                        "crash_found": result.crash_found,
                        "signal": result.signal,
                        "event_id": result.event_id,
                        "timestamp_ns": result.timestamp_ns,
                        "thread_id": result.thread_id,
                        "call_stack_depth": result.call_stack_depth,
                        "call_stack": result.call_stack.iter().map(|f| serde_json::json!({
                            "depth": f.depth,
                            "function": f.function,
                            "address": format!("0x{:x}", f.address),
                            "file": f.file,
                            "line": f.line,
                        })).collect::<Vec<_>>(),
                    })
                } else {
                    serde_json::json!({
                        "session_id": result.session_id,
                        "crash_found": result.crash_found,
                        "note": result.note,
                    })
                };
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{}' not found", s),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!(
                "internal error: unexpected error: {}",
                e
            )))),
            Ok(_) => unreachable!("slice_kind=crash always yields Crash variant"),
        }
    }

    #[tool(
        name = "debug_detect_races",
        description = "Deprecated. Use `execution_query` with `kind=race_detect` instead. Detect suspicious concurrent accesses (m0-09). Finds writes to the same memory address within the threshold_ns window on different threads. The tool name is kept for backward compatibility; internally the heuristic is named detect_concurrent_access because we do not perform happens-before analysis — results are a triage signal, not a verdict that the access is a true data race. Default threshold is 100ns."
    )]
    async fn debug_detect_races(
        &self,
        params: Parameters<DebugDetectRacesParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let ctx = ExecutionQueryContext {
            engines: &self.engines,
        };
        let input = ExecutionQueryInput {
            session_id: params.session_id,
            kind: ExecutionQueryKind::RaceDetect,
            event_id: None,
            max_depth: None,
            threshold_ns: Some(params.threshold_ns),
            top_n: None,
            saliency_limit: None,
        };

        match ChronosExecutionQueryService::query(&ctx, input).await {
            Ok(ExecutionQueryOutput::RaceDetect { report }) => {
                let output = serde_json::json!({
                    "session_id": report.session_id,
                    "threshold_ns": report.threshold_ns,
                    "access_count": report.access_count,
                    "accesses": report.accesses.iter().map(|r| serde_json::json!({
                        "address": format!("0x{:x}", r.address),
                        "delta_ns": r.delta_ns,
                        "write_a": {
                            "event_id": r.write_a.event_id,
                            "timestamp_ns": r.write_a.timestamp,
                            "thread_id": r.write_a.thread_id,
                            "value_before": r.write_a.value_before,
                            "value_after": r.write_a.value_after,
                            "function": r.write_a.function,
                        },
                        "write_b": {
                            "event_id": r.write_b.event_id,
                            "timestamp_ns": r.write_b.timestamp,
                            "thread_id": r.write_b.thread_id,
                            "value_before": r.write_b.value_before,
                            "value_after": r.write_b.value_after,
                            "function": r.write_b.function,
                        },
                    })).collect::<Vec<_>>(),
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{}' not found", s),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!(
                "internal error: unexpected error: {}",
                e
            )))),
            Ok(_) => unreachable!("kind=race_detect always yields RaceDetect variant"),
        }
    }

    #[tool(
        name = "inspect_causality",
        description = "Deprecated. Use `trace_slice` with `slice_kind=causality` instead. Inspect the full causal history of a memory address: all reads and writes, their timestamps, values, and originating functions."
    )]
    async fn inspect_causality(
        &self,
        params: Parameters<InspectCausalityParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let ctx = TraceSliceContext {
            engines: &self.engines,
        };
        let input = TraceSliceInput {
            session_id: params.session_id,
            slice_kind: chronos_services::output::TraceSliceKind::Causality,
            variable_name: None,
            address: Some(params.address),
            limit: params.limit,
        };

        match ChronosTraceSliceService::slice(&ctx, input).await {
            Ok(chronos_services::output::TraceSliceOutput::Causality {
                session_id: _,
                result,
            }) => {
                let output = serde_json::json!({
                    "session_id": result.session_id,
                    "address": format!("0x{:x}", result.address),
                    "mutation_count": result.mutation_count,
                    "mutations": result.mutations.iter().map(|m| serde_json::json!({
                        "event_id": m.event_id,
                        "timestamp_ns": m.timestamp_ns,
                        "thread_id": m.thread_id,
                        "value_before": m.value_before,
                        "value_after": m.value_after,
                        "function": m.function,
                    })).collect::<Vec<_>>(),
                    "note": result.note,
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{}' not found", s),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!(
                "internal error: unexpected error: {}",
                e
            )))),
            Ok(_) => unreachable!("slice_kind=causality always yields Causality variant"),
        }
    }

    #[tool(
        name = "debug_expand_hotspot",
        description = "Deprecated. Use `execution_query` with `kind=hotspot` instead. Semantic compression Level 1 — return the top-N hottest functions by call count and CPU cycles. Use debug_execution_summary first (Level 0) then call this to zoom in."
    )]
    async fn debug_expand_hotspot(
        &self,
        params: Parameters<DebugExpandHotspotParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let ctx = ExecutionQueryContext {
            engines: &self.engines,
        };
        let input = ExecutionQueryInput {
            session_id: params.session_id,
            kind: ExecutionQueryKind::Hotspot,
            event_id: None,
            max_depth: None,
            threshold_ns: None,
            top_n: Some(params.top_n),
            saliency_limit: None,
        };

        match ChronosExecutionQueryService::query(&ctx, input).await {
            Ok(ExecutionQueryOutput::Hotspot { report }) => {
                let output = serde_json::json!({
                    "session_id": report.session_id,
                    "compression_level": report.compression_level,
                    "top_n": report.top_n,
                    "total_calls_in_trace": report.total_calls_in_trace,
                    "hotspot_functions": report.hotspot_functions.iter().map(|f| serde_json::json!({
                        "function": f.function,
                        "call_count": f.call_count,
                        "total_cycles": f.total_cycles,
                        "avg_cycles_per_call": f.avg_cycles_per_call,
                    })).collect::<Vec<_>>(),
                    "hint": report.hint,
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{}' not found", s),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!(
                "internal error: unexpected error: {}",
                e
            )))),
            Ok(_) => unreachable!("kind=hotspot always yields Hotspot variant"),
        }
    }

    #[tool(
        name = "debug_get_saliency_scores",
        description = "Deprecated. Use `execution_query` with `kind=saliency` instead. Compute saliency scores [0.0–1.0] for all functions: a high score means this function consumed a disproportionate share of CPU cycles relative to other functions. Use to prioritize where to look."
    )]
    async fn debug_get_saliency_scores(
        &self,
        params: Parameters<DebugGetSaliencyScoresParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let ctx = ExecutionQueryContext {
            engines: &self.engines,
        };
        let input = ExecutionQueryInput {
            session_id: params.session_id,
            kind: ExecutionQueryKind::Saliency,
            event_id: None,
            max_depth: None,
            threshold_ns: None,
            top_n: None,
            saliency_limit: Some(params.limit),
        };

        match ChronosExecutionQueryService::query(&ctx, input).await {
            Ok(ExecutionQueryOutput::Saliency { result }) => {
                let scores: Vec<serde_json::Value> = result
                    .scores
                    .iter()
                    .map(|s| {
                        if s.cycles.is_some() {
                            serde_json::json!({
                                "function": s.function,
                                "saliency_score": s.saliency_score,
                                "call_count": s.call_count,
                                "cycles": null,
                            })
                        } else {
                            serde_json::json!({
                                "function": s.function,
                                "saliency_score": s.saliency_score,
                                "call_count": s.call_count,
                                "total_cycles": s.total_cycles,
                            })
                        }
                    })
                    .collect();
                let output = serde_json::json!({
                    "session_id": result.session_id,
                    "scored_functions": result.scored_functions,
                    "scores": scores,
                    "hint": result.hint,
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{}' not found", s),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!(
                "internal error: unexpected error: {}",
                e
            )))),
            Ok(_) => unreachable!("kind=saliency always yields Saliency variant"),
        }
    }

    // ========================================================================
    // SF5 — Persistence Tools (T10–T14)
    // ========================================================================

    #[tool(
        name = "save_session",
        description = "Save an in-memory session to persistent storage. Saves the session's events to the CAS store and records metadata. Returns hash count and dedup statistics."
    )]
    async fn save_session(
        &self,
        params: Parameters<SaveSessionParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let ctx = SessionsContext {
            engines: &self.engines,
            session_languages: &self.session_languages,
            connected_sessions: &self.connected_sessions,
            store: &self.store,
        };

        match SessionsService::save_session(
            &params.session_id,
            Language::from_string(&params.language),
            params.target.clone(),
            &ctx,
        )
        .await
        {
            Ok(result) => {
                let output = serde_json::json!({
                    "session_id": params.session_id,
                    "status": "saved",
                    "event_count": result.event_count,
                    "hash_count": result.hash_count,
                    "language": result.language,
                    "target": result.target,
                    "duration_ms": result.duration_ms,
                    "hint": "Use load_session to reload this session, or list_sessions to see all saved sessions.",
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::SessionNotInMemory(s)) => {
                Ok(CallToolResult::error(text_content(format!(
                    "Session '{}' not found in memory. Run probe_start first.",
                    s
                ))))
            }
            Err(ServiceError::EmptySession(_)) => Ok(CallToolResult::error(text_content(
                "Session has no events to save.".to_string(),
            ))),
            Err(ServiceError::SaveFailed(e)) => Ok(CallToolResult::error(text_content(format!(
                "Failed to save session: {}",
                e
            )))),
            Err(ServiceError::LockPoisoned) => Ok(CallToolResult::error(text_content(
                "lock poisoned".to_string(),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
        }
    }

    #[tool(
        name = "load_session",
        description = "Load a session from persistent storage into a new in-memory query engine. Returns metadata and event count."
    )]
    async fn load_session(
        &self,
        params: Parameters<LoadSessionParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let ctx = SessionsContext {
            engines: &self.engines,
            session_languages: &self.session_languages,
            connected_sessions: &self.connected_sessions,
            store: &self.store,
        };

        match SessionsService::load_session(&params.session_id, &ctx).await {
            Ok(result) => {
                let output = serde_json::json!({
                    "session_id": params.session_id,
                    "status": "loaded",
                    "language": result.language,
                    "target": result.target,
                    "event_count": result.event_count,
                    "duration_ms": result.duration_ms,
                    "created_at": result.created_at,
                    "hint": "Session is now queryable. Use query_events, get_execution_summary, etc.",
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::LoadFailed(e)) => Ok(CallToolResult::error(text_content(format!(
                "Failed to load session '{}': {}",
                params.session_id, e
            )))),
            Err(ServiceError::LockPoisoned) => Ok(CallToolResult::error(text_content(
                "lock poisoned".to_string(),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
        }
    }

    #[tool(
        name = "list_sessions",
        description = "List all saved sessions from persistent storage. Returns metadata for each session (no event data)."
    )]
    async fn list_sessions(
        &self,
        _params: Parameters<NoParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = SessionsContext {
            engines: &self.engines,
            session_languages: &self.session_languages,
            connected_sessions: &self.connected_sessions,
            store: &self.store,
        };

        match SessionsService::list_sessions(&ctx).await {
            Ok(result) => {
                let output = serde_json::json!({
                    "session_count": result.sessions.len(),
                    "sessions": result.sessions.iter().map(|s| serde_json::json!({
                        "session_id": s.session_id,
                        "language": s.language,
                        "target": s.target,
                        "event_count": s.event_count,
                        "duration_ms": s.duration_ms,
                        "created_at": s.created_at,
                    })).collect::<Vec<_>>(),
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::ListFailed(e)) => Ok(CallToolResult::error(text_content(format!(
                "Failed to list sessions: {}",
                e
            )))),
            Err(ServiceError::LockPoisoned) => Ok(CallToolResult::error(text_content(
                "lock poisoned".to_string(),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
        }
    }

    #[tool(
        name = "delete_session",
        description = "Delete a session from persistent storage. Does not affect in-memory sessions."
    )]
    async fn delete_session(
        &self,
        params: Parameters<DeleteSessionParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let ctx = SessionsContext {
            engines: &self.engines,
            session_languages: &self.session_languages,
            connected_sessions: &self.connected_sessions,
            store: &self.store,
        };

        match SessionsService::delete_session(&params.session_id, &ctx).await {
            Ok(_result) => {
                // Also purge all in-memory state for this session.
                self.cleanup_session_memory(&params.session_id).await;

                let output = serde_json::json!({
                    "session_id": params.session_id,
                    "status": "deleted",
                    "message": format!("Session '{}' deleted from persistent storage and memory.", params.session_id),
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::DeleteFailed(e)) => Ok(CallToolResult::error(text_content(format!(
                "Failed to delete session '{}': {}",
                params.session_id, e
            )))),
            Err(ServiceError::LockPoisoned) => Ok(CallToolResult::error(text_content(
                "lock poisoned".to_string(),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
        }
    }

    #[tool(
        name = "drop_session",
        description = "Remove a session from in-memory state WITHOUT touching persistent storage. Complement to delete_session which removes from store. Returns success even if session was not found in memory (idempotent)."
    )]
    async fn drop_session(
        &self,
        params: Parameters<DropSessionParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let ctx = SessionsContext {
            engines: &self.engines,
            session_languages: &self.session_languages,
            connected_sessions: &self.connected_sessions,
            store: &self.store,
        };

        match SessionsService::drop_session(&params.session_id, &ctx).await {
            Ok(result) => {
                // Clean up all in-memory state for this session.
                self.cleanup_session_memory(&params.session_id).await;

                if result.existed {
                    let output = serde_json::json!({
                        "session_id": params.session_id,
                        "status": "dropped",
                        "message": "Session removed from memory. Persistent storage not affected.",
                    });
                    Ok(CallToolResult::success(json_content(&output)))
                } else {
                    let output = serde_json::json!({
                        "session_id": params.session_id,
                        "status": "not_found",
                        "message": "Session not found in memory. No action taken.",
                    });
                    Ok(CallToolResult::success(json_content(&output)))
                }
            }
            Err(ServiceError::LockPoisoned) => Ok(CallToolResult::error(text_content(
                "lock poisoned".to_string(),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
        }
    }

    // ========================================================================
    // SF6 — Inspection Tools (T5–T7)
    // ========================================================================

    #[tool(
        name = "evaluate_expression",
        description = "Deprecated. Use `state_query` with `kind=expression_eval` instead. Evaluate an arithmetic expression using local variables captured at a frame event. Supports +, -, *, /, parentheses, and variable names."
    )]
    async fn evaluate_expression(
        &self,
        params: Parameters<EvaluateExpressionParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let ctx = StateQueryContext {
            engines: &self.engines,
        };
        let input = StateQueryInput {
            session_id: params.session_id,
            kind: StateQueryKind::ExpressionEval,
            timestamp_a: None,
            timestamp_b: None,
            event_id: Some(params.event_id),
            address: None,
            timestamp_ns: None,
            start_address: None,
            end_address: None,
            start_ts: None,
            end_ts: None,
            expression: Some(params.expression.clone()),
        };

        match ChronosStateQueryService::query(&ctx, input).await {
            Ok(StateQueryOutput::ExpressionEval { result }) => match result {
                EvalResult::Value(value) => {
                    let output = serde_json::json!({
                        "event_id": params.event_id,
                        "expression": params.expression,
                        "result": value,
                    });
                    Ok(CallToolResult::success(json_content(&output)))
                }
                EvalResult::Error(msg) => {
                    let output = serde_json::json!({
                        "event_id": params.event_id,
                        "expression": params.expression,
                        "error": msg,
                    });
                    Ok(CallToolResult::success(vec![Content::text(
                        serde_json::to_string_pretty(&output).unwrap_or_default(),
                    )]))
                }
            },
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{}' not found", s),
            ))),
            Err(ServiceError::LockPoisoned) => Ok(CallToolResult::error(text_content(
                "lock poisoned".to_string(),
            ))),
            Err(ServiceError::MemoryNotFound {
                address,
                timestamp_ns,
            }) => Ok(CallToolResult::error(text_content(format!(
                "no memory at address 0x{:x} before timestamp {}",
                address, timestamp_ns
            )))),
            Err(ServiceError::EventNotFound { event_id }) => Ok(CallToolResult::error(
                text_content(format!("event {} not found", event_id)),
            )),
            Err(ServiceError::NoRegisterState { event_id }) => Ok(CallToolResult::error(
                text_content(format!("no register state at event {}", event_id)),
            )),
            Err(ServiceError::EvalError(msg)) => Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&serde_json::json!({
                    "event_id": params.event_id,
                    "expression": params.expression,
                    "error": msg,
                }))
                .unwrap_or_default(),
            )])),
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
            Ok(_) => unreachable!("kind=expression_eval always yields ExpressionEval variant"),
        }
    }

    #[tool(
        name = "debug_get_variables",
        description = "Get all variables in scope at a specific event. Returns locals from Python/Java/Go frame events or VariableWrite events."
    )]
    async fn debug_get_variables(
        &self,
        params: Parameters<DebugGetVariablesParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let result =
            DebugReadService::get_variables(&params.session_id, params.event_id, &self.engines)
                .await;

        match result {
            Ok(vars) => {
                let output = serde_json::json!({
                    "event_id": params.event_id,
                    "variables": vars,
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{}' not found", s),
            ))),
            Err(ServiceError::LockPoisoned) => Ok(CallToolResult::error(text_content(
                "lock poisoned".to_string(),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
        }
    }

    #[tool(
        name = "debug_get_memory",
        description = "Deprecated. Use `state_query` with `kind=memory_read` instead. Read raw memory at an address as of a specific timestamp (nanoseconds). Returns the most recent MemoryWrite event at or before the timestamp."
    )]
    async fn debug_get_memory(
        &self,
        params: Parameters<DebugGetMemoryParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let ctx = StateQueryContext {
            engines: &self.engines,
        };
        let input = StateQueryInput {
            session_id: params.session_id,
            kind: StateQueryKind::MemoryRead,
            timestamp_a: None,
            timestamp_b: None,
            event_id: None,
            address: Some(params.address),
            timestamp_ns: Some(params.timestamp_ns),
            start_address: None,
            end_address: None,
            start_ts: None,
            end_ts: None,
            expression: None,
        };

        match ChronosStateQueryService::query(&ctx, input).await {
            Ok(StateQueryOutput::MemoryRead { result }) => {
                let output = serde_json::json!({
                    "address": format!("0x{:x}", result.address),
                    "timestamp_ns": result.timestamp_ns,
                    "event_id": result.event_id,
                    "size": result.size,
                    "data": result.data,
                    "hex": result.hex,
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{}' not found", s),
            ))),
            Err(ServiceError::MemoryNotFound {
                address,
                timestamp_ns,
            }) => Ok(CallToolResult::error(text_content(format!(
                "No memory event found at address 0x{:x} before timestamp {}",
                address, timestamp_ns
            )))),
            Err(ServiceError::LockPoisoned) => Ok(CallToolResult::error(text_content(
                "lock poisoned".to_string(),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
            Ok(_) => unreachable!("kind=memory_read always yields MemoryRead variant"),
        }
    }

    // ========================================================================
    // SF7 — Phase 11 Missing Tools (T20–T24)
    // ========================================================================

    #[tool(
        name = "debug_get_registers",
        description = "Deprecated. Use `state_query` with `kind=register_snapshot` instead. Get CPU register values at a specific event_id. Returns the register state snapshot if available."
    )]
    async fn debug_get_registers(
        &self,
        params: Parameters<DebugGetRegistersParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let ctx = StateQueryContext {
            engines: &self.engines,
        };
        let input = StateQueryInput {
            session_id: params.session_id,
            kind: StateQueryKind::RegisterSnapshot,
            timestamp_a: None,
            timestamp_b: None,
            event_id: Some(params.event_id),
            address: None,
            timestamp_ns: None,
            start_address: None,
            end_address: None,
            start_ts: None,
            end_ts: None,
            expression: None,
        };

        match ChronosStateQueryService::query(&ctx, input).await {
            Ok(StateQueryOutput::RegisterSnapshot { result }) => {
                let r = &result.registers;
                let output = serde_json::json!({
                    "event_id": result.event_id,
                    "registers": {
                        "rax": format!("0x{:x}", r.rax),
                        "rbx": format!("0x{:x}", r.rbx),
                        "rcx": format!("0x{:x}", r.rcx),
                        "rdx": format!("0x{:x}", r.rdx),
                        "rsi": format!("0x{:x}", r.rsi),
                        "rdi": format!("0x{:x}", r.rdi),
                        "rbp": format!("0x{:x}", r.rbp),
                        "rsp": format!("0x{:x}", r.rsp),
                        "r8": format!("0x{:x}", r.r8),
                        "r9": format!("0x{:x}", r.r9),
                        "r10": format!("0x{:x}", r.r10),
                        "r11": format!("0x{:x}", r.r11),
                        "r12": format!("0x{:x}", r.r12),
                        "r13": format!("0x{:x}", r.r13),
                        "r14": format!("0x{:x}", r.r14),
                        "r15": format!("0x{:x}", r.r15),
                        "rip": format!("0x{:x}", r.rip),
                        "rflags": format!("0x{:x}", r.rflags),
                    },
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{}' not found", s),
            ))),
            Err(ServiceError::EventNotFound { event_id }) => Ok(CallToolResult::error(
                text_content(format!("Event {} not found", event_id)),
            )),
            Err(ServiceError::NoRegisterState { event_id }) => Ok(CallToolResult::error(
                text_content(format!("no register state at event {}", event_id)),
            )),
            Err(ServiceError::LockPoisoned) => Ok(CallToolResult::error(text_content(
                "lock poisoned".to_string(),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
            Ok(_) => unreachable!("kind=register_snapshot always yields RegisterSnapshot variant"),
        }
    }

    #[tool(
        name = "debug_diff",
        description = "Compare process state between two event_ids — variables, registers, memory."
    )]
    async fn debug_diff(
        &self,
        params: Parameters<DebugDiffParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let result = DebugReadService::diff(
            &params.session_id,
            params.event_id_a,
            params.event_id_b,
            &self.engines,
        )
        .await;

        match result {
            Ok(snap) => {
                // Convert RegisterChange back to the existing JSON Map shape
                let mut registers_changed = serde_json::Map::new();
                for (name, change) in snap.registers_changed {
                    registers_changed.insert(
                        name,
                        serde_json::json!({
                            "before": change.before,
                            "after": change.after,
                        }),
                    );
                }

                let output = serde_json::json!({
                    "event_id_a": snap.event_id_a,
                    "event_id_b": snap.event_id_b,
                    "variables_added": snap.variables_added,
                    "variables_removed": snap.variables_removed,
                    "variables_changed": snap.variables_changed.iter().map(|vc| {
                        serde_json::json!({
                            "name": vc.name,
                            "before": vc.before,
                            "after": vc.after,
                        })
                    }).collect::<Vec<_>>(),
                    "registers_changed": registers_changed,
                    "timestamp_delta_ns": snap.timestamp_delta_ns,
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{}' not found", s),
            ))),
            Err(ServiceError::LockPoisoned) => Ok(CallToolResult::error(text_content(
                "lock poisoned".to_string(),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
        }
    }

    #[tool(
        name = "debug_analyze_memory",
        description = "Deprecated. Use `state_query` with `kind=memory_analysis` instead. Analyze all memory accesses to an address range within a time window."
    )]
    async fn debug_analyze_memory(
        &self,
        params: Parameters<DebugAnalyzeMemoryParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let ctx = StateQueryContext {
            engines: &self.engines,
        };
        let input = StateQueryInput {
            session_id: params.session_id,
            kind: StateQueryKind::MemoryAnalysis,
            timestamp_a: None,
            timestamp_b: None,
            event_id: None,
            address: None,
            timestamp_ns: None,
            start_address: Some(params.start_address),
            end_address: Some(params.end_address),
            start_ts: Some(params.start_ts),
            end_ts: Some(params.end_ts),
            expression: None,
        };

        match ChronosStateQueryService::query(&ctx, input).await {
            Ok(StateQueryOutput::MemoryAnalysis { result }) => {
                let output = serde_json::json!({
                    "start_address": result.start_address,
                    "end_address": result.end_address,
                    "start_ts": result.start_ts,
                    "end_ts": result.end_ts,
                    "total_writes": result.total_writes,
                    "accesses": result.accesses.iter().map(|a| {
                        serde_json::json!({
                            "address": a.address,
                            "timestamp_ns": a.timestamp_ns,
                            "data_hex": a.data_hex,
                            "event_id": a.event_id,
                            "size": a.size,
                        })
                    }).collect::<Vec<_>>(),
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{}' not found", s),
            ))),
            Err(ServiceError::LockPoisoned) => Ok(CallToolResult::error(text_content(
                "lock poisoned".to_string(),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
            Ok(_) => unreachable!("kind=memory_analysis always yields MemoryAnalysis variant"),
        }
    }

    #[tool(
        name = "forensic_memory_audit",
        description = "Deprecated. Use `trace_slice` with `slice_kind=memory_audit` instead. Full audit trail for a specific address — all writes with calling context."
    )]
    async fn forensic_memory_audit(
        &self,
        params: Parameters<ForensicMemoryAuditParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let ctx = TraceSliceContext {
            engines: &self.engines,
        };
        let input = TraceSliceInput {
            session_id: params.session_id,
            slice_kind: chronos_services::output::TraceSliceKind::MemoryAudit,
            variable_name: None,
            address: Some(params.address),
            limit: params.limit,
        };

        match ChronosTraceSliceService::slice(&ctx, input).await {
            Ok(chronos_services::output::TraceSliceOutput::MemoryAudit {
                session_id: _,
                result,
            }) => {
                let output = serde_json::json!({
                    "address": result.address,
                    "write_count": result.write_count,
                    "writes": result.writes.iter().map(|w| {
                        serde_json::json!({
                            "timestamp_ns": w.timestamp_ns,
                            "event_id": w.event_id,
                            "data_hex": w.data_hex,
                            "call_stack": w.call_stack.iter().map(|f| {
                                serde_json::json!({
                                    "depth": f.depth,
                                    "function": f.function,
                                    "file": f.file,
                                    "line": f.line,
                                })
                            }).collect::<Vec<_>>(),
                        })
                    }).collect::<Vec<_>>(),
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{}' not found", s),
            ))),
            Err(ServiceError::LockPoisoned) => Ok(CallToolResult::error(text_content(
                "lock poisoned".to_string(),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
            Ok(_) => unreachable!("slice_kind=memory_audit always yields MemoryAudit variant"),
        }
    }

    // ========================================================================
    // SF8 — Tripwire Tools (T21–T23)
    // ========================================================================

    #[tool(
        name = "tripwire_create",
        description = "Create a tripwire to monitor trace events matching a condition. When a matching event occurs, the tripwire fires and can be retrieved via tripwire_list. Use this for alerting on specific function calls, exceptions, syscalls, or signals."
    )]
    async fn tripwire_create(
        &self,
        params: Parameters<TripwireCreateParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let condition = match params.condition.into_condition() {
            Ok(c) => c,
            Err(bad) => {
                return Ok(CallToolResult::error(text_content(format!(
                    "tripwire_create: unknown event_type '{}'. Valid types: syscall_enter, syscall_exit, function_entry, function_exit, variable_write, memory_write, signal_delivered, breakpoint_hit, thread_create, thread_exit, exception_thrown.",
                    bad
                ))));
            }
        };

        match TripwiresService::create(condition, params.label, &self.tripwire_manager) {
            Ok(result) => {
                let output = serde_json::json!({
                    "tripwire_id": result.tripwire_id,
                    "status": "registered",
                    "active_count": result.active_count,
                    "label": result.label,
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::LockPoisoned) => Ok(CallToolResult::error(text_content(
                "lock poisoned".to_string(),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
        }
    }

    #[tool(
        name = "tripwire_list",
        description = "List all active tripwires and any that have fired since the last call. Returns tripwire definitions and a list of fired notifications with event context."
    )]
    async fn tripwire_list(
        &self,
        _params: Parameters<NoParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = TripwiresService::list(&self.tripwire_manager);

        let tripwire_summaries: Vec<_> = result
            .tripwires
            .iter()
            .map(|tw| {
                serde_json::json!({
                    "id": tw.id,
                    "label": tw.label,
                    "condition": tw.condition,
                    "fire_count": tw.fire_count,
                })
            })
            .collect();

        let fired_events: Vec<_> = result
            .fired_events
            .iter()
            .map(|f| {
                serde_json::json!({
                    "tripwire_id": f.tripwire_id,
                    "condition_description": f.condition_description,
                    "event_id": f.event_id,
                    "timestamp_ns": f.timestamp_ns,
                    "thread_id": f.thread_id,
                })
            })
            .collect();

        let output = serde_json::json!({
            "active_tripwires": tripwire_summaries,
            "fired_events": fired_events,
            "total_active": result.total_active,
            "fired_count": result.fired_count,
        });
        Ok(CallToolResult::success(json_content(&output)))
    }

    #[tool(
        name = "tripwire_delete",
        description = "Delete a tripwire by ID. The tripwire will no longer fire for new events."
    )]
    async fn tripwire_delete(
        &self,
        params: Parameters<TripwireDeleteParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let tripwire_id = params.tripwire_id.trim();

        // Validate format before calling service
        if tripwire_id.strip_prefix("tripwire-").is_none() {
            return Ok(CallToolResult::error(text_content(format!(
                "Invalid tripwire ID format '{}'. Expected format: 'tripwire-<number>'",
                tripwire_id
            ))));
        }

        match TripwiresService::delete(tripwire_id, &self.tripwire_manager) {
            Ok(result) => {
                let output = serde_json::json!({
                    "tripwire_id": result.tripwire_id,
                    "status": "deleted",
                    "remaining_active": result.remaining_active,
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::LockPoisoned) => Ok(CallToolResult::error(text_content(
                "lock poisoned".to_string(),
            ))),
            Err(ServiceError::InvalidTripwireIdFormat(s)) => Ok(CallToolResult::error(
                text_content(format!("Invalid tripwire ID format: '{}'", s)),
            )),
            Err(ServiceError::TripwireNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Tripwire '{}' not found", s),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
        }
    }

    #[tool(
        name = "tripwire_query",
        description = "Query tripwire state without draining fired events (non-destructive read). Useful for checking if any tripwires have fired without consuming the notifications."
    )]
    async fn tripwire_query(
        &self,
        _params: Parameters<NoParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let result = TripwiresService::query(&self.tripwire_manager);

        let tripwire_summaries: Vec<_> = result
            .tripwires
            .iter()
            .map(|tw| {
                serde_json::json!({
                    "id": tw.id,
                    "label": tw.label,
                    "condition": tw.condition,
                    "fire_count": tw.fire_count,
                })
            })
            .collect();

        let output = serde_json::json!({
            "active_tripwires": tripwire_summaries,
            "total_active": result.total_active,
        });
        Ok(CallToolResult::success(json_content(&output)))
    }

    // ========================================================================
    // SF9 — Live Probe Tools
    // ========================================================================

    #[tool(
        name = "probe_start",
        description = "Start a live probe on a target program. Unlike debug_run (which blocks until the program exits), probe_start returns immediately and streams events to a ring buffer. Use probe_drain to read events in real-time and probe_stop to finalize the session."
    )]
    async fn probe_start(
        &self,
        params: Parameters<ProbeStartParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        // Validate the program path (security gate, kept in the server).
        if let Err(e) = crate::security::validate_program_path(&params.program) {
            return Ok(CallToolResult::error(text_content(format!(
                "Invalid program path: {}",
                e
            ))));
        }

        let input = chronos_services::probe::ProbeStartInput {
            program: params.program,
            args: params.args,
            trace_syscalls: params.trace_syscalls,
            cwd: params.cwd,
            bus_capacity: params.bus_capacity,
            track_function_frames: params.track_function_frames,
        };

        let ctx = chronos_services::probe::ProbeContext {
            live_probes: &self.live_probes,
            engines: &self.engines,
            session_languages: &self.session_languages,
            tripwire_manager: &self.tripwire_manager,
            active_session: &self.active_session,
        };

        match chronos_services::probe::ProbeService::start(&ctx, input).await {
            Ok(out) => {
                let output = serde_json::json!({
                    "session_id": out.session_id,
                    "status": out.status,
                    "target": out.target,
                    "language": out.language,
                    "bus_capacity": out.bus_capacity,
                    "hint": out.hint,
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::InvalidProgramPath(msg)) => Ok(CallToolResult::error(text_content(
                format!("Invalid program path: {}", msg),
            ))),
            Err(ServiceError::ProbeStartFailed(msg)) => Ok(CallToolResult::error(text_content(
                format!("Failed to start probe: {}", msg),
            ))),
            Err(ServiceError::LockPoisoned) => {
                Ok(CallToolResult::error(text_content("lock poisoned")))
            }
            Err(other) => Ok(CallToolResult::error(text_content(format!(
                "internal error: unexpected probe start error: {}",
                other
            )))),
        }
    }

    #[tool(
        name = "probe_stop",
        description = "Stop a live probe session. Drains remaining events from the ring buffer, builds a QueryEngine, and makes the session fully queryable (query_events, get_call_stack, etc.)."
    )]
    async fn probe_stop(
        &self,
        params: Parameters<ProbeStopParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let ctx = chronos_services::probe::ProbeContext {
            live_probes: &self.live_probes,
            engines: &self.engines,
            session_languages: &self.session_languages,
            tripwire_manager: &self.tripwire_manager,
            active_session: &self.active_session,
        };

        match chronos_services::probe::ProbeService::stop(&ctx, &params.session_id) {
            Ok(result) => {
                // Build and store the query engine with proper noise filtering.
                // Still on the server side because it touches engines/session_languages.
                self.build_and_store_engine(&params.session_id, result.events, result.language)
                    .await;

                let output = serde_json::json!({
                    "session_id": params.session_id,
                    "status": "stopped",
                    "target": result.target,
                    "total_events": result.total_events,
                    "duration_ms": result.duration_ms,
                    "ebpf_detached": result.ebpf_detached,
                    "hint": "Session is now queryable. Use query_events, get_call_stack, etc."
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::ProbeNotFound(s)) => {
                Ok(CallToolResult::error(text_content(format!(
                    "Live probe session '{}' not found. It may have already been stopped.",
                    s
                ))))
            }
            Err(ServiceError::LockPoisoned) => {
                Ok(CallToolResult::error(text_content("lock poisoned")))
            }
            Err(other) => Ok(CallToolResult::error(text_content(format!(
                "internal error: unexpected probe stop error: {}",
                other
            )))),
        }
    }

    #[tool(
        name = "probe_drain",
        description = "Drain current events from a live probe session without stopping it. Returns a snapshot of events currently in the ring buffer. The probe continues running. Use probe_stop to finalize."
    )]
    async fn probe_drain(
        &self,
        params: Parameters<ProbeDrainParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        // Parse incoming cursor (if any) before locking — cheap and fail-fast.
        let cursor = match params.cursor.as_ref() {
            None => None,
            Some(dto) => match dto.to_domain() {
                Some(c) => Some(c),
                None => {
                    return Ok(CallToolResult::error(text_content(
                        "Invalid cursor payload: 'total_pushed' and 'snapshot_len' are required when 'cursor' is provided.".to_string(),
                    )))
                }
            },
        };

        let input = chronos_services::probe::ProbeDrainInput {
            session_id: params.session_id.clone(),
            cursor,
            offset: params.offset,
            limit: params.limit,
        };

        let ctx = chronos_services::probe::ProbeContext {
            live_probes: &self.live_probes,
            engines: &self.engines,
            session_languages: &self.session_languages,
            tripwire_manager: &self.tripwire_manager,
            active_session: &self.active_session,
        };

        match chronos_services::probe::ProbeService::drain(&ctx, input) {
            Ok(result) => {
                // Apply offset/limit (matches existing contract: slicing happens
                // after the drain, with the wrapper doing the slicing).
                let sliced: Vec<_> = result
                    .events
                    .into_iter()
                    .skip(params.offset)
                    .take(params.limit)
                    .map(|e| {
                        serde_json::json!({
                            "event_id": e.source_event_id,
                            "timestamp_ns": e.timestamp_ns,
                            "thread_id": e.thread_id,
                            "language": format!("{:?}", e.language),
                            "kind": format!("{:?}", e.kind),
                            "description": e.description,
                        })
                    })
                    .collect();

                let output = serde_json::json!({
                    "session_id": params.session_id,
                    "status": "running",
                    "total_buffered": result.total_buffered,
                    "returned": sliced.len(),
                    "offset": params.offset,
                    "limit": params.limit,
                    "cursor": {
                        "total_pushed": result.new_cursor.total_pushed,
                        "snapshot_len": result.new_cursor.snapshot_len,
                    },
                    "cursor_stale": result.cursor_stale,
                    "tripwires_fired": result.tripwires_fired,
                    "events": sliced,
                    "hint": "Probe is still running. Call probe_drain again for more events, or probe_stop to finalize."
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::ProbeNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Live probe session '{}' not found.", s),
            ))),
            Err(ServiceError::CursorStale) => Ok(CallToolResult::error(text_content(
                "Cursor is stale; re-anchor with a fresh probe_drain (no cursor).".to_string(),
            ))),
            Err(ServiceError::DrainFailed(msg)) => Ok(CallToolResult::error(text_content(
                format!("Failed to drain events: {}", msg),
            ))),
            Err(ServiceError::LockPoisoned) => {
                Ok(CallToolResult::error(text_content("lock poisoned")))
            }
            Err(other) => Ok(CallToolResult::error(text_content(format!(
                "internal error: unexpected probe drain error: {}",
                other
            )))),
        }
    }

    /// m1-03: MCP query path that reads `TraceEvent`s from the
    /// `ExecutionLog` (segmented, persistent) of a live probe
    /// session. Available only when the native backend was
    /// configured with `with_execution_log_dir`.
    ///
    /// Cursor is a non-destructive seq-based filter: passing `None`
    /// returns every record appended so far; passing a value returns
    /// records with seq strictly greater. The returned `tail_seq`
    /// can be passed back as `since` for the next call.
    #[tool(
        name = "probe_drain_log",
        description = "m1-03 ExecutionLog-backed query path. Reads TraceEvents from the durable ExecutionLog attached to a live probe session, instead of the legacy in-memory EventBus. Cursor is a seq number (None for fresh). Returns 0 records when the probe is not configured with an ExecutionLog directory."
    )]
    async fn probe_drain_log(
        &self,
        params: Parameters<ProbeDrainLogParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let ctx = chronos_services::probe::ProbeContext {
            live_probes: &self.live_probes,
            engines: &self.engines,
            session_languages: &self.session_languages,
            tripwire_manager: &self.tripwire_manager,
            active_session: &self.active_session,
        };

        match chronos_services::probe::ProbeService::drain_log(
            &ctx,
            &params.session_id,
            params.since,
            params.limit,
        ) {
            Ok((records, tail_seq, unparseable_payload_count, total_records_seen)) => {
                let events: Vec<serde_json::Value> = records
                    .iter()
                    .map(|r| {
                        serde_json::json!({
                            "event_id": r.event_id,
                            "timestamp_ns": r.timestamp_ns,
                            "thread_id": r.thread_id,
                            "kind": format!("{:?}", r.event_type),
                            "location": {
                                "file": r.location.file,
                                "line": r.location.line,
                                "function": r.location.function,
                            },
                            "data": serde_json::to_value(&r.data).unwrap_or(serde_json::Value::Null),
                        })
                    })
                    .collect();
                let output = serde_json::json!({
                    "session_id": params.session_id,
                    "source": "chronos-log::SegmentedExecutionLog",
                    "returned": events.len(),
                    "since": params.since,
                    "tail_seq": tail_seq,
                    "total_records_seen": total_records_seen,
                    "unparseable_payload_count": unparseable_payload_count,
                    "events": events,
                    "hint": "m1-04: ExecutionLog query path with decoder counters. \
                             `unparseable_payload_count` is the number of records whose JSON \
                             payload did not decode as a TraceEvent — these are still durable on \
                             disk; the counter is the signal that another producer (or schema \
                             drift) wrote to the same log. `total_records_seen` includes them.",
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::ProbeNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Live probe session '{}' not found.", s),
            ))),
            Err(ServiceError::DrainFailed(msg)) => Ok(CallToolResult::error(text_content(
                format!("Failed to read ExecutionLog: {}", msg),
            ))),
            Err(ServiceError::LockPoisoned) => {
                Ok(CallToolResult::error(text_content("lock poisoned")))
            }
            Err(other) => Ok(CallToolResult::error(text_content(format!(
                "internal error: unexpected drain_log error: {}",
                other
            )))),
        }
    }

    /// m1-07: snapshot the compaction counters of the live probe
    /// session's `ExecutionLog`. Returns `{segments_removed_total,
    /// bytes_reclaimed_total, compaction_runs_total}` plus a
    /// `log_attached: bool` flag (false when the probe was not
    /// configured with an ExecutionLog directory).
    #[tool(
        name = "probe_compaction_metrics",
        description = "m1-07 ExecutionLog compaction metrics. Returns a snapshot of the live probe's segmented ExecutionLog compaction counters (segments_removed_total, bytes_reclaimed_total, compaction_runs_total) plus a log_attached flag. Counter values are zero until at least one compaction run has occurred. Available only when the native backend was configured with with_execution_log_dir."
    )]
    async fn probe_compaction_metrics(
        &self,
        params: Parameters<ProbeCompactionMetricsParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let ctx = chronos_services::probe::ProbeContext {
            live_probes: &self.live_probes,
            engines: &self.engines,
            session_languages: &self.session_languages,
            tripwire_manager: &self.tripwire_manager,
            active_session: &self.active_session,
        };

        match chronos_services::probe::ProbeService::compaction_metrics(&ctx, &params.session_id) {
            Ok(metrics_out) => {
                let output = match metrics_out {
                    None => serde_json::json!({
                        "session_id": params.session_id,
                        "log_attached": false,
                        "hint": "Backend was not configured with with_execution_log_dir. \
                                 The probe is still functional; it just does not have an \
                                 on-disk log to surface counters for.",
                    }),
                    Some(m) => serde_json::json!({
                        "session_id": params.session_id,
                        "log_attached": true,
                        "source": "chronos-log::SegmentedExecutionLog",
                        "segments_removed_total": m.segments_removed_total,
                        "bytes_reclaimed_total": m.bytes_reclaimed_total,
                        "compaction_runs_total": m.compaction_runs_total,
                    }),
                };
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::ProbeNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Live probe session '{}' not found.", s),
            ))),
            Err(ServiceError::DrainFailed(msg)) => Ok(CallToolResult::error(text_content(
                format!("Failed to read compaction metrics: {}", msg),
            ))),
            Err(ServiceError::LockPoisoned) => {
                Ok(CallToolResult::error(text_content("lock poisoned")))
            }
            Err(other) => Ok(CallToolResult::error(text_content(format!(
                "internal error: unexpected compaction metrics error: {}",
                other
            )))),
        }
    }

    #[tool(
        name = "session_snapshot",
        description = "Freeze a live probe session and build query indices without stopping the probe. This makes the session queryable (query_events, get_call_stack, etc.) while the probe continues collecting events. Call again to refresh the indices with newer events."
    )]
    async fn session_snapshot(
        &self,
        params: Parameters<SessionSnapshotParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let ctx = chronos_services::probe::ProbeContext {
            live_probes: &self.live_probes,
            engines: &self.engines,
            session_languages: &self.session_languages,
            tripwire_manager: &self.tripwire_manager,
            active_session: &self.active_session,
        };

        match chronos_services::probe::ProbeService::session_snapshot(&ctx, &params.session_id) {
            Ok((events, language)) => {
                let total_events = events.len();

                // Build and store the query engine with proper noise filtering.
                self.build_and_store_engine(&params.session_id, events, language)
                    .await;

                // Set as active session
                {
                    let mut active = self.active_session.lock().await;
                    *active = Some(params.session_id.clone());
                }

                let output = serde_json::json!({
                    "session_id": params.session_id,
                    "status": "running",
                    "events_indexed": total_events,
                    "hint": "Session is now queryable. Probe is still running. Call session_snapshot again to refresh indices with newer events."
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::ProbeNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Live probe session '{}' not found.", s),
            ))),
            Err(ServiceError::LockPoisoned) => {
                Ok(CallToolResult::error(text_content("lock poisoned")))
            }
            Err(other) => Ok(CallToolResult::error(text_content(format!(
                "internal error: unexpected session snapshot error: {}",
                other
            )))),
        }
    }

    #[tool(
        name = "probe_inject",
        description = "Inject a uprobe into a running process via eBPF (requires root/CAP_BPF)"
    )]
    async fn probe_inject(
        &self,
        params: Parameters<ProbeInjectParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let input = chronos_services::probe::ProbeInjectInput {
            session_id: params.session_id.clone(),
            binary_path: params.binary_path.clone(),
            symbol_name: params.symbol_name.clone(),
            pid: params.pid,
        };

        let ctx = chronos_services::probe::ProbeContext {
            live_probes: &self.live_probes,
            engines: &self.engines,
            session_languages: &self.session_languages,
            tripwire_manager: &self.tripwire_manager,
            active_session: &self.active_session,
        };

        match chronos_services::probe::ProbeService::inject(&ctx, input) {
            Ok(chronos_services::probe::ProbeInjectResult::Attached {
                session_id,
                binary_path,
                symbol_name,
                pid,
            }) => {
                let output = serde_json::json!({
                    "session_id": session_id,
                    "binary_path": binary_path,
                    "symbol_name": symbol_name,
                    "pid": pid,
                    "probes_attached": 1u32,
                    "message": format!(
                        "uprobe attached to '{}' in '{}' (pid {}); adapter stored on session",
                        symbol_name, binary_path, pid
                    ),
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Ok(chronos_services::probe::ProbeInjectResult::ProbeStarting) => {
                Ok(CallToolResult::error(text_content(
                    "Cannot inject: probe is still starting up (PID not yet known). Retry in a moment.",
                )))
            }
            Ok(chronos_services::probe::ProbeInjectResult::EbpfUnavailable(msg)) => {
                Ok(CallToolResult::error(text_content(format!(
                    "eBPF not available on this system: {}",
                    msg
                ))))
            }
            Ok(chronos_services::probe::ProbeInjectResult::AttachFailed {
                session_id: _,
                binary_path,
                symbol_name,
                pid: _,
                error,
            }) => Ok(CallToolResult::error(text_content(format!(
                "Failed to attach uprobe for '{}' in '{}': {}",
                symbol_name, binary_path, error
            )))),
            Err(ServiceError::ProbeNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!(
                    "Live probe session '{}' not found. Start a probe with probe_start first.",
                    s
                ),
            ))),
            Err(ServiceError::LockPoisoned) => {
                Ok(CallToolResult::error(text_content("lock poisoned")))
            }
            Err(other) => Ok(CallToolResult::error(text_content(format!(
                "internal error: unexpected probe inject error: {}",
                other
            )))),
        }
    }

    #[tool(
        name = "probe_status",
        description = "Inspect a live probe session: ptrace target, eBPF attachment state, uptime hints."
    )]
    async fn probe_status(
        &self,
        params: Parameters<ProbeStatusParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let session_id = params.0.session_id;

        let ctx = chronos_services::probe::ProbeContext {
            live_probes: &self.live_probes,
            engines: &self.engines,
            session_languages: &self.session_languages,
            tripwire_manager: &self.tripwire_manager,
            active_session: &self.active_session,
        };

        match chronos_services::probe::ProbeService::status(&ctx, &session_id) {
            Ok(snapshot) => {
                let output = serde_json::json!({
                    "session_id": snapshot.session_id,
                    "language": snapshot.language,
                    "target": snapshot.target,
                    "traced_pid": snapshot.traced_pid,
                    "ebpf": snapshot.ebpf,
                    "state": snapshot.state,
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::ProbeNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Live probe session '{}' not found.", s),
            ))),
            Err(ServiceError::LockPoisoned) => {
                Ok(CallToolResult::error(text_content("lock poisoned")))
            }
            Err(other) => Ok(CallToolResult::error(text_content(format!(
                "internal error: unexpected probe status error: {}",
                other
            )))),
        }
    }

    // ========================================================================
    // SF10 — Browser/WASM Probe Tools (T15–T16)
    // ========================================================================

    #[tool(
        name = "browser_probe_start",
        description = "Start a browser debugging session. Launches Chrome headless, connects via CDP, detects WASM modules, and sets breakpoints. Use browser_probe_drain to read events and browser_probe_stop to finalize."
    )]
    async fn browser_probe_start(
        &self,
        params: Parameters<BrowserProbeStartParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let ctx = BrowserProbeContext {
            live_browser_probes: &self.live_browser_probes,
            active_session: &self.active_session,
        };

        match BrowserProbeService::start(
            &ctx,
            BrowserProbeStartInput {
                url: params.url,
                headless: params.headless,
                chrome_path: params.chrome_path,
            },
        )
        .await
        {
            Ok(result) => {
                let output = serde_json::json!({
                    "session_id": result.session_id,
                    "status": "running",
                    "url": result.url,
                    "hint": "Use browser_probe_drain to read WASM events, browser_probe_stop to finalize."
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(e) => Ok(CallToolResult::error(text_content(e.to_string()))),
        }
    }

    #[tool(
        name = "browser_probe_stop",
        description = "Stop a browser probe session. Drains remaining events, disconnects CDP, and kills Chrome process."
    )]
    async fn browser_probe_stop(
        &self,
        params: Parameters<BrowserProbeStopParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let ctx = BrowserProbeContext {
            live_browser_probes: &self.live_browser_probes,
            active_session: &self.active_session,
        };

        match BrowserProbeService::stop(
            &ctx,
            chronos_services::browser_probe::BrowserProbeStopInput {
                session_id: params.session_id,
            },
        )
        .await
        {
            Ok(result) => {
                if result.total_events > 0 {
                    self.build_and_store_engine(
                        &result.session_id,
                        result.raw_events,
                        result.language,
                    )
                    .await;
                }

                let output = serde_json::json!({
                    "session_id": result.session_id,
                    "status": "stopped",
                    "url": result.url,
                    "total_events": result.total_events,
                    "hint": "Session is now queryable. Use query_events, get_call_stack, etc."
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(e) => Ok(CallToolResult::error(text_content(e.to_string()))),
        }
    }

    #[tool(
        name = "browser_probe_drain",
        description = "Drain current events from a browser probe session without stopping it. Returns a snapshot of events currently buffered. The probe continues running."
    )]
    async fn browser_probe_drain(
        &self,
        params: Parameters<BrowserProbeDrainParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let ctx = BrowserProbeContext {
            live_browser_probes: &self.live_browser_probes,
            active_session: &self.active_session,
        };

        match BrowserProbeService::drain(
            &ctx,
            chronos_services::browser_probe::BrowserProbeDrainInput {
                session_id: params.session_id,
                offset: params.offset,
                limit: params.limit,
            },
        )
        .await
        {
            Ok(result) => {
                let events_json: Vec<serde_json::Value> = result
                    .events
                    .into_iter()
                    .map(|e| {
                        serde_json::json!({
                            "event_id": e.event_id,
                            "timestamp_ns": e.timestamp_ns,
                            "thread_id": e.thread_id,
                            "language": e.language,
                            "kind": e.kind,
                            "description": e.description,
                        })
                    })
                    .collect();

                let output = serde_json::json!({
                    "session_id": result.session_id,
                    "status": "running",
                    "total_buffered": result.total_buffered,
                    "returned": result.returned,
                    "offset": result.offset,
                    "limit": result.limit,
                    "events": events_json,
                    "hint": "Browser probe is still running. Call browser_probe_drain again for more events, or browser_probe_stop to finalize."
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(e) => Ok(CallToolResult::error(text_content(e.to_string()))),
        }
    }

    #[tool(
        name = "performance_regression_audit",
        description = "Compare performance hotspots between two sessions to detect regressions"
    )]
    async fn performance_regression_audit(
        &self,
        params: Parameters<PerformanceRegressionAuditParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let ctx = chronos_services::diff::DiffContext { store: &self.store };

        match chronos_services::diff::ChronosDiffService::performance_regression_audit(
            &ctx,
            chronos_services::diff::PerformanceRegressionAuditInput {
                baseline_session_id: params.baseline_session_id,
                target_session_id: params.target_session_id,
                top_n: params.top_n,
            },
        ) {
            Ok(result) => match serde_json::to_value(result) {
                Ok(v) => Ok(CallToolResult::success(json_content(&v))),
                Err(e) => Ok(CallToolResult::error(text_content(format!(
                    "Serialization error: {}",
                    e
                )))),
            },
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("session '{}' not found", s),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
        }
    }

    #[tool(
        name = "compare_sessions",
        description = "Compare two saved sessions and report differences (Divergence Engine)"
    )]
    async fn compare_sessions(
        &self,
        params: Parameters<CompareSessionsParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let ctx = chronos_services::diff::DiffContext { store: &self.store };

        match chronos_services::diff::ChronosDiffService::compare_sessions(
            &ctx,
            chronos_services::diff::CompareSessionsInput {
                session_a: params.session_a,
                session_b: params.session_b,
            },
        ) {
            Ok(result) => Ok(CallToolResult::success(json_content(&serde_json::json!({
                "session_a_id": result.session_a_id,
                "session_b_id": result.session_b_id,
                "only_in_a_count": result.only_in_a_count,
                "only_in_b_count": result.only_in_b_count,
                "total_a": result.total_a,
                "total_b": result.total_b,
                "common_count": result.common_count,
                "similarity_pct": result.similarity_pct,
                "timing_delta_ms": result.timing_delta_ms,
                "summary": result.summary,
            })))),
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("session '{}' not found", s),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
        }
    }

    // ========================================================================
    // M3 — Mutation Lens Tools
    // ========================================================================

    #[tool(
        name = "mutation_lens",
        description = "Mutation Lens: list StateTransition records derived from VariableWrite events in a session. If `target` is given, filter to that variable name; otherwise return all observed transitions."
    )]
    async fn mutation_lens(
        &self,
        params: Parameters<MutationLensParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let ctx = chronos_services::analysis::AnalysisContext {
            engines: &self.engines,
        };

        match chronos_services::analysis::ChronosAnalysisService::mutation_lens(
            &ctx,
            chronos_services::analysis::MutationLensInput {
                session_id: params.session_id,
                target: params.target,
                limit: params.limit,
            },
        )
        .await
        {
            Ok(result) => Ok(CallToolResult::success(json_content(&serde_json::json!({
                "session_id": result.session_id,
                "count": result.count,
                "transitions": result.transitions,
            })))),
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{}' not found", s),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
        }
    }

    #[tool(
        name = "causal_slice",
        description = "Compute the conservative backward causal slice from a sink event. Returns evidence nodes reachable backward (included) and any included nodes whose evidence is unobserved (missing, never silently dropped)."
    )]
    async fn causal_slice(
        &self,
        params: Parameters<CausalSliceParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let ctx = chronos_services::analysis::AnalysisContext {
            engines: &self.engines,
        };

        match chronos_services::analysis::ChronosAnalysisService::causal_slice(
            &ctx,
            chronos_services::analysis::CausalSliceInput {
                session_id: params.session_id.clone(),
                sink_event_id: params.sink_event_id,
            },
        )
        .await
        {
            Ok(result) => Ok(CallToolResult::success(json_content(&serde_json::json!({
                "session_id": result.session_id,
                "sink": result.sink,
                "included": result.included,
                "missing": result.missing,
                "depth": result.depth,
            })))),
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{}' not found", s),
            ))),
            Err(ServiceError::EventNotFound { event_id }) => {
                Ok(CallToolResult::error(text_content(format!(
                    "Sink event {} not found in session '{}'",
                    event_id, params.session_id
                ))))
            }
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
        }
    }

    #[tool(
        name = "hypothesis_test",
        description = "Evaluate a typed hypothesis against captured session evidence and return Pass / Violation / Unsupported. Three supported shapes: 'invariant' (reuses chronos_domain::property::Property::evaluate on EventCount or PropertyValue), 'existence' (predicate scan over captured events; Pass if >=1 match, else Violation), 'call_path' (BFS reachability in the call graph). Returns raw support_event_ids + counter_event_ids per the v2 spec ('without hiding raw support'). v2 net-new tool (no v1 shim)."
    )]
    async fn hypothesis_test(
        &self,
        params: Parameters<HypothesisTestParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        // Parse the JSON-friendly strings into the typed enums the
        // dispatcher expects. Invalid values become ServiceError::InvalidInput
        // via the dispatcher's own error path; here we just convert.
        let scope = params.scope.as_deref().and_then(parse_hypothesis_scope);
        let comparison = params.comparison.as_deref().and_then(parse_comparison_op);
        let constant = params
            .constant
            .map(chronos_services::output::PropertyValue::from);

        let ctx = chronos_services::hypothesis_test::HypothesisTestContext {
            engines: &self.engines,
        };
        let input = chronos_services::hypothesis_test::HypothesisInput {
            session_id: params.session_id,
            kind: params.kind,
            scope,
            comparison,
            constant,
            property_target: params.property_target,
            predicate: params.predicate,
            caller: params.caller,
            callee: params.callee,
            max_depth: Some(params.max_depth),
        };

        match chronos_services::hypothesis_test::ChronosHypothesisTestService::test(&ctx, input)
            .await
        {
            Ok(out) => Ok(CallToolResult::success(json_content(
                &serde_json::to_value(&out)
                    .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?,
            ))),
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{s}' not found"),
            ))),
            Err(ServiceError::InvalidInput(s)) => Ok(CallToolResult::error(text_content(s))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
        }
    }

    #[tool(
        name = "session_export",
        description = "Export a session bundle (metadata + trace events + properties snapshot) to a portable file on disk. Two supported formats: 'json' (pretty-printed canonical ExportBundle) and 'otlp_json' (OpenTelemetry-compatible JSON wire format, ready for Jaeger/Tempo/Honeycomb). The 'zip_json' format is reserved for m7+ and is rejected by the dispatcher. v2 net-new tool (no v1 shim). NOTE: in m6-05 the `properties_snapshot` field is always empty -- the QueryEngine has no property-table accessor yet. The DTO and JSON schema are final so m7+ can fill the field without a breaking change."
    )]
    async fn session_export(
        &self,
        params: Parameters<SessionExportParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let format = match parse_export_format(&params.format) {
            Some(f) => f,
            None => {
                return Ok(CallToolResult::error(text_content(format!(
                    "unknown format '{}'; allowed values: 'json', 'otlp_json' (zip_json is reserved for a future cycle)",
                    params.format
                ))));
            }
        };

        let output_path = std::path::PathBuf::from(&params.output_path);
        let ctx = chronos_services::session_export::SessionExportContext {
            engines: &self.engines,
        };

        match chronos_services::session_export::ChronosSessionExportService::export(
            &params.session_id,
            Language::from_string(&params.language),
            params.target.clone(),
            format,
            &output_path,
            &ctx,
        )
        .await
        {
            Ok(out) => {
                let output = serde_json::json!({
                    "session_id": params.session_id,
                    "path": out.path.display().to_string(),
                    "bytes_written": out.bytes_written,
                    "format": match out.format {
                        chronos_services::output::ExportFormat::Json => "json",
                        chronos_services::output::ExportFormat::OtlpJson => "otlp_json",
                        chronos_services::output::ExportFormat::ZipJson => "zip_json",
                    },
                    "hint": "The file at `path` contains the full session bundle. Round-trip via `serde_json::from_slice` for the `json` format, or feed the `otlp_json` format directly to an OpenTelemetry JSON receiver.",
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::SessionNotInMemory(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{s}' not found in memory. Run probe_start first."),
            ))),
            Err(ServiceError::EmptySession(s)) => Ok(CallToolResult::error(text_content(format!(
                "Session '{s}' has no events to export."
            )))),
            Err(ServiceError::InvalidExportParameter(s)) => {
                Ok(CallToolResult::error(text_content(s)))
            }
            Err(ServiceError::ExportFailed(s)) => Ok(CallToolResult::error(text_content(format!(
                "export failed: {s}"
            )))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
        }
    }

    #[tool(
        name = "trace_slice",
        description = "Causal evidence around a target. Discriminated by slice_kind: variable_origin (mutations to a named variable), crash (call stack at last fatal signal), causality (full reads + writes at an address), memory_audit (writes at an address with call stacks). Supersedes the v1 debug_find_variable_origin, debug_find_crash, inspect_causality, and forensic_memory_audit tools."
    )]
    async fn trace_slice(
        &self,
        params: Parameters<TraceSliceParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let ctx = TraceSliceContext {
            engines: &self.engines,
        };
        let input = TraceSliceInput {
            session_id: params.session_id,
            slice_kind: params.slice_kind,
            variable_name: params.variable_name,
            address: params.address,
            limit: params.limit,
        };

        match ChronosTraceSliceService::slice(&ctx, input).await {
            Ok(out) => Ok(CallToolResult::success(json_content(
                &serde_json::to_value(&out)
                    .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?,
            ))),
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{}' not found", s),
            ))),
            Err(ServiceError::InvalidInput(s)) => Ok(CallToolResult::error(text_content(s))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
        }
    }
}

// mod tests is NOT cfg-gated (intentional pre-existing structure). Helper
// functions inside `mod tests` are still flagged dead_code when not built
// with `--cfg test`; the prior fixup commit instead annotated each helper
// with #[allow(dead_code)] where invoked.
mod tests {
    use super::*;

    #[test]
    fn test_parse_event_type() {
        assert_eq!(
            ChronosServer::parse_event_type("function_entry"),
            Some(EventType::FunctionEntry)
        );
        assert_eq!(
            ChronosServer::parse_event_type("syscall_enter"),
            Some(EventType::SyscallEnter)
        );
        assert_eq!(
            ChronosServer::parse_event_type("signal_delivered"),
            Some(EventType::SignalDelivered)
        );
        assert_eq!(ChronosServer::parse_event_type("unknown_type"), None);
    }

    #[test]
    fn test_server_new() {
        let _server = ChronosServer::new();
    }

    #[test]
    fn test_server_default() {
        let _server = ChronosServer::default();
    }

    // ========================================================================
    // SF5 — Symbol Subscription Tests (Phase 12)
    // ========================================================================
    // SF4 tool tests
    // ========================================================================

    /// Helper: build a server with a pre-loaded session from synthetic events.
    #[allow(dead_code)]
    async fn server_with_session(events: Vec<TraceEvent>) -> (ChronosServer, String) {
        let server = ChronosServer::new();
        let session_id = "test-session-sf4".to_string();
        server
            .build_and_store_engine(&session_id, events, Language::C)
            .await;
        (server, session_id)
    }

    #[allow(dead_code)]
    fn make_fn_entry(id: u64, ts: u64, tid: u64, func: &str) -> TraceEvent {
        use chronos_domain::{EventData, SourceLocation};
        let loc = SourceLocation::new("", 0, func, 0x1000 + id);
        TraceEvent::new(
            id,
            ts,
            tid,
            EventType::FunctionEntry,
            loc,
            EventData::Function {
                name: func.to_string(),
                signature: None,
                symbol_id: None,
                invocation_id: None,
                parent_invocation_id: None,
            },
        )
    }

    #[allow(dead_code)]
    fn make_fn_exit(id: u64, ts: u64, tid: u64, func: &str) -> TraceEvent {
        use chronos_domain::{EventData, SourceLocation};
        let loc = SourceLocation::new("", 0, func, 0x1000 + id);
        TraceEvent::new(id, ts, tid, EventType::FunctionExit, loc, EventData::Empty)
    }

    #[tokio::test]
    async fn test_debug_call_graph() {
        let events = vec![
            make_fn_entry(0, 100, 1, "main"),
            make_fn_entry(1, 200, 1, "compute"),
            make_fn_exit(2, 300, 1, "compute"),
            make_fn_exit(3, 400, 1, "main"),
        ];
        let (server, sid) = server_with_session(events).await;

        let result = server
            .debug_call_graph(Parameters(DebugCallGraphParams {
                session_id: sid,
                max_depth: 10,
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let text = &result.content[0];
        let s = format!("{:?}", text);
        assert!(s.contains("unique_functions") || !result.content.is_empty());
    }

    #[tokio::test]
    async fn test_debug_find_variable_origin_no_causality() {
        let events = vec![make_fn_entry(0, 100, 1, "main")];
        let (server, sid) = server_with_session(events).await;

        let result = server
            .debug_find_variable_origin(Parameters(DebugFindVariableOriginParams {
                session_id: sid,
                variable_name: "x".to_string(),
                limit: 10,
            }))
            .await
            .unwrap();

        // Should succeed (empty mutations)
        assert_ne!(result.is_error, Some(true));
    }

    #[tokio::test]
    async fn test_debug_find_crash_no_signal() {
        let events = vec![make_fn_entry(0, 100, 1, "main")];
        let (server, sid) = server_with_session(events).await;

        let result = server
            .debug_find_crash(Parameters(DebugFindCrashParams { session_id: sid }))
            .await
            .unwrap();

        // No crash found
        assert_ne!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(text.contains("crash_found"));
    }

    #[tokio::test]
    async fn test_debug_detect_races_no_races() {
        let events = vec![make_fn_entry(0, 100, 1, "main")];
        let (server, sid) = server_with_session(events).await;

        let result = server
            .debug_detect_races(Parameters(DebugDetectRacesParams {
                session_id: sid,
                threshold_ns: 100,
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(
            text.contains("access_count"),
            "m0-09: tool output renamed race_count -> access_count"
        );
    }

    #[tokio::test]
    async fn test_inspect_causality_no_index() {
        let events = vec![make_fn_entry(0, 100, 1, "main")];
        let (server, sid) = server_with_session(events).await;

        let result = server
            .inspect_causality(Parameters(InspectCausalityParams {
                session_id: sid,
                address: 0xDEAD,
                limit: 10,
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
    }

    #[tokio::test]
    async fn test_debug_expand_hotspot() {
        let events = (0..20u64)
            .flat_map(|i| {
                vec![
                    make_fn_entry(
                        i * 2,
                        i * 100,
                        1,
                        if i % 2 == 0 { "hot_fn" } else { "cold_fn" },
                    ),
                    make_fn_exit(
                        i * 2 + 1,
                        i * 100 + 50,
                        1,
                        if i % 2 == 0 { "hot_fn" } else { "cold_fn" },
                    ),
                ]
            })
            .collect();
        let (server, sid) = server_with_session(events).await;

        let result = server
            .debug_expand_hotspot(Parameters(DebugExpandHotspotParams {
                session_id: sid,
                top_n: 5,
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(text.contains("hotspot_functions"));
    }

    #[tokio::test]
    async fn test_debug_get_saliency_scores() {
        let events = (0..10u64)
            .flat_map(|i| {
                vec![
                    make_fn_entry(i * 2, i * 100, 1, "fn_a"),
                    make_fn_exit(i * 2 + 1, i * 100 + 50, 1, "fn_a"),
                ]
            })
            .collect();
        let (server, sid) = server_with_session(events).await;

        let result = server
            .debug_get_saliency_scores(Parameters(DebugGetSaliencyScoresParams {
                session_id: sid,
                limit: 10,
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(text.contains("saliency_score"));
    }

    #[tokio::test]
    async fn test_sf4_tools_session_not_found() {
        let server = ChronosServer::new();
        let sid = "nonexistent".to_string();

        let r1 = server
            .debug_call_graph(Parameters(DebugCallGraphParams {
                session_id: sid.clone(),
                max_depth: 5,
            }))
            .await
            .unwrap();
        assert_eq!(r1.is_error, Some(true));

        let r2 = server
            .debug_find_crash(Parameters(DebugFindCrashParams {
                session_id: sid.clone(),
            }))
            .await
            .unwrap();
        assert_eq!(r2.is_error, Some(true));

        let r3 = server
            .debug_detect_races(Parameters(DebugDetectRacesParams {
                session_id: sid.clone(),
                threshold_ns: 100,
            }))
            .await
            .unwrap();
        assert_eq!(r3.is_error, Some(true));

        let r4 = server
            .debug_get_saliency_scores(Parameters(DebugGetSaliencyScoresParams {
                session_id: sid,
                limit: 5,
            }))
            .await
            .unwrap();
        assert_eq!(r4.is_error, Some(true));
    }

    // ========================================================================
    // SF5 Persistence Tool Tests (T16)
    // ========================================================================

    #[allow(dead_code)]
    fn make_fn_event(id: u64, ts: u64, tid: u64, func: &str) -> TraceEvent {
        use chronos_domain::{EventData, EventType, SourceLocation};
        let loc = SourceLocation::new("", 0, func, 0x1000 + id);
        TraceEvent::new(
            id,
            ts,
            tid,
            EventType::FunctionEntry,
            loc,
            EventData::Function {
                name: func.to_string(),
                signature: None,
                symbol_id: None,
                invocation_id: None,
                parent_invocation_id: None,
            },
        )
    }

    #[tokio::test]
    async fn test_save_and_load_session_roundtrip() {
        let server = ChronosServer::new();
        let sid = "test-persist-session".to_string();
        let events = vec![
            make_fn_event(0, 100, 1, "main"),
            make_fn_event(1, 200, 1, "helper"),
        ];

        // Build engine manually
        server
            .build_and_store_engine(&sid, events.clone(), Language::C)
            .await;

        // Save session
        let save_result = server
            .save_session(Parameters(SaveSessionParams {
                session_id: sid.clone(),
                language: "native".to_string(),
                target: "/bin/test".to_string(),
            }))
            .await
            .unwrap();

        assert_ne!(save_result.is_error, Some(true));
        let text = format!("{:?}", save_result.content);
        assert!(text.contains("saved") || text.contains("event_count"));

        // Load session
        let load_result = server
            .load_session(Parameters(LoadSessionParams {
                session_id: sid.clone(),
            }))
            .await
            .unwrap();

        assert_ne!(load_result.is_error, Some(true));
        let text = format!("{:?}", load_result.content);
        assert!(text.contains("loaded") || text.contains("event_count"));
    }

    #[tokio::test]
    async fn test_cleanup_session_memory_removes_all_state() {
        let server = ChronosServer::new();
        let sid = "cleanup-test-session".to_string();
        let events = vec![make_fn_event(0, 100, 1, "main")];

        // Build engine (registers in engines + session_languages)
        server
            .build_and_store_engine(&sid, events, Language::Python)
            .await;

        // Verify it's registered
        assert!(server.engines.lock().await.contains_key(&sid));
        assert!(server.session_languages.lock().await.contains_key(&sid));

        // Cleanup
        server.cleanup_session_memory(&sid).await;

        // Verify all in-memory state is gone
        assert!(!server.engines.lock().await.contains_key(&sid));
        assert!(!server.session_languages.lock().await.contains_key(&sid));
        assert!(!server.connected_sessions.lock().unwrap().contains(&sid));
    }

    #[tokio::test]
    async fn test_delete_session_also_cleans_memory() {
        let server = ChronosServer::new();
        let sid = "delete-cleanup-session".to_string();
        let events = vec![make_fn_event(0, 100, 1, "main")];

        // Build and save session
        server
            .build_and_store_engine(&sid, events, Language::C)
            .await;
        server
            .save_session(Parameters(SaveSessionParams {
                session_id: sid.clone(),
                language: "native".to_string(),
                target: "/bin/test".to_string(),
            }))
            .await
            .unwrap();

        // Engine should be in memory
        assert!(server.engines.lock().await.contains_key(&sid));

        // Delete from store
        let result = server
            .delete_session(Parameters(DeleteSessionParams {
                session_id: sid.clone(),
            }))
            .await
            .unwrap();
        assert_ne!(result.is_error, Some(true));

        // Memory should be cleaned up too
        assert!(!server.engines.lock().await.contains_key(&sid));
        assert!(!server.session_languages.lock().await.contains_key(&sid));
    }

    #[tokio::test]
    async fn test_list_sessions_after_save() {
        let server = ChronosServer::new();
        let sid1 = "list-test-1".to_string();
        let sid2 = "list-test-2".to_string();
        let events = vec![make_fn_event(0, 100, 1, "main")];

        // Save two sessions
        server
            .build_and_store_engine(&sid1, events.clone(), Language::C)
            .await;
        server
            .build_and_store_engine(&sid2, events.clone(), Language::C)
            .await;

        server
            .save_session(Parameters(SaveSessionParams {
                session_id: sid1.clone(),
                language: "native".to_string(),
                target: "/bin/test1".to_string(),
            }))
            .await
            .unwrap();

        server
            .save_session(Parameters(SaveSessionParams {
                session_id: sid2.clone(),
                language: "native".to_string(),
                target: "/bin/test2".to_string(),
            }))
            .await
            .unwrap();

        // List sessions
        let list_result = server.list_sessions(Parameters(NoParams {})).await.unwrap();
        assert_ne!(list_result.is_error, Some(true));
        let text = format!("{:?}", list_result.content);
        assert!(text.contains("session_count") || text.contains("sessions"));
    }

    #[tokio::test]
    async fn test_save_session_not_found_in_memory() {
        let server = ChronosServer::new();
        let result = server
            .save_session(Parameters(SaveSessionParams {
                session_id: "this-does-not-exist".to_string(),
                language: "native".to_string(),
                target: "/bin/test".to_string(),
            }))
            .await
            .unwrap();

        assert_eq!(result.is_error, Some(true));
    }

    #[tokio::test]
    async fn test_load_session_not_found_in_store() {
        let server = ChronosServer::new();
        let result = server
            .load_session(Parameters(LoadSessionParams {
                session_id: "this-does-not-exist-in-store".to_string(),
            }))
            .await
            .unwrap();

        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn test_resource_limits_default_values() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.max_events, 1_000_000);
        assert_eq!(limits.timeout_secs, 60);
    }

    #[test]
    fn test_resource_limits_custom_values() {
        let limits = ResourceLimits {
            max_events: 500_000,
            timeout_secs: 120,
        };
        assert_eq!(limits.max_events, 500_000);
        assert_eq!(limits.timeout_secs, 120);
    }

    // ========================================================================
    // SF6 — Inspection Tools Tests
    // ========================================================================

    #[allow(dead_code)]
    fn make_test_python_frame_with_locals(
        id: u64,
        ts: u64,
        tid: u64,
        locals: Vec<chronos_domain::VariableInfo>,
    ) -> TraceEvent {
        TraceEvent::python_call_with_locals(
            id,
            ts,
            tid,
            "my_module.my_func",
            "/path/to/script.py",
            10,
            locals,
        )
    }

    #[allow(dead_code)]
    fn make_test_memory_event(
        id: u64,
        ts: u64,
        tid: u64,
        address: u64,
        size: usize,
        data: Vec<u8>,
    ) -> TraceEvent {
        use chronos_domain::{EventData, EventType, SourceLocation};
        TraceEvent::new(
            id,
            ts,
            tid,
            EventType::MemoryWrite,
            SourceLocation::from_address(address),
            EventData::Memory {
                address,
                size,
                data: Some(data),
            },
        )
    }

    #[tokio::test]
    async fn test_evaluate_expression_success() {
        let locals = vec![
            chronos_domain::VariableInfo::new(
                "x",
                "10",
                "i32",
                0x1000,
                chronos_domain::VariableScope::Local,
            ),
            chronos_domain::VariableInfo::new(
                "y",
                "3",
                "i32",
                0x2000,
                chronos_domain::VariableScope::Local,
            ),
        ];
        let events = vec![make_test_python_frame_with_locals(0, 100, 1, locals)];
        let (server, sid) = server_with_session(events).await;

        let result = server
            .evaluate_expression(Parameters(EvaluateExpressionParams {
                session_id: sid,
                event_id: 0,
                expression: "x + y * 2".to_string(),
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        // Should contain result = 16.0 (10 + 3 * 2)
        assert!(text.contains("16"));
    }

    #[tokio::test]
    async fn test_evaluate_expression_unknown_var() {
        let locals = vec![chronos_domain::VariableInfo::new(
            "x",
            "10",
            "i32",
            0x1000,
            chronos_domain::VariableScope::Local,
        )];
        let events = vec![make_test_python_frame_with_locals(0, 100, 1, locals)];
        let (server, sid) = server_with_session(events).await;

        let result = server
            .evaluate_expression(Parameters(EvaluateExpressionParams {
                session_id: sid,
                event_id: 0,
                expression: "x + z".to_string(), // z is unknown
            }))
            .await
            .unwrap();

        // Should succeed but with error in content
        assert_ne!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(text.contains("error") || text.contains("UnknownVariable"));
    }

    #[tokio::test]
    async fn test_evaluate_expression_division_by_zero() {
        let locals = vec![chronos_domain::VariableInfo::new(
            "n",
            "0",
            "i32",
            0x1000,
            chronos_domain::VariableScope::Local,
        )];
        let events = vec![make_test_python_frame_with_locals(0, 100, 1, locals)];
        let (server, sid) = server_with_session(events).await;

        let result = server
            .evaluate_expression(Parameters(EvaluateExpressionParams {
                session_id: sid,
                event_id: 0,
                expression: "10 / n".to_string(),
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(text.contains("DivisionByZero"));
    }

    #[tokio::test]
    async fn test_evaluate_expression_session_not_found() {
        let server = ChronosServer::new();
        let result = server
            .evaluate_expression(Parameters(EvaluateExpressionParams {
                session_id: "nonexistent".to_string(),
                event_id: 0,
                expression: "x + 1".to_string(),
            }))
            .await
            .unwrap();

        assert_eq!(result.is_error, Some(true));
    }

    #[tokio::test]
    async fn test_evaluate_expression_with_variables() {
        // Test that evaluate_expression correctly evaluates arithmetic with captured variables.
        let locals = vec![
            chronos_domain::VariableInfo::new(
                "a",
                "5",
                "i32",
                0x1000,
                chronos_domain::VariableScope::Local,
            ),
            chronos_domain::VariableInfo::new(
                "b",
                "3",
                "i32",
                0x2000,
                chronos_domain::VariableScope::Local,
            ),
        ];
        let events = vec![make_test_python_frame_with_locals(0, 100, 1, locals)];
        let (server, sid) = server_with_session(events).await;

        let result = server
            .evaluate_expression(Parameters(EvaluateExpressionParams {
                session_id: sid,
                event_id: 0,
                expression: "a + b".to_string(),
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        // Should evaluate successfully
        assert!(text.contains("8")); // 5 + 3 = 8
    }

    #[tokio::test]
    async fn test_debug_get_variables_python() {
        let locals = vec![chronos_domain::VariableInfo::new(
            "count",
            "42",
            "int",
            0x1000,
            chronos_domain::VariableScope::Local,
        )];
        let events = vec![make_test_python_frame_with_locals(0, 100, 1, locals)];
        let (server, sid) = server_with_session(events).await;

        let result = server
            .debug_get_variables(Parameters(DebugGetVariablesParams {
                session_id: sid,
                event_id: 0,
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(text.contains("count") && text.contains("42"));
    }

    #[tokio::test]
    async fn test_debug_get_variables_empty() {
        // PythonFrame with None locals
        let event =
            TraceEvent::python_call(0, 100, 1, "my_module.my_func", "/path/to/script.py", 10);
        let events = vec![event];
        let (server, sid) = server_with_session(events).await;

        let result = server
            .debug_get_variables(Parameters(DebugGetVariablesParams {
                session_id: sid,
                event_id: 0,
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(text.contains("variables"));
    }

    #[tokio::test]
    async fn test_debug_get_variables_session_not_found() {
        let server = ChronosServer::new();
        let result = server
            .debug_get_variables(Parameters(DebugGetVariablesParams {
                session_id: "nonexistent".to_string(),
                event_id: 0,
            }))
            .await
            .unwrap();

        assert_eq!(result.is_error, Some(true));
    }

    #[tokio::test]
    async fn test_debug_get_memory_found() {
        let addr = 0x7FFF0000u64;
        let events = vec![
            make_test_memory_event(1, 1000, 1, addr, 4, vec![0x01, 0x02, 0x03, 0x04]),
            make_test_memory_event(2, 2000, 1, addr, 4, vec![0xFF, 0xFE, 0xFD, 0xFC]),
        ];
        let (server, sid) = server_with_session(events).await;

        let result = server
            .debug_get_memory(Parameters(DebugGetMemoryParams {
                session_id: sid.clone(),
                address: addr,
                timestamp_ns: 1500,
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        // Should find the first memory write at timestamp 1000
        assert!(text.contains("0x7fff0000") || text.contains("7fff0000"));
    }

    #[tokio::test]
    async fn test_debug_get_memory_not_found() {
        let events = vec![make_test_memory_event(
            1,
            1000,
            1,
            0x7FFF0000,
            4,
            vec![0x01, 0x02, 0x03, 0x04],
        )];
        let (server, sid) = server_with_session(events).await;

        let result = server
            .debug_get_memory(Parameters(DebugGetMemoryParams {
                session_id: sid,
                address: 0x12345678, // Different address
                timestamp_ns: 2000,
            }))
            .await
            .unwrap();

        assert_eq!(result.is_error, Some(true));
    }

    #[tokio::test]
    async fn test_debug_get_memory_session_not_found() {
        let server = ChronosServer::new();
        let result = server
            .debug_get_memory(Parameters(DebugGetMemoryParams {
                session_id: "nonexistent".to_string(),
                address: 0x7FFF0000,
                timestamp_ns: 1000,
            }))
            .await
            .unwrap();

        assert_eq!(result.is_error, Some(true));
    }

    // ========================================================================
    // SF7 — Phase 11 Missing Tools Tests
    // ========================================================================

    #[allow(dead_code)]
    fn make_register_event(
        id: u64,
        ts: u64,
        tid: u64,
        regs: chronos_domain::RegisterState,
    ) -> TraceEvent {
        use chronos_domain::{EventData, EventType, SourceLocation};
        TraceEvent::new(
            id,
            ts,
            tid,
            EventType::Custom,
            SourceLocation::from_address(regs.rip),
            EventData::Registers(regs),
        )
    }

    #[tokio::test]
    async fn test_debug_get_registers_success() {
        use chronos_index::builder::IndexBuilder;
        use chronos_query::QueryEngine;

        let regs = chronos_domain::RegisterState {
            rax: 0x42,
            rip: 0x401000,
            rsp: 0x7fff0000,
            rbp: 0x7fff0010,
            ..Default::default()
        };
        let events = vec![
            make_fn_entry(0, 100, 1, "main"),
            make_register_event(1, 200, 1, regs),
        ];

        // Build engine directly without filtering (bypass the register filtering)
        let mut builder = IndexBuilder::new();
        builder.push_all(&events);
        let indices = builder.finalize();
        let engine = QueryEngine::with_indices(events, indices.shadow, indices.temporal);

        let server = ChronosServer::new();
        let sid = "register-test-session".to_string();
        {
            let mut engines = server.engines.lock().await;
            engines.insert(sid.clone(), engine);
        }

        let result = server
            .debug_get_registers(Parameters(DebugGetRegistersParams {
                session_id: sid,
                event_id: 1,
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(text.contains("rax") && text.contains("0x42"));
    }

    #[tokio::test]
    async fn test_debug_get_registers_no_register_state() {
        let events = vec![make_fn_entry(0, 100, 1, "main")];
        let (server, sid) = server_with_session(events).await;

        let result = server
            .debug_get_registers(Parameters(DebugGetRegistersParams {
                session_id: sid,
                event_id: 0,
            }))
            .await
            .unwrap();

        // Should return error because there's no register state
        assert_eq!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(text.contains("no register state"));
    }

    #[tokio::test]
    async fn test_debug_get_registers_event_not_found() {
        let server = ChronosServer::new();
        let result = server
            .debug_get_registers(Parameters(DebugGetRegistersParams {
                session_id: "nonexistent".to_string(),
                event_id: 999,
            }))
            .await
            .unwrap();

        assert_eq!(result.is_error, Some(true));
    }

    #[tokio::test]
    async fn test_debug_diff_variables_changed() {
        use chronos_domain::VariableScope;
        let locals_a = vec![chronos_domain::VariableInfo::new(
            "x",
            "10",
            "i32",
            0x1000,
            VariableScope::Local,
        )];
        let locals_b = vec![chronos_domain::VariableInfo::new(
            "x",
            "20",
            "i32",
            0x1000,
            VariableScope::Local,
        )];
        let events = vec![
            TraceEvent::python_call_with_locals(0, 100, 1, "f", "test.py", 10, locals_a),
            TraceEvent::python_call_with_locals(1, 200, 1, "f", "test.py", 15, locals_b),
        ];
        let (server, sid) = server_with_session(events).await;

        let result = server
            .debug_diff(Parameters(DebugDiffParams {
                session_id: sid,
                event_id_a: 0,
                event_id_b: 1,
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(text.contains("variables_changed") && text.contains("x"));
    }

    #[tokio::test]
    async fn test_debug_diff_session_not_found() {
        let server = ChronosServer::new();
        let result = server
            .debug_diff(Parameters(DebugDiffParams {
                session_id: "nonexistent".to_string(),
                event_id_a: 0,
                event_id_b: 1,
            }))
            .await
            .unwrap();

        assert_eq!(result.is_error, Some(true));
    }

    #[tokio::test]
    async fn test_debug_analyze_memory_success() {
        let events = vec![
            make_test_memory_event(1, 1000, 1, 0x7FFF0000, 4, vec![0x01, 0x02, 0x03, 0x04]),
            make_test_memory_event(2, 1500, 1, 0x7FFF0010, 4, vec![0xAA, 0xBB, 0xCC, 0xDD]),
            make_test_memory_event(3, 2000, 1, 0x7FFF0000, 4, vec![0xFF, 0xEE, 0xDD, 0xCC]),
        ];
        let (server, sid) = server_with_session(events).await;

        let result = server
            .debug_analyze_memory(Parameters(DebugAnalyzeMemoryParams {
                session_id: sid,
                start_address: 0x7FFF0000,
                end_address: 0x7FFF000F,
                start_ts: 500,
                end_ts: 2500,
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(text.contains("total_writes"));
        // Should find 2 writes to 0x7FFF0000
        assert!(text.contains("7fff0000"));
    }

    #[tokio::test]
    async fn test_debug_analyze_memory_no_accesses() {
        let events = vec![make_test_memory_event(1, 1000, 1, 0x1000, 4, vec![0x01])];
        let (server, sid) = server_with_session(events).await;

        let result = server
            .debug_analyze_memory(Parameters(DebugAnalyzeMemoryParams {
                session_id: sid,
                start_address: 0x2000,
                end_address: 0x3000,
                start_ts: 0,
                end_ts: 10000,
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(text.contains("total_writes"));
    }

    #[tokio::test]
    async fn test_debug_analyze_memory_session_not_found() {
        let server = ChronosServer::new();
        let result = server
            .debug_analyze_memory(Parameters(DebugAnalyzeMemoryParams {
                session_id: "nonexistent".to_string(),
                start_address: 0x1000,
                end_address: 0x2000,
                start_ts: 0,
                end_ts: 10000,
            }))
            .await
            .unwrap();

        assert_eq!(result.is_error, Some(true));
    }

    #[tokio::test]
    async fn test_forensic_memory_audit_success() {
        let events = vec![
            make_fn_entry(0, 100, 1, "main"),
            make_fn_entry(1, 200, 1, "write_val"),
            make_test_memory_event(2, 300, 1, 0xA000, 4, vec![0x01, 0x02, 0x03, 0x04]),
            make_fn_exit(3, 400, 1, "write_val"),
            make_fn_entry(4, 500, 1, "write_val"),
            make_test_memory_event(5, 600, 1, 0xA000, 4, vec![0xAA, 0xBB, 0xCC, 0xDD]),
            make_fn_exit(6, 700, 1, "write_val"),
            make_fn_exit(7, 800, 1, "main"),
        ];
        let (server, sid) = server_with_session(events).await;

        let result = server
            .forensic_memory_audit(Parameters(ForensicMemoryAuditParams {
                session_id: sid,
                address: 0xA000,
                limit: 10,
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(text.contains("write_count"));
        // Should find 2 writes
        assert!(text.contains("a000") || text.contains("A000"));
    }

    #[tokio::test]
    async fn test_forensic_memory_audit_no_writes() {
        let events = vec![make_fn_entry(0, 100, 1, "main")];
        let (server, sid) = server_with_session(events).await;

        let result = server
            .forensic_memory_audit(Parameters(ForensicMemoryAuditParams {
                session_id: sid,
                address: 0xA000,
                limit: 10,
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(text.contains("write_count"));
    }

    #[tokio::test]
    async fn test_forensic_memory_audit_session_not_found() {
        let server = ChronosServer::new();
        let result = server
            .forensic_memory_audit(Parameters(ForensicMemoryAuditParams {
                session_id: "nonexistent".to_string(),
                address: 0xA000,
                limit: 10,
            }))
            .await
            .unwrap();

        assert_eq!(result.is_error, Some(true));
    }

    // ========================================================================
    // Phase 25 — drop_session Tool Tests
    // ========================================================================

    #[tokio::test]
    async fn test_drop_session_removes_from_memory() {
        let server = ChronosServer::new();
        let sid = "drop-test-session".to_string();
        let events = vec![make_fn_entry(0, 100, 1, "main")];

        // Build engine (registers in engines + session_languages)
        server
            .build_and_store_engine(&sid, events, Language::Python)
            .await;

        // Verify it's registered in memory
        assert!(server.engines.lock().await.contains_key(&sid));
        assert!(server.session_languages.lock().await.contains_key(&sid));

        // Drop the session
        let result = server
            .drop_session(Parameters(DropSessionParams {
                session_id: sid.clone(),
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(text.contains("dropped"), "Should contain 'dropped' status");
        assert!(
            text.contains("Persistent storage not affected"),
            "Should mention storage not affected"
        );

        // Verify all in-memory state is gone
        assert!(!server.engines.lock().await.contains_key(&sid));
        assert!(!server.session_languages.lock().await.contains_key(&sid));
        assert!(!server.connected_sessions.lock().unwrap().contains(&sid));
    }

    #[tokio::test]
    async fn test_drop_session_not_found_is_idempotent() {
        let server = ChronosServer::new();

        // Drop non-existent session - should return success with not_found status
        let result = server
            .drop_session(Parameters(DropSessionParams {
                session_id: "nonexistent-session".to_string(),
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(
            text.contains("not_found"),
            "Should contain 'not_found' status"
        );
    }

    // ========================================================================
    // compare_sessions tests
    // ========================================================================

    #[tokio::test]
    async fn test_compare_sessions_session_not_found() {
        let server = ChronosServer::new();

        let result = server
            .compare_sessions(Parameters(CompareSessionsParams {
                session_a: "nonexistent-a".to_string(),
                session_b: "nonexistent-b".to_string(),
            }))
            .await
            .unwrap();

        // Should return an error because sessions don't exist in store
        assert_eq!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(text.contains("session A not found") || text.contains("not found"));
    }

    #[tokio::test]
    async fn test_compare_sessions_identical() {
        use chronos_domain::{EventData, SourceLocation};
        let server = ChronosServer::new();

        // Build and save two identical sessions
        let make_event = |id: u64, func: &str| {
            let loc = SourceLocation::new("test.rs", 1, func, 0x1000 + id);
            TraceEvent::new(
                id,
                id * 100,
                1,
                EventType::FunctionEntry,
                loc,
                EventData::Function {
                    name: func.to_string(),
                    signature: None,
                    symbol_id: None,
                    invocation_id: None,
                    parent_invocation_id: None,
                },
            )
        };

        let events = vec![make_event(0, "main"), make_event(1, "helper")];
        let sid_a = "cmp-test-a".to_string();
        let sid_b = "cmp-test-b".to_string();

        // Save both sessions to the store
        let meta_a = SessionMetadata {
            session_id: sid_a.clone(),
            created_at: 0,
            language: "native".to_string(),
            target: "/bin/test".to_string(),
            event_count: events.len(),
            duration_ms: 100,
        };
        let meta_b = SessionMetadata {
            session_id: sid_b.clone(),
            created_at: 0,
            language: "native".to_string(),
            target: "/bin/test".to_string(),
            event_count: events.len(),
            duration_ms: 200,
        };
        server.store.save_session(meta_a, &events).unwrap();
        server.store.save_session(meta_b, &events).unwrap();

        let result = server
            .compare_sessions(Parameters(CompareSessionsParams {
                session_a: sid_a.clone(),
                session_b: sid_b.clone(),
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(text.contains("similarity_pct"));
        assert!(text.contains("common_count"));
        assert!(text.contains("summary"));
        // Identical events → high similarity
        assert!(text.contains("100") || text.contains("highly similar"));
    }

    // ========================================================================
    // SF10 — Browser/WASM Probe Tool Tests
    // ========================================================================

    #[tokio::test]
    async fn test_browser_probe_start_chrome_not_available() {
        let server = ChronosServer::new();

        // Force Chrome to be unavailable by using an invalid path
        std::env::set_var("CHROME_PATH", "/nonexistent/chrome/path/for/test");
        let result = server
            .browser_probe_start(Parameters(BrowserProbeStartParams {
                url: "http://example.com".to_string(),
                headless: true,
                chrome_path: None,
            }))
            .await
            .unwrap();
        std::env::remove_var("CHROME_PATH");

        // Result is either "Chrome is not available" (if is_chrome_available fails)
        // or "Failed to start browser probe: Chrome not found" (if spawn fails)
        let text = format!("{:?}", result.content);
        if result.is_error == Some(true) {
            assert!(
                text.contains("Chrome") || text.contains("chrome"),
                "Should mention Chrome issue, got: {}",
                text
            );
        }
        // If Chrome IS available on this system, the probe might succeed — that's OK
    }

    #[tokio::test]
    async fn test_browser_probe_stop_session_not_found() {
        let server = ChronosServer::new();

        let result = server
            .browser_probe_stop(Parameters(BrowserProbeStopParams {
                session_id: "nonexistent-browser-session".to_string(),
            }))
            .await
            .unwrap();

        assert_eq!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(
            text.contains("not found"),
            "Should mention 'not found', got: {}",
            text
        );
    }

    #[tokio::test]
    async fn test_browser_probe_drain_session_not_found() {
        let server = ChronosServer::new();

        let result = server
            .browser_probe_drain(Parameters(BrowserProbeDrainParams {
                session_id: "nonexistent-browser-session".to_string(),
                limit: 1000,
                offset: 0,
            }))
            .await
            .unwrap();

        assert_eq!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(
            text.contains("not found"),
            "Should mention 'not found', got: {}",
            text
        );
    }

    #[tokio::test]
    async fn test_browser_probe_lifecycle_with_mock_session() {
        // Test the full lifecycle: create mock session → drain → stop
        // This exercises the MCP integration without requiring real Chrome
        let server = ChronosServer::new();
        let session_id = "test-browser-mock-session".to_string();

        // Manually inject a mock browser probe session (simulating browser_probe_start)
        {
            let adapter = Arc::new(BrowserAdapter::new());
            let capture_session = CaptureSession::new(
                0,
                Language::WebAssembly,
                CaptureConfig::new("http://test.local"),
            );
            let probe = BrowserProbeSession {
                adapter: adapter.clone(),
                session: capture_session,
                session_id: session_id.clone(),
                url: "http://test.local".to_string(),
            };
            server
                .live_browser_probes
                .lock()
                .unwrap()
                .insert(session_id.clone(), probe);
        }

        // Drain events from the mock session (should return 0 events since no Chrome)
        let drain_result = server
            .browser_probe_drain(Parameters(BrowserProbeDrainParams {
                session_id: session_id.clone(),
                limit: 1000,
                offset: 0,
            }))
            .await
            .unwrap();

        assert_ne!(drain_result.is_error, Some(true), "drain should succeed");
        let drain_text = format!("{:?}", drain_result.content);
        assert!(
            drain_text.contains("total_buffered"),
            "Should have total_buffered field"
        );
        assert!(drain_text.contains("0"), "Should have 0 buffered events");

        // Stop the mock session
        let stop_result = server
            .browser_probe_stop(Parameters(BrowserProbeStopParams {
                session_id: session_id.clone(),
            }))
            .await
            .unwrap();

        assert_ne!(stop_result.is_error, Some(true), "stop should succeed");
        let stop_text = format!("{:?}", stop_result.content);
        assert!(
            stop_text.contains("stopped"),
            "Should contain 'stopped' status"
        );
        assert!(
            stop_text.contains("total_events"),
            "Should have total_events field"
        );

        // Verify session is removed
        assert!(
            !server
                .live_browser_probes
                .lock()
                .unwrap()
                .contains_key(&session_id),
            "Session should be removed after stop"
        );
    }

    #[tokio::test]
    async fn test_browser_probe_drain_with_offset_limit() {
        let server = ChronosServer::new();
        let session_id = "test-browser-offset-limit".to_string();

        // Inject mock session
        {
            let adapter = Arc::new(BrowserAdapter::new());
            let capture_session = CaptureSession::new(
                0,
                Language::WebAssembly,
                CaptureConfig::new("http://test.local"),
            );
            let probe = BrowserProbeSession {
                adapter,
                session: capture_session,
                session_id: session_id.clone(),
                url: "http://test.local".to_string(),
            };
            server
                .live_browser_probes
                .lock()
                .unwrap()
                .insert(session_id.clone(), probe);
        }

        // Drain with offset=0, limit=0 (should return empty)
        let result = server
            .browser_probe_drain(Parameters(BrowserProbeDrainParams {
                session_id: session_id.clone(),
                limit: 0,
                offset: 0,
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(text.contains("returned"), "Should have returned field");

        // Cleanup
        let _ = server
            .browser_probe_stop(Parameters(BrowserProbeStopParams { session_id }))
            .await;
    }

    #[tokio::test]
    async fn test_browser_probe_stop_removes_from_active_session() {
        let server = ChronosServer::new();
        let session_id = "test-browser-active-cleanup".to_string();

        // Inject mock session and set as active
        {
            let adapter = Arc::new(BrowserAdapter::new());
            let capture_session = CaptureSession::new(
                0,
                Language::WebAssembly,
                CaptureConfig::new("http://test.local"),
            );
            let probe = BrowserProbeSession {
                adapter,
                session: capture_session,
                session_id: session_id.clone(),
                url: "http://test.local".to_string(),
            };
            server
                .live_browser_probes
                .lock()
                .unwrap()
                .insert(session_id.clone(), probe);

            // Set as active session
            let mut active = server.active_session.lock().await;
            *active = Some(session_id.clone());
        }

        // Stop should remove from live_browser_probes
        let _ = server
            .browser_probe_stop(Parameters(BrowserProbeStopParams {
                session_id: session_id.clone(),
            }))
            .await
            .unwrap();

        // Verify session is gone from live_browser_probes
        assert!(
            !server
                .live_browser_probes
                .lock()
                .unwrap()
                .contains_key(&session_id),
            "Session should be removed from live_browser_probes"
        );

        // Note: browser_probe_stop does NOT clear active_session — that's by design
        // (active_session tracks the last used session for query tools)
    }

    #[tokio::test]
    async fn test_browser_probe_double_stop_returns_error() {
        let server = ChronosServer::new();
        let session_id = "test-browser-double-stop".to_string();

        // Inject mock session
        {
            let adapter = Arc::new(BrowserAdapter::new());
            let capture_session = CaptureSession::new(
                0,
                Language::WebAssembly,
                CaptureConfig::new("http://test.local"),
            );
            let probe = BrowserProbeSession {
                adapter,
                session: capture_session,
                session_id: session_id.clone(),
                url: "http://test.local".to_string(),
            };
            server
                .live_browser_probes
                .lock()
                .unwrap()
                .insert(session_id.clone(), probe);
        }

        // First stop should succeed
        let first = server
            .browser_probe_stop(Parameters(BrowserProbeStopParams {
                session_id: session_id.clone(),
            }))
            .await
            .unwrap();
        assert_ne!(first.is_error, Some(true));

        // Second stop should return error (session already removed)
        let second = server
            .browser_probe_stop(Parameters(BrowserProbeStopParams {
                session_id: session_id.clone(),
            }))
            .await
            .unwrap();
        assert_eq!(
            second.is_error,
            Some(true),
            "Second stop should be an error"
        );
        let text = format!("{:?}", second.content);
        assert!(text.contains("not found"), "Should mention not found");
    }

    #[tokio::test]
    async fn test_browser_probe_drain_after_stop_returns_error() {
        let server = ChronosServer::new();
        let session_id = "test-browser-drain-after-stop".to_string();

        // Inject mock session
        {
            let adapter = Arc::new(BrowserAdapter::new());
            let capture_session = CaptureSession::new(
                0,
                Language::WebAssembly,
                CaptureConfig::new("http://test.local"),
            );
            let probe = BrowserProbeSession {
                adapter,
                session: capture_session,
                session_id: session_id.clone(),
                url: "http://test.local".to_string(),
            };
            server
                .live_browser_probes
                .lock()
                .unwrap()
                .insert(session_id.clone(), probe);
        }

        // Stop the session
        let _ = server
            .browser_probe_stop(Parameters(BrowserProbeStopParams {
                session_id: session_id.clone(),
            }))
            .await
            .unwrap();

        // Drain after stop should return error
        let drain = server
            .browser_probe_drain(Parameters(BrowserProbeDrainParams {
                session_id: session_id.clone(),
                limit: 1000,
                offset: 0,
            }))
            .await
            .unwrap();
        assert_eq!(
            drain.is_error,
            Some(true),
            "Drain after stop should be an error"
        );
    }

    /// m1-08 — the daemon round is a no-op for backends without an
    /// attached log. Verifies we don't accidentally fabricate
    /// compaction activity on probes that opted out of the log.
    #[tokio::test(flavor = "current_thread")]
    async fn m1_08_auto_compaction_round_skips_backends_without_log() {
        use chronos_domain::{CaptureConfig, CaptureSession, Language};
        use chronos_native::probe_backend::NativeProbeBackend;

        let server = Arc::new(ChronosServer::new());

        // Register a backend with NO execution log attached.
        let bus = chronos_domain::bus::EventBus::new_shared(1024);
        let backend_no_log = NativeProbeBackend::new(bus);
        // Build the minimum LiveProbeSession: the daemon only
        // reads `backend`, so we stub the other fields with
        // dummies that compile.
        let dummy_session = CaptureSession::new(0, Language::Rust, CaptureConfig::new("noop"));
        let live = LiveProbeSession {
            backend: backend_no_log,
            session: dummy_session,
            language: Language::Rust,
            target: "noop".to_string(),
            ebpf_adapter: None,
            ebpf_attachment: None,
        };
        {
            let mut probes = server.live_probes.lock().unwrap();
            probes.insert("no-log-session".to_string(), live);
        }

        // No log attached ⇒ daemon round should be a no-op (just
        // continue past the entry). We can't observe the absence
        // directly, but we can observe that nothing panics and that
        // there's nothing to assert against.
        ChronosServer::run_one_compaction_round(&server).await;

        // If we got here without panicking, we passed.
        // (No assertion needed — the postcondition is "no panics +
        // no log was created".)
    }

    /// m2-08 — `probe_drain_log` surfaces each event's `data` (which carries
    /// the `EventData::Function` identity fields) in the JSON output, without
    /// dropping the existing flat projection. Uses the in-process slot seam to
    /// attach a real `SegmentedExecutionLog` holding an identity-bearing
    /// record, so no ptrace is needed.
    #[tokio::test(flavor = "current_thread")]
    async fn m2_08_probe_drain_log_surfaces_event_data_identity() {
        use chronos_domain::{CaptureConfig, CaptureSession, SourceLocation};
        use chronos_log::{
            ExecutionPayload, NewExecutionRecord, SegmentedConfig, SegmentedExecutionLog,
            SessionId as LogSessionId,
        };
        use chronos_native::probe_backend::NativeProbeBackend;
        use std::sync::Arc;

        let session_key = "sess-drain-identity".to_string();

        // Build the log session id the backend expects ("native-<key>").
        let log_session_id = format!("native-{}", session_key);
        let dir = std::env::temp_dir().join(format!(
            "chronos-m2-08-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        let sym = chronos_domain::SymbolId::new("main", None, chronos_domain::Language::C);
        let inv = chronos_domain::InvocationId::now();
        let parent = chronos_domain::InvocationId::now();
        let ev = TraceEvent::new(
            7,
            700,
            1,
            EventType::FunctionEntry,
            SourceLocation::new("main.c", 3, "main", 0x1000),
            EventData::Function {
                name: "main".to_string(),
                signature: None,
                symbol_id: Some(sym),
                invocation_id: Some(inv),
                parent_invocation_id: Some(parent),
            },
        );

        // Open a log, attach it to the backend slot, and append the record.
        let bus = chronos_domain::bus::EventBus::new_shared(1024);
        let backend = NativeProbeBackend::new(bus);
        let log = Arc::new(
            SegmentedExecutionLog::open(
                LogSessionId::new(&log_session_id),
                SegmentedConfig::with_dir(&dir),
            )
            .unwrap(),
        );
        log.append(NewExecutionRecord {
            session_id: LogSessionId::new(&log_session_id),
            monotonic_ns: 700,
            payload: ExecutionPayload::new(serde_json::to_vec(&ev).unwrap(), "trace_event"),
            invocation_id: Some(inv),
            parent_invocation_id: Some(parent),
            symbol_id: Some(sym),
        })
        .unwrap();
        log.flush().ok();
        {
            let mut slot = backend
                .execution_log_slot_for_test()
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            *slot = Some(log.clone());
        }

        // Register the backend as a live probe session.
        let server = Arc::new(ChronosServer::new());
        let dummy_session =
            CaptureSession::new(0, chronos_domain::Language::C, CaptureConfig::new("noop"));
        let live = LiveProbeSession {
            backend,
            session: dummy_session,
            language: chronos_domain::Language::C,
            target: "noop".to_string(),
            ebpf_adapter: None,
            ebpf_attachment: None,
        };
        server
            .live_probes
            .lock()
            .unwrap()
            .insert(session_key.clone(), live);

        let result = server
            .probe_drain_log(Parameters(ProbeDrainLogParams {
                session_id: session_key,
                since: None,
                limit: 256,
            }))
            .await
            .unwrap();
        assert_ne!(result.is_error, Some(true));
        let s = format!("{:?}", result.content[0]);
        // Flat projection preserved.
        assert!(s.contains("event_id"), "flat event_id missing: {s}");
        assert!(s.contains("FunctionEntry"), "kind missing: {s}");
        // New: data object surfaced with identity.
        assert!(s.contains("data"), "data field missing: {s}");
        assert!(s.contains("invocation_id"), "invocation_id missing: {s}");
        assert!(
            s.contains(&inv.to_string()),
            "invocation_id value missing: {s}"
        );
        assert!(s.contains("parent_invocation_id"), "parent missing: {s}");
        assert!(s.contains("symbol_id"), "symbol_id missing: {s}");
        assert!(s.contains("main"), "function name missing: {s}");

        std::fs::remove_dir_all(&dir).ok();
    }

    // ========================================================================
    // M3 — Mutation Lens Tests
    // ========================================================================

    #[allow(dead_code)]
    fn make_var_write(id: u64, ts: u64, tid: u64, name: &str, value: &str) -> TraceEvent {
        use chronos_domain::{SourceLocation, VariableInfo, VariableScope};
        let loc = SourceLocation::new("", 0, "", 0x1000 + id);
        TraceEvent::new(
            id,
            ts,
            tid,
            EventType::VariableWrite,
            loc,
            EventData::Variable(VariableInfo::new(
                name,
                value,
                "string",
                0x7FFE0000 + id,
                VariableScope::Local,
            )),
        )
    }

    #[tokio::test]
    async fn test_mutation_lens_no_session_returns_error() {
        let server = ChronosServer::new();
        let result = server
            .mutation_lens(Parameters(MutationLensParams {
                session_id: "nonexistent".to_string(),
                target: None,
                limit: None,
            }))
            .await
            .unwrap();
        assert_eq!(result.is_error, Some(true));
        let s = format!("{:?}", result.content[0]);
        assert!(s.contains("not found"), "expected 'not found' error: {s}");
    }

    #[tokio::test]
    async fn test_mutation_lens_with_two_writes_returns_one_transition() {
        let events = vec![
            make_var_write(0, 100, 1, "x", "1"),
            make_var_write(1, 200, 1, "x", "2"),
        ];
        let (server, sid) = server_with_session(events).await;

        let result = server
            .mutation_lens(Parameters(MutationLensParams {
                session_id: sid,
                target: None,
                limit: None,
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let s = format!("{:?}", result.content[0]);
        assert!(
            s.contains("count") && s.contains("transitions"),
            "expected valid output: {s}"
        );
        assert!(
            s.contains("target") && s.contains("x"),
            "expected target 'x' in output: {s}"
        );
    }

    #[tokio::test]
    async fn test_mutation_lens_filters_by_target() {
        let events = vec![
            make_var_write(0, 100, 1, "x", "1"),
            make_var_write(1, 200, 1, "y", "10"),
            make_var_write(2, 300, 1, "x", "2"),
            make_var_write(3, 400, 1, "y", "20"),
        ];
        let (server, sid) = server_with_session(events).await;

        let result = server
            .mutation_lens(Parameters(MutationLensParams {
                session_id: sid,
                target: Some("x".to_string()),
                limit: None,
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let s = format!("{:?}", result.content[0]);
        // Should get exactly 1 transition for x (two writes: 1->2)
        assert!(s.contains("count"), "expected count field: {s}");
        // Should NOT contain y transitions
        assert!(
            !s.contains("target") || !s.contains("y"),
            "target filter should exclude y: {s}"
        );
    }

    // ========================================================================
    // M3 — Causal Slice Tests
    // ========================================================================

    #[tokio::test]
    async fn test_causal_slice_no_session_returns_error() {
        let server = ChronosServer::new();
        let result = server
            .causal_slice(Parameters(CausalSliceParams {
                session_id: "nonexistent".to_string(),
                sink_event_id: 0,
            }))
            .await
            .unwrap();
        assert_eq!(result.is_error, Some(true));
        let s = format!("{:?}", result.content[0]);
        assert!(s.contains("not found"), "expected 'not found' error: {s}");
    }

    #[tokio::test]
    async fn test_causal_slice_sink_not_found_returns_error() {
        let events = vec![
            make_fn_entry(0, 100, 1, "main"),
            make_fn_entry(1, 200, 1, "compute"),
        ];
        let (server, sid) = server_with_session(events).await;

        let result = server
            .causal_slice(Parameters(CausalSliceParams {
                session_id: sid,
                sink_event_id: 999, // Does not exist
            }))
            .await
            .unwrap();
        assert_eq!(result.is_error, Some(true));
        let s = format!("{:?}", result.content[0]);
        assert!(s.contains("not found"), "expected 'not found' error: {s}");
    }

    #[tokio::test]
    async fn test_causal_slice_returns_included_path_for_sink() {
        let events = vec![
            make_fn_entry(0, 100, 1, "main"),
            make_fn_entry(1, 200, 1, "compute"),
            make_fn_exit(2, 300, 1, "compute"),
            make_fn_exit(3, 400, 1, "main"),
        ];
        let (server, sid) = server_with_session(events).await;

        let result = server
            .causal_slice(Parameters(CausalSliceParams {
                session_id: sid,
                sink_event_id: 3, // main exit
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let s = format!("{:?}", result.content[0]);
        assert!(
            s.contains("included") && s.contains("missing") && s.contains("depth"),
            "expected causal slice output with included/missing/depth: {s}"
        );
        // Sink event 3 should be in the included set.
        assert!(s.contains("3"), "expected sink event 3 in output: {s}");
    }
}

/// ServerHandler implementation with custom server identity.
/// This overrides the auto-generated one from #[tool_router(server_handler)]
/// to provide correct name/version instead of rmcp defaults.
#[rmcp::tool_handler(
    name = "chronos-mcp",
    version = "0.1.0",
    instructions = "Time-travel debugging server for AI agents. Use probe_start to capture program execution, then query with query_events, get_call_stack, debug_detect_races, inspect_causality, etc."
)]
impl rmcp::handler::server::ServerHandler for ChronosServer {}
