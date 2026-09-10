//! Native probe service — live ptrace-based probes (`probe_*` tool family).
//!
//! This module owns the long-lived state associated with a live native probe:
//! `LiveProbeSession` (carrying the `NativeProbeBackend`, the underlying
//! `CaptureSession`, and the optional eBPF adapter). The MCP-server tool
//! functions in `chronos-mcp` are thin wrappers that delegate here.
//!
//! Browser-probe sessions live in a sibling service (deferred to m5-06b).

use std::sync::Arc;

use chronos_domain::{CaptureSession, Language};
use chronos_native::probe_backend::NativeProbeBackend;

/// A live native probe session.
///
/// Unlike `debug_run` which blocks until the program exits, a live probe streams
/// events to an `EventBus` ring buffer in real-time. Events can be drained at any
/// time via `probe_drain`, and the probe is stopped via `probe_stop`.
pub struct LiveProbeSession {
    /// The native probe backend driving the ptrace loop.
    pub backend: NativeProbeBackend,
    /// The capture session returned by `start_probe`.
    pub session: CaptureSession,
    /// Language of the target program.
    pub language: Language,
    /// Path to the target binary.
    pub target: String,
    /// eBPF adapter owned by this session, if any uprobes have been injected.
    /// Stored here so the lifecycle is observable: subsequent `probe_inject`
    /// calls reuse the same adapter, and `probe_stop` detaches cleanly.
    pub ebpf_adapter: Option<Arc<chronos_ebpf::EbpfAdapter>>,
    /// Most recent eBPF attachment metadata (binary_path, symbol_name, pid).
    pub ebpf_attachment: Option<EbpfAttachmentInfo>,
}

/// Metadata for the eBPF attachment of a live probe session.
#[derive(Debug, Clone)]
pub struct EbpfAttachmentInfo {
    /// Library / binary path the uprobe was attached to.
    pub binary_path: String,
    /// Symbol the uprobe was attached to.
    pub symbol_name: String,
    /// Pid the uprobe was attached to.
    pub pid: u32,
}
