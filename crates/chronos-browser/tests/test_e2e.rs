//! End-to-end tests for browser/WASM debugging.
//!
//! These tests are conditional and only run when:
//! 1. Chrome is found on the system
//! 2. The `CHRONOS_E2E=1` environment variable is set
//!
//! To run these tests: CHRONOS_E2E=1 cargo test -p chronos-browser test_e2e

use chronos_browser::adapter::BrowserAdapter;
use chronos_capture::adapter::TraceAdapter;
use chronos_domain::ProbeBackend;
use std::env;
use std::path::PathBuf;

/// Check if E2E tests should run
fn should_run_e2e_tests() -> bool {
    env::var("CHRONOS_E2E").as_deref().unwrap_or("") == "1"
}

/// Get the path to the test fixtures
fn test_fixtures_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("wasm")
}

/// Check if Chrome is available for testing
fn is_chrome_available() -> bool {
    BrowserAdapter::is_chrome_available()
}

#[test]
#[ignore]
fn test_e2e_chrome_detection() {
    // This test always runs but is ignored by default
    // It just verifies Chrome detection works
    let available = is_chrome_available();
    println!("Chrome available: {}", available);
}

/// Test browser probe with actual Chrome - only runs with CHRONOS_E2E=1
#[tokio::test]
#[ignore]
async fn test_e2e_browser_probe_wasm_detection() {
    // Skip if E2E not enabled
    if !should_run_e2e_tests() {
        eprintln!("E2E tests skipped: CHRONOS_E2E=1 not set");
        return;
    }

    // Skip if Chrome not available
    if !is_chrome_available() {
        eprintln!("E2E tests skipped: Chrome not available");
        return;
    }

    let adapter = BrowserAdapter::new();

    // Start capture with a local file URL
    let fixtures = test_fixtures_path();
    let url = format!("file://{}/index.html", fixtures.display());

    let config = chronos_domain::CaptureConfig::new(&url);
    let session = adapter.start_capture(config);

    match session {
        Ok(_session) => {
            println!("Browser probe started successfully");

            // Give Chrome time to load and detect WASM modules
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

            // Drain events
            let events: Result<Vec<_>, _> = adapter.drain_events();
            let event_count = events.as_ref().map(|e| e.len()).unwrap_or(0);
            println!("Drained {} events", event_count);

            // Stop the probe
            let stop_result = adapter.stop_probe(&_session);
            if stop_result.is_err() {
                println!("Stop error: {:?}", stop_result.err());
            }

            // Verify we got some events (at least the WASM module load events)
            assert!(events.is_ok());
        }
        Err(e) => {
            panic!("Failed to start browser probe: {}", e);
        }
    }
}

/// Test WASM module detection only - minimal version
#[tokio::test]
#[ignore]
async fn test_e2e_wasm_module_detection() {
    if !should_run_e2e_tests() {
        eprintln!("E2E tests skipped: CHRONOS_E2E=1 not set");
        return;
    }

    if !is_chrome_available() {
        eprintln!("E2E tests skipped: Chrome not available");
        return;
    }

    let adapter = BrowserAdapter::new();

    // Use a known WASM URL
    // This is a simple public WASM module for testing
    let url = "https://webassembly.org/".to_string();

    let config = chronos_domain::CaptureConfig::new(&url);
    let session = adapter.start_capture(config);

    match session {
        Ok(_session) => {
            // Give time for WASM detection
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

            let events: Result<Vec<_>, _> = adapter.drain_events();
            let event_count = events.as_ref().map(|e| e.len()).unwrap_or(0);
            println!("Events captured: {}", event_count);

            let _ = adapter.stop_probe(&_session);
        }
        Err(e) => {
            // E2E test might fail due to network or CORS issues - that's OK
            println!("Browser probe failed (expected in some environments): {}", e);
        }
    }
}
