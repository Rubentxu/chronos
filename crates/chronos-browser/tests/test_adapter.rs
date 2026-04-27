//! Integration tests for BrowserAdapter with mock CDP.

use chronos_browser::adapter::BrowserAdapter;
use chronos_capture::adapter::TraceAdapter;
use chronos_domain::{ProbeBackend, Language};

#[test]
fn test_browser_adapter_creation() {
    let adapter = BrowserAdapter::new();
    assert_eq!(TraceAdapter::name(&adapter), "browser-wasm");
    assert_eq!(adapter.get_language(), Language::WebAssembly);
}

#[test]
fn test_browser_adapter_default() {
    let adapter = BrowserAdapter::default();
    assert_eq!(TraceAdapter::name(&adapter), "browser-wasm");
}

#[test]
fn test_browser_adapter_is_available() {
    // This test just verifies the method doesn't panic
    // The actual result depends on whether Chrome is installed
    let _available = BrowserAdapter::is_chrome_available();
}

#[test]
fn test_browser_adapter_language() {
    let adapter = BrowserAdapter::new();
    assert_eq!(adapter.get_language(), Language::WebAssembly);
}

#[test]
fn test_browser_adapter_name() {
    let adapter = BrowserAdapter::new();
    assert_eq!(TraceAdapter::name(&adapter), "browser-wasm");
}

// Note: These tests use mock data since we can't actually connect to Chrome in unit tests
// The real integration tests (test_e2e.rs) test actual Chrome connections

#[test]
fn test_browser_adapter_drain_events_empty() {
    let adapter = BrowserAdapter::new();
    // Initially, drain should return empty since no events have been captured
    let result = adapter.drain_events();
    assert!(result.is_ok());
    let events = result.unwrap();
    assert!(events.is_empty());
}

#[test]
fn test_browser_adapter_drain_raw_events_empty() {
    let adapter = BrowserAdapter::new();
    // Initially, drain_raw should return empty
    let events = adapter.drain_raw_events();
    assert!(events.is_empty());
}
