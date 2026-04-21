//! Trace adapter trait — the interface all language adapters implement.

use chronos_domain::{CaptureConfig, CaptureSession, Language, TraceError, TraceEvent};

/// Trait that all trace adapters must implement.
///
/// Each language (C/Rust, Python, Java, etc.) has its own adapter that
/// implements this trait to capture execution events.
pub trait TraceAdapter: Send + Sync {
    /// Start capturing a new process.
    ///
    /// Forks and execs the target binary under trace capture.
    fn start_capture(&self, config: CaptureConfig) -> Result<CaptureSession, TraceError>;

    /// Stop an active capture session.
    ///
    /// Detaches from the target process and finalizes the trace.
    fn stop_capture(&self, session: &CaptureSession) -> Result<(), TraceError>;

    /// Attach to an already running process.
    ///
    /// Uses PTRACE_ATTACH (native) or language-specific attach mechanism.
    fn attach_to_process(
        &self,
        pid: u32,
        config: CaptureConfig,
    ) -> Result<CaptureSession, TraceError>;

    /// Get the language this adapter supports.
    fn get_language(&self) -> Language;

    /// Check if this adapter supports expression evaluation.
    fn supports_expression_eval(&self) -> bool {
        false
    }

    /// Get a human-readable name for this adapter.
    fn name(&self) -> &str;
}

/// Event receiver that consumes trace events from an adapter.
pub trait EventReceiver: Send + Sync {
    /// Handle a single trace event.
    fn on_event(&self, event: TraceEvent) -> Result<(), TraceError>;
}

/// A simple event receiver that collects events into a Vec.
pub struct VecEventReceiver {
    events: std::sync::Mutex<Vec<TraceEvent>>,
}

impl VecEventReceiver {
    pub fn new() -> Self {
        Self {
            events: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn take_events(&self) -> Vec<TraceEvent> {
        let mut guard = self.events.lock().unwrap();
        std::mem::take(&mut *guard)
    }

    pub fn event_count(&self) -> usize {
        self.events.lock().unwrap().len()
    }
}

impl Default for VecEventReceiver {
    fn default() -> Self {
        Self::new()
    }
}

impl EventReceiver for VecEventReceiver {
    fn on_event(&self, event: TraceEvent) -> Result<(), TraceError> {
        self.events.lock().unwrap().push(event);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockAdapter;

    impl TraceAdapter for MockAdapter {
        fn start_capture(&self, _config: CaptureConfig) -> Result<CaptureSession, TraceError> {
            Ok(CaptureSession::new(1, Language::Rust, _config))
        }

        fn stop_capture(&self, _session: &CaptureSession) -> Result<(), TraceError> {
            Ok(())
        }

        fn attach_to_process(
            &self,
            _pid: u32,
            config: CaptureConfig,
        ) -> Result<CaptureSession, TraceError> {
            Ok(CaptureSession::new(_pid, Language::C, config))
        }

        fn get_language(&self) -> Language {
            Language::Rust
        }

        fn name(&self) -> &str {
            "mock"
        }
    }

    #[test]
    fn test_mock_adapter() {
        let adapter = MockAdapter;
        assert_eq!(adapter.get_language(), Language::Rust);
        assert_eq!(adapter.name(), "mock");
        assert!(!adapter.supports_expression_eval());

        let config = CaptureConfig::new("test.rs");
        let session = adapter.start_capture(config).unwrap();
        assert_eq!(session.pid, 1);
    }

    #[test]
    fn test_vec_event_receiver() {
        let receiver = VecEventReceiver::new();
        assert_eq!(receiver.event_count(), 0);

        let event = TraceEvent::function_entry(1, 100, 1, "main", 0x1000);
        receiver.on_event(event).unwrap();
        assert_eq!(receiver.event_count(), 1);

        let events = receiver.take_events();
        assert_eq!(events.len(), 1);
        assert_eq!(receiver.event_count(), 0);
    }
}
