//! Diff engine — compares two sessions' events using hash-based set difference.

use crate::storage::SessionMetadata;
use blake3::hash;
use chronos_domain::TraceEvent;
use lz4_flex::compress_prepend_size;
use serde::{Deserialize, Serialize};

/// A comparison report between two sessions.
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

/// The diff engine — compares sessions by hashing events.
///
/// Compares two sessions using hash-based set difference. Events are hashed
/// with BLAKE3 after serialization; identical events produce identical hashes,
/// enabling efficient deduplication-aware comparison.
pub struct TraceDiff;

impl TraceDiff {
    /// Compare two sets of events using BLAKE3 hash-based symmetric difference.
    ///
    /// Events are hashed and compared as sets. Events present in both = common.
    /// Events in only one = difference.
    pub fn compare(
        session_a_id: &str,
        session_b_id: &str,
        events_a: &[TraceEvent],
        events_b: &[TraceEvent],
        meta_a: &SessionMetadata,
        meta_b: &SessionMetadata,
    ) -> DiffReport {
        // Hash all events
        let hashes_a: std::collections::HashSet<String> =
            events_a.iter().map(hash_event).collect();

        let hashes_b: std::collections::HashSet<String> =
            events_b.iter().map(hash_event).collect();

        let hashes_a: Vec<String> = hashes_a.into_iter().collect();
        let hashes_b: Vec<String> = hashes_b.into_iter().collect();

        let set_a: std::collections::HashSet<_> = hashes_a.iter().collect();
        let set_b: std::collections::HashSet<_> = hashes_b.iter().collect();

        // Symmetric difference
        let only_in_a: Vec<String> = set_a.difference(&set_b).copied().cloned().collect();
        let only_in_b: Vec<String> = set_b.difference(&set_a).copied().cloned().collect();
        let common_count = set_a.intersection(&set_b).count();

        // Build hash → event maps for reconstruction
        let map_a: std::collections::HashMap<_, _> = events_a
            .iter()
            .map(|e| (hash_event(e), e.clone()))
            .collect();
        let map_b: std::collections::HashMap<_, _> = events_b
            .iter()
            .map(|e| (hash_event(e), e.clone()))
            .collect();

        let only_a_events: Vec<TraceEvent> = only_in_a
            .iter()
            .filter_map(|h| map_a.get(h).cloned())
            .collect();
        let only_b_events: Vec<TraceEvent> = only_in_b
            .iter()
            .filter_map(|h| map_b.get(h).cloned())
            .collect();

        // Compute similarity
        let total_unique = only_in_a.len() + only_in_b.len() + common_count;
        let similarity_pct = if total_unique > 0 {
            (common_count as f64 / total_unique as f64) * 100.0
        } else {
            100.0
        };

        // Timing delta
        let timing_delta = Some(TimingDelta {
            duration_ms_a: meta_a.duration_ms,
            duration_ms_b: meta_b.duration_ms,
            delta_ms: meta_b.duration_ms as i64 - meta_a.duration_ms as i64,
            slower_session: if meta_b.duration_ms > meta_a.duration_ms {
                Some(meta_b.session_id.clone())
            } else if meta_a.duration_ms > meta_b.duration_ms {
                Some(meta_a.session_id.clone())
            } else {
                None
            },
        });

        DiffReport {
            session_a_id: session_a_id.to_string(),
            session_b_id: session_b_id.to_string(),
            only_in_a: only_a_events,
            only_in_b: only_b_events,
            common_count,
            similarity_pct,
            timing_delta,
        }
    }
}

/// Hash a single event with BLAKE3 after bincode serialization + lz4 compression.
fn hash_event(event: &TraceEvent) -> String {
    let serialized = bincode::serialize(event).unwrap_or_default();
    let compressed = compress_prepend_size(&serialized);
    hash(&compressed).to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::{EventData, EventType, SourceLocation};

    fn make_event(id: u64, func: &str) -> TraceEvent {
        TraceEvent::new(
            id,
            id * 100,
            1,
            EventType::FunctionEntry,
            SourceLocation::new("test.rs", 10, func, 0x1000 + id),
            EventData::Function {
                name: func.to_string(),
                signature: None,
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
        // 1 common / 3 total = 33.33...%
        assert!((report.similarity_pct - 33.333).abs() < 0.01);
    }

    #[test]
    fn test_diff_timing_delta() {
        let events = vec![make_event(1, "main")];
        let meta_a = make_meta("a", 500);
        let meta_b = make_meta("b", 800);

        let report = TraceDiff::compare("a", "b", &events, &events, &meta_a, &meta_b);
        let td = report.timing_delta.unwrap();
        assert_eq!(td.duration_ms_a, 500);
        assert_eq!(td.duration_ms_b, 800);
        assert_eq!(td.delta_ms, 300);
        assert_eq!(td.slower_session.as_deref(), Some("b"));
    }

    #[test]
    fn test_diff_report_serialization_roundtrip() {
        let events = vec![make_event(1, "main")];
        let meta = make_meta("a", 500);

        let report = TraceDiff::compare("a", "b", &events, &events, &meta, &meta);
        let json = serde_json::to_string(&report).unwrap();
        let roundtrip: DiffReport = serde_json::from_str(&json).unwrap();

        assert_eq!(roundtrip.session_a_id, "a");
        assert_eq!(roundtrip.session_b_id, "b");
        assert_eq!(roundtrip.common_count, 1);
        assert!((roundtrip.similarity_pct - 100.0).abs() < f64::EPSILON);
    }
}
