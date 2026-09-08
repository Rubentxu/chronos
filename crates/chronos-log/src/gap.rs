//! `Gap` — explicit gap descriptor recorded into the log when the
//! producer could not capture some evidence.
//!
//! A missing event and a negative fact are not equivalent. When the
//! producer knows evidence was lost, it records a `Gap` so consumers
//! see the discontinuity instead of guessing.

use crate::seq::EventSeq;
use serde::{Deserialize, Serialize};

/// A range of sequence numbers that the producer could not capture.
///
/// `first_missing <= last_missing`. The next successful `append`
/// after recording a gap MUST return a seq greater than
/// `gap.last_missing` (so the gap is observable in the seq space).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gap {
    pub first_missing: EventSeq,
    pub last_missing: EventSeq,
    pub reason: GapReason,
    /// Free-form identifier of the producer (e.g. "ebpf-rb",
    /// "ptrace-syscall", "session-attach"). Useful for diagnostics.
    pub source: String,
}

impl Gap {
    pub fn new(
        first_missing: EventSeq,
        last_missing: EventSeq,
        reason: GapReason,
        source: impl Into<String>,
    ) -> Self {
        Self {
            first_missing,
            last_missing,
            reason,
            source: source.into(),
        }
    }

    /// Returns true if `seq` falls inside the gap range.
    pub fn covers(&self, seq: EventSeq) -> bool {
        seq >= self.first_missing && seq <= self.last_missing
    }
}

/// Why evidence was lost. Mirrors the canonical reasons from the
/// ExecutionLog spec (`docs/.../EXECUTION_LOG.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GapReason {
    /// Kernel ring buffer was full when the producer tried to push.
    KernelRingOverflow,
    /// Adapter-side buffer was full (e.g. userspace channel).
    AdapterBufferOverflow,
    /// Process detached before its tail was drained.
    ProcessDetached,
    /// Transport-level failure (pipe closed, MCP disconnect, etc.).
    TransportFailure,
    /// A persisted segment failed its checksum / parse on reload.
    CorruptSegment,
    /// Evidence type the producer cannot represent; logged but not
    /// delivered to consumers.
    UnsupportedEvidence,
}

impl std::fmt::Display for GapReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            GapReason::KernelRingOverflow => "kernel_ring_overflow",
            GapReason::AdapterBufferOverflow => "adapter_buffer_overflow",
            GapReason::ProcessDetached => "process_detached",
            GapReason::TransportFailure => "transport_failure",
            GapReason::CorruptSegment => "corrupt_segment",
            GapReason::UnsupportedEvidence => "unsupported_evidence",
        };
        f.write_str(s)
    }
}
