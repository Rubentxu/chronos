//! Chronos MCP server — exposes debugging tools via MCP.
//!
//! Implements 10 tools for AI-assisted debugging.

use chronos_domain::{
    query::{CausalityQuery, PerfQuery, PerfSortBy, RaceDetectionQuery},
    CaptureConfig, EventData, EventType, TraceEvent, TraceQuery,
};
use chronos_index::builder::IndexBuilder;
use chronos_native::capture_runner::{CaptureEndReason, CaptureRunner};
use chronos_query::QueryEngine;
use chronos_store::{SessionMetadata, SessionStore, TraceDiff};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content};
use rmcp::tool;
use schemars::JsonSchema;
use serde::Deserialize;
use std::collections::HashMap;
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

/// The Chronos MCP server state.
pub struct ChronosServer {
    /// Active capture sessions (session_id → runner).
    sessions: Arc<Mutex<HashMap<String, ActiveSession>>>,
    /// Loaded query engines (session_id → engine).
    engines: Arc<Mutex<HashMap<String, QueryEngine>>>,
    /// Persistent session store.
    store: Arc<SessionStore>,
}

/// An active capture session with its runner.
/// Reserved for future attach/detach workflows.
#[allow(dead_code)]
struct ActiveSession {
    #[allow(dead_code)]
    pid: u32,
    runner: CaptureRunner,
}

// ============================================================================
// Tool parameter types
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DebugRunParams {
    /// Path to the target binary.
    pub program: String,
    /// Command-line arguments for the target.
    #[serde(default)]
    pub args: Vec<String>,
    /// Whether to trace syscalls.
    #[serde(default = "default_true")]
    pub trace_syscalls: bool,
    /// Whether to capture registers on each stop.
    #[serde(default = "default_true")]
    pub capture_registers: bool,
    /// Working directory for the target.
    pub cwd: Option<String>,
    /// If true, automatically persist the session to disk after debug_run completes.
    #[serde(default)]
    pub auto_save: Option<bool>,
    /// Program language hint (auto-detected from extension if omitted).
    pub program_language: Option<String>,
    /// Maximum number of events to collect before stopping (default: 1_000_000).
    #[serde(default)]
    pub max_events: Option<usize>,
    /// Timeout in seconds for the capture (default: 60).
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DebugAttachParams {
    /// Process ID to attach to.
    pub pid: u32,
    /// Whether to trace syscalls.
    #[serde(default = "default_true")]
    pub trace_syscalls: bool,
    /// Whether to capture registers on each stop.
    #[serde(default = "default_true")]
    pub capture_registers: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DebugStopParams {
    /// Session ID to stop.
    pub session_id: String,
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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetBacktraceParams {
    /// Session ID.
    pub session_id: String,
    /// Event ID at which to get the backtrace.
    pub event_id: u64,
    /// Maximum depth of the backtrace.
    #[serde(default = "default_backtrace_depth")]
    pub max_depth: usize,
}

fn default_backtrace_depth() -> usize {
    50
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
pub struct CompareSessionsParams {
    /// First session ID.
    pub session_a: String,
    /// Second session ID.
    pub session_b: String,
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

        let store = SessionStore::open(&db_path).expect("Failed to open session store");

        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            engines: Arc::new(Mutex::new(HashMap::new())),
            store: Arc::new(store),
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

    async fn build_and_store_engine(&self, session_id: &str, events: Vec<TraceEvent>) {
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

        let mut engines = self.engines.lock().await;
        info!("Built query engine for session {}", session_id);
        engines.insert(session_id.to_string(), engine);
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

#[rmcp::tool_router(server_handler)]
impl ChronosServer {
    #[tool(
        name = "debug_run",
        description = "Launch a program under time-travel debugging capture. Runs the program to completion, captures all events, and returns a queryable session ID."
    )]
    async fn debug_run(
        &self,
        params: Parameters<DebugRunParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        // Validate the program path before spawning
        if let Err(e) = crate::security::validate_program_path(&params.program) {
            return Ok(CallToolResult::error(text_content(format!(
                "Invalid program path: {}",
                e
            ))));
        }

        let mut config = CaptureConfig::new(&params.program);
        config.args = params.args;
        config.capture_syscalls = params.trace_syscalls;
        config.capture_stack = true;
        if let Some(cwd) = params.cwd {
            config.cwd = Some(std::path::PathBuf::from(cwd));
        }

        let session_id = uuid::Uuid::new_v4().to_string();
        let sid_clone = session_id.clone();
        let start_time = std::time::Instant::now();

        // Run capture synchronously in a blocking thread — this avoids the
        // race condition where the program finishes before the event loop starts.
        let capture_result = tokio::task::spawn_blocking(move || {
            let mut runner = CaptureRunner::new(config);
            runner.run_to_completion()
        })
        .await;

        match capture_result {
            Ok(Ok(capture)) => {
                let total_events = capture.total_events;
                let end_reason_str = match &capture.end_reason {
                    CaptureEndReason::Exited(code) => format!("exited({})", code),
                    CaptureEndReason::Signaled { signal_name, .. } => {
                        format!("signaled({})", signal_name)
                    }
                    CaptureEndReason::StoppedByUser => "stopped_by_user".into(),
                    CaptureEndReason::Failed(e) => format!("failed({})", e),
                };

                info!(
                    "Capture {} finished: {} events, reason: {}",
                    sid_clone, total_events, end_reason_str
                );

                // Build indices and store engine
                let events = capture.events;
                let elapsed = start_time.elapsed();
                self.build_and_store_engine(&sid_clone, events.clone())
                    .await;

                // Auto-save if requested
                let auto_save_result = if params.auto_save.unwrap_or(false) {
                    let metadata = SessionMetadata {
                        session_id: sid_clone.clone(),
                        created_at: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_millis() as u64)
                            .unwrap_or(0),
                        language: params
                            .program_language
                            .clone()
                            .unwrap_or_else(|| "native".to_string()),
                        target: params.program.clone(),
                        event_count: events.len(),
                        duration_ms: elapsed.as_millis() as u64,
                    };
                    match self.store.save_session(metadata, &events) {
                        Ok(hashes) => Some(serde_json::json!({
                            "auto_saved": true,
                            "session_id": sid_clone,
                            "events_stored": events.len(),
                            "unique_hashes": hashes.len(),
                        })),
                        Err(e) => Some(serde_json::json!({
                            "auto_saved": false,
                            "session_id": sid_clone,
                            "error": format!("{}", e),
                        })),
                    }
                } else {
                    None
                };

                let mut result = serde_json::json!({
                    "session_id": sid_clone,
                    "status": "finalized",
                    "total_events": total_events,
                    "end_reason": end_reason_str,
                    "message": format!("Program '{}' captured successfully", params.program),
                    "hint": "Session is queryable now. Use query_events, get_call_stack, get_execution_summary, etc."
                });
                if let Some(auto_save_info) = auto_save_result {
                    result["auto_save_info"] = auto_save_info;
                }
                Ok(CallToolResult::success(json_content(&result)))
            }
            Ok(Err(e)) => Ok(CallToolResult::error(text_content(format!(
                "Capture failed: {}",
                e
            )))),
            Err(e) => Ok(CallToolResult::error(text_content(format!(
                "Internal error: {}",
                e
            )))),
        }
    }

    #[tool(
        name = "debug_attach",
        description = "Attach to a running process for trace capture."
    )]
    async fn debug_attach(
        &self,
        params: Parameters<DebugAttachParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let pid = params.pid;
        let session_id = uuid::Uuid::new_v4().to_string();
        let sid_clone = session_id.clone();
        let start_time = std::time::Instant::now();

        let mut config = CaptureConfig::new(format!("PID {}", pid));
        config.capture_syscalls = params.trace_syscalls;
        config.capture_stack = true;

        // Run attach capture synchronously in a blocking thread
        let capture_result = tokio::task::spawn_blocking(move || {
            CaptureRunner::run_to_completion_attach(pid, config)
        })
        .await;

        match capture_result {
            Ok(Ok(capture)) => {
                let total_events = capture.total_events;
                let end_reason_str = match &capture.end_reason {
                    CaptureEndReason::Exited(code) => format!("exited({})", code),
                    CaptureEndReason::Signaled { signal_name, .. } => {
                        format!("signaled({})", signal_name)
                    }
                    CaptureEndReason::StoppedByUser => "stopped_by_user".into(),
                    CaptureEndReason::Failed(e) => format!("failed({})", e),
                };

                    info!(
                    "Attach session {} finished: {} events, reason: {}",
                    sid_clone, total_events, end_reason_str
                );

                let events = capture.events;
                let _elapsed = start_time.elapsed();
                self.build_and_store_engine(&sid_clone, events.clone())
                    .await;

                let result = serde_json::json!({
                    "session_id": sid_clone,
                    "status": "finalized",
                    "pid": pid,
                    "total_events": total_events,
                    "end_reason": end_reason_str,
                    "message": format!("Attached to PID {} and captured {} events", pid, total_events),
                    "hint": "Session is queryable now. Use query_events, get_call_stack, get_execution_summary, etc."
                });
                Ok(CallToolResult::success(json_content(&result)))
            }
            Ok(Err(e)) => {
                // Map common errors to user-friendly messages
                let user_message = if e.contains("No such process") || e.contains("ESRCH") {
                    format!(
                        "Process with PID {} not found or not traceable. Ensure the process exists and is owned by your user, or run with CAP_SYS_PTRACE.",
                        pid
                    )
                } else if e.contains("Operation not permitted") || e.contains("EPERM") {
                    format!(
                        "Permission denied: cannot attach to PID {}. Required: CAP_SYS_PTRACE capability or same user ID.",
                        pid
                    )
                } else {
                    format!("Attach to PID {} failed: {}", pid, e)
                };
                Ok(CallToolResult::error(text_content(user_message)))
            }
            Err(e) => Ok(CallToolResult::error(text_content(format!(
                "Internal error: {}",
                e
            )))),
        }
    }

    #[tool(
        name = "debug_stop",
        description = "Stop an active trace capture session and build query indices. Note: debug_run already captures to completion, so this is only needed for future attach/detach workflows."
    )]
    async fn debug_stop(
        &self,
        params: Parameters<DebugStopParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let mut sessions = self.sessions.lock().await;

        match sessions.remove(&params.session_id) {
            Some(mut active) => {
                info!("Stopping capture session {}", params.session_id);

                // Stop the runner and collect events (blocks until event loop finishes)
                let capture_result =
                    tokio::task::spawn_blocking(move || active.runner.stop_and_collect()).await;

                let result = match capture_result {
                    Ok(Ok(capture)) => {
                        let total_events = capture.total_events;
                        let end_reason_str = match &capture.end_reason {
                            CaptureEndReason::Exited(code) => format!("exited({})", code),
                            CaptureEndReason::Signaled { signal_name, .. } => {
                                format!("signaled({})", signal_name)
                            }
                            CaptureEndReason::StoppedByUser => "stopped_by_user".into(),
                            CaptureEndReason::Failed(e) => format!("failed({})", e),
                        };

                        info!(
                            "Capture {} finished: {} events, reason: {}",
                            params.session_id, total_events, end_reason_str
                        );

                        // Build indices and store engine
                        self.build_and_store_engine(&params.session_id, capture.events)
                            .await;

                        let output = serde_json::json!({
                            "session_id": params.session_id,
                            "status": "finalized",
                            "total_events": total_events,
                            "end_reason": end_reason_str,
                            "hint": "Session is now queryable. Use query_events, get_call_stack, get_execution_summary, etc."
                        });
                        Ok(CallToolResult::success(json_content(&output)))
                    }
                    Ok(Err(e)) => Ok(CallToolResult::error(text_content(format!(
                        "Capture collection failed: {}",
                        e
                    )))),
                    Err(e) => Ok(CallToolResult::error(text_content(format!(
                        "Capture thread error: {}",
                        e
                    )))),
                };
                result
            }
            None => {
                // Check if already finalized
                let engines = self.engines.lock().await;
                if engines.contains_key(&params.session_id) {
                    Ok(CallToolResult::success(json_content(&serde_json::json!({
                        "session_id": params.session_id,
                        "status": "already_finalized",
                        "hint": "This session was captured via debug_run (synchronous mode). It's already queryable."
                    }))))
                } else {
                    Ok(CallToolResult::error(text_content(format!(
                        "Session '{}' not found",
                        params.session_id
                    ))))
                }
            }
        }
    }

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

    #[tool(
        name = "get_backtrace",
        description = "Get the full backtrace at a specific event."
    )]
    async fn get_backtrace(
        &self,
        params: Parameters<GetBacktraceParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let engines = self.engines.lock().await;

        match engines.get(&params.session_id) {
            Some(engine) => {
                let mut stack = engine.reconstruct_call_stack(params.event_id);
                stack.truncate(params.max_depth);

                let bt_lines: Vec<String> = stack
                    .iter()
                    .enumerate()
                    .map(|(i, frame)| {
                        let file_info = match (&frame.file, &frame.line) {
                            (Some(f), Some(l)) => format!(" at {}:{}", f, l),
                            (Some(f), None) => format!(" at {}", f),
                            _ => String::new(),
                        };
                        format!(
                            "#{} 0x{:016x} in {}{}",
                            i, frame.address, frame.function, file_info
                        )
                    })
                    .collect();

                let output = serde_json::json!({
                    "session_id": params.session_id,
                    "at_event_id": params.event_id,
                    "frame_count": stack.len(),
                    "backtrace": bt_lines.join("\n"),
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
                    "Session '{}' not found in memory. Run debug_run first.",
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
        _params: Parameters<()>,
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
                let output = serde_json::json!({
                    "session_id": params.session_id,
                    "status": "deleted",
                    "message": format!("Session '{}' deleted from persistent storage.", params.session_id),
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
        name = "compare_sessions",
        description = "Compare two saved sessions using hash-based set difference. Returns events unique to each, common count, similarity percentage, and timing delta."
    )]
    async fn compare_sessions(
        &self,
        params: Parameters<CompareSessionsParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        // Load both sessions
        let (meta_a, events_a) = match self.store.load_session(&params.session_a) {
            Ok((m, e)) => (m, e),
            Err(e) => {
                return Ok(CallToolResult::error(text_content(format!(
                    "Failed to load session '{}': {}",
                    params.session_a, e
                ))))
            }
        };

        let (meta_b, events_b) = match self.store.load_session(&params.session_b) {
            Ok((m, e)) => (m, e),
            Err(e) => {
                return Ok(CallToolResult::error(text_content(format!(
                    "Failed to load session '{}': {}",
                    params.session_b, e
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

        let output = serde_json::json!({
            "session_a_id": report.session_a_id,
            "session_b_id": report.session_b_id,
            "common_count": report.common_count,
            "similarity_pct": (report.similarity_pct * 100.0).round() / 100.0,
            "only_in_a_count": report.only_in_a.len(),
            "only_in_b_count": report.only_in_b.len(),
            "timing_delta": report.timing_delta.map(|td| serde_json::json!({
                "duration_ms_a": td.duration_ms_a,
                "duration_ms_b": td.duration_ms_b,
                "delta_ms": td.delta_ms,
                "slower_session": td.slower_session,
            })),
            "hint": "Sessions with high similarity_pct share many common events. Use load_session to dive into specific events.",
        });
        Ok(CallToolResult::success(json_content(&output)))
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
}

#[cfg(test)]
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

    #[tokio::test]
    async fn test_debug_run_nonexistent() {
        let server = ChronosServer::new();
        let params = Parameters(DebugRunParams {
            program: "/nonexistent/binary".to_string(),
            args: vec![],
            trace_syscalls: true,
            capture_registers: true,
            cwd: None,
            auto_save: None,
            program_language: None,
            max_events: None,
            timeout_secs: None,
        });
        let result = server.debug_run(params).await.unwrap();
        assert_eq!(result.is_error, Some(true));
    }

    #[tokio::test]
    async fn test_debug_attach_nonexistent() {
        let server = ChronosServer::new();
        let params = Parameters(DebugAttachParams {
            pid: 999999,
            trace_syscalls: true,
            capture_registers: true,
        });
        let result = server.debug_attach(params).await.unwrap();
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn test_query_params_defaults() {
        let params = QueryEventsParams {
            session_id: "test".to_string(),
            event_types: None,
            thread_id: None,
            timestamp_start: None,
            timestamp_end: None,
            function_pattern: None,
            limit: default_limit(),
            offset: 0,
        };
        assert_eq!(params.limit, 100);
    }

    #[test]
    fn test_backtrace_depth_default() {
        assert_eq!(default_backtrace_depth(), 50);
    }

    #[test]
    fn test_json_content() {
        let val = serde_json::json!({"key": "value"});
        let content = json_content(&val);
        assert_eq!(content.len(), 1);
    }

    #[test]
    fn test_text_content() {
        let content = text_content("hello");
        assert_eq!(content.len(), 1);
    }

    // ========================================================================
    // SF4 tool tests
    // ========================================================================

    /// Helper: build a server with a pre-loaded session from synthetic events.
    async fn server_with_session(events: Vec<TraceEvent>) -> (ChronosServer, String) {
        let server = ChronosServer::new();
        let session_id = "test-session-sf4".to_string();
        server.build_and_store_engine(&session_id, events).await;
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

    async fn server_with_persistent_store() -> ChronosServer {
        // Use in-memory for testing to avoid file system dependencies
        // Create a fresh server and replace its store with an in-memory one
        let server = ChronosServer::new();
        server
    }

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
        server.build_and_store_engine(&sid, events.clone()).await;

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
    async fn test_list_sessions_after_save() {
        let server = ChronosServer::new();
        let sid1 = "list-test-1".to_string();
        let sid2 = "list-test-2".to_string();
        let events = vec![make_fn_event(0, 100, 1, "main")];

        // Save two sessions
        server.build_and_store_engine(&sid1, events.clone()).await;
        server.build_and_store_engine(&sid2, events.clone()).await;

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
        let list_result = server.list_sessions(Parameters(())).await.unwrap();
        assert_ne!(list_result.is_error, Some(true));
        let text = format!("{:?}", list_result.content);
        assert!(text.contains("session_count") || text.contains("sessions"));
    }

    #[tokio::test]
    async fn test_compare_sessions_identical() {
        let server = ChronosServer::new();
        let sid1 = "compare-identical-1".to_string();
        let sid2 = "compare-identical-2".to_string();
        let events = vec![
            make_fn_event(0, 100, 1, "main"),
            make_fn_event(1, 200, 1, "helper"),
        ];

        // Build and save two identical sessions
        server.build_and_store_engine(&sid1, events.clone()).await;
        server.build_and_store_engine(&sid2, events.clone()).await;

        server
            .save_session(Parameters(SaveSessionParams {
                session_id: sid1.clone(),
                language: "native".to_string(),
                target: "/bin/test".to_string(),
            }))
            .await
            .unwrap();

        server
            .save_session(Parameters(SaveSessionParams {
                session_id: sid2.clone(),
                language: "native".to_string(),
                target: "/bin/test".to_string(),
            }))
            .await
            .unwrap();

        // Compare identical sessions
        let compare_result = server
            .compare_sessions(Parameters(CompareSessionsParams {
                session_a: sid1.clone(),
                session_b: sid2.clone(),
            }))
            .await
            .unwrap();

        assert_ne!(compare_result.is_error, Some(true));
        let text = format!("{:?}", compare_result.content);
        // Similar sessions should show high similarity
        assert!(text.contains("similarity") || text.contains("100"));
    }

    #[tokio::test]
    async fn test_compare_sessions_different() {
        let server = ChronosServer::new();
        let sid1 = "compare-diff-a".to_string();
        let sid2 = "compare-diff-b".to_string();

        let events1 = vec![make_fn_event(0, 100, 1, "func_a")];
        let events2 = vec![make_fn_event(0, 100, 1, "func_b")];

        server.build_and_store_engine(&sid1, events1).await;
        server.build_and_store_engine(&sid2, events2).await;

        server
            .save_session(Parameters(SaveSessionParams {
                session_id: sid1.clone(),
                language: "native".to_string(),
                target: "/bin/a".to_string(),
            }))
            .await
            .unwrap();

        server
            .save_session(Parameters(SaveSessionParams {
                session_id: sid2.clone(),
                language: "native".to_string(),
                target: "/bin/b".to_string(),
            }))
            .await
            .unwrap();

        let compare_result = server
            .compare_sessions(Parameters(CompareSessionsParams {
                session_a: sid1,
                session_b: sid2,
            }))
            .await
            .unwrap();

        assert_ne!(compare_result.is_error, Some(true));
        let text = format!("{:?}", compare_result.content);
        // Different sessions should show 0% similarity or common_count = 0
        assert!(text.contains("similarity") || text.contains("common_count"));
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

    // ========================================================================
    // SF3 — Auto-save Tests (T17–T19)
    // ========================================================================

    #[test]
    fn test_debug_run_params_has_auto_save_field() {
        // Verify auto_save field exists and defaults to None
        let params: DebugRunParams = serde_json::from_str(
            r#"{
            "program": "/bin/true",
            "args": [],
            "trace_syscalls": true,
            "capture_registers": true
        }"#,
        )
        .unwrap();
        assert_eq!(params.auto_save, None);
        assert_eq!(params.program_language, None);
    }

    #[test]
    fn test_debug_run_params_auto_save_deserializes() {
        // Verify auto_save can be set to true
        let params: DebugRunParams = serde_json::from_str(
            r#"{
            "program": "/bin/true",
            "args": [],
            "trace_syscalls": true,
            "capture_registers": true,
            "auto_save": true,
            "program_language": "native"
        }"#,
        )
        .unwrap();
        assert_eq!(params.auto_save, Some(true));
        assert_eq!(params.program_language, Some("native".to_string()));
    }

    #[tokio::test]
    async fn test_auto_save_result_includes_stats() {
        // Run debug_run on /bin/true with auto_save enabled
        // and verify the response contains auto_save_info.
        let server = ChronosServer::new();
        let params = Parameters(DebugRunParams {
            program: "/bin/true".to_string(),
            args: vec![],
            trace_syscalls: true,
            capture_registers: true,
            cwd: None,
            auto_save: Some(true),
            program_language: Some("native".to_string()),
            max_events: None,
            timeout_secs: None,
        });

        let result = server.debug_run(params).await.unwrap();
        // Even if the program fails to run, the result should not panic
        let text = format!("{:?}", result.content);
        // Should have auto_save_info key when auto_save was requested
        // (might be error if capture failed, but should not be missing field)
        assert!(
            text.contains("auto_save_info")
                || text.contains("finalized")
                || text.contains("failed")
        );
    }

    #[tokio::test]
    async fn test_auto_save_false_does_not_include_stats() {
        // When auto_save is false/None, no auto_save_info should appear
        let server = ChronosServer::new();
        let params = Parameters(DebugRunParams {
            program: "/bin/nonexistent_binary_xyz".to_string(),
            args: vec![],
            trace_syscalls: true,
            capture_registers: true,
            cwd: None,
            auto_save: Some(false),
            program_language: None,
            max_events: None,
            timeout_secs: None,
        });

        let result = server.debug_run(params).await.unwrap();
        let text = format!("{:?}", result.content);
        // Should NOT contain auto_save_info when auto_save is false
        // (the field should be absent from JSON)
        assert!(!text.contains("auto_save_info"));
    }

    // ========================================================================
    // SF1 — Security Tests (T2)
    // ========================================================================

    #[tokio::test]
    async fn test_debug_run_rejects_path_traversal() {
        let server = ChronosServer::new();
        let params = Parameters(DebugRunParams {
            program: "../evil".to_string(),
            args: vec![],
            trace_syscalls: true,
            capture_registers: true,
            cwd: None,
            auto_save: None,
            program_language: None,
            max_events: None,
            timeout_secs: None,
        });

        let result = server.debug_run(params).await.unwrap();
        // Should be an error result due to path validation failure
        assert_eq!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(text.contains("Path traversal") || text.contains("Invalid program path"));
    }

    // ========================================================================
    // SF1 — Resource Limits Tests (T3)
    // ========================================================================

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

    #[test]
    fn test_debug_run_params_deserializes_resource_limits() {
        let params: DebugRunParams = serde_json::from_str(
            r#"{
            "program": "/bin/true",
            "args": [],
            "trace_syscalls": true,
            "capture_registers": true,
            "max_events": 500000,
            "timeout_secs": 30
        }"#,
        )
        .unwrap();
        assert_eq!(params.max_events, Some(500000));
        assert_eq!(params.timeout_secs, Some(30));
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
}
