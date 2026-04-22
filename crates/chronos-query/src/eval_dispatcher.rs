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

/// SessionEvalDispatcher routes evaluate requests to the appropriate backend
/// based on the session's language.
pub struct SessionEvalDispatcher {
    backends: HashMap<Language, Box<dyn EvalBackend>>,
}

impl SessionEvalDispatcher {
    /// Create a new dispatcher with the given backends.
    pub fn new(backends: HashMap<Language, Box<dyn EvalBackend>>) -> Self {
        Self { backends }
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
        Self { backends }
    }

    /// Add a backend for a specific language.
    pub fn with_backend<L: Into<Language>>(mut self, language: L, backend: Box<dyn EvalBackend>) -> Self {
        self.backends.insert(language.into(), backend);
        self
    }

    /// Evaluate an expression for the given session.
    ///
    /// Routes based on session language:
    /// - `Language::Python` → uses DAP backend if available
    /// - `Language::JavaScript` → uses CDP backend if available
    /// - Native languages (C, C++, Rust) and eBPF → uses ExprEvaluator fallback
    /// - Other languages → returns `UnsupportedOperation`
    pub async fn evaluate(
        &self,
        session: &CaptureSession,
        expr: &str,
        frame_id: Option<u64>,
    ) -> EvalResult {
        let language = session.language;

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
}
