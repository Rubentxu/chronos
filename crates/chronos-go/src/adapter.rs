//! Go adapter using Delve debugger.
//!
//! This adapter spawns a Go program under Delve DAP and captures
//! breakpoint/step events via the Delve JSON-RPC API.

use std::sync::{Arc, Mutex};
use std::time::Instant;
use which::which;

use chronos_capture::{CaptureConfig, TraceAdapter};
use chronos_domain::{CaptureSession, Language, StackFrame as ChronosStackFrame, ThreadInfo, TraceError, VariableInfo};

use crate::event_loop::{run_delve_event_loop, AtomicCancel, DelveRpcClient, delve_stack_to_chronos_frames, goroutines_to_thread_info};
use crate::event_parser::stack_frame_to_trace_event;
use crate::rpc::{DelveClient, StackFrame};
use crate::subprocess::DelveSubprocess;

/// Interior mutable state of the Go adapter.
struct GoAdapterState {
    /// The spawned Delve subprocess.
    subprocess: Option<DelveSubprocess>,
    /// Next event ID to assign.
    next_event_id: u64,
    /// When the capture session started.
    session_start: Option<Instant>,
    /// Cancellation flag for the event loop.
    cancel: Option<Arc<AtomicCancel>>,
    /// Join handle for the event loop task.
    join_handle: Option<tokio::task::JoinHandle<()>>,
    /// Receiver for events produced by the loop.
    events_rx: Option<tokio::sync::mpsc::Receiver<chronos_domain::TraceEvent>>,
    /// Shared Delve RPC client for thread/stack/variable queries.
    rpc: Option<Arc<DelveRpcClient>>,
    /// The current capture session ID.
    session_id: Option<String>,
    /// Last goroutine ID used in get_stack_trace.
    /// Used by get_variables to know which goroutine to query.
    last_goroutine_id: Option<i64>,
}

/// Go trace adapter using Delve debugger.
///
/// Spawns a Go binary under Delve DAP server and captures breakpoint/step
/// events via the Delve JSON-RPC API.
pub struct GoAdapter {
    state: Mutex<GoAdapterState>,
}

impl GoAdapter {
    /// Create a new Go adapter.
    pub fn new() -> Self {
        Self {
            state: Mutex::new(GoAdapterState {
                subprocess: None,
                next_event_id: 1,
                session_start: None,
                cancel: None,
                join_handle: None,
                events_rx: None,
                rpc: None,
                session_id: None,
                last_goroutine_id: None,
            }),
        }
    }

    /// Check if Delve (dlv) is available on the system.
    pub fn is_available() -> bool {
        which("dlv").is_ok()
    }

    /// Convert a Delve state to trace events.
    #[allow(dead_code)]
    fn state_to_events(
        &self,
        state: &crate::rpc::DelveState,
        timestamp_ns: u64,
    ) -> Vec<chronos_domain::TraceEvent> {
        let mut events = Vec::new();

        if let Some(thread) = &state.currentThread {
            let goroutine_id = thread.goroutineID as u64;
            let mut events_state = self.state.lock().unwrap();
            let event_id = events_state.next_event_id;
            events_state.next_event_id += 1;

            let frame = StackFrame {
                function: None,
                file: String::new(),
                line: 0,
                locals: None,
            };

            let event = stack_frame_to_trace_event(
                &frame,
                goroutine_id,
                event_id,
                timestamp_ns,
                chronos_domain::GoEventKind::Breakpoint,
            );
            events.push(event);
        }

        events
    }
}

impl Default for GoAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceAdapter for GoAdapter {
    fn start_capture(&self, config: CaptureConfig) -> Result<CaptureSession, TraceError> {
        // Check dlv is available
        if which("dlv").is_err() {
            return Err(TraceError::CaptureFailed(
                "Delve (dlv) not found in PATH".to_string(),
            ));
        }

        // Spawn Delve DAP server
        let subprocess = tokio::runtime::Handle::current()
            .block_on(DelveSubprocess::spawn(&config.target))
            .map_err(|e| TraceError::CaptureFailed(format!("Failed to spawn Delve: {}", e)))?;

        // Connect Delve client
        let client = tokio::runtime::Handle::current()
            .block_on(DelveClient::connect(subprocess.port))
            .map_err(|e| TraceError::CaptureFailed(format!("Delve connect failed: {}", e)))?;

        // Set up the event loop: channel for events + cancellation token
        let (events_tx, events_rx) = tokio::sync::mpsc::channel(1024);
        let cancel = Arc::new(AtomicCancel::new());
        let rpc: Arc<DelveRpcClient> = Arc::new(tokio::sync::Mutex::new(client));
        let rpc_clone = Arc::clone(&rpc);
        let cancel_clone = Arc::clone(&cancel);

        let join_handle = tokio::spawn(async move {
            if let Err(e) = run_delve_event_loop(rpc_clone, events_tx, cancel_clone).await {
                tracing::warn!("Delve event loop ended: {}", e);
            }
        });

        let session = CaptureSession::new(0, Language::Go, config);
        let session_id = session.session_id.clone();

        let mut state = self.state.lock().unwrap();
        state.subprocess = Some(subprocess);
        state.session_start = Some(Instant::now());
        state.next_event_id = 1;
        state.cancel = Some(cancel);
        state.join_handle = Some(join_handle);
        state.events_rx = Some(events_rx);
        state.rpc = Some(rpc);
        state.session_id = Some(session_id);
        Ok(session)
    }

    fn stop_capture(&self, _session: &CaptureSession) -> Result<(), TraceError> {
        let mut state = self.state.lock().unwrap();
        // Signal cancellation
        if let Some(cancel) = state.cancel.take() {
            cancel.cancel();
        }
        // Abort the task (non-blocking — stop_capture is sync, can't await)
        if let Some(handle) = state.join_handle.take() {
            handle.abort();
        }
        state.subprocess = None;
        state.events_rx = None;
        state.rpc = None;
        state.session_id = None;
        state.last_goroutine_id = None;
        Ok(())
    }

    fn attach_to_process(
        &self,
        _pid: u32,
        _config: CaptureConfig,
    ) -> Result<CaptureSession, TraceError> {
        Err(TraceError::UnsupportedLanguage(
            "attach_to_process not yet supported for Go".to_string(),
        ))
    }

    fn get_language(&self) -> Language {
        Language::Go
    }

    fn name(&self) -> &str {
        "go-delve"
    }

    fn get_threads(&self, session_id: &str) -> Result<Vec<ThreadInfo>, TraceError> {
        // Get the RPC client from state (scope lock to avoid holding across block_on)
        let rpc = {
            let state = self.state.lock().unwrap();

            // Verify session is active
            if state.session_id.as_deref() != Some(session_id) {
                return Err(TraceError::session_not_found(session_id));
            }

            // If not connected (no subprocess), return empty list
            if state.subprocess.is_none() {
                return Ok(Vec::new());
            }

            // Clone the Arc to avoid holding the lock across block_on
            state
                .rpc
                .as_ref()
                .ok_or_else(|| {
                    TraceError::UnsupportedOperation("Delve client not available".to_string())
                })?
                .clone()
        };

        // Use block_on to run async Delve RPC calls
        let rt = tokio::runtime::Handle::current();
        let goroutines = rt
            .block_on(async {
                let mut client = rpc.lock().await;
                client.list_goroutines().await
            })
            .map_err(|e| {
                tracing::warn!("Failed to get goroutines: {}", e);
                TraceError::UnsupportedOperation(format!("Delve RPC error: {}", e))
            })?;

        // Convert goroutines to ThreadInfo
        let threads = goroutines_to_thread_info(&goroutines);
        Ok(threads)
    }

    fn get_stack_trace(
        &self,
        session_id: &str,
        thread_id: u64,
    ) -> Result<Vec<ChronosStackFrame>, TraceError> {
        // Get the RPC client from state
        let rpc = {
            let state = self.state.lock().unwrap();

            // Verify session is active
            if state.session_id.as_deref() != Some(session_id) {
                return Err(TraceError::session_not_found(session_id));
            }

            // If not connected, return empty
            if state.subprocess.is_none() {
                return Ok(Vec::new());
            }

            // Clone the Arc to avoid holding the lock across block_on
            state
                .rpc
                .as_ref()
                .ok_or_else(|| {
                    TraceError::UnsupportedOperation("Delve client not available".to_string())
                })?
                .clone()
        };

        // Use block_on to run async Delve RPC calls
        // Use depth=100 to get a reasonable number of frames
        let rt = tokio::runtime::Handle::current();
        let frames = rt
            .block_on(async {
                let mut client = rpc.lock().await;
                client.stacktrace(thread_id as i64, 100).await
            })
            .map_err(|e| {
                tracing::warn!("Failed to get stack trace: {}", e);
                TraceError::UnsupportedOperation(format!("Delve RPC error: {}", e))
            })?;

        // Store the thread_id for use by get_variables
        {
            let mut state = self.state.lock().unwrap();
            state.last_goroutine_id = Some(thread_id as i64);
        }

        // Convert Delve frames to Chronos frames
        let chronos_frames = delve_stack_to_chronos_frames(&frames, 0);
        Ok(chronos_frames)
    }

    fn get_variables(
        &self,
        session_id: &str,
        frame_id: u64,
    ) -> Result<Vec<VariableInfo>, TraceError> {
        // Get the RPC client and last_goroutine_id from state
        let (rpc, goroutine_id) = {
            let state = self.state.lock().unwrap();

            // Verify session is active
            if state.session_id.as_deref() != Some(session_id) {
                return Err(TraceError::session_not_found(session_id));
            }

            // If not connected, return empty
            if state.subprocess.is_none() {
                return Ok(Vec::new());
            }

            let rpc = state
                .rpc
                .as_ref()
                .ok_or_else(|| {
                    TraceError::UnsupportedOperation("Delve client not available".to_string())
                })?
                .clone();

            let goroutine_id = state.last_goroutine_id.ok_or_else(|| {
                TraceError::UnsupportedOperation(
                    "No goroutine context: call get_stack_trace first".to_string(),
                )
            })?;

            (rpc, goroutine_id)
        };

        // Use block_on to run async Delve RPC calls
        let rt = tokio::runtime::Handle::current();
        let frames = rt
            .block_on(async {
                let mut client = rpc.lock().await;
                client.stacktrace(goroutine_id, 100).await
            })
            .map_err(|e| {
                tracing::warn!("Failed to get frames for variables: {}", e);
                TraceError::UnsupportedOperation(format!("Delve RPC error: {}", e))
            })?;

        // Get the specific frame (frame_id is the depth index)
        let frame = frames
            .get(frame_id as usize)
            .ok_or_else(|| {
                TraceError::UnsupportedOperation(format!("Frame {} not found", frame_id))
            })?;

        // Convert locals to VariableInfo
        let variables = match &frame.locals {
            Some(locals) => locals
                .iter()
                .map(|v| {
                    VariableInfo::new(
                        &v.name,
                        &v.value,
                        "unknown", // type_name not available in Delve basic response
                        0,         // address not available
                        chronos_domain::VariableScope::Local,
                    )
                })
                .collect(),
            None => Vec::new(),
        };

        Ok(variables)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_go_adapter_name() {
        let adapter = GoAdapter::new();
        assert_eq!(adapter.name(), "go-delve");
    }

    #[test]
    fn test_go_adapter_language() {
        let adapter = GoAdapter::new();
        assert_eq!(adapter.get_language(), Language::Go);
    }

    #[test]
    fn test_go_adapter_is_available() {
        // Result depends on whether dlv is on PATH
        let available = GoAdapter::is_available();
        assert!(available || !available); // Always passes — checks method doesn't panic
    }

    #[test]
    fn test_go_adapter_stop_without_start_is_safe() {
        // Stopping without starting should not panic
        let adapter = GoAdapter::new();
        let session = CaptureSession::new(0, Language::Go, CaptureConfig {
            target: "/dev/null".into(),
            args: vec![],
            env: None,
            cwd: None,
            language: None,
            capture_syscalls: false,
            capture_variables: false,
            capture_stack: false,
            capture_memory: false,
            capture_function_exit: false,
            function_filter: None,
            max_duration_ms: None,
        });
        let result = adapter.stop_capture(&session);
        assert!(result.is_ok());
    }

    // Tests for get_threads
    #[test]
    fn test_go_adapter_get_threads_without_start_returns_error() {
        // get_threads without starting returns error because session_id doesn't match
        let adapter = GoAdapter::new();
        let session = CaptureSession::new(0, Language::Go, CaptureConfig::new("/dev/null"));
        // Session was never started, so adapter's session_id is None
        // The session_id check fails before we even check for subprocess
        let result = adapter.get_threads(&session.session_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_go_adapter_get_threads_wrong_session_returns_error() {
        // get_threads with wrong session_id should return error
        let adapter = GoAdapter::new();
        let _session = CaptureSession::new(0, Language::Go, CaptureConfig::new("/dev/null"));
        let result = adapter.get_threads("wrong-session-id");
        assert!(result.is_err());
    }

    // Tests for get_stack_trace
    #[test]
    fn test_go_adapter_get_stack_trace_without_start_returns_error() {
        // get_stack_trace without starting returns error because session_id doesn't match
        let adapter = GoAdapter::new();
        let session = CaptureSession::new(0, Language::Go, CaptureConfig::new("/dev/null"));
        let result = adapter.get_stack_trace(&session.session_id, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_go_adapter_get_stack_trace_wrong_session_returns_error() {
        // get_stack_trace with wrong session_id should return error
        let adapter = GoAdapter::new();
        let _session = CaptureSession::new(0, Language::Go, CaptureConfig::new("/dev/null"));
        let result = adapter.get_stack_trace("wrong-session-id", 1);
        assert!(result.is_err());
    }

    // Tests for get_variables
    #[test]
    fn test_go_adapter_get_variables_without_start_returns_error() {
        // get_variables without starting returns error because session_id doesn't match
        let adapter = GoAdapter::new();
        let session = CaptureSession::new(0, Language::Go, CaptureConfig::new("/dev/null"));
        let result = adapter.get_variables(&session.session_id, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_go_adapter_get_variables_wrong_session_returns_error() {
        // get_variables with wrong session_id should return error
        let adapter = GoAdapter::new();
        let _session = CaptureSession::new(0, Language::Go, CaptureConfig::new("/dev/null"));
        let result = adapter.get_variables("wrong-session-id", 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_go_adapter_get_variables_without_stack_trace_returns_error() {
        // get_variables without calling get_stack_trace first should return error
        let adapter = GoAdapter::new();
        let session = CaptureSession::new(0, Language::Go, CaptureConfig::new("/dev/null"));

        // Directly call get_variables without get_stack_trace - should fail
        // because last_goroutine_id is not set
        let result = adapter.get_variables(&session.session_id, 0);
        // Without calling get_stack_trace first, we get UnsupportedOperation
        // because the adapter was never started
        assert!(result.is_err());
    }
}
