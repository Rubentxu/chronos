//! SessionEvalDispatcher — routes evaluate requests to the appropriate backend.
//!
//! This module provides multi-language expression evaluation by routing requests
//! based on the session's language to DAP (Python), CDP (JavaScript), or the
//! built-in ExprEvaluator (Native/eBPF).

use crate::engine::ExprEvaluator;
use chronos_domain::{CaptureSession, Language, TraceError};
use std::collections::HashMap;

/// Result of an evaluate operation — returns a string representation.
pub type EvalResult = Result<String, TraceError>;

/// Trait for evaluation backends.
///
/// Implementors handle expression evaluation for a specific language or
/// debugging protocol.
pub trait EvalBackend: Send + Sync {
    /// Evaluate an expression synchronously.
    fn evaluate_sync(&self, expr: &str, frame_id: Option<u64>) -> EvalResult;
}

/// A simple evaluator backend that uses ExprEvaluator with pre-built local variables.
pub struct SimpleEvalBackend {
    locals: HashMap<String, String>,
}

impl SimpleEvalBackend {
    pub fn new(locals: HashMap<String, String>) -> Self {
        Self { locals }
    }
}

impl EvalBackend for SimpleEvalBackend {
    fn evaluate_sync(&self, expr: &str, _frame_id: Option<u64>) -> EvalResult {
        let evaluator = ExprEvaluator::new(self.locals.clone());
        evaluator
            .evaluate(expr)
            .map(|v| v.to_string())
            .map_err(|e| TraceError::InvalidExpression(format!("{:?}", e)))
    }
}

/// No-op backend that returns UnsupportedOperation.
/// Used when no actual debugger adapter is connected.
pub struct NoOpEvalBackend;

impl EvalBackend for NoOpEvalBackend {
    fn evaluate_sync(&self, _expr: &str, _frame_id: Option<u64>) -> EvalResult {
        Err(TraceError::UnsupportedOperation(
            "No evaluation backend available for this language".to_string(),
        ))
    }
}

/// SessionEvalDispatcher routes evaluate requests to the appropriate backend
/// based on the session's language.
pub struct SessionEvalDispatcher {
    /// Backends registered per language.
    backends: HashMap<Language, Box<dyn EvalBackend>>,
    /// Per-session backend overrides (session_id → backend).
    /// These take precedence over language-based backends.
    session_backends: HashMap<String, Box<dyn EvalBackend>>,
}

impl SessionEvalDispatcher {
    /// Create a new dispatcher with the given backends.
    pub fn new(backends: HashMap<Language, Box<dyn EvalBackend>>) -> Self {
        Self {
            backends,
            session_backends: HashMap::new(),
        }
    }

    /// Create a dispatcher with only the native evaluator (no DAP/CDP backends).
    ///
    /// For languages without a debugging protocol (C, C++, Rust, eBPF),
    /// falls back to arithmetic expression evaluation.
    pub fn with_native_evaluator(locals: HashMap<String, String>) -> Self {
        let mut backends = HashMap::new();
        // Create separate backends for each native language
        backends.insert(Language::C, Box::new(SimpleEvalBackend::new(locals.clone())) as Box<dyn EvalBackend>);
        backends.insert(Language::Cpp, Box::new(SimpleEvalBackend::new(locals.clone())) as Box<dyn EvalBackend>);
        backends.insert(Language::Rust, Box::new(SimpleEvalBackend::new(locals.clone())) as Box<dyn EvalBackend>);
        backends.insert(Language::Ebpf, Box::new(SimpleEvalBackend::new(locals)) as Box<dyn EvalBackend>);
        Self {
            backends,
            session_backends: HashMap::new(),
        }
    }

    /// Add a backend for a specific language.
    pub fn with_backend<L: Into<Language>>(mut self, language: L, backend: Box<dyn EvalBackend>) -> Self {
        self.backends.insert(language.into(), backend);
        self
    }

    /// Register an EvalBackend for a specific session.
    ///
    /// This allows per-session evaluation backends, such as DAP-based
    /// evaluation for Python sessions or CDP-based evaluation for JS sessions.
    ///
    /// # Arguments
    /// * `session_id` - The unique session identifier
    /// * `backend` - The evaluation backend to use for this session
    pub fn register(&mut self, session_id: String, backend: Box<dyn EvalBackend>) {
        self.session_backends.insert(session_id, backend);
    }

    /// Register a no-op backend for a session with an unsupported language.
    ///
    /// This is used when a session is created for a language that doesn't
    /// have an active debugger adapter connected.
    pub fn register_noop(&mut self, session_id: String) {
        self.session_backends.insert(session_id, Box::new(NoOpEvalBackend));
    }

    /// Remove a session backend (called on session cleanup/drop).
    pub fn unregister(&mut self, session_id: &str) {
        self.session_backends.remove(session_id);
    }

    /// Check if a session has a registered backend.
    pub fn has_session_backend(&self, session_id: &str) -> bool {
        self.session_backends.contains_key(session_id)
    }

    /// Synchronous evaluate for testing purposes.
    ///
    /// This provides a synchronous interface to the dispatcher by borrowing
    /// the internal state. It's intended for testing only.
    pub fn blocking_evaluate(&self, session: &CaptureSession, expr: &str) -> EvalResult {
        let language = session.language;
        let session_id = session.session_id.to_string();

        // Check for per-session backend first
        if let Some(backend) = self.session_backends.get(&session_id) {
            return backend.evaluate_sync(expr, None);
        }

        // Check language-specific backends
        if let Some(backend) = self.backends.get(&language) {
            return backend.evaluate_sync(expr, None);
        }

        Err(TraceError::UnsupportedOperation(format!(
            "No evaluator backend available for {}",
            language
        )))
    }

    /// Evaluate an expression for the given session.
    ///
    /// Routes based on session:
    /// 1. First checks if there's a per-session backend registered (takes precedence)
    /// 2. Then checks language-specific backends
    /// 3. Falls back to UnsupportedOperation if no backend found
    pub async fn evaluate(
        &self,
        session: &CaptureSession,
        expr: &str,
        frame_id: Option<u64>,
    ) -> EvalResult {
        let language = session.language;
        let session_id = session.session_id.to_string();

        // Check for per-session backend first (takes precedence)
        if let Some(backend) = self.session_backends.get(&session_id) {
            return backend.evaluate_sync(expr, frame_id);
        }

        // Languages that use DAP/CDP backends
        if language == Language::Python || language == Language::JavaScript {
            if let Some(backend) = self.backends.get(&language) {
                return backend.evaluate_sync(expr, frame_id);
            }
            return Err(TraceError::UnsupportedOperation(format!(
                "evaluate_expression not available for {}: DAP/CDP adapter not connected",
                language
            )));
        }

        // Native languages and eBPF use ExprEvaluator fallback
        if matches!(
            language,
            Language::C | Language::Cpp | Language::Rust | Language::Ebpf
        ) {
            if let Some(backend) = self.backends.get(&language) {
                return backend.evaluate_sync(expr, frame_id);
            }
            return Err(TraceError::UnsupportedOperation(format!(
                "No evaluator backend registered for {}",
                language
            )));
        }

        Err(TraceError::UnsupportedOperation(format!(
            "evaluate_expression not supported for language: {}",
            language
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_eval_backend_arithmetic() {
        let locals = HashMap::from([
            ("a".to_string(), "5".to_string()),
            ("b".to_string(), "3".to_string()),
        ]);
        let backend = SimpleEvalBackend::new(locals);

        assert_eq!(backend.evaluate_sync("a + b", None).unwrap(), "8");
        assert_eq!(backend.evaluate_sync("a - b", None).unwrap(), "2");
        assert_eq!(backend.evaluate_sync("a * b", None).unwrap(), "15");
        assert_eq!(backend.evaluate_sync("a / b", None).unwrap(), "1.6666666666666667");
    }

    #[test]
    fn test_simple_eval_backend_unknown_variable() {
        let locals = HashMap::new();
        let backend = SimpleEvalBackend::new(locals);

        let result = backend.evaluate_sync("unknown_var + 1", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_simple_eval_backend_division_by_zero() {
        let locals = HashMap::from([("zero".to_string(), "0".to_string())]);
        let backend = SimpleEvalBackend::new(locals);

        let result = backend.evaluate_sync("10 / zero", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_dispatcher_with_custom_backend_sync() {
        struct MockBackend;
        impl EvalBackend for MockBackend {
            fn evaluate_sync(&self, expr: &str, _frame_id: Option<u64>) -> EvalResult {
                Ok(format!("mock:{}", expr))
            }
        }

        let mut backends = HashMap::new();
        backends.insert(Language::Python, Box::new(MockBackend) as Box<dyn EvalBackend>);
        let dispatcher = SessionEvalDispatcher::new(backends);

        // Test that Python uses the custom backend (sync test since we call evaluate_sync directly)
        let backend = dispatcher.backends.get(&Language::Python).unwrap();
        let result = backend.evaluate_sync("my_expr", None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "mock:my_expr");
    }

    #[test]
    fn test_dispatcher_native_languages_have_backends() {
        let locals = HashMap::from([("x".to_string(), "10".to_string())]);
        let dispatcher = SessionEvalDispatcher::with_native_evaluator(locals);

        // C should have a backend
        let backend = dispatcher.backends.get(&Language::C);
        assert!(backend.is_some());

        // C++ should have a backend
        let backend = dispatcher.backends.get(&Language::Cpp);
        assert!(backend.is_some());

        // Rust should have a backend
        let backend = dispatcher.backends.get(&Language::Rust);
        assert!(backend.is_some());

        // eBPF should have a backend
        let backend = dispatcher.backends.get(&Language::Ebpf);
        assert!(backend.is_some());
    }

    #[test]
    fn test_dispatcher_does_not_have_python_backend_by_default() {
        let locals = HashMap::new();
        let dispatcher = SessionEvalDispatcher::with_native_evaluator(locals);

        // Python should NOT have a backend in the native-only dispatcher
        let backend = dispatcher.backends.get(&Language::Python);
        assert!(backend.is_none());
    }

    #[test]
    fn test_register_session_backend() {
        struct MockBackend;
        impl EvalBackend for MockBackend {
            fn evaluate_sync(&self, expr: &str, _frame_id: Option<u64>) -> EvalResult {
                Ok(format!("session_backend:{}", expr))
            }
        }

        let locals = HashMap::new();
        let mut dispatcher = SessionEvalDispatcher::with_native_evaluator(locals);

        // Register a session-specific backend
        dispatcher.register("session-123".to_string(), Box::new(MockBackend));

        // Check that session backend is registered
        assert!(dispatcher.has_session_backend("session-123"));
        assert!(!dispatcher.has_session_backend("session-456"));
    }

    #[test]
    fn test_register_noop_backend() {
        let locals = HashMap::new();
        let mut dispatcher = SessionEvalDispatcher::with_native_evaluator(locals);

        // Register a no-op backend for unsupported language
        dispatcher.register_noop("python-session".to_string());

        // Check that session backend is registered
        assert!(dispatcher.has_session_backend("python-session"));
    }

    #[test]
    fn test_session_backend_takes_precedence() {
        struct CustomBackend;
        impl EvalBackend for CustomBackend {
            fn evaluate_sync(&self, expr: &str, _frame_id: Option<u64>) -> EvalResult {
                Ok(format!("custom:{}", expr))
            }
        }

        let locals = HashMap::from([("x".to_string(), "10".to_string())]);
        let mut dispatcher = SessionEvalDispatcher::with_native_evaluator(locals);

        // Register a custom backend for a Python session
        dispatcher.register("py-session".to_string(), Box::new(CustomBackend));

        // Create a minimal capture session
        let session = chronos_domain::CaptureSession::minimal(
            "py-session".to_string(),
            Language::Python,
        );

        // The custom backend should be used, not the native Python (which doesn't exist anyway)
        let result = dispatcher.blocking_evaluate(&session, "x + 1");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "custom:x + 1");
    }

    #[test]
    fn test_noop_backend_returns_unsupported() {
        let locals = HashMap::new();
        let mut dispatcher = SessionEvalDispatcher::with_native_evaluator(locals);

        // Register a no-op backend
        dispatcher.register_noop("unknown-session".to_string());

        let session = chronos_domain::CaptureSession::minimal(
            "unknown-session".to_string(),
            Language::Unknown,
        );

        let result = dispatcher.blocking_evaluate(&session, "x + 1");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), chronos_domain::TraceError::UnsupportedOperation(_)));
    }
}
