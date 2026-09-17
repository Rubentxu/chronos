//! Integration tests for BrowserAdapter with mock CDP.

use chronos_browser::adapter::BrowserAdapter;
use chronos_domain::ProbeBackend;

#[test]
fn test_browser_adapter_creation() {
    let adapter = BrowserAdapter::new();
    assert_eq!(ProbeBackend::name(&adapter), "browser-wasm");
}

#[test]
fn test_browser_adapter_default() {
    let adapter = BrowserAdapter::default();
    assert_eq!(ProbeBackend::name(&adapter), "browser-wasm");
}

#[test]
fn test_browser_adapter_is_available() {
    // This test just verifies the method doesn't panic
    // The actual result depends on whether Chrome is installed
    let _available = BrowserAdapter::is_chrome_available();
}

#[test]
fn test_browser_adapter_name() {
    let adapter = BrowserAdapter::new();
    assert_eq!(ProbeBackend::name(&adapter), "browser-wasm");
}

#[test]
fn test_browser_adapter_drain_events_empty() {
    let adapter = BrowserAdapter::new();
    // Initially, drain should return empty since no events have been captured
    let result = adapter.take_semantic_events();
    assert!(result.is_ok());
    let events = result.unwrap();
    assert!(events.is_empty());
}

#[test]
fn test_browser_adapter_raw_events_empty() {
    let adapter = BrowserAdapter::new();
    // Initially, raw_events should return empty
    let events = adapter.raw_events();
    assert!(events.is_empty());
}
