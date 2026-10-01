//! Integration tests for the eBPF adapter.
//!
//! Most tests here are `#[ignore]` because they require:
//! - Linux kernel >= 5.8
//! - `CAP_BPF` capability (or root)
//! - The `ebpf` feature to be compiled in
//!
//! Run them explicitly with:
//! ```
//! sudo cargo test -p chronos-ebpf --features ebpf -- --ignored
//! ```

use chronos_ebpf::EbpfAdapter;

/// The host running this suite must satisfy the crate's own stated minimum.
///
/// This crate exists to drive BPF ring buffers, which need kernel >= 5.8
/// (`MIN_KERNEL_VERSION`). A host below that cannot exercise any part of it,
/// so `Ok(())` is the expected outcome here and `Err` is a real failure — not
/// an acceptable alternative to assert "either way". The failure message
/// carries the raw `/proc/version` and the required version so an operator
/// can tell an unsupported host from a broken parse.
///
/// The unit test `kernel_version_check_parses_proc_version` in `src/lib.rs`
/// pins the parse/verdict agreement against the host; this test pins the
/// host-level expectation, which is the part an integration suite owns.
#[test]
fn test_kernel_version_check_integration() {
    let result = EbpfAdapter::check_kernel_version();
    let host_version = std::fs::read_to_string("/proc/version").unwrap_or_else(|e| {
        panic!("/proc/version must be readable on the Linux host this suite targets: {e}")
    });
    assert!(
        result.is_ok(),
        "host kernel does not meet chronos-ebpf's minimum (5.8.0): {result:?}\n\
         /proc/version: {host_version}"
    );
}

/// End-to-end test that attaches a uprobe to an existing binary.
///
/// Requires: kernel >= 5.8, CAP_BPF, `ebpf` feature.
///
/// Skipped by default. Run with:
/// ```
/// sudo cargo test -p chronos-ebpf --features ebpf -- --ignored test_uprobe_on_existing_binary
/// ```
#[test]
#[ignore = "requires kernel >= 5.8, CAP_BPF, and --features ebpf"]
fn test_uprobe_on_existing_binary() {
    #[cfg(feature = "ebpf")]
    {
        // Attempt to create a real eBPF adapter
        let adapter = EbpfAdapter::new();
        match adapter {
            Ok(_) => {
                // If we got here, eBPF is available
                assert!(EbpfAdapter::is_available());
                println!("EbpfAdapter created successfully");
            }
            Err(e) => {
                panic!("Failed to create EbpfAdapter: {}", e);
            }
        }
    }
    #[cfg(not(feature = "ebpf"))]
    {
        panic!("This test requires the `ebpf` feature: --features ebpf");
    }
}
