//! Tests for JavaTarget (JDWP-based debugging).
//!
//! These tests verify JavaTarget creation, JDWP port configuration,
//! and container options generation without requiring real containers.

#![cfg(feature = "java")]

use chronos_sandbox::container::quadlet::{ptrace_container_options, ContainerOptions};
use chronos_sandbox::targets::java::JavaTarget;
use chronos_sandbox::targets::DebugTarget;

/// Tests that JavaTarget can be created with default JDWP port (5005).
#[test]
fn test_java_target_creation() {
    let target = JavaTarget::new();
    assert!(!target.is_attached(), "JavaTarget should not be attached on creation");
}

/// Tests that JavaTarget can be created with custom port.
#[test]
fn test_java_target_custom_port() {
    let target = JavaTarget::with_port(8000);
    assert!(!target.is_attached(), "JavaTarget should not be attached on creation");
    // The port field is private, but we can verify via spawn behavior
}

/// Tests that ContainerOptions expose the JDWP port (5005).
#[test]
fn test_java_container_options() {
    // Java uses ptrace_container_options for now since it also needs debugging caps
    let opts: ContainerOptions = ptrace_container_options("java-debug:latest", "test-java", 5005);

    assert_eq!(opts.name, "test-java");
    assert_eq!(opts.image, "java-debug:latest");
    assert!(
        opts.ports.iter().any(|p| p.host == 5005 && p.container == 5005),
        "JDWP port 5005 should be exposed"
    );
}

/// Full session test that requires real Podman and a Java container.
/// This test is ignored by default and only runs with `cargo test -- --ignored`.
#[test]
#[ignore]
// requires: podman
fn test_java_full_session() {
    // This test would:
    // 1. Start a Java container with JDWP agent
    // 2. Launch a Java program with -agentlib:jdwp
    // 3. Attach via JDWP
    // 4. Set a breakpoint
    // 5. Verify breakpoint hit
    // 6. Detach and cleanup

    let target = JavaTarget::new();
    let result = target.spawn("java", &["-cp", "/app", "Main"]);
    println!("Java full session test result: {:?}", result);
}
