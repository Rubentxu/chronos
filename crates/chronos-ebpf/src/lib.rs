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
use chronos_capture::TraceAdapter as CaptureTraceAdapter;
use chronos_domain::semantic::{SemanticEvent, SemanticEventKind};
use chronos_domain::{
    CaptureConfig, CaptureSession, Language, ProbeBackend, TraceError,
};
#[cfg(feature = "ebpf")]
use std::sync::Mutex;
use thiserror::Error;

#[cfg(feature = "ebpf")]
mod ebpf_impl;

#[cfg(feature = "ebpf")]
pub use ebpf_impl::EbpfAdapterInner;

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
    /// The eBPF adapter inner state, protected by a mutex for interior mutability.
    /// We use a Mutex because `stop_capture` takes `&self` but `detach_all`
    /// requires mutable access to the inner state.
    #[cfg(feature = "ebpf")]
    inner: Mutex<ebpf_impl::EbpfAdapterInner>,
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
            ebpf_impl::EbpfAdapterInner::new().map(|inner| Self {
                inner: Mutex::new(inner),
            })
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

    /// Attach uprobes to the functions specified in the config for a given PID.
    #[cfg(feature = "ebpf")]
    fn attach_probes(
        &self,
        pid: i32,
        binary: &str,
        config: &CaptureConfig,
    ) -> Result<(), EbpfError> {
        let inner = self.inner.lock().map_err(|e| EbpfError::LoadError(e.to_string()))?;

        let symbols = config.function_filter.as_deref().unwrap_or(&[]);
        if symbols.is_empty() {
            // If no function filter specified, attach to common entry points
            // For now, we require explicit function names
            return Err(EbpfError::Uprobe(
                "function_filter is required for eBPF tracing".to_string(),
            ));
        }

        for symbol in symbols {
            inner.attach_uprobe(Some(pid), binary, symbol)?;
        }
        Ok(())
    }

    /// Attach a uprobe to a specific symbol in a running process.
    ///
    /// Requires CAP_BPF or root. The binary must have a symbol table
    /// or debug info for the symbol to be resolved.
    pub fn attach_uprobe(&self, pid: u32, binary: &str, symbol: &str) -> Result<(), EbpfError> {
        #[cfg(feature = "ebpf")]
        {
            let inner = self.inner.lock().map_err(|e| EbpfError::LoadError(e.to_string()))?;
            inner.attach_uprobe(Some(pid as i32), binary, symbol)
        }
        #[cfg(not(feature = "ebpf"))]
        {
            let _ = (pid, binary, symbol);
            Err(EbpfError::Unavailable {
                reason: "chronos-ebpf compiled without `ebpf` feature".to_string(),
            })
        }
    }

    /// Detach all uprobes and clean up.
    #[cfg(feature = "ebpf")]
    fn detach_probes(&self) -> Result<(), EbpfError> {
        let mut inner = self.inner.lock().map_err(|e| EbpfError::LoadError(e.to_string()))?;
        inner.detach_all();
        Ok(())
    }
}

impl ProbeBackend for EbpfAdapter {
    fn is_available(&self) -> bool {
        Self::is_available()
    }

    fn name(&self) -> &str {
        "ebpf"
    }

    fn drain_events(&mut self) -> Result<Vec<SemanticEvent>, TraceError> {
        #[cfg(feature = "ebpf")]
        {
            use chronos_domain::{EventData, EventType};
            let inner = self.inner.lock().map_err(|e| TraceError::CaptureFailed(e.to_string()))?;
            let raw_events = inner.drain_events();
            Ok(raw_events.into_iter().map(|e| {
                let fn_name = match &e.data {
                    EventData::EbpfUprobeHit { symbol_name, .. } => symbol_name.clone(),
                    _ => e.location.function.clone(),
                };
                let kind = match e.event_type {
                    EventType::FunctionEntry => SemanticEventKind::FunctionCalled {
                        function: fn_name.clone(),
                        module: None,
                        arguments: vec![],
                    },
                    EventType::FunctionExit => SemanticEventKind::FunctionReturned {
                        function: fn_name.clone(),
                        return_value: None,
                    },
                    _ => SemanticEventKind::Unresolved,
                };
                SemanticEvent {
                    source_event_id: e.event_id,
                    timestamp_ns: e.timestamp_ns,
                    thread_id: e.thread_id,
                    language: Language::Ebpf,
                    kind,
                    description: format!("{:?} @ {}", e.event_type, fn_name),
                }
            }).collect())
        }
        #[cfg(not(feature = "ebpf"))]
        {
            Err(TraceError::capture_failed("eBPF support not compiled in"))
        }
    }
}

impl CaptureTraceAdapter for EbpfAdapter {
    /// Start capturing a new process.
    ///
    /// Forks and execs the target binary, then attaches eBPF uprobes to
    /// the functions specified in `config.function_filter`.
    fn start_capture(&self, _config: CaptureConfig) -> Result<CaptureSession, TraceError> {
        #[cfg(feature = "ebpf")]
        {
            use std::process::{Command, Stdio};

            // Check that the target binary exists
            let target_path = std::path::Path::new(&config.target);
            if !target_path.exists() {
                return Err(TraceError::CaptureFailed(format!(
                    "Target binary not found: {}",
                    config.target
                )));
            }

            // Fork and exec the target
            let mut child = Command::new(&config.target)
                .args(&config.args)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| TraceError::CaptureFailed(format!("Failed to spawn {}: {}", config.target, e)))?;

            let pid = child.id();

            // Attach uprobes to the specified functions
            self.attach_probes(pid as i32, &config.target, &config)
                .map_err(|e| TraceError::capture_failed(e))?;

            let session = CaptureSession::new(pid, Language::Ebpf, config);
            Ok(session)
        }
        #[cfg(not(feature = "ebpf"))]
        {
            Err(TraceError::capture_failed(
                "eBPF support not compiled in",
            ))
        }
    }

    /// Stop an active capture session.
    ///
    /// Detaches eBPF uprobes and terminates the traced process.
    fn stop_capture(&self, _session: &CaptureSession) -> Result<(), TraceError> {
        #[cfg(feature = "ebpf")]
        {
            // Detach all uprobes
            self.detach_probes().map_err(|e| TraceError::capture_failed(e))?;

            // Try to terminate the process
            let pid = _session.pid as i32;
            if pid > 0 {
                let _ = std::process::Command::new("kill")
                    .arg("-9")
                    .arg(pid.to_string())
                    .output();
            }
            Ok(())
        }
        #[cfg(not(feature = "ebpf"))]
        {
            Err(TraceError::capture_failed(
                "eBPF support not compiled in",
            ))
        }
    }

    /// Attach to an already running process.
    ///
    /// Attaches eBPF uprobes to the functions specified in `config.function_filter`
    /// on the given process.
    fn attach_to_process(
        &self,
        _pid: u32,
        _config: CaptureConfig,
    ) -> Result<CaptureSession, TraceError> {
        #[cfg(feature = "ebpf")]
        {
            // Attach uprobes to the specified functions
            self.attach_probes(_pid as i32, &_config.target, &_config)
                .map_err(|e| TraceError::capture_failed(e))?;

            let session = CaptureSession::new(_pid, Language::Ebpf, _config);
            Ok(session)
        }
        #[cfg(not(feature = "ebpf"))]
        {
            Err(TraceError::capture_failed(
                "eBPF support not compiled in",
            ))
        }
    }

    /// Get the language this adapter supports.
    fn get_language(&self) -> Language {
        Language::Ebpf
    }

    /// Get a human-readable name for this adapter.
    fn name(&self) -> &str {
        "ebpf"
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

impl ProbeBackend for MockEbpfAdapter {
    fn is_available(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        "ebpf-mock"
    }

    fn drain_events(&mut self) -> Result<Vec<SemanticEvent>, TraceError> {
        use chronos_domain::{EventData, EventType};
        let raw_events = self.buffer.drain_all();
        Ok(raw_events.into_iter().map(|e| {
            let fn_name = match &e.data {
                EventData::EbpfUprobeHit { symbol_name, .. } => symbol_name.clone(),
                _ => e.location.function.clone().unwrap_or_default(),
            };
            let kind = match e.event_type {
                EventType::FunctionEntry => SemanticEventKind::FunctionCalled {
                    function: fn_name.clone(),
                    module: None,
                    arguments: vec![],
                },
                EventType::FunctionExit => SemanticEventKind::FunctionReturned {
                    function: fn_name.clone(),
                    return_value: None,
                },
                _ => SemanticEventKind::Unresolved,
            };
            SemanticEvent {
                source_event_id: e.event_id,
                timestamp_ns: e.timestamp_ns,
                thread_id: e.thread_id,
                language: Language::Ebpf,
                kind,
                description: format!("{:?} @ {}", e.event_type, fn_name),
            }
        }).collect())
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
    use chronos_domain::ProbeBackend;
    use chronos_domain::semantic::SemanticEventKind;

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
        // Events are now properly mapped to semantic kinds
        assert!(matches!(&events[0].kind, SemanticEventKind::FunctionCalled { function, .. } if function == "alpha"));
        assert!(matches!(&events[1].kind, SemanticEventKind::FunctionCalled { function, .. } if function == "beta"));
        assert!(matches!(&events[2].kind, SemanticEventKind::FunctionReturned { .. }));
        // Check metadata via source_event_id and timestamp_ns
        assert_eq!(events[0].source_event_id, 0);
        assert_eq!(events[1].source_event_id, 1);
        assert_eq!(events[2].source_event_id, 2);
        assert_eq!(events[0].timestamp_ns, 100);
        assert_eq!(events[1].timestamp_ns, 200);
        assert_eq!(events[2].timestamp_ns, 300);
        // Check description contains event type info
        assert!(events[0].description.contains("FunctionEntry"));
        assert!(events[2].description.contains("FunctionExit"));
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
        let adapter: Box<dyn ProbeBackend> = Box::new(MockEbpfAdapter::empty());
        assert_eq!(adapter.name(), "ebpf-mock");
    }
}
