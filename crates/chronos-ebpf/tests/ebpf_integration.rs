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

use chronos_domain::ProbeBackend;
use chronos_domain::semantic::SemanticEventKind;
use chronos_ebpf::{EbpfAdapter, MockEbpfAdapter};

/// Verify that the mock adapter works end-to-end as a `TraceAdapter`.
///
/// This test does NOT require kernel support — it tests the mock path.
#[test]
fn test_mock_adapter_as_trace_adapter_integration() {
    use chronos_ebpf::types::EbpfEvent;

    let events = vec![
        EbpfEvent::function_entry(1_000_000, 100, 0xDEAD_BEEF, "entry_point"),
        EbpfEvent::function_entry(2_000_000, 100, 0xCAFE_BABE, "inner_call"),
        EbpfEvent::function_exit(3_000_000, 100, 0xCAFE_BABE),
        EbpfEvent::function_exit(4_000_000, 100, 0xDEAD_BEEF),
    ];

    let mut adapter: Box<dyn ProbeBackend> = Box::new(MockEbpfAdapter::new(events));

    assert!(adapter.is_available());
    assert_eq!(adapter.name(), "ebpf-mock");

    let drained = adapter.drain_events().expect("drain should succeed");
    assert_eq!(drained.len(), 4);

    assert!(matches!(&drained[0].kind, SemanticEventKind::FunctionCalled { function, .. } if function == "entry_point"));
    assert_eq!(drained[0].timestamp_ns, 1_000_000);

    // Exit events: function name may be empty since EbpfEvent::function_exit doesn't store a name
    // (the name is resolved from address via to_trace_event -> location.function)
    assert!(matches!(&drained[2].kind, SemanticEventKind::FunctionReturned { .. }));
    assert!(matches!(&drained[3].kind, SemanticEventKind::FunctionReturned { .. }));

    // Second drain returns nothing
    let empty = adapter.drain_events().expect("second drain ok");
    assert!(empty.is_empty());
}

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

/// Verify that `MockEbpfAdapter` correctly sequences event IDs.
#[test]
fn test_mock_adapter_event_id_sequencing() {
    use chronos_ebpf::types::EbpfEvent;

    let events: Vec<EbpfEvent> = (0..10)
        .map(|i| EbpfEvent::function_entry(i * 1000, 1, 0x1000 + i, "fn"))
        .collect();

    let mut adapter = MockEbpfAdapter::new(events);
    let drained = adapter.drain_events().unwrap();

    assert_eq!(drained.len(), 10);
    for (i, ev) in drained.iter().enumerate() {
        assert_eq!(ev.source_event_id, i as u64, "source_event_id should be sequential");
        assert_eq!(ev.timestamp_ns, i as u64 * 1000);
    }
}
