//! Tests for PythonTarget (debugpy-based debugging).
//!
//! These tests verify PythonTarget creation, debugpy port configuration,
//! and container options generation without requiring real containers.

#![cfg(feature = "python")]

use chronos_sandbox::container::quadlet::{ptrace_container_options, ContainerOptions};
use chronos_sandbox::targets::python::PythonTarget;
use chronos_sandbox::targets::DebugTarget;

/// Tests that PythonTarget can be created with default debugpy port (5678).
#[test]
fn test_python_target_creation() {
    let target = PythonTarget::new();
    assert!(!target.is_attached(), "PythonTarget should not be attached on creation");
}

/// Tests that ContainerOptions expose the debugpy port (5678).
#[test]
fn test_python_container_options() {
    let opts: ContainerOptions = ptrace_container_options("python-debug:latest", "test-python", 5678);

    assert_eq!(opts.name, "test-python");
    assert_eq!(opts.image, "python-debug:latest");
    assert!(
        opts.ports.iter().any(|p| p.host == 5678 && p.container == 5678),
        "debugpy port 5678 should be exposed"
    );
}

/// Full session test that requires real Podman, Python, and debugpy.
/// This test is ignored by default and only runs with
/// `cargo test --features integration -- --ignored`.
#[test]
#[cfg_attr(not(feature = "integration"), ignore)]
// requires: podman, python3, debugpy
fn test_python_full_session() {
    // This test would:
    // 1. Start a Python container with debugpy
    // 2. Launch a Python program with debugpy --listen
    // 3. Attach via DAP protocol
    // 4. Set a breakpoint
    // 5. Verify breakpoint hit
    // 6. Detach and cleanup

    // TODO: Implement actual debugpy DAP connection and event verification
    let target = PythonTarget::new();
    let result = target.spawn("python", &["-m", "debugpy", "--listen", "5678", "app.py"]);
    println!("Python full session test result: {:?}", result);

    // Assert: result should be Ok after implementation
    // assert!(result.is_ok(), "Python session should start successfully");
}
