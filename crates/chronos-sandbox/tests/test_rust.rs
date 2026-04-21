//! Tests for RustTarget (ptrace-based debugging).
//!
//! These tests verify RustTarget creation, ptrace capability detection,
//! and container options generation without requiring real containers.

use chronos_sandbox::container::quadlet::{ptrace_container_options, ContainerOptions};
use chronos_sandbox::targets::rust::RustTarget;
use chronos_sandbox::targets::DebugTarget;

/// Tests that RustTarget can be created with default configuration.
#[test]
fn test_rust_target_creation() {
    let target = RustTarget::new();
    assert!(!target.is_attached(), "RustTarget should not be attached on creation");
}

/// Tests ptrace capability detection.
/// On Linux systems, this verifies the target correctly reports its state.
#[test]
fn test_rust_target_ptrace_cap_detection() {
    let target = RustTarget::new();
    // After creation, should not be attached
    assert!(!target.is_attached(), "Target should not be attached initially");

    // On non-Linux systems, attach should fail gracefully
    #[cfg(not(target_os = "linux"))]
    {
        let result = target.attach(1);
        assert!(result.is_err(), "attach should fail on non-Linux");
    }
}

/// Tests that ContainerOptions generated for Rust/ptrace have SYS_PTRACE capability.
#[test]
fn test_rust_container_options() {
    let opts: ContainerOptions = ptrace_container_options("rust-debug:latest", "test-rust", 2345);

    assert_eq!(opts.name, "test-rust");
    assert_eq!(opts.image, "rust-debug:latest");
    assert!(opts.caps.contains(&"SYS_PTRACE".to_string()), "Should have SYS_PTRACE cap");
    assert!(
        opts.security_opt.contains(&"seccomp=unconfined".to_string()),
        "Should have seccomp=unconfined"
    );
    assert!(
        opts.ports.iter().any(|p| p.host == 2345 && p.container == 2345),
        "Port 2345 should be exposed"
    );
    assert_eq!(opts.network, Some("chronos-network".to_string()));
}

/// Full session test that requires real Podman and a running container.
/// This test is ignored by default and only runs with `cargo test -- --ignored`.
#[test]
#[ignore]
// requires: podman
fn test_rust_full_session() {
    // This test would:
    // 1. Start a Rust container with ptrace capabilities
    // 2. Spawn a simple Rust program
    // 3. Attach via ptrace
    // 4. Set a breakpoint
    // 5. Verify breakpoint hit
    // 6. Detach and cleanup

    let target = RustTarget::new();
    let result = target.spawn("echo", &["hello"]);
    // Real implementation would run actual container and debuggee
    println!("Rust full session test result: {:?}", result);
}
