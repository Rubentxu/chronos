//! Tests for GoTarget (Delve-based debugging).
//!
//! These tests verify GoTarget creation, Delve port configuration,
//! and container options generation without requiring real containers.

#![cfg(feature = "go")]

use chronos_sandbox::container::quadlet::{ptrace_container_options, ContainerOptions};
use chronos_sandbox::targets::go::GoTarget;
use chronos_sandbox::targets::DebugTarget;

/// Tests that GoTarget can be created with default Delve port (2345).
#[test]
fn test_go_target_creation() {
    let target = GoTarget::new();
    assert!(!target.is_attached(), "GoTarget should not be attached on creation");
}

/// Tests that ContainerOptions expose the Delve port (2345).
#[test]
fn test_go_container_options() {
    let opts: ContainerOptions = ptrace_container_options("go-debug:latest", "test-go", 2345);

    assert_eq!(opts.name, "test-go");
    assert_eq!(opts.image, "go-debug:latest");
    assert!(
        opts.ports.iter().any(|p| p.host == 2345 && p.container == 2345),
        "Delve port 2345 should be exposed"
    );
}

/// Full session test that requires real Podman and a Go container.
/// This test is ignored by default and only runs with `cargo test -- --ignored`.
#[test]
#[ignore]
// requires: podman
fn test_go_full_session() {
    // This test would:
    // 1. Start a Go container with Delve
    // 2. Launch a Go program via dlv debug
    // 3. Attach via Delve RPC
    // 4. Set a breakpoint
    // 5. Verify breakpoint hit
    // 6. Detach and cleanup

    let target = GoTarget::new();
    let result = target.spawn("dlv", &["debug", "--", "main.go"]);
    println!("Go full session test result: {:?}", result);
}
