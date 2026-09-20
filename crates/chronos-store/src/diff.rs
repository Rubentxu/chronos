//! Deprecated: zero-size `TraceDiff` wrapper kept for callers that
//! import the static compare function from `chronos_store::diff::TraceDiff`.
//!
//! REC-C3.5-residual-inversion R.2 (2026-09-20): the algorithm moved
//! to `chronos_store::diff_engine_adapter::Blake3DiffEngine` and the
//! [`DiffEngine`] port in `chronos_domain::ports::diff`. The
//! `TraceDiff::compare(...)` function here is a thin delegator kept as
//! a transitional seam so internal tests + the bench don't have to
//! import the new path during the R.5 verification wave. New callers
//! must consume the port through the composition root.

use crate::diff_engine_adapter::Blake3DiffEngine;
use crate::storage::SessionMetadata;
pub use chronos_domain::ports::diff::{DiffEngine, DiffReport, TimingDelta};

/// Deprecated zero-size struct preserved for tests + bench. Use the
/// [`DiffEngine`] port and [`Blake3DiffEngine`] adapter for production
/// code paths.
#[deprecated(
    since = "0.1.1",
    note = "use chronos_domain::ports::diff::DiffEngine + chronos_store::diff_engine_adapter::Blake3DiffEngine"
)]
pub struct TraceDiff;

#[allow(deprecated)]
impl TraceDiff {
    /// Compare two sets of events using BLAKE3 hash-based symmetric difference.
    ///
    /// This is a transitional delegator to
    /// `Blake3DiffEngine::compare(...)`. New callers should consume the
    /// [`DiffEngine`] port directly.
    pub fn compare(
        session_a_id: &str,
        session_b_id: &str,
        events_a: &[chronos_domain::TraceEvent],
        events_b: &[chronos_domain::TraceEvent],
        meta_a: &SessionMetadata,
        meta_b: &SessionMetadata,
    ) -> DiffReport {
        Blake3DiffEngine.compare(
            session_a_id,
            session_b_id,
            events_a,
            events_b,
            meta_a,
            meta_b,
        )
    }
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;
    use chronos_domain::{EventData, EventType, SourceLocation};

    fn make_event(id: u64, func: &str) -> chronos_domain::TraceEvent {
        chronos_domain::TraceEvent::new(
            id,
            chronos_domain::MonotonicNs::from(id * 100),
            1,
            EventType::FunctionEntry,
            SourceLocation::new("test.rs", 10, func, 0x1000 + id),
            EventData::Function {
                name: func.to_string(),
                signature: None,
                symbol_id: None,
                invocation_id: None,
                parent_invocation_id: None,
            },
        )
    }

    fn make_meta(id: &str, dur_ms: u64) -> SessionMetadata {
        SessionMetadata {
            session_id: id.to_string(),
            created_at: 1000,
            language: "native".to_string(),
            target: "/bin/test".to_string(),
            event_count: 0,
            duration_ms: dur_ms,
            tail_sealed: false,
            sealed_at: None,
        }
    }

    #[test]
    fn test_diff_identical_sessions() {
        let events = vec![make_event(1, "main"), make_event(2, "helper")];
        let meta = make_meta("a", 500);

        let report = TraceDiff::compare("a", "b", &events, &events, &meta, &meta);
        assert_eq!(report.common_count, 2);
        assert!(report.only_in_a.is_empty());
        assert!(report.only_in_b.is_empty());
        assert!((report.similarity_pct - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_diff_completely_different() {
        let events_a = vec![make_event(1, "main")];
        let events_b = vec![make_event(2, "helper")];
        let meta_a = make_meta("a", 500);
        let meta_b = make_meta("b", 600);

        let report = TraceDiff::compare("a", "b", &events_a, &events_b, &meta_a, &meta_b);
        assert_eq!(report.common_count, 0);
        assert_eq!(report.only_in_a.len(), 1);
        assert_eq!(report.only_in_b.len(), 1);
        assert!(report.similarity_pct < 0.01);
    }

    #[test]
    fn test_diff_partial_overlap() {
        let events_a = vec![make_event(1, "main"), make_event(2, "helper")];
        let events_b = vec![make_event(1, "main"), make_event(3, "other")];
        let meta_a = make_meta("a", 500);
        let meta_b = make_meta("b", 600);

        let report = TraceDiff::compare("a", "b", &events_a, &events_b, &meta_a, &meta_b);
        assert_eq!(report.common_count, 1);
        assert_eq!(report.only_in_a.len(), 1);
        assert_eq!(report.only_in_b.len(), 1);
        let total_unique = report.only_in_a.len() + report.only_in_b.len() + report.common_count;
        let expected_similarity = (report.common_count as f64 / total_unique as f64) * 100.0;
        assert!((report.similarity_pct - expected_similarity).abs() < 0.01);
    }

    #[test]
    fn test_diff_empty_events() {
        let events: Vec<chronos_domain::TraceEvent> = vec![];
        let meta_a = make_meta("a", 500);
        let meta_b = make_meta("b", 600);

        let report = TraceDiff::compare("a", "b", &events, &events, &meta_a, &meta_b);
        assert_eq!(report.common_count, 0);
        assert!(report.only_in_a.is_empty());
        assert!(report.only_in_b.is_empty());
        assert!((report.similarity_pct - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_diff_timing_delta() {
        let events = vec![make_event(1, "main")];
        let meta_a = make_meta("a", 500);
        let meta_b = make_meta("b", 800);

        let report = TraceDiff::compare("a", "b", &events, &events, &meta_a, &meta_b);
        let timing = report.timing_delta.expect("timing delta should be present");
        assert_eq!(timing.duration_ms_a, 500);
        assert_eq!(timing.duration_ms_b, 800);
        assert_eq!(timing.delta_ms, 300);
        assert_eq!(timing.slower_session, Some("b".to_string()));
    }
}
