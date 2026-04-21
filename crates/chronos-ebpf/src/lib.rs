//! eBPF-based tracing adapter for Chronos.
//!
//! This crate provides a `chronos-ebpf` adapter that uses Linux eBPF uprobes
//! and ring buffers to capture function entry/exit events with near-zero overhead.
//!
//! # Feature flag
//!
//! eBPF support is compiled in only when the `ebpf` feature is enabled:
//!
//! ```toml
//! chronos-ebpf = { path = "...", features = ["ebpf"] }
//! ```
//!
//! Without the feature flag, all types are present but [`EbpfAdapter::is_available`]
//! returns `false` and all operations return [`EbpfError::Unavailable`].

pub mod ring_buffer;
pub mod types;
pub mod uprobe;

use crate::ring_buffer::MockRingBuffer;
use chronos_domain::{TraceAdapter, TraceError, TraceEvent};
use thiserror::Error;

#[cfg(feature = "ebpf")]
mod ebpf_impl;

/// Errors produced by the eBPF adapter.
#[derive(Debug, Error)]
pub enum EbpfError {
    /// eBPF feature was not compiled in, or the kernel is too old (< 5.8).
    #[error("eBPF is unavailable: {reason}")]
    Unavailable { reason: String },

    /// Failed to load an eBPF program or map.
    #[error("eBPF load error: {0}")]
    LoadError(String),

    /// Insufficient capabilities (CAP_BPF / CAP_PERFMON required).
    #[error("insufficient capabilities for eBPF: {0}")]
    PermissionDenied(String),

    /// Ring buffer poll error.
    #[error("ring buffer error: {0}")]
    RingBuffer(String),

    /// Uprobe attachment error.
    #[error("uprobe error: {0}")]
    Uprobe(String),
}

/// Minimum kernel version required for BPF ring buffers (5.8.0).
pub const MIN_KERNEL_VERSION: (u32, u32, u32) = (5, 8, 0);

/// Central entry point for the eBPF tracing adapter.
///
/// When compiled without the `ebpf` feature, all operations return
/// [`EbpfError::Unavailable`] gracefully.
#[derive(Debug)]
pub struct EbpfAdapter {
    #[cfg(feature = "ebpf")]
    inner: ebpf_impl::EbpfAdapterInner,
    #[cfg(not(feature = "ebpf"))]
    _phantom: std::marker::PhantomData<()>,
}

impl EbpfAdapter {
    /// Create a new adapter.
    ///
    /// Returns `Err(EbpfError::Unavailable)` if:
    /// - The `ebpf` feature is not compiled in.
    /// - The running kernel is older than 5.8.
    /// - The process lacks `CAP_BPF`.
    pub fn new() -> Result<Self, EbpfError> {
        #[cfg(feature = "ebpf")]
        {
            ebpf_impl::EbpfAdapterInner::new().map(|inner| Self { inner })
        }
        #[cfg(not(feature = "ebpf"))]
        {
            Err(EbpfError::Unavailable {
                reason: "chronos-ebpf compiled without `ebpf` feature".to_string(),
            })
        }
    }

    /// Returns `true` if eBPF is compiled in AND the runtime checks pass.
    ///
    /// This is always `false` without the `ebpf` feature flag.
    pub fn is_available() -> bool {
        #[cfg(feature = "ebpf")]
        {
            ebpf_impl::EbpfAdapterInner::check_availability().is_ok()
        }
        #[cfg(not(feature = "ebpf"))]
        {
            false
        }
    }

    /// Check the running kernel version against [`MIN_KERNEL_VERSION`].
    ///
    /// Returns `Ok(())` if the kernel is >= 5.8, `Err` otherwise.
    pub fn check_kernel_version() -> Result<(), EbpfError> {
        kernel_version_check()
    }
}

impl TraceAdapter for EbpfAdapter {
    fn is_available(&self) -> bool {
        EbpfAdapter::is_available()
    }

    fn name(&self) -> &str {
        "ebpf"
    }

    fn drain_events(&mut self) -> Result<Vec<TraceEvent>, TraceError> {
        #[cfg(feature = "ebpf")]
        {
            Ok(self.inner.drain_events())
        }
        #[cfg(not(feature = "ebpf"))]
        {
            Ok(vec![])
        }
    }
}

/// A mock implementation of `TraceAdapter` backed by `MockRingBuffer`.
///
/// Always available (no `ebpf` feature required). Useful for tests and
/// environments where eBPF is not available.
pub struct MockEbpfAdapter {
    buffer: MockRingBuffer,
}

impl MockEbpfAdapter {
    /// Create a mock adapter with pre-loaded events.
    pub fn new(events: Vec<crate::types::EbpfEvent>) -> Self {
        Self {
            buffer: MockRingBuffer::new(events),
        }
    }

    /// Create an empty mock adapter.
    pub fn empty() -> Self {
        Self::new(vec![])
    }
}

impl TraceAdapter for MockEbpfAdapter {
    fn is_available(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        "ebpf-mock"
    }

    fn drain_events(&mut self) -> Result<Vec<TraceEvent>, TraceError> {
        Ok(self.buffer.drain_all())
    }
}

/// Parse `/proc/version` and compare against [`MIN_KERNEL_VERSION`].
pub(crate) fn kernel_version_check() -> Result<(), EbpfError> {
    let proc_version = std::fs::read_to_string("/proc/version").unwrap_or_default();

    // Extract the kernel version string (e.g. "5.15.0-91-generic")
    let version_str = proc_version
        .split_whitespace()
        .nth(2)
        .unwrap_or("0.0.0");

    let parts: Vec<u32> = version_str
        .split('.')
        .take(3)
        .map(|s| s.parse::<u32>().unwrap_or(0))
        .collect();

    let major = parts.first().copied().unwrap_or(0);
    let minor = parts.get(1).copied().unwrap_or(0);
    let patch = parts.get(2).copied().unwrap_or(0);

    let (min_maj, min_min, _min_patch) = MIN_KERNEL_VERSION;

    if major > min_maj || (major == min_maj && minor >= min_min) {
        Ok(())
    } else {
        Err(EbpfError::Unavailable {
            reason: format!(
                "kernel {}.{}.{} < required {}.{}.{}",
                major,
                minor,
                patch,
                MIN_KERNEL_VERSION.0,
                MIN_KERNEL_VERSION.1,
                MIN_KERNEL_VERSION.2,
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ebpf_adapter_not_available_without_feature() {
        // Without the `ebpf` feature, is_available() must return false.
        #[cfg(not(feature = "ebpf"))]
        assert!(!EbpfAdapter::is_available());

        // And new() must return Unavailable error.
        #[cfg(not(feature = "ebpf"))]
        {
            let err = EbpfAdapter::new().unwrap_err();
            assert!(matches!(err, EbpfError::Unavailable { .. }));
        }
    }

    #[test]
    fn test_kernel_version_check_parses_proc_version() {
        // This test runs on any kernel. It just verifies the parsing doesn't panic.
        let _ = EbpfAdapter::check_kernel_version();
    }

    #[test]
    fn test_min_kernel_version_constant() {
        let (maj, min, patch) = MIN_KERNEL_VERSION;
        assert_eq!(maj, 5);
        assert_eq!(min, 8);
        assert_eq!(patch, 0);
    }
}

#[cfg(test)]
mod adapter_tests {
    use super::*;
    use crate::types::EbpfEvent;
    use chronos_domain::{EventType, TraceAdapter};

    #[test]
    fn test_mock_ebpf_adapter_name() {
        let adapter = MockEbpfAdapter::empty();
        assert_eq!(adapter.name(), "ebpf-mock");
    }

    #[test]
    fn test_mock_ebpf_adapter_is_available() {
        let adapter = MockEbpfAdapter::empty();
        assert!(adapter.is_available());
    }

    #[test]
    fn test_mock_ebpf_adapter_drain_empty() {
        let mut adapter = MockEbpfAdapter::empty();
        let events = adapter.drain_events().unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_mock_ebpf_adapter_drain_events() {
        let raw_events = vec![
            EbpfEvent::function_entry(100, 1, 0x1000, "alpha"),
            EbpfEvent::function_entry(200, 2, 0x2000, "beta"),
            EbpfEvent::function_exit(300, 1, 0x1000),
        ];
        let mut adapter = MockEbpfAdapter::new(raw_events);

        let events = adapter.drain_events().unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].event_type, EventType::FunctionEntry);
        assert_eq!(events[2].event_type, EventType::FunctionExit);
        assert_eq!(events[0].timestamp_ns, 100);
        assert_eq!(events[1].timestamp_ns, 200);
    }

    #[test]
    fn test_mock_ebpf_adapter_drain_twice_returns_empty() {
        let raw_events = vec![EbpfEvent::function_entry(1, 1, 0x1, "f")];
        let mut adapter = MockEbpfAdapter::new(raw_events);

        let first = adapter.drain_events().unwrap();
        assert_eq!(first.len(), 1);

        let second = adapter.drain_events().unwrap();
        assert!(second.is_empty());
    }

    #[test]
    fn test_ebpf_adapter_name_via_trait_object() {
        let adapter: Box<dyn TraceAdapter> = Box::new(MockEbpfAdapter::empty());
        assert_eq!(adapter.name(), "ebpf-mock");
    }
}
