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
    // Whether Chrome is installed is a property of the host, so the absolute
    // value is not asserted: with a browser both values are `true`, without
    // one both are `false`.
    //
    // What is asserted here, from outside the crate, is the wiring the callers
    // actually use: `BrowserProbeFactoryImpl::create` gates on this decision,
    // and it reaches it through the `ProbeBackend` impl, not through the
    // static helper. The gate once answered `true` on every host because the
    // probe behind it was a temp-dir allocation; a drift between the two
    // entries would reopen that hole silently.
    //
    // The binary-discovery contract behind the static helper is pinned in
    // `src/adapter.rs::tests` (including the host-independent locator cases).
    let via_trait = ProbeBackend::is_available(&BrowserAdapter::new());
    let via_helper = BrowserAdapter::is_chrome_available();
    assert_eq!(
        via_trait, via_helper,
        "ProbeBackend::is_available must answer with the same discovery decision as \
         is_chrome_available, otherwise the factory gates on a probe the adapter \
         does not use"
    );

    let second = BrowserAdapter::is_chrome_available();
    assert_eq!(
        via_helper, second,
        "availability must be a stable decision, not something re-evaluated to a \
         different answer while the environment is unchanged"
    );
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
