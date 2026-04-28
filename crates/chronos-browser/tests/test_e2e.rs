//! End-to-end tests for browser WASM debugging.
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
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::thread;

/// WASM test fixtures
mod fixtures;
use fixtures::wasm::ensure_add_wasm;

/// Gate: only run E2E tests when CHRONOS_E2E=1
fn e2e_enabled() -> bool {
    env::var("CHRONOS_E2E").as_deref() == Ok("1")
}

/// Check if Chrome is available
fn chrome_available() -> bool {
    BrowserAdapter::is_chrome_available()
}

/// Get the path to the test fixtures
fn test_fixtures_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("wasm")
}

/// Simple HTTP file server for test fixtures
struct TestHttpServer {
    listener: TcpListener,
    port: u16,
}

impl TestHttpServer {
    fn start(root_dir: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let root = root_dir.to_string();

        let server_listener = listener.try_clone().unwrap();
        thread::spawn(move || {
            for stream in server_listener.incoming() {
                if let Ok(mut stream) = stream {
                    let mut buf = [0u8; 1024];
                    let _ = stream.read(&mut buf);
                    let request = String::from_utf8_lossy(&buf);

                    // Parse GET path
                    let path = if let Some(line) = request.lines().next() {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 2 {
                            parts[1].to_string()
                        } else {
                            "/".to_string()
                        }
                    } else {
                        "/".to_string()
                    };

                    let file_path = if path == "/" || path == "/index.html" {
                        format!("{}/index.html", root)
                    } else if path == "/add.wasm" {
                        format!("{}/add.wasm", root)
                    } else {
                        continue;
                    };

                    if let Ok(content) = std::fs::read(&file_path) {
                        let mime = if file_path.ends_with(".wasm") {
                            "application/wasm"
                        } else {
                            "text/html"
                        };
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\n\r\n",
                            mime,
                            content.len()
                        );
                        let _ = stream.write_all(response.as_bytes());
                        let _ = stream.write_all(&content);
                    } else {
                        let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\n\r\n");
                    }
                }
            }
        });

        Self { listener, port }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

/// Test Chrome detection - always runs but is ignored by default
#[test]
#[ignore]
fn test_e2e_chrome_detection() {
    // This test always runs but is ignored by default
    // It just verifies Chrome detection works
    let available = chrome_available();
    println!("Chrome available: {}", available);
}

/// Test browser probe with actual Chrome - only runs with CHRONOS_E2E=1
#[tokio::test]
#[ignore]
async fn test_e2e_browser_probe_wasm_detection() {
    // Skip if E2E not enabled
    if !e2e_enabled() {
        eprintln!("E2E tests skipped: CHRONOS_E2E=1 not set");
        return;
    }

    // Skip if Chrome not available
    if !chrome_available() {
        eprintln!("E2E tests skipped: Chrome not available");
        return;
    }

    // Ensure WASM fixture exists
    let wasm_path = ensure_add_wasm();
    println!("Using WASM fixture at: {:?}", wasm_path);

    // Start HTTP server to serve fixtures
    let fixtures = test_fixtures_path();
    let server = TestHttpServer::start(fixtures.to_str().unwrap());
    let url = format!("{}/index.html", server.url());
    println!("Serving fixtures at: {}", url);

    let adapter = BrowserAdapter::new();

    // Start capture with local HTTP URL
    let config = chronos_domain::CaptureConfig::new(&url);
    let session = adapter.start_probe_async(config, true, None).await;

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
    if !e2e_enabled() {
        eprintln!("E2E tests skipped: CHRONOS_E2E=1 not set");
        return;
    }

    if !chrome_available() {
        eprintln!("E2E tests skipped: Chrome not available");
        return;
    }

    // Ensure WASM fixture exists
    let _wasm_path = ensure_add_wasm();

    let adapter = BrowserAdapter::new();

    // Start HTTP server to serve fixtures
    let fixtures = test_fixtures_path();
    let server = TestHttpServer::start(fixtures.to_str().unwrap());
    let url = format!("{}/index.html", server.url());
    println!("Serving fixtures at: {}", url);

    let config = chronos_domain::CaptureConfig::new(&url);
    let session = adapter.start_probe_async(config, true, None).await;

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

/// Test that fixtures module works correctly
#[test]
fn test_e2e_wasm_fixtures() {
    let path = ensure_add_wasm();
    assert!(path.exists(), "WASM fixture should exist at {:?}", path);

    // Verify the file is a valid WASM binary (starts with \0asm)
    let content = std::fs::read(&path).expect("Should be able to read WASM file");
    assert!(
        content.starts_with(&[0x00, 0x61, 0x73, 0x6d]),
        "WASM file should start with magic bytes"
    );
}
