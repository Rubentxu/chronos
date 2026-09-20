//! `DiffEngine` port (REC-C3.5-residual-inversion R.2).
//!
//! Compares two saved sessions and produces a [`DiffReport`] describing
//! the symmetric difference of their event sets, similarity percentage
//! and (when both sessions have a duration) the timing delta.
//!
//! This port replaces the previous direct dependency on
//! `chronos_store::TraceDiff::compare` from
//! `services::ChronosDiffService::compare_sessions` (REC-C3-hexagonal-closure
//! left this as a residual edge). The composition root
//! (`chronos_mcp::composition::default_diff_engine`) supplies the
//! concrete `Blake3DiffEngine` adapter.
//!
//! ## Why a port
//!
//! The diff function is **already pure** (BLAKE3 hashing of `TraceEvent`
//! bytes, then a set symmetric-difference), so the port is the obvious
//! hexagonal refactor:
//!
//! - `chronos-domain` declares the contract.
//! - `chronos-store` provides the production adapter
//!   (`Blake3DiffEngine`).
//! - `chronos-services` consumes the port through `DiffContext::engine`.
//!
//! Audit §3.2 A1 (ISP) is preserved: the contract is narrow (one
//! method, six parameters). Audit §6.2 (composition only at driving
//! adapter) is satisfied because services no longer imports
//! `chronos_store::diff` for the compare function — only the
//! composition root does, and it does so to build the port impl.

use serde::{Deserialize, Serialize};

use crate::session::SessionMetadata;
use crate::trace::TraceEvent;

/// A comparison report between two sessions.
///
/// Output type for [`DiffEngine::compare`]. Lives in `chronos-domain`
/// because it is the contract return shape; the concrete impl lives in
/// `chronos-store::diff_engine_adapter::Blake3DiffEngine`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffReport {
    /// ID of the first session.
    pub session_a_id: String,
    /// ID of the second session.
    pub session_b_id: String,
    /// Events only in session A (by hash).
    pub only_in_a: Vec<TraceEvent>,
    /// Events only in session B (by hash).
    pub only_in_b: Vec<TraceEvent>,
    /// Number of common events.
    pub common_count: usize,
    /// Similarity percentage 0.0..100.0.
    pub similarity_pct: f64,
    /// Timing delta between sessions, if computable.
    pub timing_delta: Option<TimingDelta>,
}

/// Timing comparison between two sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingDelta {
    /// Duration of session A in milliseconds.
    pub duration_ms_a: u64,
    /// Duration of session B in milliseconds.
    pub duration_ms_b: u64,
    /// Difference in ms (b - a).
    pub delta_ms: i64,
    /// Which session was slower, if any.
    pub slower_session: Option<String>,
}

/// Compare two saved sessions and produce a [`DiffReport`].
///
/// Narrow port: one method, six parameters, synchronous. Audit §3.2 A1
/// (ISP) is preserved. Implementations should be pure (no I/O, no
/// logging) — the symmetry-difference is a deterministic function of
/// the input events + metadata.
pub trait DiffEngine: Send + Sync {
    /// Compare two sets of events using BLAKE3 hash-based symmetric
    /// difference. Events present in both = common. Events in only one
    /// = difference.
    fn compare(
        &self,
        session_a_id: &str,
        session_b_id: &str,
        events_a: &[TraceEvent],
        events_b: &[TraceEvent],
        meta_a: &SessionMetadata,
        meta_b: &SessionMetadata,
    ) -> DiffReport;
}
