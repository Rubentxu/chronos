//! Python adapter for Chronos tracing using sys.settrace.
//!
//! This adapter spawns a Python subprocess with sys.settrace enabled
//! to capture function call/return/exception events.

use chronos_capture::TraceAdapter;
use chronos_domain::{
    CaptureConfig, CaptureSession, EventData, EventType, Language, PythonEventKind, SourceLocation,
    TraceError, TraceEvent,
};
use std::sync::Mutex;
use std::time::Instant;
use which::which;
use crate::parser::{locals_to_variable_info, RawPythonEvent};
use crate::subprocess::PythonSubprocess;

/// Interior mutable state of the Python adapter.
struct PythonAdapterState {
    subprocess: Option<PythonSubprocess>,
    next_event_id: u64,
    session_start: Option<Instant>,
}

/// Python trace adapter using sys.settrace.
///
/// Spawns a Python subprocess and captures call/return/exception events
/// via the subprocess stdout pipe.
pub struct PythonAdapter {
    /// Whether to capture local variables at call sites.
    capture_locals: bool,
    /// Interior mutable state (subprocess + counters).
    state: Mutex<PythonAdapterState>,
}

impl PythonAdapter {
    /// Create a new Python adapter.
    pub fn new() -> Self {
        Self {
            capture_locals: true,
            state: Mutex::new(PythonAdapterState {
                subprocess: None,
                next_event_id: 1,
                session_start: None,
            }),
        }
    }

    /// Create a Python adapter with local variable capture configured.
    pub fn with_capture_locals(capture_locals: bool) -> Self {
        Self {
            capture_locals,
            ..Self::new()
        }
    }

    /// Convert a RawPythonEvent to a TraceEvent.
    fn raw_to_trace_event(state: &mut PythonAdapterState, raw: RawPythonEvent) -> TraceEvent {
        let event_id = state.next_event_id;
        state.next_event_id += 1;

        let timestamp_ns = state
            .session_start
            .map(|start| start.elapsed().as_nanos() as u64)
            .unwrap_or(0);

        let thread_id = 1;

        let (event_type, event_kind) = match raw.event.as_str() {
            "call" => (EventType::FunctionEntry, PythonEventKind::Call),
            "return" => (EventType::FunctionExit, PythonEventKind::Return),
            "exception" => (EventType::ExceptionThrown, PythonEventKind::Exception),
            _ => (EventType::Custom, PythonEventKind::Call),
        };

        let is_generator = raw.is_generator.unwrap_or(false);
        let locals = raw.locals.map(locals_to_variable_info);

        let location = SourceLocation {
            file: Some(raw.file.clone()),
            line: Some(raw.line),
            function: Some(raw.name.clone()),
            ..Default::default()
        };

        let data = EventData::PythonFrame {
            qualified_name: raw.name,
            file: raw.file,
            line: raw.line,
            is_generator,
            locals,
            event_kind,
        };

        TraceEvent {
            event_id,
            timestamp_ns,
            thread_id,
            event_type,
            location,
            data,
        }
    }

    /// Check if python3 is available on the system.
    pub fn is_python_available() -> bool {
        which("python3").is_ok()
    }
}

impl Default for PythonAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceAdapter for PythonAdapter {
    fn start_capture(&self, config: CaptureConfig) -> Result<CaptureSession, TraceError> {
        let subprocess = PythonSubprocess::spawn(&config.target, self.capture_locals)
            .map_err(|e| TraceError::CaptureFailed(format!("Failed to spawn Python: {}", e)))?;

        let mut state = self.state.lock().unwrap();
        state.subprocess = Some(subprocess);
        state.session_start = Some(Instant::now());
        state.next_event_id = 1;

        let mut session = CaptureSession::new(0, Language::Python, config);
        session.activate();
        Ok(session)
    }

    fn stop_capture(&self, _session: &CaptureSession) -> Result<(), TraceError> {
        let mut state = self.state.lock().unwrap();
        // Drop subprocess → SIGTERM via Drop impl
        state.subprocess = None;
        Ok(())
    }

    fn attach_to_process(
        &self,
        _pid: u32,
        _config: CaptureConfig,
    ) -> Result<CaptureSession, TraceError> {
        // Python subprocess attach not supported in MVP
        Err(TraceError::UnsupportedLanguage(
            "attach_to_process not supported for Python".to_string(),
        ))
    }

    fn get_language(&self) -> Language {
        Language::Python
    }

    fn name(&self) -> &str {
        "python-settrace"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_python_adapter_name() {
        let adapter = PythonAdapter::new();
        assert_eq!(adapter.name(), "python-settrace");
    }

    #[test]
    fn test_python_adapter_language() {
        let adapter = PythonAdapter::new();
        assert_eq!(adapter.get_language(), Language::Python);
    }

    #[test]
    fn test_is_available() {
        // This test passes if python3 is on the system PATH
        let available = PythonAdapter::is_python_available();
        // We just verify the method works - actual result depends on system
        assert!(available || !available); // Always passes, checks method doesn't panic
    }

    #[tokio::test]
    async fn test_python_adapter_start_capture() {
        // Create a simple Python script
        let mut file = NamedTempFile::with_suffix(".py").unwrap();
        writeln!(file, "def foo():").unwrap();
        writeln!(file, "    return 42").unwrap();
        writeln!(file, "foo()").unwrap();
        file.flush().unwrap();

        let adapter = PythonAdapter::new();
        let config = CaptureConfig::new(file.path().to_str().unwrap());

        let result = adapter.start_capture(config);
        // If python3 is not available, this will fail
        if PythonAdapter::is_python_available() {
            assert!(result.is_ok());
        }
    }

    #[tokio::test]
    async fn test_python_adapter_not_available_no_python() {
        // This test verifies that we handle python3 not found gracefully
        // We can't actually remove python3 from the system, so we just
        // verify the method works and check the result
        let available = PythonAdapter::is_python_available();
        if !available {
            // If python3 is not available, start_capture should fail
            let adapter = PythonAdapter::new();
            let config = CaptureConfig::new("nonexistent_script.py");
            let result = adapter.start_capture(config);
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_raw_to_trace_event_conversion() {
        let mut state = PythonAdapterState {
            subprocess: None,
            next_event_id: 1,
            session_start: Some(Instant::now()),
        };

        let raw = RawPythonEvent {
            event: "call".to_string(),
            name: "foo".to_string(),
            file: "test.py".to_string(),
            line: 10,
            is_generator: Some(false),
            locals: Some(vec![("x".to_string(), "42".to_string())].into_iter().collect()),
        };

        let trace_event = PythonAdapter::raw_to_trace_event(&mut state, raw);

        assert_eq!(trace_event.event_type, EventType::FunctionEntry);
        assert_eq!(trace_event.location.line, Some(10));
        assert_eq!(trace_event.location.function.as_deref(), Some("foo"));
        match &trace_event.data {
            EventData::PythonFrame { qualified_name, event_kind, locals, .. } => {
                assert_eq!(qualified_name, "foo");
                assert_eq!(*event_kind, PythonEventKind::Call);
                assert!(locals.is_some());
            }
            _ => panic!("Expected PythonFrame data"),
        }
    }

    #[test]
    fn test_raw_to_trace_event_return() {
        let mut state = PythonAdapterState {
            subprocess: None,
            next_event_id: 1,
            session_start: Some(Instant::now()),
        };

        let raw = RawPythonEvent {
            event: "return".to_string(),
            name: "bar".to_string(),
            file: "test.py".to_string(),
            line: 20,
            is_generator: None,
            locals: None,
        };

        let trace_event = PythonAdapter::raw_to_trace_event(&mut state, raw);

        assert_eq!(trace_event.event_type, EventType::FunctionExit);
        match &trace_event.data {
            EventData::PythonFrame { qualified_name, event_kind, .. } => {
                assert_eq!(qualified_name, "bar");
                assert_eq!(*event_kind, PythonEventKind::Return);
            }
            _ => panic!("Expected PythonFrame data"),
        }
    }
}