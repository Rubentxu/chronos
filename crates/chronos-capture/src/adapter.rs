//! Trace adapter trait — the interface all language adapters implement.
//!
//! # Interface segregation (REC-C4 / SOLID-001)
//!
//! The adapter surface is split into two capability traits so consumers can
//! depend on only what they use:
//!
//! - [`CaptureLifecycle`]: process lifecycle control (start/stop/attach). This
//!   is the surface consumed in production by the capture pipeline.
//! - [`DebugInspect`]: interactive debug reads (threads, stacks, variables,
//!   runtime info, expression evaluation). Per the REC-C4 explore evidence,
//!   this surface has **no production consumers**: debug reads go through the
//!   `QueryEngine` against recorded events, not through live adapters.
//!
//! [`TraceAdapter`] remains the full legacy interface (lifecycle + debug
//! inspection + identity) so every existing adapter implementation and call
//! site keeps compiling unchanged. The blanket impls below derive both
//! capability traits from any `TraceAdapter`, which lets new consumers bind
//! to the narrow capability they need (e.g. `T: CaptureLifecycle` without the
//! debug-read surface) without touching the ten existing implementations.
//! Both capability traits are re-exported from this module and the crate root.

use chronos_domain::{
    CaptureConfig, CaptureSession, Language, RuntimeInfo, StackFrame, ThreadInfo, TraceError,
    TraceEvent, VariableInfo,
};

/// Trait that all trace adapters must implement.
///
/// Each language (C/Rust, Python, Java, etc.) has its own adapter that
/// implements this trait to capture execution events. Provides the union of
/// the [`CaptureLifecycle`] and [`DebugInspect`] capabilities plus identity
/// methods; see the module docs for the REC-C4 capability split.
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

    /// Get all threads in the target process.
    ///
    /// Returns a list of ThreadInfo with id, name, and current state.
    fn get_threads(&self, _session_id: &str) -> Result<Vec<ThreadInfo>, TraceError> {
        Err(TraceError::UnsupportedOperation("get_threads".to_string()))
    }

    /// Get the stack trace for a specific thread.
    ///
    /// Returns frames ordered innermost-to-outermost (index 0 is the leaf).
    fn get_stack_trace(
        &self,
        _session_id: &str,
        _thread_id: u64,
    ) -> Result<Vec<StackFrame>, TraceError> {
        Err(TraceError::UnsupportedOperation(
            "get_stack_trace".to_string(),
        ))
    }

    /// Get variables visible at a specific stack frame.
    ///
    /// The frame_id corresponds to the depth index from get_stack_trace.
    fn get_variables(
        &self,
        _session_id: &str,
        _frame_id: u64,
    ) -> Result<Vec<VariableInfo>, TraceError> {
        Err(TraceError::UnsupportedOperation(
            "get_variables".to_string(),
        ))
    }

    /// Get runtime metadata for the target process.
    ///
    /// Returns RuntimeInfo with language, version, pid, and uptime.
    fn get_runtime_info(&self, _session_id: &str) -> Result<RuntimeInfo, TraceError> {
        Err(TraceError::UnsupportedOperation(
            "get_runtime_info".to_string(),
        ))
    }

    /// Evaluate an expression in the context of a stack frame.
    ///
    /// The frame_id corresponds to the depth index from get_stack_trace.
    fn evaluate_expression(
        &self,
        _session_id: &str,
        _expr: &str,
        _frame_id: u64,
    ) -> Result<String, TraceError> {
        Err(TraceError::UnsupportedOperation(
            "evaluate_expression".to_string(),
        ))
    }
}

/// Capability trait for capture lifecycle control: start, stop, attach.
///
/// This is the surface consumed in production by the capture pipeline. It is
/// blanket-implemented for every [`TraceAdapter`] below, so all existing
/// adapters satisfy it without changes; new consumers may bind on
/// `T: CaptureLifecycle` alone to depend only on lifecycle control.
pub trait CaptureLifecycle: Send + Sync {
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
}

/// Blanket impl: any [`TraceAdapter`] exposes the lifecycle capability.
impl<T: TraceAdapter> CaptureLifecycle for T {
    fn start_capture(&self, _config: CaptureConfig) -> Result<CaptureSession, TraceError> {
        T::start_capture(self, _config)
    }

    fn stop_capture(&self, _session: &CaptureSession) -> Result<(), TraceError> {
        T::stop_capture(self, _session)
    }

    fn attach_to_process(
        &self,
        _pid: u32,
        _config: CaptureConfig,
    ) -> Result<CaptureSession, TraceError> {
        T::attach_to_process(self, _pid, _config)
    }
}

/// Capability trait for interactive debug inspection of a live capture.
///
/// All methods default to `Err(TraceError::UnsupportedOperation)` (and
/// `supports_expression_eval` defaults to `false`), so adapters only provide
/// what their language runtime actually supports.
///
/// Per the REC-C4 explore evidence this surface has no production consumers:
/// debug reads are served by the `QueryEngine` over recorded events. The
/// blanket impl below derives it from [`TraceAdapter`] for ad-hoc/debugging
/// use without forcing any adapter to implement anything extra.
pub trait DebugInspect: Send + Sync {
    /// Get all threads in the target process.
    ///
    /// Returns a list of ThreadInfo with id, name, and current state.
    fn get_threads(&self, _session_id: &str) -> Result<Vec<ThreadInfo>, TraceError> {
        Err(TraceError::UnsupportedOperation("get_threads".to_string()))
    }

    /// Get the stack trace for a specific thread.
    ///
    /// Returns frames ordered innermost-to-outermost (index 0 is the leaf).
    fn get_stack_trace(
        &self,
        _session_id: &str,
        _thread_id: u64,
    ) -> Result<Vec<StackFrame>, TraceError> {
        Err(TraceError::UnsupportedOperation(
            "get_stack_trace".to_string(),
        ))
    }

    /// Get variables visible at a specific stack frame.
    ///
    /// The frame_id corresponds to the depth index from get_stack_trace.
    fn get_variables(
        &self,
        _session_id: &str,
        _frame_id: u64,
    ) -> Result<Vec<VariableInfo>, TraceError> {
        Err(TraceError::UnsupportedOperation(
            "get_variables".to_string(),
        ))
    }

    /// Get runtime metadata for the target process.
    ///
    /// Returns RuntimeInfo with language, version, pid, and uptime.
    fn get_runtime_info(&self, _session_id: &str) -> Result<RuntimeInfo, TraceError> {
        Err(TraceError::UnsupportedOperation(
            "get_runtime_info".to_string(),
        ))
    }

    /// Evaluate an expression in the context of a stack frame.
    ///
    /// The frame_id corresponds to the depth index from get_stack_trace.
    fn evaluate_expression(
        &self,
        _session_id: &str,
        _expr: &str,
        _frame_id: u64,
    ) -> Result<String, TraceError> {
        Err(TraceError::UnsupportedOperation(
            "evaluate_expression".to_string(),
        ))
    }

    /// Check if this adapter supports expression evaluation.
    fn supports_expression_eval(&self) -> bool {
        false
    }
}

/// Blanket impl: any [`TraceAdapter`] exposes the debug-inspect capability.
impl<T: TraceAdapter> DebugInspect for T {
    fn get_threads(&self, _session_id: &str) -> Result<Vec<ThreadInfo>, TraceError> {
        T::get_threads(self, _session_id)
    }

    fn get_stack_trace(
        &self,
        _session_id: &str,
        _thread_id: u64,
    ) -> Result<Vec<StackFrame>, TraceError> {
        T::get_stack_trace(self, _session_id, _thread_id)
    }

    fn get_variables(
        &self,
        _session_id: &str,
        _frame_id: u64,
    ) -> Result<Vec<VariableInfo>, TraceError> {
        T::get_variables(self, _session_id, _frame_id)
    }

    fn get_runtime_info(&self, _session_id: &str) -> Result<RuntimeInfo, TraceError> {
        T::get_runtime_info(self, _session_id)
    }

    fn evaluate_expression(
        &self,
        _session_id: &str,
        _expr: &str,
        _frame_id: u64,
    ) -> Result<String, TraceError> {
        T::evaluate_expression(self, _session_id, _expr, _frame_id)
    }

    fn supports_expression_eval(&self) -> bool {
        T::supports_expression_eval(self)
    }
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
    use chronos_domain::MonotonicNs;

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
        // UFCS: disambiguate between `TraceAdapter::supports_expression_eval`
        // and the derived `DebugInspect::supports_expression_eval` blanket impl.
        assert!(!TraceAdapter::supports_expression_eval(&adapter));

        let config = CaptureConfig::new("test.rs");
        let session = TraceAdapter::start_capture(&adapter, config).unwrap();
        assert_eq!(session.pid, 1);
    }

    #[test]
    fn test_vec_event_receiver() {
        let receiver = VecEventReceiver::new();
        assert_eq!(receiver.event_count(), 0);

        let event = TraceEvent::function_entry(1, MonotonicNs::from(100), 1, "main", 0x1000);
        receiver.on_event(event).unwrap();
        assert_eq!(receiver.event_count(), 1);

        let events = receiver.take_events();
        assert_eq!(events.len(), 1);
        assert_eq!(receiver.event_count(), 0);
    }

    #[test]
    fn test_trace_adapter_default_get_threads() {
        let adapter = MockAdapter;
        let result = TraceAdapter::get_threads(&adapter, "session-1");
        assert!(result.is_err());
        match result {
            Err(TraceError::UnsupportedOperation(op)) => {
                assert_eq!(op, "get_threads");
            }
            _ => panic!("Expected UnsupportedOperation error"),
        }
    }

    #[test]
    fn test_trace_adapter_default_get_stack_trace() {
        let adapter = MockAdapter;
        let result = TraceAdapter::get_stack_trace(&adapter, "session-1", 42);
        assert!(result.is_err());
        match result {
            Err(TraceError::UnsupportedOperation(op)) => {
                assert_eq!(op, "get_stack_trace");
            }
            _ => panic!("Expected UnsupportedOperation error"),
        }
    }

    #[test]
    fn test_trace_adapter_default_get_variables() {
        let adapter = MockAdapter;
        let result = TraceAdapter::get_variables(&adapter, "session-1", 0);
        assert!(result.is_err());
        match result {
            Err(TraceError::UnsupportedOperation(op)) => {
                assert_eq!(op, "get_variables");
            }
            _ => panic!("Expected UnsupportedOperation error"),
        }
    }

    #[test]
    fn test_trace_adapter_default_get_runtime_info() {
        let adapter = MockAdapter;
        let result = TraceAdapter::get_runtime_info(&adapter, "session-1");
        assert!(result.is_err());
        match result {
            Err(TraceError::UnsupportedOperation(op)) => {
                assert_eq!(op, "get_runtime_info");
            }
            _ => panic!("Expected UnsupportedOperation error"),
        }
    }

    #[test]
    fn test_trace_adapter_default_evaluate_expression() {
        let adapter = MockAdapter;
        let result = TraceAdapter::evaluate_expression(&adapter, "session-1", "x + y", 0);
        assert!(result.is_err());
        match result {
            Err(TraceError::UnsupportedOperation(op)) => {
                assert_eq!(op, "evaluate_expression");
            }
            _ => panic!("Expected UnsupportedOperation error"),
        }
    }
}
