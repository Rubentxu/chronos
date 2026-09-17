//! Adapter trait for trace capture backends.
//!
//! Any capture backend (ptrace/native, eBPF, mock) implements `TraceAdapter`
//! to provide a uniform interface for the query engine and MCP server.

use crate::{CaptureSession, TraceError, TraceEvent};

// ============================================================================
// ProbeBackend — query-side trait (pull, non-blocking drain)
// ============================================================================

/// A probe backend that can be queried for trace events.
///
/// This is the "LLM side" of the adapter: once probes are installed,
/// the LLM calls `drain_events()` to pull buffered events. This is
/// non-blocking and returns immediately with whatever is available.
///
/// Implementations:
/// - `EbpfAdapter` in `chronos-ebpf`: eBPF ring-buffer polling
/// - `MockEbpfAdapter` in `chronos-ebpf`: test/mock
pub trait ProbeBackend: Send {
    /// Returns `true` if this backend is available in the current environment.
    ///
    /// For eBPF: checks kernel >= 5.8 and CAP_BPF.
    /// For native: always true on Linux.
    fn is_available(&self) -> bool;

    /// Human-readable name of this backend.
    fn name(&self) -> &str;

    /// Stop the probe and release all resources.
    ///
    /// This is **blocking**: it signals the probe thread to stop and waits
    /// (bounded by an implementation-defined timeout) for the capture thread
    /// to exit before returning. This guarantees that a subsequent read of the
    /// session's `ExecutionLog` observes every event the probe emitted (no
    /// drain/stop race: MS-RACE-FIX, ADR-0005).
    ///
    /// REC-C2.2.4: `drain_events` and `drain_raw_events` were REMOVED from this
    /// trait. Both were destructive reads of an adapter-owned buffer, so they
    /// could never be the authoritative source: the first consumer to call them
    /// emptied the buffer, and a bounded buffer silently capped the answer.
    /// Consumers read the session's `ExecutionLog` instead.
    fn stop_probe(&self, session: &CaptureSession) -> Result<(), TraceError>;
}

// ============================================================================
// TraceAdapter — kept as alias for backward compatibility
// The `ProbeBackend` trait above is the canonical replacement.
// ============================================================================

/// Alias for `ProbeBackend` — maintained so existing code doesn't break.
///
/// In the new architecture, only the "probe/query" side adapters
/// (eBPF) use this. The language adapters use `chronos_capture::TraceAdapter`.
#[deprecated(since = "0.2.0", note = "Use `ProbeBackend` instead")]
pub trait TraceAdapter: Send {
    /// Returns `true` if this adapter is available in the current environment.
    fn is_available(&self) -> bool;

    /// Human-readable name of this adapter.
    fn name(&self) -> &str;

    /// Drain all buffered events that arrived since the last call.
    fn drain_events(&mut self) -> Result<Vec<TraceEvent>, TraceError>;
}
