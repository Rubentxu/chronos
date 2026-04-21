//! Chronos MCP server — exposes debugging tools via MCP.
//!
//! Implements 10 tools for AI-assisted debugging.

use chronos_domain::{
    CaptureConfig, EventType, TraceEvent, TraceQuery,
};
use chronos_index::builder::IndexBuilder;
use chronos_native::capture_runner::{CaptureRunner, CaptureEndReason};
use chronos_query::QueryEngine;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content};
use rmcp::tool;
use schemars::JsonSchema;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

/// The Chronos MCP server state.
pub struct ChronosServer {
    /// Active capture sessions (session_id → runner).
    sessions: Arc<Mutex<HashMap<String, ActiveSession>>>,
    /// Loaded query engines (session_id → engine).
    engines: Arc<Mutex<HashMap<String, QueryEngine>>>,
}

/// An active capture session with its runner.
struct ActiveSession {
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
// Helpers
// ============================================================================

impl ChronosServer {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            engines: Arc::new(Mutex::new(HashMap::new())),
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
        let mut builder = IndexBuilder::new();
        builder.push_all(&events);
        let indices = builder.finalize();

        let engine = QueryEngine::with_indices(
            events,
            indices.shadow,
            indices.temporal,
        );

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
    vec![Content::text(serde_json::to_string_pretty(value).unwrap_or_default())]
}

fn text_content(text: impl Into<String>) -> Vec<Content> {
    vec![Content::text(text.into())]
}

// ============================================================================
// Tool handlers using rmcp macros
// ============================================================================

#[rmcp::tool_router(server_handler)]
impl ChronosServer {
    #[tool(name = "debug_run", description = "Launch a program under time-travel debugging capture. Runs the program to completion, captures all events, and returns a queryable session ID.")]
    async fn debug_run(
        &self,
        params: Parameters<DebugRunParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let mut config = CaptureConfig::new(&params.program);
        config.args = params.args;
        config.capture_syscalls = params.trace_syscalls;
        config.capture_stack = true;
        if let Some(cwd) = params.cwd {
            config.cwd = Some(std::path::PathBuf::from(cwd));
        }

        let session_id = uuid::Uuid::new_v4().to_string();
        let sid_clone = session_id.clone();

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
                self.build_and_store_engine(&sid_clone, capture.events).await;

                let result = serde_json::json!({
                    "session_id": sid_clone,
                    "status": "finalized",
                    "total_events": total_events,
                    "end_reason": end_reason_str,
                    "message": format!("Program '{}' captured successfully", params.program),
                    "hint": "Session is queryable now. Use query_events, get_call_stack, get_execution_summary, etc."
                });
                Ok(CallToolResult::success(json_content(&result)))
            }
            Ok(Err(e)) => Ok(CallToolResult::error(text_content(format!(
                "Capture failed: {}", e
            )))),
            Err(e) => Ok(CallToolResult::error(text_content(format!(
                "Internal error: {}", e
            )))),
        }
    }

    #[tool(name = "debug_attach", description = "Attach to a running process for trace capture.")]
    async fn debug_attach(
        &self,
        params: Parameters<DebugAttachParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        // Attach-based capture will be supported in a future update.
        // For now, return an informational message.
        Ok(CallToolResult::error(text_content(format!(
            "Attach to PID {} is not yet supported. Use debug_run to launch a program under trace.",
            params.pid
        ))))
    }

    #[tool(name = "debug_stop", description = "Stop an active trace capture session and build query indices. Note: debug_run already captures to completion, so this is only needed for future attach/detach workflows.")]
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
                let capture_result = tokio::task::spawn_blocking(move || {
                    active.runner.stop_and_collect()
                })
                .await;

                let result = match capture_result {
                    Ok(Ok(capture)) => {
                        let total_events = capture.total_events;
                        let end_reason_str = match &capture.end_reason {
                            CaptureEndReason::Exited(code) => format!("exited({})", code),
                            CaptureEndReason::Signaled { signal_name, .. } => format!("signaled({})", signal_name),
                            CaptureEndReason::StoppedByUser => "stopped_by_user".into(),
                            CaptureEndReason::Failed(e) => format!("failed({})", e),
                        };

                        info!(
                            "Capture {} finished: {} events, reason: {}",
                            params.session_id, total_events, end_reason_str
                        );

                        // Build indices and store engine
                        self.build_and_store_engine(&params.session_id, capture.events).await;

                        let output = serde_json::json!({
                            "session_id": params.session_id,
                            "status": "finalized",
                            "total_events": total_events,
                            "end_reason": end_reason_str,
                            "hint": "Session is now queryable. Use query_events, get_call_stack, get_execution_summary, etc."
                        });
                        Ok(CallToolResult::success(json_content(&output)))
                    }
                    Ok(Err(e)) => {
                        Ok(CallToolResult::error(text_content(format!(
                            "Capture collection failed: {}", e
                        ))))
                    }
                    Err(e) => {
                        Ok(CallToolResult::error(text_content(format!(
                            "Capture thread error: {}", e
                        ))))
                    }
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
                        "Session '{}' not found", params.session_id
                    ))))
                }
            }
        }
    }

    #[tool(name = "query_events", description = "Query trace events with filters. Returns paginated results.")]
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
                    "Session '{}' not found or not finalized", params.session_id
                ))));
            }
        };

        let mut query = TraceQuery::new(&params.session_id)
            .pagination(params.limit, params.offset);

        if let Some(ref types) = params.event_types {
            let event_types: Vec<EventType> = types.iter()
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

    #[tool(name = "get_event", description = "Get detailed information about a specific trace event.")]
    async fn get_event(
        &self,
        params: Parameters<GetEventParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let engines = self.engines.lock().await;

        match engines.get(&params.session_id) {
            Some(engine) => {
                match engine.get_event_by_id(params.event_id) {
                    Some(event) => {
                        let json = serde_json::to_string_pretty(&event).unwrap_or_default();
                        Ok(CallToolResult::success(vec![Content::text(json)]))
                    }
                    None => {
                        Ok(CallToolResult::error(text_content(format!(
                            "Event {} not found", params.event_id
                        ))))
                    }
                }
            }
            None => {
                Ok(CallToolResult::error(text_content(format!(
                    "Session '{}' not found", params.session_id
                ))))
            }
        }
    }

    #[tool(name = "get_call_stack", description = "Reconstruct the call stack at a specific event.")]
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
            None => {
                Ok(CallToolResult::error(text_content(format!(
                    "Session '{}' not found", params.session_id
                ))))
            }
        }
    }

    #[tool(name = "get_execution_summary", description = "Get execution summary: event counts, top functions, issues.")]
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
            None => {
                Ok(CallToolResult::error(text_content(format!(
                    "Session '{}' not found", params.session_id
                ))))
            }
        }
    }

    #[tool(name = "state_diff", description = "Compare program state (registers) between two timestamps.")]
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
            None => {
                Ok(CallToolResult::error(text_content(format!(
                    "Session '{}' not found", params.session_id
                ))))
            }
        }
    }

    #[tool(name = "list_threads", description = "List all thread IDs in the trace.")]
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
            None => {
                Ok(CallToolResult::error(text_content(format!(
                    "Session '{}' not found", params.session_id
                ))))
            }
        }
    }

    #[tool(name = "get_backtrace", description = "Get the full backtrace at a specific event.")]
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

                let bt_lines: Vec<String> = stack.iter().enumerate().map(|(i, frame)| {
                    let file_info = match (&frame.file, &frame.line) {
                        (Some(f), Some(l)) => format!(" at {}:{}", f, l),
                        (Some(f), None) => format!(" at {}", f),
                        _ => String::new(),
                    };
                    format!("#{} 0x{:016x} in {}{}", i, frame.address, frame.function, file_info)
                }).collect();

                let output = serde_json::json!({
                    "session_id": params.session_id,
                    "at_event_id": params.event_id,
                    "frame_count": stack.len(),
                    "backtrace": bt_lines.join("\n"),
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            None => {
                Ok(CallToolResult::error(text_content(format!(
                    "Session '{}' not found", params.session_id
                ))))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_event_type() {
        assert_eq!(ChronosServer::parse_event_type("function_entry"), Some(EventType::FunctionEntry));
        assert_eq!(ChronosServer::parse_event_type("syscall_enter"), Some(EventType::SyscallEnter));
        assert_eq!(ChronosServer::parse_event_type("signal_delivered"), Some(EventType::SignalDelivered));
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
}
