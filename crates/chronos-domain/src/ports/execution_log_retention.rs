//! REC-C3.3.2.5 — execution-log capability split.
//!
//! Three responsibilities, three ports:
//!
//! ```text
//! ExecutionLogProvider     canonical evidence (append, read, gap, tail, seal)
//! ExecutionLogRetention    frontier of available evidence (retained_from movement)
//! ExecutionLogMaintenance  physical reclamation (flush, compact_retired, metrics)
//! ```
//!
//! ## Why split
//!
//! Before C3.3.2.5 the services-side `SessionExecutionLog` carried a
//! `ProviderKind` enum tag and downcast on every maintenance call
//! (`flush`, `compaction_metrics`, `compact_up_to`, `retain_up_to`,
//! `maybe_compact`, `record_gap_on_segmented`). The tag was a
//! closed 3-variant escape hatch; the canonical path went through the
//! port but the maintenance path did not.
//!
//! Two concrete consequences:
//!
//! - `ExecutionLogMaintenanceUnsupported` was a normal runtime error
//!   in production. Adapters that did not implement a maintenance
//!   capability surfaced a typed failure mid-call instead of being
//!   routed to a narrower port that simply wasn't satisfied.
//! - `retain_up_to` and `compact_up_to` were both callable from the
//!   application layer on the same wrapper. Their connascence — both
//!   destroyed evidence, both took a seq — let an application-layer
//!   caller delete records before the logical retention frontier
//!   moved, which split the "logical availability" contract.
//!
//! The split is deliberate:
//!
//! ```text
//! Retention decides what is logically in scope.
//! Maintenance reclaims only what is already logically out of scope.
//! ```
//!
//! There is **no** `compact_up_to(seq)` on the maintenance port:
//! maintenance has no opinion on which evidence is logically
//! available; that is the retention port's job.

use std::fmt;

use crate::seq::EventSeq;

/// Outcome of moving the logical retention boundary forward.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetentionOutcome {
    /// The new `retained_from` value after the move.
    pub new_retained_from: EventSeq,
    /// Whether the boundary actually moved (false when the move was
    /// a no-op because `new_retained_from` was already at or behind
    /// the previous boundary).
    pub boundary_moved: bool,
}

impl fmt::Display for RetentionOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "retained_from={} (boundary_moved={})",
            self.new_retained_from, self.boundary_moved
        )
    }
}

/// Error type for the retention port.
///
/// Distinct from `ExecutionLogError` because the retention port is
/// allowed to fail for reasons the evidence port never sees
/// (requesting a backward move, sealing the retention frontier, …).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RetentionError {
    /// The requested `new_retained_from` is at or behind the current
    /// boundary. Retention only moves forward.
    #[error(
        "retention boundary cannot move backwards: requested {requested}, current {current}"
    )]
    BackwardsMove {
        requested: EventSeq,
        current: EventSeq,
    },

    /// The retention frontier is sealed for this session (the
    /// evidence log was sealed).
    #[error("retention frontier for session {session_id} is sealed")]
    Sealed { session_id: String },

    /// The requested boundary is past the highest allocated seq —
    /// retention cannot move past evidence that has not been written.
    #[error(
        "retention boundary {requested} is past the highest allocated seq {highest_allocated}"
    )]
    PastAllocated {
        requested: EventSeq,
        highest_allocated: EventSeq,
    },

    /// Underlying storage rejected the request (segmented: file
    /// rename failed; in-memory: invariant violation).
    #[error("retention update failed: {detail}")]
    Unavailable { detail: String },
}

/// Logical-retention port for an execution log bound to a single session.
///
/// The retention frontier is the **earliest seq still queryable**. It
/// only moves forward. Maintenance (the other port) is allowed to
/// reclaim physical storage for records strictly before the frontier
/// — never for records at or after it.
pub trait ExecutionLogRetention: Send + Sync {
    /// Move the logical retention frontier to `new_retained_from`.
    ///
    /// - On success returns the new frontier and whether it actually
    ///   moved (idempotent re-request returns the same value with
    ///   `boundary_moved=false`).
    /// - `new_retained_from <= self.retained_from()` returns
    ///   [`RetentionError::BackwardsMove`].
    /// - `new_retained_from > self.highest_allocated()` returns
    ///   [`RetentionError::PastAllocated`].
    /// - After a successful seal, every call returns
    ///   [`RetentionError::Sealed`].
    fn advance_retained_from(
        &self,
        new_retained_from: EventSeq,
    ) -> Result<RetentionOutcome, RetentionError>;

    /// Current retention frontier (mirrors
    /// [`crate::ports::execution_log::ExecutionLogProvider::retained_from`]
    /// so the retention port is fully usable without the evidence
    /// port when a caller only needs the boundary).
    fn retained_from(&self) -> EventSeq;

    /// Highest allocated seq (record OR gap) on this session. The
    /// retention frontier cannot move past this value.
    fn highest_allocated(&self) -> Option<EventSeq>;
}
