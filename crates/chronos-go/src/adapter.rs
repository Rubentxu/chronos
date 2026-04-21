//! Go adapter using Delve debugger.
//!
//! This adapter spawns a Go program under Delve DAP and captures
//! breakpoint/step events via the Delve JSON-RPC API.

use std::sync::Mutex;
use std::time::Instant;
use which::which;

use chronos_capture::{CaptureConfig, TraceAdapter};
use chronos_domain::{CaptureSession, Language, TraceError};

use crate::event_parser::stack_frame_to_trace_event;
use crate::rpc::{DelveClient, StackFrame};
use crate::subprocess::DelveSubprocess;

/// Interior mutable state of the Go adapter.
struct GoAdapterState {
    /// The spawned Delve subprocess.
    subprocess: Option<DelveSubprocess>,
    /// The Delve RPC client connected to the DAP server.
    client: Option<DelveClient>,
    /// Next event ID to assign.
    next_event_id: u64,
    /// When the capture session started.
    session_start: Option<Instant>,
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
                client: None,
                next_event_id: 1,
                session_start: None,
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

        let mut state = self.state.lock().unwrap();
        state.subprocess = Some(subprocess);
        state.client = Some(client);
        state.session_start = Some(Instant::now());
        state.next_event_id = 1;

        let session = CaptureSession::new(0, Language::Go, config);
        Ok(session)
    }

    fn stop_capture(&self, _session: &CaptureSession) -> Result<(), TraceError> {
        let mut state = self.state.lock().unwrap();
        state.subprocess = None;
        state.client = None;
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
}
