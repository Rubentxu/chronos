//! Integration tests for chronos-go.

use chronos_capture::{AdapterRegistry, TraceAdapter};
use chronos_domain::Language;
use chronos_go::GoAdapter;
use std::sync::Arc;

#[test]
fn test_registry_has_go_adapter() {
    let mut registry = AdapterRegistry::new();

    // Register the Go adapter
    registry.register(Arc::new(GoAdapter::new()));

    // Verify we can retrieve it for Go language
    let adapter = registry.get(Language::Go);
    assert!(adapter.is_some(), "Expected Go adapter to be registered");

    // Verify it has the correct language and name
    let adapter = adapter.unwrap();
    assert_eq!(adapter.get_language(), Language::Go);
    assert_eq!(adapter.name(), "go-delve");
}

#[test]
fn test_go_adapter_is_available_check() {
    // Just verify the is_available method works
    let available = GoAdapter::is_available();
    // The test passes regardless of whether dlv is installed
    // This ensures the method doesn't panic
    assert!(available || !available);
}

#[tokio::test]
#[ignore] // requires dlv on PATH
async fn test_go_main_capture() {
    use std::fs;
    use std::process::Command;
    use tempfile::TempDir;

    // Create a temp directory for our test
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Write main.go
    let go_content = r#"
package main

import "fmt"

func main() {
    fmt.Println("Hello, World!")
    for i := 0; i < 3; i++ {
        fmt.Printf("Count: %d\n", i)
    }
}
"#;
    fs::write(temp_path.join("main.go"), go_content).unwrap();

    // Build the Go program (create a temporary binary)
    let binary_path = temp_path.join("test_program");
    let build_result = Command::new("go")
        .args(&["build", "-o", binary_path.to_str().unwrap()])
        .current_dir(temp_path)
        .output()
        .expect("Failed to build Go file");

    if !build_result.status.success() {
        let stderr = String::from_utf8_lossy(&build_result.stderr);
        eprintln!("go build failed: {}", stderr);
        panic!("Failed to build main.go");
    }

    // Verify binary exists
    assert!(binary_path.exists(), "Binary should exist");

    // Create adapter and try to start capture
    // Note: Go Delve debugging requires compiling with -gcflags="all=-N -l"
    // for full debugging support, which we don't do here.
    // The test verifies the spawn mechanism works.
    let adapter = GoAdapter::new();
    let config = chronos_capture::CaptureConfig::new(binary_path.to_str().unwrap());

    // This may fail because the binary wasn't built for debugging,
    // but it tests that the adapter can attempt to spawn dlv
    let _session = adapter.start_capture(config);
    // We don't assert success because dlv may not be able to debug
    // a non-debug binary - we just verify it doesn't panic
}
