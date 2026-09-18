//! REC-C3.3.2.5 — physical-storage-maintenance port for execution logs.
//!
//! The maintenance port handles the *physical* side of evidence
//! retention: flushing pending writes, reclaiming segments whose
//! evidence is no longer logically reachable, and reporting the
//! resulting counters.
//!
//! ## Deliberately missing: `compact_up_to(seq)`
//!
//! The old `compact_up_to(seq)` API let the application layer pick
//! the cutoff. That crossed a wire: maintenance has no opinion on
//! which evidence is *logically* reachable; only the
//! [`ExecutionLogRetention`](super::execution_log_retention::ExecutionLogRetention)
//! port does. The new port exposes only `compact_retired()` which
//! reclaims what `retained_from` already declares out of scope.
//!
//! Connascence rationale: `retain_up_to(X)` + `compact_up_to(X)`
//! pulled the same sequence number through two independent code
//! paths. The split removes that — callers first move the logical
//! boundary, then ask maintenance to reclaim what is already
//! logically gone.

use std::fmt;

use crate::seq::EventSeq;

/// Counter snapshot reported by the maintenance port.
///
/// Mirror of `chronos_log::CompactionMetrics` translated into the
/// domain. The maintenance port does NOT expose every concrete
/// counter the segmented backend tracks (e.g. segment rollover
/// times); it exposes the smallest surface the application layer
/// reasons about.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompactionMetrics {
    /// Number of segments reclaimed since the log was opened.
    pub segments_reclaimed: u64,
    /// Number of compaction passes that returned at least one segment.
    pub compaction_passes: u64,
    /// Number of compaction passes that reclaimed zero segments.
    pub no_op_passes: u64,
    /// Highest seq ever observed on this log (regardless of current
    /// `retained_from`).
    pub highest_seq_observed: Option<EventSeq>,
}

impl fmt::Display for CompactionMetrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "segments_reclaimed={} passes={} no_op_passes={} highest_seq={:?}",
            self.segments_reclaimed,
            self.compaction_passes,
            self.no_op_passes,
            self.highest_seq_observed
        )
    }
}

/// Outcome of one `compact_retired` pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionReport {
    /// Counter snapshot AFTER the pass. Mirrors a `CompactionMetrics`
    /// delta-merge: the returned struct is the new full state, not a
    /// per-call delta.
    pub metrics: CompactionMetrics,
    /// Physical paths reclaimed by this pass. Empty for the in-memory
    /// adapter.
    pub reclaimed_paths: Vec<String>,
}

impl fmt::Display for CompactionReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "compact_retired: {} (reclaimed {} paths)",
            self.metrics,
            self.reclaimed_paths.len()
        )
    }
}

/// Application-shape error for the maintenance port.
///
/// Distinct from `ExecutionLogError` (the evidence port) because the
/// two ports fail for different reasons. Maintenance can fail at the
/// I/O layer without implying canonical evidence is corrupt.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExecutionLogMaintenanceError {
    /// Underlying storage rejected the request (file system full,
    /// permission denied, segment rename failed, …).
    #[error("execution log maintenance unavailable: {detail}")]
    Unavailable { detail: String },

    /// The maintenance port cannot run because the evidence port is
    /// in an inconsistent state (the retention frontier is past the
    /// highest allocated seq, the seal is open mid-flush, …).
    #[error("execution log maintenance inconsistent state: {detail}")]
    Inconsistent { detail: String },
}

/// Physical-storage-maintenance port for an execution log bound to a
/// single session.
///
/// The port deliberately exposes only three operations:
///
/// 1. [`flush`](Self::flush) — make pending writes durable.
/// 2. [`compact_retired`](Self::compact_retired) — reclaim only
///    records that the retention port has already declared
///    out-of-scope. There is no parameter; the cutoff is owned by the
///    retention port.
/// 3. [`metrics`](Self::metrics) — counter snapshot.
///
/// Anything that needed a `seq` parameter on the old API moved to
/// [`ExecutionLogRetention`](super::execution_log_retention::ExecutionLogRetention).
pub trait ExecutionLogMaintenance: Send + Sync {
    /// Make pending writes durable. Returns once the backend is in a
    /// consistent state on stable storage.
    fn flush(&self) -> Result<(), ExecutionLogMaintenanceError>;

    /// Reclaim physical storage for records strictly before the
    /// current retention frontier. The frontier is read from the
    /// sibling retention port; this method has no `seq` parameter on
    /// purpose.
    fn compact_retired(&self) -> Result<CompactionReport, ExecutionLogMaintenanceError>;

    /// Counter snapshot. Cheap; safe to call on the hot path.
    fn metrics(&self) -> CompactionMetrics;
}
