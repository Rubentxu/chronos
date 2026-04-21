//! Tests for NodejsTarget (CDP/V8 inspector-based debugging).
//!
//! These tests verify NodejsTarget creation, CDP port configuration,
//! and container options generation without requiring real containers.

#![cfg(feature = "nodejs")]

use chronos_sandbox::container::quadlet::{ptrace_container_options, ContainerOptions};
use chronos_sandbox::targets::nodejs::NodeJsTarget;
use chronos_sandbox::targets::DebugTarget;

/// Tests that NodeJsTarget can be created with default CDP port (9229).
#[test]
fn test_nodejs_target_creation() {
    let target = NodeJsTarget::new();
    assert!(!target.is_attached(), "NodeJsTarget should not be attached on creation");
}

/// Tests that ContainerOptions expose the CDP port (9229).
#[test]
fn test_nodejs_container_options() {
    let opts: ContainerOptions = ptrace_container_options("nodejs-debug:latest", "test-nodejs", 9229);

    assert_eq!(opts.name, "test-nodejs");
    assert_eq!(opts.image, "nodejs-debug:latest");
    assert!(
        opts.ports.iter().any(|p| p.host == 9229 && p.container == 9229),
        "CDP port 9229 should be exposed"
    );
}

/// Full session test that requires real Podman and a Node.js container.
/// This test is ignored by default and only runs with `cargo test -- --ignored`.
#[test]
#[ignore]
// requires: podman
fn test_nodejs_full_session() {
    // This test would:
    // 1. Start a Node.js container with --inspect flag
    // 2. Launch a Node.js program with --inspect=9229
    // 3. Attach via Chrome DevTools Protocol
    // 4. Set a breakpoint
    // 5. Verify breakpoint hit
    // 6. Detach and cleanup

    let target = NodeJsTarget::new();
    let result = target.spawn("node", &["--inspect=9229", "app.js"]);
    println!("Node.js full session test result: {:?}", result);
}
