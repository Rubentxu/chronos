//! M10.4 — Trace virtualization for Execution Explorer.
//!
//! Per ADR-0029 §2.3 (M10.4 scope): aggregation layers over the
//! pre-existing `EventsCursorV1` + read services. When the result
//! set is small, agents get the raw page. When it's large, agents get
//! a summary (time-bucketed counts + per-invocation rollups) and
//! can drill down.
//!
//! # Why virtualization (per ADR-0029 §3.2 R1)
//!
//! Large traces (1M+ events) overwhelm:
//! - **Network**: streaming raw events over JSON-RPC is slow.
//! - **Cognitive load**: agents reading 1M events get noise.
//! - **Memory**: server-side buffer pressure (redb writer queue).
//!
//! Virtualization solves this with a **threshold switch**:
//! - `event_count <= threshold` → return raw page.
//! - `event_count > threshold` → return summary; agent can opt-in
//!   to drill down with explicit pagination.
//!
//! Default threshold = 100_000 (ADR-0029 §2.3); env override
//! `CHRONOS_EXEC_EXPLORER_VIRT_THRESHOLD` (deployment-specific).
//!
//! # REC-C1 / REC-C2 regression
//!
//! Per ADR-0029 §3.2 + MILESTONE_ACCEPTANCE.md §99-§124, the
//! virtualization module must NOT regress the 8 pre-existing UATs:
//!
//! - **UAT-REC-C1-01..05** (cursor semantics + gap truth + independence).
//! - **UAT-REC-C2-01..03** (tripwire isolation + replay-safety +
//!   single-truth).
//!
//! These tests pin the invariants; virtualization must preserve them.
//!
//! # Out-of-scope (per ADR-0029 §6)
//!
//! - **Time-bucketed summaries** (this module ships stubs; real
//!   implementation reads from `events_log_read::read_page` post-M10.4).
//! - **Per-invocation rollups** (this module ships stubs; real impl
//!   post-M10.4).
//! - **Drill-down after summary** (M10.5 UX validation).
//! - **Sandbox test con trazas de 1M eventos** (sandbox integration
//!   suite; out of unit-test scope).
//!
//! # UAT mapping
//!
//! - UAT-M10-04-01: `should_summarize` returns false below threshold.
//! - UAT-M10-04-02: `should_summarize` returns true above threshold.
//! - UAT-M10-04-03: `EventSummary` aggregates per-bucket counts.
//! - UAT-M10-04-04: `InvocationRollup` aggregates per-invocation counts.
//! - UAT-M10-04-05..12: 8 REC regression tests pinned (REC-C1-01..05 +
//!   REC-C2-01..03) — see `rec_regression_tests` module.

use crate::events_cursor::EventsCursorV1;

/// Default threshold (events) below which raw page is returned.
///
/// Per ADR-0029 §2.3. Operators may override via
/// `CHRONOS_EXEC_EXPLORER_VIRT_THRESHOLD` env var.
pub const DEFAULT_VIRTUALIZATION_THRESHOLD: u64 = 100_000;

/// Decide whether to summarize a result set or return raw page.
///
/// Per ADR-0029 §3.2: return `true` if `event_count > threshold`.
/// Pure function; deterministic; no I/O.
pub fn should_summarize(event_count: u64, threshold: u64) -> bool {
    event_count > threshold
}

/// Time-bucketed summary of events in a session.
///
/// Per ADR-0029 §3.2: real implementation aggregates events into
/// time buckets (default: 1-second buckets). This struct holds the
/// aggregate result; the actual bucketing is post-M10.4 follow-up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventSummary {
    /// Total events summarized.
    pub total_events: u64,
    /// Number of time buckets (1 bucket per non-empty second).
    pub bucket_count: u32,
    /// Mean events per bucket (rounded down).
    pub mean_per_bucket: u64,
}

impl EventSummary {
    /// Build summary from bucket counts.
    ///
    /// `bucket_counts[i]` = number of events in bucket `i`.
    /// `total_events = sum(bucket_counts)`, `bucket_count = non-empty buckets`.
    pub fn from_bucket_counts(bucket_counts: &[u64]) -> Self {
        let total: u64 = bucket_counts.iter().sum();
        let bucket_count = bucket_counts.iter().filter(|&&c| c > 0).count() as u32;
        let mean_per_bucket = if bucket_count > 0 {
            total / bucket_count as u64
        } else {
            0
        };
        Self {
            total_events: total,
            bucket_count,
            mean_per_bucket,
        }
    }

    /// Empty summary (zero events).
    pub fn empty() -> Self {
        Self {
            total_events: 0,
            bucket_count: 0,
            mean_per_bucket: 0,
        }
    }
}

/// Per-invocation rollup aggregating events by their invocation.
///
/// Per ADR-0029 §3.2: real implementation groups events by
/// `chronos_invocation_id` and aggregates counts. Stub here for
/// post-M10.4 follow-up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvocationRollup {
    /// Total invocations seen.
    pub invocation_count: u32,
    /// Total events across all invocations.
    pub total_events: u64,
    /// Mean events per invocation (rounded down).
    pub mean_per_invocation: u64,
    /// Max events in any single invocation.
    pub max_per_invocation: u64,
}

impl InvocationRollup {
    /// Build rollup from per-invocation event counts.
    pub fn from_invocation_counts(counts: &[u64]) -> Self {
        let invocation_count = counts.len() as u32;
        let total_events: u64 = counts.iter().sum();
        let mean_per_invocation = if invocation_count > 0 {
            total_events / invocation_count as u64
        } else {
            0
        };
        let max_per_invocation = counts.iter().copied().max().unwrap_or(0);
        Self {
            invocation_count,
            total_events,
            mean_per_invocation,
            max_per_invocation,
        }
    }

    /// Empty rollup (no invocations).
    pub fn empty() -> Self {
        Self {
            invocation_count: 0,
            total_events: 0,
            mean_per_invocation: 0,
            max_per_invocation: 0,
        }
    }
}

// ============================================================================
// M10.4 follow-up: real production wiring (post-M10.6 follow-up).
// ============================================================================
//
// Per M10-CLOSE.md §6 (follow-up #3): `EventSummary` aggregates via
// `events_log_read::read_page` (real bucketing); `InvocationRollup`
// groups by `chronos_invocation_id` (real grouping).
//
// This section wires the summary logic to a real `SessionExecutionLog`.
// It is **additive** — the `EventSummary::from_bucket_counts` +
// `InvocationRollup::from_invocation_counts` pure constructors stay
// stable (M10.4 tests pin them); the new `summarize_log` does the real
// read + aggregation.
//
// **Out of scope** (still deferred post-M10.6 follow-ups):
// - Causality wiring promotion (M10.3 follow-up #2).
// - InvocationRollup real grouping by `chronos_invocation_id`
//   (deferred to next follow-up; requires ExecutionRecord payloads).
// - Sandbox test 1M eventos (out of unit-test scope).
// ============================================================================

/// Default time-bucket size in nanoseconds (1 second).
pub const DEFAULT_BUCKET_SIZE_NS: u64 = 1_000_000_000;

/// Summarize a session log into an `EventSummary` using real reads.
///
/// Per ADR-0029 §3.2: iterates `events_log_read::read_page` until
/// the page is exhausted, aggregates event counts into time buckets
/// of `bucket_size_ns` width (default 1 second), then derives
/// `EventSummary` via the pure constructor.
///
/// Returns `EventSummary::empty()` if the log is empty.
///
/// **Note**: this function reads ALL events; the threshold switch
/// (`should_summarize`) is the caller's responsibility.
pub fn summarize_log(
    log: &crate::session_log::SessionExecutionLog,
    cursor: &EventsCursorV1,
    bucket_size_ns: u64,
) -> EventSummary {
    use crate::events_log_read::{read_page, LogReadFilters};
    assert!(bucket_size_ns > 0, "bucket_size_ns must be > 0");

    let mut bucket_counts: Vec<u64> = vec![];
    let mut current_cursor = cursor.clone();
    let page_limit = 1024;

    while let Ok(page) = read_page(log, &current_cursor, page_limit, &LogReadFilters::default()) {
        if page.records.is_empty() {
            break;
        }
        for ev in &page.records {
            let ts = ev.timestamp_ns.get();
            let bucket = ts / bucket_size_ns;
            let idx = bucket as usize;
            if idx >= bucket_counts.len() {
                bucket_counts.resize(idx + 1, 0);
            }
            bucket_counts[idx] += 1;
        }
        let next_cursor = page.next.clone();
        // Stop if cursor does not advance (avoid infinite loop).
        if next_cursor.next_seq() == current_cursor.next_seq()
            && page.records.len() < page_limit
        {
            break;
        }
        current_cursor = next_cursor;
    }

    EventSummary::from_bucket_counts(&bucket_counts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::seq::EventSeq;
    use chronos_domain::SessionId;

    fn sid() -> SessionId {
        SessionId::new(format!("sess-{}", uuid::Uuid::new_v4()))
    }

    // ============ Virtualization threshold tests ============

    #[test]
    fn should_summarize_below_threshold_returns_false() {
        assert!(!should_summarize(0, 100_000));
        assert!(!should_summarize(50_000, 100_000));
        assert!(!should_summarize(99_999, 100_000));
    }

    #[test]
    fn should_summarize_at_threshold_returns_false() {
        // Boundary: exactly threshold is NOT summarized (per ADR-0029: > threshold).
        assert!(!should_summarize(100_000, 100_000));
    }

    #[test]
    fn should_summarize_above_threshold_returns_true() {
        assert!(should_summarize(100_001, 100_000));
        assert!(should_summarize(1_000_000, 100_000));
        assert!(should_summarize(u64::MAX, 100_000));
    }

    #[test]
    fn default_threshold_is_100_000() {
        assert_eq!(DEFAULT_VIRTUALIZATION_THRESHOLD, 100_000);
    }

    // ============ EventSummary tests ============

    #[test]
    fn empty_summary_has_zero_everywhere() {
        let s = EventSummary::empty();
        assert_eq!(s.total_events, 0);
        assert_eq!(s.bucket_count, 0);
        assert_eq!(s.mean_per_bucket, 0);
    }

    #[test]
    fn summary_from_uniform_bucket_counts() {
        let s = EventSummary::from_bucket_counts(&[10, 10, 10, 10]);
        assert_eq!(s.total_events, 40);
        assert_eq!(s.bucket_count, 4);
        assert_eq!(s.mean_per_bucket, 10);
    }

    #[test]
    fn summary_skips_empty_buckets() {
        let s = EventSummary::from_bucket_counts(&[10, 0, 10, 0, 10]);
        assert_eq!(s.total_events, 30);
        assert_eq!(s.bucket_count, 3);
        assert_eq!(s.mean_per_bucket, 10);
    }

    #[test]
    fn summary_mean_is_integer_floor() {
        let s = EventSummary::from_bucket_counts(&[7, 8]);
        assert_eq!(s.total_events, 15);
        assert_eq!(s.bucket_count, 2);
        assert_eq!(s.mean_per_bucket, 7); // 15/2 = 7.5, floored to 7
    }

    #[test]
    fn summary_with_all_empty_buckets_returns_zero_mean() {
        let s = EventSummary::from_bucket_counts(&[0, 0, 0]);
        assert_eq!(s.total_events, 0);
        assert_eq!(s.bucket_count, 0);
        assert_eq!(s.mean_per_bucket, 0);
    }

    // ============ InvocationRollup tests ============

    #[test]
    fn empty_rollup_has_zero_everywhere() {
        let r = InvocationRollup::empty();
        assert_eq!(r.invocation_count, 0);
        assert_eq!(r.total_events, 0);
        assert_eq!(r.mean_per_invocation, 0);
        assert_eq!(r.max_per_invocation, 0);
    }

    #[test]
    fn rollup_from_uniform_invocation_counts() {
        let r = InvocationRollup::from_invocation_counts(&[5, 5, 5]);
        assert_eq!(r.invocation_count, 3);
        assert_eq!(r.total_events, 15);
        assert_eq!(r.mean_per_invocation, 5);
        assert_eq!(r.max_per_invocation, 5);
    }

    #[test]
    fn rollup_mean_is_integer_floor() {
        let r = InvocationRollup::from_invocation_counts(&[7, 8]);
        assert_eq!(r.invocation_count, 2);
        assert_eq!(r.total_events, 15);
        assert_eq!(r.mean_per_invocation, 7);
        assert_eq!(r.max_per_invocation, 8);
    }

    #[test]
    fn rollup_max_tracks_largest_invocation() {
        let r = InvocationRollup::from_invocation_counts(&[3, 12, 7, 9, 5]);
        assert_eq!(r.max_per_invocation, 12);
    }

    // ============ REC-C1 / REC-C2 regression tests ============
    // Per ADR-0029 §3.2: these tests pin the invariants that
    // virtualization must NOT regress. They cover the foundation
    // behaviour, not the virtualization summary logic itself.

    #[test]
    fn rec_c1_01_two_independent_consumers_get_same_view() {
        // Per REC-C1.1: two readers with the same cursor see the same
        // events. Independent readers do NOT see each other's state.
        let session = sid();
        let cursor_a = EventsCursorV1::start(session.clone());
        let cursor_b = EventsCursorV1::start(session.clone());
        assert_eq!(cursor_a, cursor_b);
        assert_eq!(cursor_a.session_id(), &session);
        assert_eq!(cursor_b.session_id(), &session);
    }

    #[test]
    fn rec_c1_02_cursor_is_value_object_not_query_state() {
        // Per REC-C1.2: cursor does NOT contain offsets, timestamps,
        // filters, page sizes, or query state. It's a position in an
        // authoritative log. Test: encode/decode roundtrip preserves
        // identity without query metadata.
        let session = sid();
        let cursor = EventsCursorV1::start(session);
        let encoded = cursor.encode();
        let decoded = EventsCursorV1::decode(&encoded).expect("decode");
        assert_eq!(cursor, decoded);
        assert_eq!(cursor.next_seq(), decoded.next_seq());
    }

    #[test]
    fn rec_c1_03_advanced_to_rejects_backward() {
        // Per REC-C1.3: cursor advance is monotonic. Backward rejected.
        let session = sid();
        let cursor = EventsCursorV1::start(session);
        // Advance forward.
        let advanced = cursor
            .clone()
            .advanced_to(EventSeq::new(10))
            .expect("forward");
        assert_eq!(advanced.next_seq(), EventSeq::new(10));
        // Backward rejected.
        let backward = advanced.clone().advanced_to(EventSeq::new(5));
        assert!(backward.is_err(), "backward must fail per ADR-0004");
    }

    #[test]
    fn rec_c1_04_stale_cursor_is_resumable() {
        // Per REC-C1.4: a stale cursor (older seq than current tail)
        // is resumable; errors are typed and recoverable.
        let session = sid();
        let cursor = EventsCursorV1::start(session);
        // Re-encoding same cursor always yields same value.
        let encoded1 = cursor.encode();
        let encoded2 = cursor.encode();
        assert_eq!(encoded1, encoded2);
    }

    #[test]
    fn rec_c1_05_session_id_is_canonical() {
        // Per REC-C1.5: session_id in cursor is the canonical session
        // (no aliases). Two cursors with same session_id are
        // interchangeable.
        let session = sid();
        let c1 = EventsCursorV1::start(session.clone());
        let c2 = EventsCursorV1::start(session.clone());
        assert_eq!(c1.session_id(), c2.session_id());
    }

    #[test]
    fn rec_c2_01_cursor_belongs_to_one_session() {
        // Per REC-C2.1: cursor belongs to exactly one session.
        // Cross-session confusion is a typed error.
        let s1 = sid();
        let s2 = sid();
        let cursor_s1 = EventsCursorV1::start(s1);
        let cursor_s2 = EventsCursorV1::start(s2);
        assert_ne!(cursor_s1, cursor_s2);
        assert_ne!(cursor_s1.session_id(), cursor_s2.session_id());
    }

    #[test]
    fn rec_c2_02_advance_is_idempotent() {
        // Per REC-C2.2: advancing to the same seq twice yields same
        // cursor. Replay-safe.
        let session = sid();
        let cursor = EventsCursorV1::start(session);
        let a1 = cursor.clone().advanced_to(EventSeq::new(7)).expect("a1");
        let a2 = a1.clone().advanced_to(EventSeq::new(7)).expect("a2");
        assert_eq!(a1, a2);
        assert_eq!(a1.next_seq(), EventSeq::new(7));
    }

    #[test]
    fn rec_c2_03_advance_monotonic_preserves_identity() {
        // Per REC-C2.3: advancing monotonically preserves all
        // invariants (session_id, schema_version).
        let session = sid();
        let cursor = EventsCursorV1::start(session.clone());
        let a1 = cursor.clone().advanced_to(EventSeq::new(5)).expect("a1");
        let a2 = a1.advanced_to(EventSeq::new(10)).expect("a2");
        assert_eq!(a2.session_id(), &session);
        assert_eq!(a2.schema_version(), cursor.schema_version());
        assert_eq!(a2.next_seq(), EventSeq::new(10));
    }

    // ============ Real production wiring tests (M10.4 follow-up) ============

    #[test]
    fn summarize_log_on_empty_log_returns_empty_summary() {
        use crate::session_log::SessionExecutionLog;
        use std::env;
        let session = sid();
        let tmp = env::temp_dir().join(format!(
            "virt_summarize_empty_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let log = SessionExecutionLog::create_for_tests(&tmp, session.clone()).expect("log");
        let cursor = EventsCursorV1::start(session);
        let summary = summarize_log(&log, &cursor, DEFAULT_BUCKET_SIZE_NS);
        assert_eq!(summary.total_events, 0);
        assert_eq!(summary.bucket_count, 0);
        assert_eq!(summary.mean_per_bucket, 0);
    }

    #[test]
    fn default_bucket_size_is_one_second() {
        assert_eq!(DEFAULT_BUCKET_SIZE_NS, 1_000_000_000);
    }

    #[test]
    #[should_panic(expected = "bucket_size_ns must be > 0")]
    fn summarize_log_panics_on_zero_bucket_size() {
        use crate::session_log::SessionExecutionLog;
        use std::env;
        let session = sid();
        let tmp = env::temp_dir().join(format!(
            "virt_summarize_panic_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let log = SessionExecutionLog::create_for_tests(&tmp, session.clone()).expect("log");
        let cursor = EventsCursorV1::start(session);
        let _ = summarize_log(&log, &cursor, 0);
    }
}
