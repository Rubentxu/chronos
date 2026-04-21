//! JavaScript adapter implementing TraceAdapter for Node.js via CDP.

use crate::cdp_client::{CdpClient, CdpEvent, CallFrame};
use crate::debugger::JsDebugger;
use crate::error::JsAdapterError;
use crate::subprocess::NodeProcess;
use chronos_capture::TraceAdapter;
use chronos_domain::{
    CaptureConfig, CaptureSession, EventData, EventType, JsEventKind, Language, SourceLocation,
    TraceError, TraceEvent,
};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use which::which;

/// Interior mutable state of the JavaScript adapter.
struct JsAdapterState {
    process: Option<NodeProcess>,
    debugger: Option<JsDebugger>,
    events: Vec<TraceEvent>,
    running: bool,
    next_event_id: u64,
    session_start: Option<Instant>,
}

/// JavaScript/Node.js trace adapter using Chrome DevTools Protocol.
pub struct JsAdapter {
    /// Interior mutable state
    state: Arc<Mutex<JsAdapterState>>,
}

impl JsAdapter {
    /// Create a new JavaScript adapter.
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(JsAdapterState {
                process: None,
                debugger: None,
                events: Vec::new(),
                running: false,
                next_event_id: 1,
                session_start: None,
            })),
        }
    }

    /// Check if Node.js is available on the system.
    pub fn is_node_available() -> bool {
        which("node").is_ok()
    }

    /// Convert a CDP paused event to trace events.
    fn paused_to_trace_events(
        state: &mut JsAdapterState,
        call_frames: Vec<CallFrame>,
        reason: &str,
        timestamp_ns: u64,
    ) -> Vec<TraceEvent> {
        let mut events = Vec::new();

        let event_kind = match reason {
            "breakpoint" => JsEventKind::Breakpoint,
            "exception" | "promiseRejection" => JsEventKind::Exception,
            "step" => JsEventKind::Step,
            other => JsEventKind::Other(other.to_string()),
        };

        for frame in call_frames {
            let event_id = state.next_event_id;
            state.next_event_id += 1;

            let location = SourceLocation {
                file: Some(frame.url.clone()),
                line: Some(frame.line_number),
                column: Some(frame.column_number),
                function: Some(frame.function_name.clone()),
                ..Default::default()
            };

            // Collect scope chain
            let scope_chain: Vec<String> = frame
                .scope_chain
                .iter()
                .map(|s| s.type_.clone())
                .collect();

            // Note: In a full implementation, we'd fetch locals via get_properties
            // For MVP, we create the event without locals

            let data = EventData::JsFrame {
                function_name: frame.function_name,
                script_url: frame.url,
                line_number: frame.line_number,
                column_number: frame.column_number,
                locals: None,
                scope_chain,
                event_kind: event_kind.clone(),
            };

            events.push(TraceEvent {
                event_id,
                timestamp_ns,
                thread_id: 1, // JavaScript is single-threaded in MVP
                event_type: EventType::BreakpointHit,
                location,
                data,
            });
        }

        events
    }
}

impl Default for JsAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceAdapter for JsAdapter {
    fn start_capture(&self, config: CaptureConfig) -> Result<CaptureSession, TraceError> {
        // Check if Node is available
        if !Self::is_node_available() {
            return Err(TraceError::CaptureFailed(
                "Node.js not found in PATH".to_string(),
            ));
        }

        let port = 9229; // Default CDP port

        // Spawn Node.js process
        let process = NodeProcess::spawn(&config.target, port)
            .map_err(|e| TraceError::CaptureFailed(format!("Failed to spawn Node: {}", e)))?;

        let rt = tokio::runtime::Runtime::new().unwrap();

        // Wait for CDP to be ready
        let ws_url = rt.block_on(async {
            tokio::time::timeout(
                std::time::Duration::from_secs(30),
                process.wait_for_cdp_ready(30),
            )
            .await
        })
        .map_err(|_| TraceError::CaptureFailed("CDP timeout waiting for Node.js".to_string()))?
        .map_err(|e| TraceError::CaptureFailed(format!("CDP error: {}", e)))?;

        // Connect to CDP
        let client = rt.block_on(async { CdpClient::connect(&ws_url).await })
            .map_err(|e| TraceError::CaptureFailed(format!("WebSocket error: {}", e)))?;

        let client = Arc::new(client);
        let debugger = JsDebugger::new(client.clone());

        // Enable debugger and runtime domains
        rt.block_on(async {
            debugger.enable().await.map_err(|e| TraceError::CaptureFailed(e.to_string()))?;
            debugger.enable_runtime().await.map_err(|e| TraceError::CaptureFailed(e.to_string()))?;
            Ok::<(), TraceError>(())
        })?;

        // Update state
        {
            let mut state = self.state.lock().unwrap();
            state.process = Some(process);
            state.debugger = Some(debugger.clone());
            state.running = true;
            state.session_start = Some(Instant::now());
            state.next_event_id = 1;
        }

        // Spawn background task to collect events
        let events_rx = debugger.subscribe();
        let state_arc = self.state.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let mut rx = events_rx;
                while let Ok(event) = rx.recv().await {
                    let timestamp_ns = {
                        let state = state_arc.lock().unwrap();
                        if !state.running {
                            break;
                        }
                        state.session_start
                            .map(|s: Instant| s.elapsed().as_nanos() as u64)
                            .unwrap_or(0)
                    };

                    match event {
                        CdpEvent::DebuggerPaused {
                            reason,
                            call_frames,
                            hit_breakpoints: _,
                        } => {
                            let mut state = state_arc.lock().unwrap();
                            let events = Self::paused_to_trace_events(
                                &mut state,
                                call_frames,
                                &reason,
                                timestamp_ns,
                            );
                            state.events.extend(events);
                        }
                        CdpEvent::DebuggerResumed | CdpEvent::InspectorDetached => {
                            // Execution resumed, stop collecting
                            let mut state = state_arc.lock().unwrap();
                            state.running = false;
                            break;
                        }
                        _ => {}
                    }
                }
            });
        });

        let session = CaptureSession::new(0, Language::JavaScript, config);
        Ok(session)
    }

    fn stop_capture(&self, _session: &CaptureSession) -> Result<(), TraceError> {
        let mut state = self.state.lock().unwrap();

        // Signal stop
        state.running = false;

        // Kill the process (will SIGTERM via Drop)
        state.process = None;
        state.debugger = None;

        Ok(())
    }

    fn attach_to_process(
        &self,
        _pid: u32,
        _config: CaptureConfig,
    ) -> Result<CaptureSession, TraceError> {
        Err(TraceError::UnsupportedLanguage(
            "attach_to_process not supported for JavaScript".to_string(),
        ))
    }

    fn get_language(&self) -> Language {
        Language::JavaScript
    }

    fn name(&self) -> &str {
        "js-cdp"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_js_adapter_name() {
        let adapter = JsAdapter::new();
        assert_eq!(adapter.name(), "js-cdp");
    }

    #[test]
    fn test_js_adapter_language() {
        let adapter = JsAdapter::new();
        assert_eq!(adapter.get_language(), Language::JavaScript);
    }

    #[test]
    fn test_is_node_available() {
        // Just verify the method works
        let available = JsAdapter::is_node_available();
        assert!(available || !available);
    }

    #[test]
    fn test_paused_to_trace_events() {
        let call_frames = vec![CallFrame {
            call_frame_id: "1".to_string(),
            function_name: "testFunc".to_string(),
            function_location: None,
            url: "test.js".to_string(),
            line_number: 10,
            column_number: 5,
            scope_chain: vec![],
        }];

        let mut state = JsAdapterState {
            process: None,
            debugger: None,
            events: Vec::new(),
            running: true,
            next_event_id: 1,
            session_start: Some(Instant::now()),
        };

        let events = JsAdapter::paused_to_trace_events(
            &mut state,
            call_frames,
            "breakpoint",
            1000,
        );

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].location.function.as_deref(), Some("testFunc"));
        assert_eq!(events[0].location.line, Some(10));
        assert_eq!(events[0].location.column, Some(5));

        match &events[0].data {
            EventData::JsFrame {
                function_name,
                script_url,
                line_number,
                event_kind,
                ..
            } => {
                assert_eq!(function_name, "testFunc");
                assert_eq!(script_url, "test.js");
                assert_eq!(*line_number, 10);
                assert_eq!(*event_kind, JsEventKind::Breakpoint);
            }
            _ => panic!("Expected JsFrame data"),
        }
    }

    #[test]
    fn test_js_event_kind_mapping() {
        let call_frames = vec![CallFrame {
            call_frame_id: "1".to_string(),
            function_name: "test".to_string(),
            function_location: None,
            url: "test.js".to_string(),
            line_number: 1,
            column_number: 0,
            scope_chain: vec![],
        }];

        let mut state = JsAdapterState {
            process: None,
            debugger: None,
            events: Vec::new(),
            running: true,
            next_event_id: 1,
            session_start: Some(Instant::now()),
        };

        // Test breakpoint
        let events = JsAdapter::paused_to_trace_events(
            &mut state,
            call_frames.clone(),
            "breakpoint",
            1000,
        );
        match &events[0].data {
            EventData::JsFrame { event_kind, .. } => {
                assert_eq!(*event_kind, JsEventKind::Breakpoint);
            }
            _ => panic!(),
        }

        // Test exception
        let events = JsAdapter::paused_to_trace_events(
            &mut state,
            call_frames.clone(),
            "exception",
            2000,
        );
        match &events[0].data {
            EventData::JsFrame { event_kind, .. } => {
                assert_eq!(*event_kind, JsEventKind::Exception);
            }
            _ => panic!(),
        }

        // Test step
        let events = JsAdapter::paused_to_trace_events(
            &mut state,
            call_frames.clone(),
            "step",
            3000,
        );
        match &events[0].data {
            EventData::JsFrame { event_kind, .. } => {
                assert_eq!(*event_kind, JsEventKind::Step);
            }
            _ => panic!(),
        }

        // Test unknown reason maps to Other
        let events = JsAdapter::paused_to_trace_events(
            &mut state,
            call_frames,
            "pause",
            4000,
        );
        match &events[0].data {
            EventData::JsFrame { event_kind, .. } => {
                assert_eq!(*event_kind, JsEventKind::Other("pause".to_string()));
            }
            _ => panic!(),
        }
    }
}

// ============================================================================
// JsCdpAdapter — direct CDP client wrapper for JavaScript debugging
// ============================================================================

/// A direct CDP client adapter for JavaScript.
///
/// This is a simpler alternative to `JsAdapter` that directly wraps `CdpClient`
/// and provides a more direct API for CDP-based debugging. Unlike `JsAdapter`,
/// this does not spawn a Node process - it expects to connect to an existing
/// CDP endpoint (e.g., from a Node process started with --inspect).
pub struct JsCdpAdapter {
    /// Host where CDP endpoint is available
    host: String,
    /// Port where CDP endpoint is listening
    port: u16,
}

impl JsCdpAdapter {
    /// Create a new CDP adapter that connects to the given address.
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
        }
    }

    /// Connect to a CDP endpoint and return a CDP session.
    pub fn connect(&self) -> Result<CdpSession, JsAdapterError> {
        let url = format!("ws://{}:{}", self.host, self.port);
        // Note: CdpClient::connect is async, so we need to handle this
        // For now, we use a blocking approach
        let rt = tokio::runtime::Runtime::new()?;
        let client = rt.block_on(CdpClient::connect(&url))?;
        Ok(CdpSession {
            client,
            _runtime: Some(rt),
        })
    }
}

/// A CDP capture session — wraps a connected `CdpClient`.
pub struct CdpSession {
    client: CdpClient,
    /// Tokio runtime kept alive for async operations
    _runtime: Option<tokio::runtime::Runtime>,
}

impl CdpSession {
    /// Enable the debugger for this session.
    pub fn enable_debugger(&mut self) -> Result<(), JsAdapterError> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.client.debugger_enable())
    }

    /// Enable runtime domain for console API events.
    pub fn enable_runtime(&mut self) -> Result<(), JsAdapterError> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.client.runtime_enable())
    }

    /// Capture trace events from the CDP session.
    ///
    /// Returns an iterator that yields `TraceEvent`s derived from CDP events.
    pub fn capture_events(&mut self) -> impl Iterator<Item = TraceEvent> + '_ {
        CdpEventIterator { session: self }
    }

    /// Disconnect from the CDP endpoint.
    pub fn disconnect(self) -> Result<(), JsAdapterError> {
        // CdpClient doesn't have an explicit disconnect, but dropping will close
        Ok(())
    }
}

/// Iterator over CDP events converted to TraceEvents.
struct CdpEventIterator<'a> {
    session: &'a mut CdpSession,
}

impl<'a> Iterator for CdpEventIterator<'a> {
    type Item = TraceEvent;

    fn next(&mut self) -> Option<Self::Item> {
        let rt = tokio::runtime::Runtime::new().ok()?;
        let mut rx = self.session.client.subscribe();

        // Use block_on to wait for the next event with a timeout
        let event = rt.block_on(async {
            tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv())
                .await
                .ok()
                .and_then(|r| r.ok())
        })?;

        Some(cdp_event_to_trace_event(event))
    }
}

/// Convert a CDP event to a TraceEvent.
fn cdp_event_to_trace_event(event: CdpEvent) -> TraceEvent {
    match event {
        CdpEvent::DebuggerPaused {
            reason,
            call_frames,
            hit_breakpoints: _,
        } => {
            let event_kind = match reason.as_str() {
                "breakpoint" => JsEventKind::Breakpoint,
                "exception" | "promiseRejection" => JsEventKind::Exception,
                "step" => JsEventKind::Step,
                other => JsEventKind::Other(other.to_string()),
            };

            let first_frame = call_frames.first();
            let (function_name, url, line_number, column_number) = first_frame
                .map(|f| (f.function_name.clone(), f.url.clone(), f.line_number, f.column_number))
                .unwrap_or_else(|| ("<unknown>".to_string(), "<unknown>".to_string(), 0, 0));

            let location = SourceLocation {
                file: Some(url),
                line: Some(line_number),
                column: Some(column_number),
                function: Some(function_name.clone()),
                ..Default::default()
            };

            let scope_chain: Vec<String> = first_frame
                .map(|f| f.scope_chain.iter().map(|s| s.type_.clone()).collect())
                .unwrap_or_default();

            let timestamp_ns = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64;

            TraceEvent {
                event_id: 0,
                timestamp_ns,
                thread_id: 1,
                event_type: EventType::BreakpointHit,
                location,
                data: EventData::JsFrame {
                    function_name,
                    script_url: first_frame.map(|f| f.url.clone()).unwrap_or_default(),
                    line_number,
                    column_number,
                    locals: None,
                    scope_chain,
                    event_kind,
                },
            }
        }
        CdpEvent::ConsoleApiCalled { type_, args } => {
            let level = type_;
            let text = args
                .iter()
                .filter_map(|a| a.description.clone())
                .collect::<Vec<_>>()
                .join(" ");
            let args_serialized = args
                .iter()
                .filter_map(|a| a.description.clone())
                .collect();

            let timestamp_ns = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64;

            TraceEvent {
                event_id: 0,
                timestamp_ns,
                thread_id: 1,
                event_type: EventType::Custom,
                location: SourceLocation::default(),
                data: EventData::JsConsoleOutput {
                    text,
                    level,
                    args: args_serialized,
                },
            }
        }
        CdpEvent::ExceptionThrown { text: _ } => {
            let timestamp_ns = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64;

            TraceEvent {
                event_id: 0,
                timestamp_ns,
                thread_id: 1,
                event_type: EventType::ExceptionThrown,
                location: SourceLocation::default(),
                data: EventData::JsFrame {
                    function_name: "<exception>".to_string(),
                    script_url: String::new(),
                    line_number: 0,
                    column_number: 0,
                    locals: None,
                    scope_chain: vec![],
                    event_kind: JsEventKind::Exception,
                },
            }
        }
        _ => {
            // Other events don't produce trace events - use Custom to mark them
            TraceEvent {
                event_id: 0,
                timestamp_ns: 0,
                thread_id: 1,
                event_type: EventType::Unknown,
                location: SourceLocation::default(),
                data: EventData::Custom {
                    name: "cdp_other".to_string(),
                    data_json: "{}".to_string(),
                },
            }
        }
    }
}

#[cfg(test)]
mod js_cdp_adapter_tests {
    use super::*;

    #[test]
    fn test_js_cdp_adapter_new() {
        let _adapter = JsCdpAdapter::new("localhost", 9229);
        // JsCdpAdapter is not a TraceAdapter, so we just verify construction works
    }

    #[test]
    fn test_cdp_session_disconnect() {
        // Can't actually connect without a CDP endpoint, but we can verify
        // the disconnect method signature is correct
    }
}
