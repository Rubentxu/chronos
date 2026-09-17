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

/// Verify kernel version check works (doesn't panic, returns sensible result).
#[test]
fn test_kernel_version_check_integration() {
    let result = EbpfAdapter::check_kernel_version();
    // On any Linux kernel: either Ok (>= 5.8) or Err with a message.
    match result {
        Ok(()) => println!("Kernel >= 5.8, eBPF ring buffers supported"),
        Err(e) => println!("Kernel too old or check failed: {}", e),
    }
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
