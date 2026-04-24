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

use chronos_domain::semantic::SemanticEvent;
use chronos_domain::{
    query::{CausalityQuery, PerfQuery, PerfSortBy, RaceDetectionQuery},
    CaptureConfig, CaptureSession, EventData, EventType, Language, ProbeBackend, TraceEvent, TraceQuery,
};
use chronos_domain::tripwire::{TripwireCondition, TripwireId, TripwireManager};
use chronos_index::builder::IndexBuilder;
use chronos_native::probe_backend::NativeProbeBackend;
use chronos_query::QueryEngine;
use chronos_store::{SessionMetadata, SessionStore, TraceDiff};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content};
use rmcp::tool;
use schemars::JsonSchema;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

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

/// A live probe session using `NativeProbeBackend`.
///
/// Unlike `debug_run` which blocks until the program exits, a live probe streams
/// events to an `EventBus` ring buffer in real-time. Events can be drained at any
/// time via `probe_drain`, and the probe is stopped via `probe_stop`.
struct LiveProbeSession {
    /// The native probe backend driving the ptrace loop.
    backend: NativeProbeBackend,
    /// The capture session returned by `start_probe`.
    session: CaptureSession,
    /// Language of the target program.
    language: Language,
    /// Path to the target binary.
    target: String,
}

/// Empty parameter type for tools that take no arguments.
///
/// This is needed because rmcp sends `{}` as default arguments when none are provided,
/// but `()` (unit) cannot deserialize from `{}`. This empty struct can deserialize
/// from an empty JSON object `{}`.
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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StateDiffParams {
    /// Session ID.
    pub session_id: String,
    /// First timestamp (nanoseconds).
    pub timestamp_a: u64,
    /// Second timestamp (nanoseconds).
    pub timestamp_b: u64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListThreadsParams {
    /// Session ID.
    pub session_id: String,
}

// ============================================================================
// SF4 — New tool parameter types
// ============================================================================

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
    fn into_condition(self) -> TripwireCondition {
        match self {
            TripwireConditionType::EventType { event_types } => {
                let types = event_types
                    .iter()
                    .filter_map(|s| ChronosServer::parse_event_type(s))
                    .collect();
                TripwireCondition::EventType(types)
            }
            TripwireConditionType::FunctionName { pattern } => {
                TripwireCondition::FunctionName { pattern }
            }
            TripwireConditionType::ExceptionType { exc_type } => {
                TripwireCondition::ExceptionType { exc_type }
            }
            TripwireConditionType::MemoryAddress { start, end } => {
                TripwireCondition::MemoryAddress { start, end }
            }
            TripwireConditionType::SyscallNumber { numbers } => {
                TripwireCondition::SyscallNumber { numbers }
            }
            TripwireConditionType::VariableName { name } => {
                TripwireCondition::VariableName { name }
            }
            TripwireConditionType::Signal { numbers } => {
                TripwireCondition::Signal { numbers }
            }
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

#[derive(Debug, serde::Serialize)]
pub struct FunctionRegressionEntry {
    pub function: String,
    pub baseline_calls: u64,
    pub target_calls: u64,
    /// Percentage change in call count (positive = more calls in target).
    pub call_delta_pct: f64,
}

#[derive(Debug, serde::Serialize)]
pub struct PerformanceRegressionAuditResult {
    pub baseline_session_id: String,
    pub target_session_id: String,
    /// Functions where call count increased significantly (>50%).
    pub regressions: Vec<FunctionRegressionEntry>,
    /// Functions where call count decreased significantly (>50% reduction).
    pub improvements: Vec<FunctionRegressionEntry>,
    /// Total functions analyzed.
    pub functions_analyzed: usize,
    /// Overall call count delta (positive = more calls in target).
    pub total_call_delta: i64,
    /// LLM-readable summary.
    pub summary: String,
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
        }
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

    async fn build_and_store_engine(&self, session_id: &str, events: Vec<TraceEvent>, language: Language) {
        // Filter out internal/noisy events before indexing:
        // - EventType::Custom with EventData::Registers → ptrace register snapshots (infrastructure noise)
        // - EventType::Unknown → unclassified ptrace stops
        // These are implementation details of the tracer, not meaningful for AI analysis.
        let events: Vec<TraceEvent> = events
            .into_iter()
            .filter(|e| {
                // Keep everything except raw register snapshots and unknowns
                !matches!((&e.event_type, &e.data), (EventType::Custom, EventData::Registers(_)) | (EventType::Unknown, _))
            })
            .collect();

        let mut builder = IndexBuilder::new();
        builder.push_all(&events);
        let indices = builder.finalize();

        let engine = QueryEngine::with_indices(events, indices.shadow, indices.temporal)
            .with_causality(indices.causality)
            .with_performance(indices.performance);

        // Store engine and language for later use
        let mut engines = self.engines.lock().await;
        let mut session_languages = self.session_languages.lock().await;
        info!("Built query engine for session {} (language: {:?})", session_id, language);
        engines.insert(session_id.to_string(), engine);
        session_languages.insert(session_id.to_string(), language);

        // Set this session as the active session
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
        let transport = (tokio::io::stdin(), tokio::io::stdout());
        let service = server.serve(transport).await?;
        info!("Chronos MCP server started on stdio");
        service.waiting().await?;
        Ok(())
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
        let engines = self.engines.lock().await;

        let engine = match engines.get(&params.session_id) {
            Some(e) => e,
            None => {
                return Ok(CallToolResult::error(text_content(format!(
                    "Session '{}' not found or not finalized",
                    params.session_id
                ))));
            }
        };

        let mut query = TraceQuery::new(&params.session_id).pagination(params.limit, params.offset);

        if let Some(ref types) = params.event_types {
            let event_types: Vec<EventType> = types
                .iter()
                .filter_map(|t| Self::parse_event_type(t))
                .collect();
            if !event_types.is_empty() {
                query = query.event_types(event_types);
            }
        }

        if let (Some(start), Some(end)) = (params.timestamp_start, params.timestamp_end) {
            query = query.time_range(start, end);
        }

        if let Some(ref pattern) = params.function_pattern {
            query = query.function_pattern(pattern);
        }

        if let Some(tid) = params.thread_id {
            query.thread_id = Some(tid);
        }

        let result = engine.execute(&query);

        let output = serde_json::json!({
            "total_matching": result.total_matching,
            "returned_count": result.events.len(),
            "next_offset": result.next_offset,
            "events": result.events.iter().map(|e| serde_json::json!({
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
        let engines = self.engines.lock().await;

        match engines.get(&params.session_id) {
            Some(engine) => match engine.get_event_by_id(params.event_id) {
                Some(event) => {
                    let json = serde_json::to_string_pretty(&event).unwrap_or_default();
                    Ok(CallToolResult::success(vec![Content::text(json)]))
                }
                None => Ok(CallToolResult::error(text_content(format!(
                    "Event {} not found",
                    params.event_id
                )))),
            },
            None => Ok(CallToolResult::error(text_content(format!(
                "Session '{}' not found",
                params.session_id
            )))),
        }
    }

    #[tool(
        name = "get_call_stack",
        description = "Reconstruct the call stack at a specific event."
    )]
    async fn get_call_stack(
        &self,
        params: Parameters<GetCallStackParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let engines = self.engines.lock().await;

        match engines.get(&params.session_id) {
            Some(engine) => {
                let stack = engine.reconstruct_call_stack(params.event_id);
                let output = serde_json::json!({
                    "session_id": params.session_id,
                    "at_event_id": params.event_id,
                    "depth": stack.len(),
                    "frames": stack.iter().map(|f| serde_json::json!({
                        "depth": f.depth,
                        "function": f.function,
                        "file": f.file,
                        "line": f.line,
                        "address": format!("0x{:x}", f.address),
                    })).collect::<Vec<_>>(),
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            None => Ok(CallToolResult::error(text_content(format!(
                "Session '{}' not found",
                params.session_id
            )))),
        }
    }

    #[tool(
        name = "get_execution_summary",
        description = "Get execution summary: event counts, top functions, issues."
    )]
    async fn get_execution_summary(
        &self,
        params: Parameters<GetExecutionSummaryParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let engines = self.engines.lock().await;

        match engines.get(&params.session_id) {
            Some(engine) => {
                let summary = engine.execution_summary(&params.session_id);
                let json = serde_json::to_string_pretty(&summary).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            None => Ok(CallToolResult::error(text_content(format!(
                "Session '{}' not found",
                params.session_id
            )))),
        }
    }

    #[tool(
        name = "state_diff",
        description = "Compare program state (registers) between two timestamps."
    )]
    async fn state_diff(
        &self,
        params: Parameters<StateDiffParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let engines = self.engines.lock().await;

        match engines.get(&params.session_id) {
            Some(engine) => {
                let diff = engine.state_diff(params.timestamp_a, params.timestamp_b);
                let json = serde_json::to_string_pretty(&diff).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            None => Ok(CallToolResult::error(text_content(format!(
                "Session '{}' not found",
                params.session_id
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
        let engines = self.engines.lock().await;

        match engines.get(&params.session_id) {
            Some(engine) => {
                let threads = engine.thread_ids();
                let output = serde_json::json!({
                    "session_id": params.session_id,
                    "thread_count": threads.len(),
                    "thread_ids": threads,
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            None => Ok(CallToolResult::error(text_content(format!(
                "Session '{}' not found",
                params.session_id
            )))),
        }
    }

    // ========================================================================
    // SF4 — Semantic Compression + Advanced Tools (T17–T23)
    // ========================================================================

    #[tool(
        name = "debug_call_graph",
        description = "Build the call graph for a session up to a given depth. Returns callers and callees for each function observed in the trace."
    )]
    async fn debug_call_graph(
        &self,
        params: Parameters<DebugCallGraphParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let engines = self.engines.lock().await;

        let engine = match engines.get(&params.session_id) {
            Some(e) => e,
            None => {
                return Ok(CallToolResult::error(text_content(format!(
                    "Session '{}' not found",
                    params.session_id
                ))))
            }
        };

        // Build a call graph from FunctionEntry events
        let mut callers: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        let mut callees: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        let mut call_counts: std::collections::HashMap<String, u64> =
            std::collections::HashMap::new();

        // Reconstruct call graph from the full event list via stack simulation per thread
        let mut stacks: std::collections::HashMap<u64, Vec<String>> =
            std::collections::HashMap::new();

        let query = TraceQuery::new(&params.session_id).pagination(usize::MAX, 0);
        let result = engine.execute(&query);

        for event in &result.events {
            let func = event.location.function.clone().unwrap_or_default();
            if func.is_empty() {
                continue;
            }
            match event.event_type {
                EventType::FunctionEntry => {
                    let stack = stacks.entry(event.thread_id).or_default();
                    *call_counts.entry(func.clone()).or_insert(0) += 1;

                    // depth gate
                    if stack.len() < params.max_depth {
                        if let Some(parent) = stack.last().cloned() {
                            callees
                                .entry(parent.clone())
                                .or_default()
                                .push(func.clone());
                            callers.entry(func.clone()).or_default().push(parent);
                        }
                        stack.push(func);
                    }
                }
                EventType::FunctionExit => {
                    let stack = stacks.entry(event.thread_id).or_default();
                    stack.pop();
                }
                _ => {}
            }
        }

        // Deduplicate edges
        for v in callers.values_mut() {
            v.sort();
            v.dedup();
        }
        for v in callees.values_mut() {
            v.sort();
            v.dedup();
        }

        let nodes: Vec<serde_json::Value> = call_counts
            .iter()
            .map(|(name, count)| {
                serde_json::json!({
                    "function": name,
                    "call_count": count,
                    "callers": callers.get(name).cloned().unwrap_or_default(),
                    "callees": callees.get(name).cloned().unwrap_or_default(),
                })
            })
            .collect();

        let output = serde_json::json!({
            "session_id": params.session_id,
            "max_depth": params.max_depth,
            "unique_functions": nodes.len(),
            "nodes": nodes,
        });
        Ok(CallToolResult::success(json_content(&output)))
    }

    #[tool(
        name = "debug_find_variable_origin",
        description = "Trace the origin of a variable: find all write mutations to it and reconstruct its lineage. Uses the CausalityIndex."
    )]
    async fn debug_find_variable_origin(
        &self,
        params: Parameters<DebugFindVariableOriginParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let engines = self.engines.lock().await;

        let engine = match engines.get(&params.session_id) {
            Some(e) => e,
            None => {
                return Ok(CallToolResult::error(text_content(format!(
                    "Session '{}' not found",
                    params.session_id
                ))))
            }
        };

        let query = CausalityQuery {
            session_id: params.session_id.clone(),
            address: None,
            variable_name: Some(params.variable_name.clone()),
            before_timestamp: None,
            full_lineage: true,
        };

        match engine.query_causality(&query) {
            Some(result) => {
                let mut mutations = result.mutations;
                mutations.truncate(params.limit);
                let output = serde_json::json!({
                    "session_id": params.session_id,
                    "variable_name": params.variable_name,
                    "mutation_count": mutations.len(),
                    "mutations": mutations.iter().map(|m| serde_json::json!({
                        "event_id": m.event_id,
                        "timestamp_ns": m.timestamp,
                        "thread_id": m.thread_id,
                        "value_before": m.value_before,
                        "value_after": m.value_after,
                        "function": m.function,
                    })).collect::<Vec<_>>(),
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            None => Ok(CallToolResult::success(json_content(&serde_json::json!({
                "session_id": params.session_id,
                "variable_name": params.variable_name,
                "mutation_count": 0,
                "mutations": [],
                "note": "No causality index or no writes to this variable found",
            })))),
        }
    }

    #[tool(
        name = "debug_find_crash",
        description = "Identify the crash point in a trace: find the last event before a fatal signal (SIGSEGV, SIGABRT, etc.) and return the call stack at that point."
    )]
    async fn debug_find_crash(
        &self,
        params: Parameters<DebugFindCrashParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let engines = self.engines.lock().await;

        let engine = match engines.get(&params.session_id) {
            Some(e) => e,
            None => {
                return Ok(CallToolResult::error(text_content(format!(
                    "Session '{}' not found",
                    params.session_id
                ))))
            }
        };

        // Find fatal signals in the trace
        let fatal_signals = [
            "SIGSEGV", "SIGABRT", "SIGBUS", "SIGILL", "SIGFPE", "SIGKILL",
        ];

        let query = TraceQuery::new(&params.session_id)
            .event_types(vec![EventType::SignalDelivered])
            .pagination(usize::MAX, 0);
        let result = engine.execute(&query);

        let crash_event = result.events.iter().find(|e| {
            if let EventData::Signal { signal_name, .. } = &e.data {
                fatal_signals.contains(&signal_name.as_str())
            } else {
                // Fallback: check function field for signal name hint
                false
            }
        });

        match crash_event {
            Some(ev) => {
                let stack = engine.reconstruct_call_stack(ev.event_id);
                let signal_name = if let EventData::Signal { signal_name, .. } = &ev.data {
                    signal_name.clone()
                } else {
                    "unknown".to_string()
                };

                let output = serde_json::json!({
                    "session_id": params.session_id,
                    "crash_found": true,
                    "signal": signal_name,
                    "event_id": ev.event_id,
                    "timestamp_ns": ev.timestamp_ns,
                    "thread_id": ev.thread_id,
                    "call_stack_depth": stack.len(),
                    "call_stack": stack.iter().map(|f| serde_json::json!({
                        "depth": f.depth,
                        "function": f.function,
                        "address": format!("0x{:x}", f.address),
                        "file": f.file,
                        "line": f.line,
                    })).collect::<Vec<_>>(),
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            None => Ok(CallToolResult::success(json_content(&serde_json::json!({
                "session_id": params.session_id,
                "crash_found": false,
                "note": "No fatal signal found in the trace",
            })))),
        }
    }

    #[tool(
        name = "debug_detect_races",
        description = "Detect data races: find writes to the same memory address within the threshold_ns window on different threads. Default threshold is 100ns."
    )]
    async fn debug_detect_races(
        &self,
        params: Parameters<DebugDetectRacesParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let engines = self.engines.lock().await;

        let engine = match engines.get(&params.session_id) {
            Some(e) => e,
            None => {
                return Ok(CallToolResult::error(text_content(format!(
                    "Session '{}' not found",
                    params.session_id
                ))))
            }
        };

        let query = RaceDetectionQuery {
            session_id: params.session_id.clone(),
            time_range: None,
            threshold_ns: params.threshold_ns,
        };
        let result = engine.detect_races(&query);

        let output = serde_json::json!({
            "session_id": params.session_id,
            "threshold_ns": params.threshold_ns,
            "race_count": result.races.len(),
            "races": result.races.iter().map(|r| serde_json::json!({
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

    #[tool(
        name = "inspect_causality",
        description = "Inspect the full causal history of a memory address: all reads and writes, their timestamps, values, and originating functions."
    )]
    async fn inspect_causality(
        &self,
        params: Parameters<InspectCausalityParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let engines = self.engines.lock().await;

        let engine = match engines.get(&params.session_id) {
            Some(e) => e,
            None => {
                return Ok(CallToolResult::error(text_content(format!(
                    "Session '{}' not found",
                    params.session_id
                ))))
            }
        };

        let query = CausalityQuery {
            session_id: params.session_id.clone(),
            address: Some(params.address),
            variable_name: None,
            before_timestamp: None,
            full_lineage: true,
        };

        match engine.query_causality(&query) {
            Some(result) => {
                let mut mutations = result.mutations;
                mutations.truncate(params.limit);
                let output = serde_json::json!({
                    "session_id": params.session_id,
                    "address": format!("0x{:x}", params.address),
                    "mutation_count": mutations.len(),
                    "mutations": mutations.iter().map(|m| serde_json::json!({
                        "event_id": m.event_id,
                        "timestamp_ns": m.timestamp,
                        "thread_id": m.thread_id,
                        "value_before": m.value_before,
                        "value_after": m.value_after,
                        "function": m.function,
                    })).collect::<Vec<_>>(),
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            None => Ok(CallToolResult::success(json_content(&serde_json::json!({
                "session_id": params.session_id,
                "address": format!("0x{:x}", params.address),
                "mutation_count": 0,
                "mutations": [],
                "note": "No causality index or no writes to this address found",
            })))),
        }
    }

    #[tool(
        name = "debug_expand_hotspot",
        description = "Semantic compression Level 1 — return the top-N hottest functions by call count and CPU cycles. Use debug_execution_summary first (Level 0) then call this to zoom in."
    )]
    async fn debug_expand_hotspot(
        &self,
        params: Parameters<DebugExpandHotspotParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let engines = self.engines.lock().await;

        let engine = match engines.get(&params.session_id) {
            Some(e) => e,
            None => {
                return Ok(CallToolResult::error(text_content(format!(
                    "Session '{}' not found",
                    params.session_id
                ))))
            }
        };

        // Get top functions by calls from execution summary
        let summary = engine.execution_summary(&params.session_id);
        let top_by_calls: Vec<serde_json::Value> = summary
            .top_functions
            .iter()
            .take(params.top_n)
            .map(|f| {
                // Try to get perf data for this function
                let perf_entry = engine
                    .query_perf(&PerfQuery {
                        session_id: params.session_id.clone(),
                        function_filter: Some(f.name.clone()),
                        sort_by: PerfSortBy::Cycles,
                        limit: 1,
                    })
                    .and_then(|r| r.functions.into_iter().next());

                serde_json::json!({
                    "function": f.name,
                    "call_count": f.call_count,
                    "total_cycles": perf_entry.as_ref().map(|p| p.total_cycles),
                    "avg_cycles_per_call": perf_entry.as_ref().map(|p| p.avg_cycles),
                })
            })
            .collect();

        let total_calls: u64 = summary.top_functions.iter().map(|f| f.call_count).sum();

        let output = serde_json::json!({
            "session_id": params.session_id,
            "compression_level": "hotspot",
            "top_n": params.top_n,
            "total_calls_in_trace": total_calls,
            "hotspot_functions": top_by_calls,
            "hint": "Use debug_call_graph for full call graph or query_events to drill into specific functions",
        });
        Ok(CallToolResult::success(json_content(&output)))
    }

    #[tool(
        name = "debug_get_saliency_scores",
        description = "Compute saliency scores [0.0–1.0] for all functions: a high score means this function consumed a disproportionate share of CPU cycles relative to other functions. Use to prioritize where to look."
    )]
    async fn debug_get_saliency_scores(
        &self,
        params: Parameters<DebugGetSaliencyScoresParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let engines = self.engines.lock().await;

        let engine = match engines.get(&params.session_id) {
            Some(e) => e,
            None => {
                return Ok(CallToolResult::error(text_content(format!(
                    "Session '{}' not found",
                    params.session_id
                ))))
            }
        };

        let summary = engine.execution_summary(&params.session_id);

        // Get perf data for all top functions
        let perf_result = engine.query_perf(&PerfQuery {
            session_id: params.session_id.clone(),
            function_filter: None,
            sort_by: PerfSortBy::Cycles,
            limit: params.limit,
        });

        let scores: Vec<serde_json::Value> = if let Some(perf) = perf_result {
            // Compute total cycles
            let total_cycles: u64 = perf.functions.iter().map(|e| e.total_cycles).sum();

            perf.functions
                .iter()
                .take(params.limit)
                .map(|entry| {
                    let score = if total_cycles > 0 {
                        entry.total_cycles as f64 / total_cycles as f64
                    } else {
                        // Fallback: call count ratio
                        let total_calls: u64 =
                            summary.top_functions.iter().map(|f| f.call_count).sum();
                        if total_calls > 0 {
                            entry.call_count as f64 / total_calls as f64
                        } else {
                            0.0
                        }
                    };
                    serde_json::json!({
                        "function": entry.name.as_deref().unwrap_or("<unknown>"),
                        "saliency_score": (score * 10000.0).round() / 10000.0,
                        "call_count": entry.call_count,
                        "total_cycles": entry.total_cycles,
                    })
                })
                .collect()
        } else {
            // No perf index: fall back to call-count based scoring
            let total_calls: u64 = summary.top_functions.iter().map(|f| f.call_count).sum();
            summary
                .top_functions
                .iter()
                .take(params.limit)
                .map(|f| {
                    let score = if total_calls > 0 {
                        f.call_count as f64 / total_calls as f64
                    } else {
                        0.0
                    };
                    serde_json::json!({
                        "function": f.name,
                        "saliency_score": (score * 10000.0).round() / 10000.0,
                        "call_count": f.call_count,
                        "cycles": null,
                    })
                })
                .collect()
        };

        let output = serde_json::json!({
            "session_id": params.session_id,
            "scored_functions": scores.len(),
            "scores": scores,
            "hint": "saliency_score near 1.0 means this function dominated CPU time. Use debug_expand_hotspot to zoom in.",
        });
        Ok(CallToolResult::success(json_content(&output)))
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
        let engines = self.engines.lock().await;

        let engine = match engines.get(&params.session_id) {
            Some(e) => e,
            None => {
                return Ok(CallToolResult::error(text_content(format!(
                    "Session '{}' not found in memory. Run probe_start first.",
                    params.session_id
                ))))
            }
        };

        let events = engine.get_all_events();
        let event_count = events.len();

        if event_count == 0 {
            return Ok(CallToolResult::error(text_content(
                "Session has no events to save.".to_string(),
            )));
        }

        // Determine duration from first/last events
        let (duration_ms, created_at) =
            if let (Some(first), Some(last)) = (events.first(), events.last()) {
                let dur_ns = last.timestamp_ns.saturating_sub(first.timestamp_ns);
                (dur_ns / 1_000_000, last.timestamp_ns / 1_000_000)
            } else {
                (0, 0)
            };

        let metadata = SessionMetadata {
            session_id: params.session_id.clone(),
            created_at,
            language: params.language.clone(),
            target: params.target.clone(),
            event_count,
            duration_ms,
        };

        let hashes = match self.store.save_session(metadata.clone(), &events) {
            Ok(h) => h,
            Err(e) => {
                return Ok(CallToolResult::error(text_content(format!(
                    "Failed to save session: {}",
                    e
                ))))
            }
        };

        let output = serde_json::json!({
            "session_id": params.session_id,
            "status": "saved",
            "event_count": event_count,
            "hash_count": hashes.len(),
            "language": params.language,
            "target": params.target,
            "duration_ms": duration_ms,
            "hint": "Use load_session to reload this session, or list_sessions to see all saved sessions.",
        });
        Ok(CallToolResult::success(json_content(&output)))
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

        let (metadata, events) = match self.store.load_session(&params.session_id) {
            Ok((m, e)) => (m, e),
            Err(e) => {
                return Ok(CallToolResult::error(text_content(format!(
                    "Failed to load session '{}': {}",
                    params.session_id, e
                ))))
            }
        };

        // Build engine from loaded events
        let mut builder = IndexBuilder::new();
        builder.push_all(&events);
        let indices = builder.finalize();

        let engine = QueryEngine::with_indices(events, indices.shadow, indices.temporal)
            .with_causality(indices.causality)
            .with_performance(indices.performance);

        let mut engines = self.engines.lock().await;
        engines.insert(params.session_id.clone(), engine);

        let output = serde_json::json!({
            "session_id": params.session_id,
            "status": "loaded",
            "language": metadata.language,
            "target": metadata.target,
            "event_count": metadata.event_count,
            "duration_ms": metadata.duration_ms,
            "created_at": metadata.created_at,
            "hint": "Session is now queryable. Use query_events, get_execution_summary, etc.",
        });
        Ok(CallToolResult::success(json_content(&output)))
    }

    #[tool(
        name = "list_sessions",
        description = "List all saved sessions from persistent storage. Returns metadata for each session (no event data)."
    )]
    async fn list_sessions(
        &self,
        _params: Parameters<NoParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let sessions = match self.store.list_sessions() {
            Ok(s) => s,
            Err(e) => {
                return Ok(CallToolResult::error(text_content(format!(
                    "Failed to list sessions: {}",
                    e
                ))))
            }
        };

        let output = serde_json::json!({
            "session_count": sessions.len(),
            "sessions": sessions.iter().map(|s| serde_json::json!({
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

    #[tool(
        name = "delete_session",
        description = "Delete a session from persistent storage. Does not affect in-memory sessions."
    )]
    async fn delete_session(
        &self,
        params: Parameters<DeleteSessionParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        match self.store.delete_session(&params.session_id) {
            Ok(()) => {
                // Also purge all in-memory state for this session.
                self.cleanup_session_memory(&params.session_id).await;

                let output = serde_json::json!({
                    "session_id": params.session_id,
                    "status": "deleted",
                    "message": format!("Session '{}' deleted from persistent storage and memory.", params.session_id),
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(e) => Ok(CallToolResult::error(text_content(format!(
                "Failed to delete session '{}': {}",
                params.session_id, e
            )))),
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

        // Check if session exists in memory
        let session_existed = self.engines.lock().await.contains_key(&params.session_id);

        // Clean up all in-memory state for this session
        self.cleanup_session_memory(&params.session_id).await;

        if session_existed {
            let output = serde_json::json!({
                "session_id": params.session_id,
                "status": "dropped",
                "message": "Session removed from memory. Persistent storage not affected.",
            });
            Ok(CallToolResult::success(json_content(&output)))
        } else {
            // Idempotent: return success even if not found
            let output = serde_json::json!({
                "session_id": params.session_id,
                "status": "not_found",
                "message": "Session not found in memory. No action taken.",
            });
            Ok(CallToolResult::success(json_content(&output)))
        }
    }


    // ========================================================================
    // SF6 — Inspection Tools (T5–T7)
    // ========================================================================

    #[tool(
        name = "evaluate_expression",
        description = "Evaluate an arithmetic expression using local variables captured at a frame event. Supports +, -, *, /, parentheses, and variable names."
    )]
    async fn evaluate_expression(
        &self,
        params: Parameters<EvaluateExpressionParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        // Use QueryEngine to evaluate the arithmetic expression
        let engines = self.engines.lock().await;
        match engines.get(&params.session_id) {
            Some(engine) => {
                match engine.evaluate_expression(params.event_id, &params.expression) {
                    Ok(result) => {
                        let output = serde_json::json!({
                            "event_id": params.event_id,
                            "expression": params.expression,
                            "result": result,
                        });
                        Ok(CallToolResult::success(json_content(&output)))
                    }
                    Err(e) => {
                        let output = serde_json::json!({
                            "event_id": params.event_id,
                            "expression": params.expression,
                            "error": format!("{:?}", e),
                        });
                        Ok(CallToolResult::success(vec![Content::text(
                            serde_json::to_string_pretty(&output).unwrap_or_default(),
                        )]))
                    }
                }
            }
            None => Ok(CallToolResult::error(text_content(format!(
                "Session '{}' not found",
                params.session_id
            )))),
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
        let engines = self.engines.lock().await;

        match engines.get(&params.session_id) {
            Some(engine) => {
                let vars = engine.get_variables_at_event(params.event_id);
                let output = serde_json::json!({
                    "event_id": params.event_id,
                    "variables": vars,
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            None => Ok(CallToolResult::error(text_content(format!(
                "Session '{}' not found",
                params.session_id
            )))),
        }
    }

    #[tool(
        name = "debug_get_memory",
        description = "Read raw memory at an address as of a specific timestamp (nanoseconds). Returns the most recent MemoryWrite event at or before the timestamp."
    )]
    async fn debug_get_memory(
        &self,
        params: Parameters<DebugGetMemoryParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let engines = self.engines.lock().await;

        match engines.get(&params.session_id) {
            Some(engine) => {
                match engine.get_memory_at(params.address, params.timestamp_ns) {
                    Some(mem) => {
                        let hex = mem
                            .data
                            .iter()
                            .map(|b| format!("{:02x}", b))
                            .collect::<Vec<_>>()
                            .join("");
                        let output = serde_json::json!({
                            "address": format!("0x{:x}", mem.address),
                            "timestamp_ns": mem.timestamp_ns,
                            "event_id": mem.event_id,
                            "size": mem.size,
                            "data": mem.data,
                            "hex": hex,
                        });
                        Ok(CallToolResult::success(json_content(&output)))
                    }
                    None => Ok(CallToolResult::error(text_content(format!(
                        "No memory event found at address 0x{:x} before timestamp {}",
                        params.address, params.timestamp_ns
                    )))),
                }
            }
            None => Ok(CallToolResult::error(text_content(format!(
                "Session '{}' not found",
                params.session_id
            )))),
        }
    }

    // ========================================================================
    // SF7 — Phase 11 Missing Tools (T20–T24)
    // ========================================================================

    #[tool(
        name = "debug_get_registers",
        description = "Get CPU register values at a specific event_id. Returns the register state snapshot if available."
    )]
    async fn debug_get_registers(
        &self,
        params: Parameters<DebugGetRegistersParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let engines = self.engines.lock().await;

        let engine = match engines.get(&params.session_id) {
            Some(e) => e,
            None => {
                return Ok(CallToolResult::error(text_content(format!(
                    "Session '{}' not found",
                    params.session_id
                ))))
            }
        };

        match engine.get_event_by_id(params.event_id) {
            Some(_event) => {
                // Find register state attached to this event or the nearest prior one
                let regs = engine.find_registers_at_event(params.event_id);
                match regs {
                    Some(r) => {
                        let output = serde_json::json!({
                            "event_id": params.event_id,
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
                    None => Ok(CallToolResult::error(text_content(
                        "no register state at this event".to_string(),
                    ))),
                }
            }
            None => Ok(CallToolResult::error(text_content(format!(
                "Event {} not found",
                params.event_id
            )))),
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
        let engines = self.engines.lock().await;

        let engine = match engines.get(&params.session_id) {
            Some(e) => e,
            None => {
                return Ok(CallToolResult::error(text_content(format!(
                    "Session '{}' not found",
                    params.session_id
                ))))
            }
        };

        // Get variables at both events
        let vars_a = engine.get_variables_at_event(params.event_id_a);
        let vars_b = engine.get_variables_at_event(params.event_id_b);

        let names_a: std::collections::HashSet<_> = vars_a.iter().map(|v| v.name.clone()).collect();
        let names_b: std::collections::HashSet<_> = vars_b.iter().map(|v| v.name.clone()).collect();

        let vars_added: Vec<_> = names_b.difference(&names_a).cloned().collect();
        let vars_removed: Vec<_> = names_a.difference(&names_b).cloned().collect();

        // Find changed variables
        let mut vars_changed = Vec::new();
        for name in names_a.intersection(&names_b) {
            let val_a = vars_a.iter().find(|v| &v.name == name).map(|v| v.value.clone());
            let val_b = vars_b.iter().find(|v| &v.name == name).map(|v| v.value.clone());
            if val_a != val_b {
                vars_changed.push(serde_json::json!({
                    "name": name,
                    "before": val_a,
                    "after": val_b,
                }));
            }
        }

        // Get registers at both events
        let regs_a = engine.find_registers_at_event(params.event_id_a);
        let regs_b = engine.find_registers_at_event(params.event_id_b);

        let mut registers_changed = serde_json::Map::new();
        if let (Some(ra), Some(rb)) = (&regs_a, &regs_b) {
            let reg_fields = [
                ("rax", ra.rax, rb.rax),
                ("rbx", ra.rbx, rb.rbx),
                ("rcx", ra.rcx, rb.rcx),
                ("rdx", ra.rdx, rb.rdx),
                ("rsi", ra.rsi, rb.rsi),
                ("rdi", ra.rdi, rb.rdi),
                ("rbp", ra.rbp, rb.rbp),
                ("rsp", ra.rsp, rb.rsp),
                ("r8", ra.r8, rb.r8),
                ("r9", ra.r9, rb.r9),
                ("r10", ra.r10, rb.r10),
                ("r11", ra.r11, rb.r11),
                ("r12", ra.r12, rb.r12),
                ("r13", ra.r13, rb.r13),
                ("r14", ra.r14, rb.r14),
                ("r15", ra.r15, rb.r15),
                ("rip", ra.rip, rb.rip),
                ("rflags", ra.rflags, rb.rflags),
            ];
            for (name, val_a, val_b) in reg_fields {
                if val_a != val_b {
                    registers_changed.insert(
                        name.to_string(),
                        serde_json::json!({
                            "before": format!("0x{:x}", val_a),
                            "after": format!("0x{:x}", val_b),
                        }),
                    );
                }
            }
        }

        // Get timestamps for delta
        let event_a = engine.get_event_by_id(params.event_id_a);
        let event_b = engine.get_event_by_id(params.event_id_b);
        let timestamp_delta_ns = match (event_a, event_b) {
            (Some(ea), Some(eb)) => eb.timestamp_ns.saturating_sub(ea.timestamp_ns),
            _ => 0,
        };

        let output = serde_json::json!({
            "event_id_a": params.event_id_a,
            "event_id_b": params.event_id_b,
            "variables_added": vars_added,
            "variables_removed": vars_removed,
            "variables_changed": vars_changed,
            "registers_changed": registers_changed,
            "timestamp_delta_ns": timestamp_delta_ns,
        });
        Ok(CallToolResult::success(json_content(&output)))
    }

    #[tool(
        name = "debug_analyze_memory",
        description = "Analyze all memory accesses to an address range within a time window."
    )]
    async fn debug_analyze_memory(
        &self,
        params: Parameters<DebugAnalyzeMemoryParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let engines = self.engines.lock().await;

        let engine = match engines.get(&params.session_id) {
            Some(e) => e,
            None => {
                return Ok(CallToolResult::error(text_content(format!(
                    "Session '{}' not found",
                    params.session_id
                ))))
            }
        };

        // Get all events and filter for memory writes in the address/time range
        let all_events = engine.get_all_events();
        let mut accesses = Vec::new();
        let mut total_writes = 0u64;

        for event in all_events {
            // Filter by time window
            if event.timestamp_ns < params.start_ts || event.timestamp_ns > params.end_ts {
                continue;
            }

            // Check for memory events
            if let EventData::Memory { address, size, data } = &event.data {
                // Filter by address range
                if *address >= params.start_address && *address <= params.end_address {
                    let hex = data
                        .as_ref()
                        .map(|d| d.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(""))
                        .unwrap_or_default();
                    accesses.push(serde_json::json!({
                        "address": format!("0x{:x}", address),
                        "timestamp_ns": event.timestamp_ns,
                        "data_hex": hex,
                        "event_id": event.event_id,
                        "size": size,
                    }));
                    total_writes += 1;
                }
            }
        }

        let output = serde_json::json!({
            "start_address": format!("0x{:x}", params.start_address),
            "end_address": format!("0x{:x}", params.end_address),
            "start_ts": params.start_ts,
            "end_ts": params.end_ts,
            "total_writes": total_writes,
            "accesses": accesses,
        });
        Ok(CallToolResult::success(json_content(&output)))
    }

    #[tool(
        name = "forensic_memory_audit",
        description = "Full audit trail for a specific address — all writes with calling context."
    )]
    async fn forensic_memory_audit(
        &self,
        params: Parameters<ForensicMemoryAuditParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let engines = self.engines.lock().await;

        let engine = match engines.get(&params.session_id) {
            Some(e) => e,
            None => {
                return Ok(CallToolResult::error(text_content(format!(
                    "Session '{}' not found",
                    params.session_id
                ))))
            }
        };

        // Get all events and find memory writes to this address
        let all_events = engine.get_all_events();
        let mut writes = Vec::new();

        for event in &all_events {
            if let EventData::Memory { address, data, .. } = &event.data {
                if *address == params.address {
                    // Get call stack at this event
                    let stack = engine.reconstruct_call_stack(event.event_id);
                    let hex = data
                        .as_ref()
                        .map(|d| d.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(""))
                        .unwrap_or_default();
                    writes.push(serde_json::json!({
                        "timestamp_ns": event.timestamp_ns,
                        "event_id": event.event_id,
                        "data_hex": hex,
                        "call_stack": stack.iter().map(|f| serde_json::json!({
                            "depth": f.depth,
                            "function": f.function,
                            "file": f.file,
                            "line": f.line,
                        })).collect::<Vec<_>>(),
                    }));
                }
            }
        }

        writes.sort_by_key(|w| w["timestamp_ns"].as_u64().unwrap_or(0));
        writes.truncate(params.limit);

        let output = serde_json::json!({
            "address": format!("0x{:x}", params.address),
            "write_count": writes.len(),
            "writes": writes,
        });
        Ok(CallToolResult::success(json_content(&output)))
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
        let condition = params.condition.into_condition();
        let id = self.tripwire_manager.register(condition);

        let output = serde_json::json!({
            "tripwire_id": id.to_string(),
            "status": "registered",
            "active_count": self.tripwire_manager.active_count(),
            "label": params.label,
        });
        Ok(CallToolResult::success(json_content(&output)))
    }

    #[tool(
        name = "tripwire_list",
        description = "List all active tripwires and any that have fired since the last call. Returns tripwire definitions and a list of fired notifications with event context."
    )]
    async fn tripwire_list(
        &self,
        _params: Parameters<NoParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let tripwires = self.tripwire_manager.list();
        let fired = self.tripwire_manager.drain_fired();

        let tripwire_summaries: Vec<_> = tripwires
            .iter()
            .map(|tw| {
                serde_json::json!({
                    "id": tw.id.to_string(),
                    "label": tw.label,
                    "condition": format!("{:?}", tw.condition),
                    "fire_count": tw.fire_count,
                })
            })
            .collect();

        let fired_events: Vec<_> = fired
            .iter()
            .map(|f| {
                serde_json::json!({
                    "tripwire_id": f.tripwire_id.to_string(),
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
            "total_active": tripwires.len(),
            "fired_count": fired.len(),
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

        // Parse the tripwire ID (format: "tripwire-N")
        let id_str = params.tripwire_id.trim();
        let id_num: u64 = if id_str.starts_with("tripwire-") {
            id_str["tripwire-".len()..]
                .parse()
                .map_err(|_| rmcp::ErrorData::invalid_params(
                    format!("Invalid tripwire ID format: '{}'", params.tripwire_id),
                    None,
                ))?
        } else {
            return Ok(CallToolResult::error(text_content(format!(
                "Invalid tripwire ID format '{}'. Expected format: 'tripwire-<number>'",
                params.tripwire_id
            ))));
        };

        let id = TripwireId(id_num);
        let removed = self.tripwire_manager.remove(id);

        if removed {
            let output = serde_json::json!({
                "tripwire_id": params.tripwire_id,
                "status": "deleted",
                "remaining_active": self.tripwire_manager.active_count(),
            });
            Ok(CallToolResult::success(json_content(&output)))
        } else {
            Ok(CallToolResult::error(text_content(format!(
                "Tripwire '{}' not found",
                params.tripwire_id
            ))))
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
        let tripwires = self.tripwire_manager.list();

        let tripwire_summaries: Vec<_> = tripwires
            .iter()
            .map(|tw| {
                serde_json::json!({
                    "id": tw.id.to_string(),
                    "label": tw.label,
                    "condition": format!("{:?}", tw.condition),
                    "fire_count": tw.fire_count,
                })
            })
            .collect();

        let output = serde_json::json!({
            "active_tripwires": tripwire_summaries,
            "total_active": tripwires.len(),
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

        // Validate the program path
        if let Err(e) = crate::security::validate_program_path(&params.program) {
            return Ok(CallToolResult::error(text_content(format!(
                "Invalid program path: {}",
                e
            ))));
        }

        let session_id = uuid::Uuid::new_v4().to_string();
        let language = Language::from_path(&params.program);

        // Build capture config
        let mut config = CaptureConfig::new(&params.program);
        config.args = params.args;
        config.capture_syscalls = params.trace_syscalls;
        config.language = Some(language);

        if let Some(ref cwd) = params.cwd {
            config.cwd = Some(PathBuf::from(cwd));
        }

        // Create a fresh EventBus for this session
        let bus = chronos_domain::bus::EventBus::new_shared(params.bus_capacity);
        let backend = NativeProbeBackend::new(bus).with_language(language);

        // Start the probe (non-blocking — spawns background thread)
        let session = match backend.start_probe(config) {
            Ok(s) => s,
            Err(e) => {
                return Ok(CallToolResult::error(text_content(format!(
                    "Failed to start probe: {}",
                    e
                ))))
            }
        };

        info!(
            "Live probe started for '{}' (session: {}, bus capacity: {})",
            params.program, session_id, params.bus_capacity
        );

        // Store the live probe session
        let live_probe = LiveProbeSession {
            backend,
            session,
            language,
            target: params.program.clone(),
        };
        // Insert first so the session is immediately queryable, then mark as active.
        // Brief inconsistency window is benign in single-client MCP usage.
        self.live_probes
            .lock()
            .unwrap()
            .insert(session_id.clone(), live_probe);

        // Set as active session
        {
            let mut active = self.active_session.lock().await;
            *active = Some(session_id.clone());
        }

        let output = serde_json::json!({
            "session_id": session_id,
            "status": "running",
            "target": params.program,
            "language": format!("{:?}", language),
            "bus_capacity": params.bus_capacity,
            "hint": "Use probe_drain to read events in real-time, probe_stop to finalize."
        });
        Ok(CallToolResult::success(json_content(&output)))
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

        // Remove the live probe session
        let live_probe = {
            let mut probes = self.live_probes.lock().unwrap();
            probes.remove(&params.session_id)
        };

        let live_probe = match live_probe {
            Some(lp) => lp,
            None => {
                return Ok(CallToolResult::error(text_content(format!(
                    "Live probe session '{}' not found. It may have already been stopped.",
                    params.session_id
                ))))
            }
        };

        // Drain final raw events from the bus (for QueryEngine)
        // Note: drain_events() returns SemanticEvent for LLM-facing tools,
        // but QueryEngine needs TraceEvent, so we use drain_raw_events()
        // Order matters: drain BEFORE stop_probe to avoid losing events if stop has side effects.
        let events: Vec<TraceEvent> = live_probe.backend.drain_raw_events();

        // Stop the probe thread
        if let Err(e) = live_probe.backend.stop_probe(&live_probe.session) {
            tracing::warn!("Probe stop error for session {}: {}", params.session_id, e);
        }

        let total_events = events.len();
        let language = live_probe.language;
        let target = live_probe.target;

        // Compute duration before moving events
        let duration_ms = if let (Some(first), Some(last)) = (events.first(), events.last()) {
            last.timestamp_ns.saturating_sub(first.timestamp_ns) / 1_000_000
        } else {
            0
        };

        info!(
            "Live probe stopped for '{}' (session: {}, events: {})",
            target, params.session_id, total_events
        );

        // Build and store the query engine with proper noise filtering
        self.build_and_store_engine(&params.session_id, events, language).await;

        let output = serde_json::json!({
            "session_id": params.session_id,
            "status": "stopped",
            "target": target,
            "total_events": total_events,
            "duration_ms": duration_ms,
            "hint": "Session is now queryable. Use query_events, get_call_stack, etc."
        });
        Ok(CallToolResult::success(json_content(&output)))
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

        // Narrow lock scope: extract events inside scoped block, then drop lock before processing.
        // This avoids holding std::sync::Mutex across non-trivial event processing.
        let events: Vec<SemanticEvent> = {
            let probes = self.live_probes.lock().unwrap();
            let live_probe = match probes.get(&params.session_id) {
                Some(lp) => lp,
                None => {
                    return Ok(CallToolResult::error(text_content(format!(
                        "Live probe session '{}' not found.",
                        params.session_id
                    ))))
                }
            };
            match live_probe.backend.drain_events() {
                Ok(e) => e,
                Err(e) => {
                    return Ok(CallToolResult::error(text_content(format!(
                        "Failed to drain events: {}",
                        e
                    ))))
                }
            }
        }; // lock dropped here — process events without holding mutex

        let total = events.len();
        // Apply offset/limit
        let sliced: Vec<_> = events
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
            "total_buffered": total,
            "returned": sliced.len(),
            "offset": params.offset,
            "limit": params.limit,
            "events": sliced,
            "hint": "Probe is still running. Call probe_drain again for more events, or probe_stop to finalize."
        });
        Ok(CallToolResult::success(json_content(&output)))
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

        // Scope the sync mutex lock — drain events, then drop before any async
        let (events, language) = {
            let probes = self.live_probes.lock().unwrap();
            let live_probe = match probes.get(&params.session_id) {
                Some(lp) => lp,
                None => {
                    return Ok(CallToolResult::error(text_content(format!(
                        "Live probe session '{}' not found.",
                        params.session_id
                    ))))
                }
            };

            // Use drain_raw_events() because QueryEngine needs TraceEvent,
            // but drain_events() (ProbeBackend trait) returns SemanticEvent
            let events: Vec<TraceEvent> = live_probe.backend.drain_raw_events();
            let lang = live_probe.language;
            (events, lang)
        }; // probes lock dropped here

        let total_events = events.len();

        // Build and store the query engine with proper noise filtering
        self.build_and_store_engine(&params.session_id, events, language).await;

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

    #[tool(
        name = "probe_inject",
        description = "Inject a uprobe into a running process via eBPF (requires root/CAP_BPF)"
    )]
    async fn probe_inject(
        &self,
        params: Parameters<ProbeInjectParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        // Look up the live probe session to get the target PID
        let target_pid = {
            let probes = self.live_probes.lock().unwrap();
            match probes.get(&params.session_id) {
                Some(lp) => {
                    // For spawned probes, lp.session.pid is 0. Use the actual traced PID.
                    lp.backend.get_traced_pid()
                        .map(|p| p as u32)
                        .unwrap_or(lp.session.pid)
                }
                None => {
                    return Ok(CallToolResult::error(text_content(format!(
                        "Live probe session '{}' not found. Start a probe with probe_start first.",
                        params.session_id
                    ))))
                }
            }
        };

        let pid = params.pid.unwrap_or(target_pid);

        if pid == 0 {
            return Ok(CallToolResult::error(text_content(
                "Cannot inject: probe is still starting up (PID not yet known). Retry in a moment."
            )));
        }

        // Attempt eBPF uprobe injection
        match chronos_ebpf::EbpfAdapter::new() {
            Ok(adapter) => {
                match adapter.attach_uprobe(pid, &params.binary_path, &params.symbol_name) {
                    Ok(()) => {
                        let output = serde_json::json!({
                            "session_id": params.session_id,
                            "binary_path": params.binary_path,
                            "symbol_name": params.symbol_name,
                            "probes_attached": 1u32,
                            "message": format!(
                                "uprobe attached to '{}' in '{}' (pid {})",
                                params.symbol_name, params.binary_path, pid
                            ),
                        });
                        Ok(CallToolResult::success(json_content(&output)))
                    }
                    Err(e) => Ok(CallToolResult::error(text_content(format!(
                        "Failed to attach uprobe for '{}' in '{}': {}",
                        params.symbol_name, params.binary_path, e
                    )))),
                }
            }
            Err(e) => {
                Err(rmcp::ErrorData::invalid_params(
                    format!("eBPF not available on this system: {}", e),
                    None,
                ))
            }
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
        let top_n = params.top_n.unwrap_or(20);

        // Helper: load a session into an engine (reuse existing if already loaded).
        let load_engine = |session_id: &str| -> Result<QueryEngine, String> {
            let (_, events) = self
                .store
                .load_session(session_id)
                .map_err(|e| format!("session '{}' not found: {}", session_id, e))?;
            Ok(QueryEngine::new(events))
        };

        let baseline_engine = match load_engine(&params.baseline_session_id) {
            Ok(e) => e,
            Err(msg) => return Ok(CallToolResult::error(text_content(msg))),
        };
        let target_engine = match load_engine(&params.target_session_id) {
            Ok(e) => e,
            Err(msg) => return Ok(CallToolResult::error(text_content(msg))),
        };

        // Build function-call-count maps from execution summaries.
        let baseline_summary = baseline_engine.execution_summary(&params.baseline_session_id);
        let target_summary = target_engine.execution_summary(&params.target_session_id);

        let baseline_map: HashMap<&str, u64> = baseline_summary
            .top_functions
            .iter()
            .take(top_n)
            .map(|f| (f.name.as_str(), f.call_count))
            .collect();
        let target_map: HashMap<&str, u64> = target_summary
            .top_functions
            .iter()
            .take(top_n)
            .map(|f| (f.name.as_str(), f.call_count))
            .collect();

        // Union of all function names from both sessions.
        let all_functions: HashSet<&str> = baseline_map
            .keys()
            .chain(target_map.keys())
            .copied()
            .collect();

        let functions_analyzed = all_functions.len();
        let mut regressions: Vec<FunctionRegressionEntry> = Vec::new();
        let mut improvements: Vec<FunctionRegressionEntry> = Vec::new();
        let mut total_baseline_calls: i64 = 0;
        let mut total_target_calls: i64 = 0;

        for func_name in &all_functions {
            let baseline_calls = baseline_map.get(func_name).copied().unwrap_or(0);
            let target_calls = target_map.get(func_name).copied().unwrap_or(0);

            total_baseline_calls += baseline_calls as i64;
            total_target_calls += target_calls as i64;

            // Skip functions that appear in only one session.
            if baseline_calls == 0 || target_calls == 0 {
                continue;
            }

            let delta_pct =
                ((target_calls as f64 - baseline_calls as f64) / baseline_calls as f64) * 100.0;

            let entry = FunctionRegressionEntry {
                function: func_name.to_string(),
                baseline_calls,
                target_calls,
                call_delta_pct: (delta_pct * 100.0).round() / 100.0,
            };

            if delta_pct > 50.0 {
                regressions.push(entry);
            } else if delta_pct < -50.0 {
                improvements.push(entry);
            }
        }

        // Sort: regressions descending, improvements ascending (most-improved first).
        regressions.sort_by(|a, b| {
            b.call_delta_pct
                .partial_cmp(&a.call_delta_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        improvements.sort_by(|a, b| {
            a.call_delta_pct
                .partial_cmp(&b.call_delta_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let total_call_delta = total_target_calls - total_baseline_calls;

        let summary = if regressions.is_empty() {
            format!(
                "No significant regressions found. Target had {} total calls vs {} in baseline.",
                total_target_calls, total_baseline_calls
            )
        } else {
            let top = &regressions[0];
            format!(
                "Found {} significant regression(s). Top: '{}' increased by {:.0}%. \
                 Target: {} total calls vs baseline: {}.",
                regressions.len(),
                top.function,
                top.call_delta_pct,
                total_target_calls,
                total_baseline_calls
            )
        };

        let result = PerformanceRegressionAuditResult {
            baseline_session_id: params.baseline_session_id,
            target_session_id: params.target_session_id,
            regressions,
            improvements,
            functions_analyzed,
            total_call_delta,
            summary,
        };
        match serde_json::to_value(result) {
            Ok(v) => Ok(CallToolResult::success(json_content(&v))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("Serialization error: {}", e)))),
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

        let (meta_a, events_a) = match self.store.load_session(&params.session_a) {
            Ok(r) => r,
            Err(e) => {
                return Ok(CallToolResult::error(text_content(format!(
                    "session A not found: {}",
                    e
                ))))
            }
        };
        let (meta_b, events_b) = match self.store.load_session(&params.session_b) {
            Ok(r) => r,
            Err(e) => {
                return Ok(CallToolResult::error(text_content(format!(
                    "session B not found: {}",
                    e
                ))))
            }
        };

        let report = TraceDiff::compare(
            &params.session_a,
            &params.session_b,
            &events_a,
            &events_b,
            &meta_a,
            &meta_b,
        );

        let summary = if report.similarity_pct >= 90.0 {
            format!(
                "Sessions are highly similar ({}%). Most events match.",
                report.similarity_pct.round()
            )
        } else if report.similarity_pct >= 50.0 {
            format!(
                "Sessions differ in {} events (only_in_b) vs {} (only_in_a). {}% similar.",
                report.only_in_b.len(),
                report.only_in_a.len(),
                report.similarity_pct.round()
            )
        } else {
            format!(
                "Sessions are largely different. {}% similar with {} events only in B and {} only in A.",
                report.similarity_pct.round(),
                report.only_in_b.len(),
                report.only_in_a.len()
            )
        };

        let output = serde_json::json!({
            "session_a_id": params.session_a,
            "session_b_id": params.session_b,
            "only_in_a_count": report.only_in_a.len(),
            "only_in_b_count": report.only_in_b.len(),
            "total_a": events_a.len(),
            "total_b": events_b.len(),
            "common_count": report.common_count,
            "similarity_pct": report.similarity_pct,
            "timing_delta_ms": report.timing_delta.as_ref().map(|t| t.delta_ms),
            "summary": summary,
        });
        Ok(CallToolResult::success(json_content(&output)))
    }


}


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
    async fn server_with_session(events: Vec<TraceEvent>) -> (ChronosServer, String) {
        let server = ChronosServer::new();
        let session_id = "test-session-sf4".to_string();
        server.build_and_store_engine(&session_id, events, Language::C).await;
        (server, session_id)
    }

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
            },
        )
    }

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
        assert!(s.contains("unique_functions") || result.content.len() > 0);
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
        assert!(text.contains("race_count"));
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
        server.build_and_store_engine(&sid, events.clone(), Language::C).await;

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
        server.build_and_store_engine(&sid, events, Language::Python).await;

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
        server.build_and_store_engine(&sid, events, Language::C).await;
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
        server.build_and_store_engine(&sid1, events.clone(), Language::C).await;
        server.build_and_store_engine(&sid2, events.clone(), Language::C).await;

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
            chronos_domain::VariableInfo::new("x", "10", "i32", 0x1000, chronos_domain::VariableScope::Local),
            chronos_domain::VariableInfo::new("y", "3", "i32", 0x2000, chronos_domain::VariableScope::Local),
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
        let locals = vec![
            chronos_domain::VariableInfo::new("x", "10", "i32", 0x1000, chronos_domain::VariableScope::Local),
        ];
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
        let locals = vec![
            chronos_domain::VariableInfo::new("n", "0", "i32", 0x1000, chronos_domain::VariableScope::Local),
        ];
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
            chronos_domain::VariableInfo::new("a", "5", "i32", 0x1000, chronos_domain::VariableScope::Local),
            chronos_domain::VariableInfo::new("b", "3", "i32", 0x2000, chronos_domain::VariableScope::Local),
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
        let locals = vec![
            chronos_domain::VariableInfo::new("count", "42", "int", 0x1000, chronos_domain::VariableScope::Local),
        ];
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
        let event = TraceEvent::python_call(0, 100, 1, "my_module.my_func", "/path/to/script.py", 10);
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
        let events = vec![
            make_test_memory_event(1, 1000, 1, 0x7FFF0000, 4, vec![0x01, 0x02, 0x03, 0x04]),
        ];
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

    fn make_register_event(id: u64, ts: u64, tid: u64, regs: chronos_domain::RegisterState) -> TraceEvent {
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
        use chronos_query::QueryEngine;
        use chronos_index::builder::IndexBuilder;

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
        let locals_a = vec![
            chronos_domain::VariableInfo::new("x", "10", "i32", 0x1000, VariableScope::Local),
        ];
        let locals_b = vec![
            chronos_domain::VariableInfo::new("x", "20", "i32", 0x1000, VariableScope::Local),
        ];
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
        let events = vec![
            make_test_memory_event(1, 1000, 1, 0x1000, 4, vec![0x01]),
        ];
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
        server.build_and_store_engine(&sid, events, Language::Python).await;

        // Verify it's registered in memory
        assert!(server.engines.lock().await.contains_key(&sid));
        assert!(server.session_languages.lock().await.contains_key(&sid));

        // Drop the session
        let result = server
            .drop_session(Parameters(DropSessionParams { session_id: sid.clone() }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(text.contains("dropped"), "Should contain 'dropped' status");
        assert!(text.contains("Persistent storage not affected"), "Should mention storage not affected");

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
        assert!(text.contains("not_found"), "Should contain 'not_found' status");
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
                EventData::Function { name: func.to_string(), signature: None },
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
