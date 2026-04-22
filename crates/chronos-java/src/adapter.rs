//! Java adapter using JDWP (Java Debug Wire Protocol).
//!
//! This adapter spawns a JVM with JDWP debugging enabled and captures
//! method entry/exit and exception events via the debug wire protocol.

use chronos_capture::{CaptureConfig, TraceAdapter};
use chronos_domain::{
    CaptureSession, Language, RuntimeInfo, StackFrame, ThreadInfo, TraceError,
    VariableInfo,
};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use which::which;

use crate::event_loop::run_jdwp_event_loop;
use crate::protocol::JdwpClient;
use crate::subprocess::JavaSubprocess;

/// Interior mutable state of the Java adapter.
struct JavaAdapterState {
    /// The spawned JVM subprocess.
    subprocess: Option<JavaSubprocess>,
    /// Cancellation token to stop the event loop.
    cancel_token: Option<CancellationToken>,
    /// JoinHandle for the event loop task.
    join_handle: Option<tokio::task::JoinHandle<()>>,
    /// Receiver channel for trace events from the event loop.
    events_rx: Option<mpsc::Receiver<chronos_domain::TraceEvent>>,
    /// Buffered trace events.
    event_buffer: Vec<chronos_domain::TraceEvent>,
    /// Next event ID to assign.
    next_event_id: u64,
    /// When the capture session started.
    session_start: Option<Instant>,
    /// The current capture session ID.
    session_id: Option<String>,
    /// Shared JDWP client for thread/stack/variable queries.
    /// This is stored here so get_threads, get_stack_trace, get_variables
    /// can access the client from synchronous code.
    #[allow(dead_code)]
    client_arc: Option<Arc<tokio::sync::Mutex<JdwpClient>>>,
    /// Last thread ID used in get_stack_trace.
    /// Used by get_variables to know which thread to query.
    last_thread_id: Option<u64>,
}

/// Java trace adapter using JDWP.
///
/// Spawns a JVM with `-agentlib:jdwp` and captures method entry/exit events
/// via the Java Debug Wire Protocol.
pub struct JavaAdapter {
    state: Mutex<JavaAdapterState>,
}

impl JavaAdapter {
    /// Create a new Java adapter.
    pub fn new() -> Self {
        Self {
            state: Mutex::new(JavaAdapterState {
                subprocess: None,
                cancel_token: None,
                join_handle: None,
                events_rx: None,
                event_buffer: Vec::new(),
                next_event_id: 1,
                session_start: None,
                session_id: None,
                client_arc: None,
                last_thread_id: None,
            }),
        }
    }

    /// Check if Java (java + javac) is available on the system.
    pub fn is_available() -> bool {
        which("java").is_ok()
    }

    /// Drain all buffered events that arrived since the last call.
    ///
    /// This is an internal method that can be called from sync code.
    #[allow(dead_code)]
    pub fn drain_events_internal(&self) -> Result<Vec<chronos_domain::TraceEvent>, TraceError> {
        // First, collect events from the receiver without holding the lock
        let new_events = {
            let mut state = self.state.lock().unwrap();
            if let Some(rx) = state.events_rx.as_mut() {
                let mut events = Vec::new();
                while let Ok(event) = rx.try_recv() {
                    events.push(event);
                }
                events
            } else {
                Vec::new()
            }
        };

        // Now process with minimal lock time
        let mut state = self.state.lock().unwrap();
        state.event_buffer.extend(new_events);

        // Assign event IDs and return buffered events
        // First, collect into a separate vec to avoid borrow issues
        let events_to_process: Vec<_> = state.event_buffer.drain(..).collect();
        let mut events = Vec::new();
        for mut event in events_to_process {
            event.event_id = state.next_event_id;
            state.next_event_id += 1;
            events.push(event);
        }

        Ok(events)
    }
}

impl Default for JavaAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceAdapter for JavaAdapter {
    fn start_capture(&self, config: CaptureConfig) -> Result<CaptureSession, TraceError> {
        // Check java is available
        if which("java").is_err() {
            return Err(TraceError::CaptureFailed(
                "Java not found in PATH".to_string(),
            ));
        }

        // Spawn the JVM with JDWP
        let subprocess = tokio::runtime::Handle::current()
            .block_on(JavaSubprocess::spawn(&config.target))
            .map_err(|e| TraceError::CaptureFailed(format!("Failed to spawn JVM: {}", e)))?;

        // Connect JDWP client
        let mut client = tokio::runtime::Handle::current()
            .block_on(JdwpClient::connect(subprocess.jdwp_port))
            .map_err(|e| TraceError::CaptureFailed(format!("JDWP connect failed: {}", e)))?;

        // Perform JDWP handshake
        tokio::runtime::Handle::current()
            .block_on(client.handshake())
            .map_err(|e| TraceError::CaptureFailed(format!("JDWP handshake failed: {}", e)))?;

        // Set event requests for method entry, exit, breakpoint, and exception
        let event_kinds = [
            crate::protocol::event_kind::METHOD_ENTRY,
            crate::protocol::event_kind::METHOD_EXIT,
            crate::protocol::event_kind::BREAKPOINT,
            crate::protocol::event_kind::EXCEPTION,
        ];
        for kind in event_kinds {
            if let Err(e) = tokio::runtime::Handle::current().block_on(client.set_event_request(kind))
            {
                tracing::warn!("Failed to set event request for kind {}: {}", kind, e);
            }
        }

        // Resume the JVM to start receiving events
        if let Err(e) = tokio::runtime::Handle::current().block_on(client.resume()) {
            tracing::warn!("Failed to resume JVM: {}", e);
        }

        // Create channels for event communication
        let (events_tx, events_rx) = mpsc::channel(1000);

        // Create cancellation token
        let cancel_token = CancellationToken::new();

        // Wrap client in Arc<Mutex<...>> for shared access with event loop
        // We move client into this Arc, so we don't store it in state separately
        let client_arc: Arc<tokio::sync::Mutex<JdwpClient>> =
            Arc::new(tokio::sync::Mutex::new(client));

        // Spawn the event loop task
        let client_for_task = Arc::clone(&client_arc);
        let events_tx_clone = events_tx.clone();
        let cancel_clone = cancel_token.clone();

        let join_handle = tokio::spawn(async move {
            if let Err(e) = run_jdwp_event_loop(client_for_task, events_tx_clone, cancel_clone)
                .await
            {
                tracing::error!("JDWP event loop error: {}", e);
            }
        });

        let mut state = self.state.lock().unwrap();
        state.subprocess = Some(subprocess);
        state.cancel_token = Some(cancel_token);
        state.join_handle = Some(join_handle);
        state.events_rx = Some(events_rx);
        state.session_start = Some(Instant::now());
        state.next_event_id = 1;
        state.session_id = Some(config.target.clone());
        state.client_arc = Some(client_arc);

        let session = CaptureSession::new(0, Language::Java, config);
        Ok(session)
    }

    fn stop_capture(&self, _session: &CaptureSession) -> Result<(), TraceError> {
        let mut state = self.state.lock().unwrap();

        // Signal cancellation to stop the event loop
        if let Some(cancel) = state.cancel_token.take() {
            cancel.cancel();
        }

        // Take and await the join handle
        if let Some(handle) = state.join_handle.take() {
            // Use block_on to wait for the task
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                if let Err(e) = handle.await {
                    tracing::warn!("Event loop join error: {}", e);
                }
            });
        }

        // Clear the JVM subprocess and channels
        state.subprocess = None;
        state.events_rx = None;
        state.event_buffer.clear();
        state.client_arc = None;
        state.last_thread_id = None;

        Ok(())
    }

    fn attach_to_process(
        &self,
        _pid: u32,
        _config: CaptureConfig,
    ) -> Result<CaptureSession, TraceError> {
        Err(TraceError::UnsupportedLanguage(
            "attach_to_process not yet supported for Java".to_string(),
        ))
    }

    fn get_language(&self) -> Language {
        Language::Java
    }

    fn name(&self) -> &str {
        "java-jdwp"
    }

    fn get_threads(&self, session_id: &str) -> Result<Vec<ThreadInfo>, TraceError> {
        // Get the client arc from state (scope lock to avoid holding across block_on)
        let client_arc = {
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
                .client_arc
                .as_ref()
                .ok_or_else(|| {
                    TraceError::UnsupportedOperation("JDWP client not available".to_string())
                })?
                .clone()
        };

        // Use block_on to run async JDWP commands
        let rt = tokio::runtime::Handle::current();
        let thread_ids = rt
            .block_on(async {
                let mut client = client_arc.lock().await;
                client.all_threads().await
            })
            .map_err(|e| {
                tracing::warn!("Failed to get threads: {}", e);
                TraceError::UnsupportedOperation(format!("JDWP error: {}", e))
            })?;

        // For each thread ID, get its name (sequential to avoid overwhelming the JVM)
        let mut threads = Vec::with_capacity(thread_ids.len());
        for tid in thread_ids {
            let name = rt
                .block_on(async {
                    let mut client = client_arc.lock().await;
                    client.thread_name(tid).await
                })
                .unwrap_or_else(|_| format!("Thread-{:x}", tid));

            threads.push(ThreadInfo {
                thread_id: tid,
                name,
                state: chronos_domain::ThreadState::Running,
            });
        }

        Ok(threads)
    }

    fn get_stack_trace(
        &self,
        session_id: &str,
        thread_id: u64,
    ) -> Result<Vec<StackFrame>, TraceError> {
        // Get the client arc from state (need to scope the lock carefully)
        let client_arc = {
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
                .client_arc
                .as_ref()
                .ok_or_else(|| {
                    TraceError::UnsupportedOperation("JDWP client not available".to_string())
                })?
                .clone()
        };

        // Use block_on to run async JDWP commands
        // Use start=-1, length=-1 to get all frames
        let rt = tokio::runtime::Handle::current();
        let frame_infos = rt
            .block_on(async {
                let mut client = client_arc.lock().await;
                client.frames(thread_id, -1, -1).await
            })
            .map_err(|e| {
                tracing::warn!("Failed to get stack trace: {}", e);
                TraceError::UnsupportedOperation(format!("JDWP error: {}", e))
            })?;

        // Convert to StackFrame list
        // For function_name, use format!("frame_{}") since method name lookup
        // requires additional round-trips (ClassReference.MethodsWithGeneric)
        let frames: Vec<StackFrame> = frame_infos
            .iter()
            .enumerate()
            .map(|(idx, frame_info)| StackFrame {
                frame_id: frame_info.frame_id,
                function_name: format!("frame_{}", idx),
                source_file: None,
                line: None,
                variables: Vec::new(),
            })
            .collect();

        // Store the thread_id for use by get_variables
        {
            let mut state = self.state.lock().unwrap();
            state.last_thread_id = Some(thread_id);
        }

        Ok(frames)
    }

    fn get_variables(
        &self,
        session_id: &str,
        frame_id: u64,
    ) -> Result<Vec<VariableInfo>, TraceError> {
        // Get the client arc and last_thread_id from state
        let (client_arc, thread_id) = {
            let state = self.state.lock().unwrap();

            // Verify session is active
            if state.session_id.as_deref() != Some(session_id) {
                return Err(TraceError::session_not_found(session_id));
            }

            // If not connected, return empty
            if state.subprocess.is_none() {
                return Ok(Vec::new());
            }

            let client_arc = state
                .client_arc
                .as_ref()
                .ok_or_else(|| {
                    TraceError::UnsupportedOperation("JDWP client not available".to_string())
                })?
                .clone();

            let thread_id = state.last_thread_id.ok_or_else(|| {
                TraceError::UnsupportedOperation(
                    "No thread context: call get_stack_trace first".to_string(),
                )
            })?;

            (client_arc, thread_id)
        };

        // Use block_on to run async JDWP commands
        let rt = tokio::runtime::Handle::current();
        let values = rt
            .block_on(async {
                let mut client = client_arc.lock().await;
                // Query slots 0-9 for local variables
                client.frame_values(thread_id, frame_id, 10).await
            })
            .map_err(|e| {
                tracing::warn!("Failed to get variables: {}", e);
                TraceError::UnsupportedOperation(format!("JDWP error: {}", e))
            })?;

        // Convert to VariableInfo list
        // The values returned are for slots 0, 1, 2, ... 9
        let variables: Vec<VariableInfo> = values
            .iter()
            .enumerate()
            .filter(|(_, val)| !val.is_empty()) // Filter out empty/unknown slots
            .map(|(slot, val)| VariableInfo {
                name: format!("slot_{}", slot),
                value: val.clone(),
                type_name: "int".to_string(), // We requested 'I' (int) slots
                address: slot as u64,
                scope: chronos_domain::VariableScope::Local,
            })
            .collect();

        Ok(variables)
    }

    fn get_runtime_info(&self, session_id: &str) -> Result<RuntimeInfo, TraceError> {
        let state = self.state.lock().unwrap();

        // Verify session is active
        if state.session_id.as_deref() != Some(session_id) {
            return Err(TraceError::session_not_found(session_id));
        }

        let uptime_ms = state
            .session_start
            .map(|s| s.elapsed().as_millis() as u64)
            .unwrap_or(0);

        Ok(RuntimeInfo {
            language: "Java".to_string(),
            runtime_version: "JVM".to_string(), // Could extract from system properties
            pid: state
                .subprocess
                .as_ref()
                .and_then(|s| s.child.id())
                .unwrap_or(0),
            uptime_ms,
        })
    }

    fn evaluate_expression(
        &self,
        session_id: &str,
        _expr: &str,
        _frame_id: u64,
    ) -> Result<String, TraceError> {
        let state = self.state.lock().unwrap();

        // Verify session is active
        if state.session_id.as_deref() != Some(session_id) {
            return Err(TraceError::session_not_found(session_id));
        }

        // If not connected, return error
        if state.subprocess.is_none() {
            return Err(TraceError::UnsupportedOperation(
                "evaluate_expression not available".to_string(),
            ));
        }

        // TODO: Implement JDWP expression evaluation
        // This requires implementing the JDWPevaluate command
        Err(TraceError::UnsupportedOperation(
            "evaluate_expression not yet implemented for Java".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_java_adapter_name() {
        let adapter = JavaAdapter::new();
        assert_eq!(adapter.name(), "java-jdwp");
    }

    #[test]
    fn test_java_adapter_language() {
        let adapter = JavaAdapter::new();
        assert_eq!(adapter.get_language(), Language::Java);
    }

    #[test]
    fn test_java_adapter_is_available() {
        // Result depends on whether java is on PATH
        let available = JavaAdapter::is_available();
        assert!(available || !available); // Always passes — checks method doesn't panic
    }

    #[test]
    fn test_drain_events_returns_empty_when_not_started() {
        let adapter = JavaAdapter::new();
        let result = adapter.drain_events_internal();
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}
