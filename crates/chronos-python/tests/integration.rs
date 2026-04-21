//! Integration tests for chronos-python.

use chronos_capture::{AdapterRegistry, TraceAdapter};
use chronos_domain::Language;
use chronos_python::PythonAdapter;

#[test]
fn test_registry_has_python_adapter() {
    let mut registry = AdapterRegistry::new();
    
    // Register the Python adapter
    registry.register(std::sync::Arc::new(PythonAdapter::new()));
    
    // Verify we can retrieve it for Python language
    let adapter = registry.get(Language::Python);
    assert!(adapter.is_some(), "Expected Python adapter to be registered");
    
    // Verify it has the correct language and name
    let adapter = adapter.unwrap();
    assert_eq!(adapter.get_language(), Language::Python);
    assert_eq!(adapter.name(), "python-settrace");
}

#[test]
fn test_registry_can_register_multiple_adapters() {
    let mut registry = AdapterRegistry::new();
    
    // Register Python adapter
    registry.register(std::sync::Arc::new(PythonAdapter::new()));
    
    // Should only have Python registered
    let langs = registry.registered_languages();
    assert_eq!(langs.len(), 1);
    assert!(registry.has_adapter(Language::Python));
    assert!(!registry.has_adapter(Language::Rust));
}

#[test]
fn test_python_adapter_supports_expression_eval() {
    let adapter = PythonAdapter::new();
    // Python settrace doesn't support expression eval in MVP
    assert!(!adapter.supports_expression_eval());
}
