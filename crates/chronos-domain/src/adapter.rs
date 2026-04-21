//! Adapter trait for trace capture backends.
//!
//! Any capture backend (ptrace/native, eBPF, mock) implements `TraceAdapter`
//! to provide a uniform interface for the query engine and MCP server.

use crate::{TraceError, TraceEvent};

/// A backend that can capture trace events from a running process.
///
/// Implementations include:
/// - `chronos-native`: ptrace-based capture.
/// - `chronos-ebpf`: eBPF uprobe-based capture (optional).
/// - Mock/test implementations.
pub trait TraceAdapter: Send {
    /// Returns `true` if this adapter is available in the current environment.
    ///
    /// For eBPF: checks kernel >= 5.8 and CAP_BPF.
    /// For native: always true on Linux.
    fn is_available(&self) -> bool;

    /// Human-readable name of this adapter.
    fn name(&self) -> &str;

    /// Drain all buffered events that arrived since the last call.
    ///
    /// Returns an empty vec if no events are ready (non-blocking).
    fn drain_events(&mut self) -> Result<Vec<TraceEvent>, TraceError>;
}
