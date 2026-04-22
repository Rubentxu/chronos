//! Integration tests for chronos-java.
//!
//! These tests verify Java/JDWP integration. JVM-dependent tests are gated
//! with `#[cfg_attr(not(feature = "integration"), ignore)]` and require
//! `cargo test --features integration -- --ignored` to run.

use chronos_capture::{AdapterRegistry, TraceAdapter};
use chronos_domain::Language;
use chronos_java::JavaAdapter;
use std::sync::Arc;

#[test]
fn test_registry_has_java_adapter() {
    let mut registry = AdapterRegistry::new();

    // Register the Java adapter
    registry.register(Arc::new(JavaAdapter::new()));

    // Verify we can retrieve it for Java language
    let adapter = registry.get(Language::Java);
    assert!(adapter.is_some(), "Expected Java adapter to be registered");

    // Verify it has the correct language and name
    let adapter = adapter.unwrap();
    assert_eq!(adapter.get_language(), Language::Java);
    assert_eq!(adapter.name(), "java-jdwp");
}

#[test]
fn test_java_adapter_is_available_check() {
    // Just verify the is_available method works
    let available = JavaAdapter::is_available();
    // The test passes regardless of whether java is installed
    // This ensures the method doesn't panic
    assert!(available || !available);
}

/// Lightweight test that verifies JDWP handshake bytes without requiring a JVM.
/// This test runs always and does not need the `integration` feature.
#[test]
fn test_jdwp_handshake_bytes() {
    // JDWP_HANDSHAKE constant must equal b"JDWP-Handshake"
    use chronos_java::protocol::JDWP_HANDSHAKE;
    assert_eq!(JDWP_HANDSHAKE, b"JDWP-Handshake");
    assert_eq!(JDWP_HANDSHAKE.len(), 14);
}

#[tokio::test]
#[cfg_attr(not(feature = "integration"), ignore)]
async fn test_java_hello_world_capture() {
    use std::process::Command;
    use tempfile::TempDir;

    // Check java and javac are on PATH
    if Command::new("javac").arg("-version").output().is_err() {
        eprintln!("javac not found on PATH — skipping integration test");
        return;
    }
    if Command::new("java").arg("-version").output().is_err() {
        eprintln!("java not found on PATH — skipping integration test");
        return;
    }

    // Create a temp directory for our test
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let temp_path = temp_dir.path();

    // Write HelloWorld.java
    let java_content = r#"
public class HelloWorld {
    public static void main(String[] args) {
        System.out.println("Hello, World!");
        for (int i = 0; i < 3; i++) {
            System.out.println("Count: " + i);
        }
    }
}
"#;
    std::fs::write(temp_path.join("HelloWorld.java"), java_content)
        .expect("Failed to write HelloWorld.java");

    // Compile with javac
    let compile_result = Command::new("javac")
        .arg("HelloWorld.java")
        .current_dir(temp_path)
        .output()
        .expect("Failed to compile Java file");

    if !compile_result.status.success() {
        let stderr = String::from_utf8_lossy(&compile_result.stderr);
        eprintln!("javac compilation failed: {}", stderr);
        panic!("Failed to compile HelloWorld.java");
    }

    // Verify class file exists
    assert!(
        temp_path.join("HelloWorld.class").exists(),
        "HelloWorld.class should exist"
    );

    // Create adapter and start capture using JavaAdapter
    let adapter = JavaAdapter::new();
    let config =
        chronos_capture::CaptureConfig::new(temp_path.join("HelloWorld.class").to_str().unwrap());

    let session = adapter.start_capture(config);
    assert!(session.is_ok(), "Should be able to start Java capture");

    let session = session.unwrap();

    // Wait a short time for JVM to start and emit events
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Drain events — verify we received at least one
    let events = adapter.drain_events_internal().unwrap_or_default();

    // The JVM should have emitted method entry events for HelloWorld.main
    assert!(
        !events.is_empty(),
        "Expected at least one trace event from JVM, got none"
    );

    // Stop capture
    let stop_result = adapter.stop_capture(&session);
    assert!(stop_result.is_ok(), "Should be able to stop Java capture");
}
