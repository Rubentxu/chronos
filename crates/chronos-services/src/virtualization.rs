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
//!   REC-C2-01..03) — see the `rec_c1_*` / `rec_c2_*` tests in this file's
//!   `tests` module. Naming note, because the UAT ids and the C1.x design
//!   notes are not the same list: the literal UAT-REC-C1-03 text is "gap
//!   truth", which is proven by real retention in
//!   `events_log_read::rec_c1_3_retention_gap_tests`, not by the monotonicity
//!   test that used to carry the `rec_c1_03_` prefix.

use crate::events_cursor::EventsCursorV1;
use crate::error::ServiceError;
use crate::read_budget::{BudgetExceeded, ReadBudget};

/// Why an aggregation walk stopped before covering the log.
///
/// Two causes, and they are **not** interchangeable:
///
/// - [`Self::Budget`] — the walk ran out of its D2 ceiling. The evidence is
///   readable; the walk just may not continue. Resumable from `next_seq`.
/// - [`Self::Read`] — the log could not be read. The walk stopped at a point
///   it was never able to pass, and the partial aggregate it had built is
///   **discarded**, not returned.
///
/// The distinction is the whole point. Collapsing them would either report a
/// read failure as "you hit your budget" (inviting a pointless retry that will
/// fail identically) or report a budget stop as a log error (hiding a policy
/// decision behind what looks like a fault). Before this type existed, both
/// were `Ok`.
#[derive(Debug, thiserror::Error)]
pub enum AggregateError {
    /// The walk passed its `ResourceLimits` ceiling (SCALE_BUDGETS §5 D2).
    #[error("{0}")]
    Budget(#[from] BudgetExceeded),
    /// A page could not be read. The aggregate is not returned.
    #[error("read-path aggregate stopped: {source}")]
    Read {
        /// The underlying read failure, surfaced verbatim. It is never
        /// swallowed: a decode failure at page 500 is evidence about the log,
        /// not an internal detail.
        #[source]
        source: ServiceError,
    },
}

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
///
/// # Budget (SCALE_BUDGETS §5 D2)
///
/// `budget` is charged once per page and enforces the D2 ceiling. It is a
/// **cooperative** ceiling because this function is synchronous and never
/// yields — see [`crate::read_budget`] for why `tokio::time::timeout` cannot
/// preempt it.
///
/// Whether the ceiling bites at a given size is a measurement, not an
/// argument: SCALE_BUDGETS §3.1 records that the 1.222,7 s figure originally
/// quoted here was measured against a `read_from_seq` that cloned the whole
/// log on every page, and is not reproducible on current code. The ceiling
/// stands on its own terms — it bounds the walk and reports where it stopped —
/// not on that number.
pub fn summarize_log(
    log: &crate::session_log::SessionExecutionLog,
    cursor: &EventsCursorV1,
    bucket_size_ns: u64,
    budget: &mut ReadBudget,
) -> Result<EventSummary, AggregateError> {
    use crate::events_log_read::{read_page, LogReadFilters};
    assert!(bucket_size_ns > 0, "bucket_size_ns must be > 0");

    let mut bucket_counts: Vec<u64> = vec![];
    let mut current_cursor = cursor.clone();
    let page_limit = 1024;

    loop {
        // NOT `while let Ok(page)`: a read failure mid-walk must not become a
        // complete-looking aggregate. The walk below may already have folded
        // hundreds of pages into `bucket_counts` when the failing page arrives,
        // and returning `Ok(partial)` would hand the agent a summary whose
        // `total_events` silently undercounts the log. The caller cannot tell
        // a truncated aggregate from a whole one, which is the same lie as an
        // exhausted log reported as "no evidence" (ADR-0004).
        let page = match read_page(log, &current_cursor, page_limit, &LogReadFilters::default()) {
            Ok(page) => page,
            Err(e) => return Err(AggregateError::Read { source: e }),
        };
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
        // The page boundary is where the D2 ceiling is enforced: one page is
        // the unit of work this function can be interrupted after.
        budget.charge_page(page.records.len() as u64, next_cursor.next_seq().0)?;
        // Stop if cursor does not advance (avoid infinite loop).
        if next_cursor.next_seq() == current_cursor.next_seq() && page.records.len() < page_limit {
            break;
        }
        current_cursor = next_cursor;
    }

    Ok(EventSummary::from_bucket_counts(&bucket_counts))
}

/// Roll up events by **thread** (proxy for invocation grouping when
/// `chronos_invocation_id` is not yet populated in `ExecutionRecord`
/// payloads).
///
/// Per M10-CLOSE.md §6 follow-up #4: real implementation groups by
/// `chronos_invocation_id`. Since `TraceEvent` does not currently
/// expose `invocation_id` (the field lives in
/// `chronos_log::ExecutionRecord`), this rollup uses `thread_id` as
/// a defensible proxy — every invocation runs on one thread, so
/// per-thread counts are an upper bound on per-invocation counts.
///
/// When `chronos_invocation_id` becomes available in the read path
/// (post-M10.6 follow-up), the grouping key can be replaced without
/// changing the public signature.
///
/// # Budget (SCALE_BUDGETS §5 D2)
///
/// Same cooperative ceiling as [`summarize_log`], charged once per page, and
/// the same caveat about what the recorded measurement was taken against.
pub fn rollup_log(
    log: &crate::session_log::SessionExecutionLog,
    cursor: &EventsCursorV1,
    budget: &mut ReadBudget,
) -> Result<InvocationRollup, AggregateError> {
    use crate::events_log_read::{read_page, LogReadFilters};
    use std::collections::HashMap;

    let mut counts: HashMap<u64, u64> = HashMap::new();
    let mut current_cursor = cursor.clone();
    let page_limit = 1024;

    loop {
        // Same reasoning as `summarize_log`: a mid-walk read failure is not an
        // aggregate, it is an error. See the comment there.
        let page = match read_page(log, &current_cursor, page_limit, &LogReadFilters::default()) {
            Ok(page) => page,
            Err(e) => return Err(AggregateError::Read { source: e }),
        };
        if page.records.is_empty() {
            break;
        }
        for ev in &page.records {
            *counts.entry(ev.thread_id).or_insert(0) += 1;
        }
        let next_cursor = page.next.clone();
        budget.charge_page(page.records.len() as u64, next_cursor.next_seq().0)?;
        if next_cursor.next_seq() == current_cursor.next_seq() && page.records.len() < page_limit {
            break;
        }
        current_cursor = next_cursor;
    }

    let counts_vec: Vec<u64> = counts.values().copied().collect();
    Ok(InvocationRollup::from_invocation_counts(&counts_vec))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ServiceError;
    use crate::events_cursor::{EventsCursorError, EVENTS_CURSOR_V1_SCHEMA};
    use crate::events_log_read::{read_page, LogReadFilters};
    use crate::read_budget::ResourceLimits;
    use crate::session_log::SessionExecutionLog;
    use chronos_domain::seq::EventSeq;
    use chronos_domain::{
        EventData, EventType, MonotonicNs, SessionId, SourceLocation, TraceEvent,
    };
    use chronos_log::{ExecutionKind, ExecutionPayload, NewExecutionRecord};

    fn sid() -> SessionId {
        SessionId::new(format!("sess-{}", uuid::Uuid::new_v4()))
    }

    /// A ceiling these tests never reach, so any stop is a mechanism bug and
    /// not a configuration accident. The budget itself is covered by
    /// `read_budget`'s own tests, which drive it deliberately to both stops.
    fn generous_limits() -> ResourceLimits {
        ResourceLimits {
            max_events: 10_000_000,
            timeout_secs: 3_600,
        }
    }

    /// Records written before the retention pass, then retired off disk.
    const RETIRED: u64 = 100;
    /// Records written after, which survive the retention pass.
    const KEPT: u64 = 40;

    /// Append one decodable `TraceEvent` at the log's next seq.
    ///
    /// The payload must be real `TraceEvent` JSON: `read_page` fails closed
    /// on a record it cannot decode, so an opaque payload would make this
    /// fixture prove the wrong thing.
    fn push_event(log: &SessionExecutionLog, event_id: u64) {
        let event = TraceEvent::new(
            event_id,
            MonotonicNs::from(event_id * 10),
            1,
            EventType::FunctionEntry,
            SourceLocation::from_address(0),
            EventData::Empty,
        );
        let session = log.session_id().clone();
        log.append(NewExecutionRecord {
            kind: ExecutionKind::Raw,
            session_id: session,
            monotonic_ns: event_id * 10,
            payload: ExecutionPayload::new(
                serde_json::to_vec(&event).expect("encode"),
                "trace_event",
            ),
            invocation_id: None,
            parent_invocation_id: None,
            symbol_id: None,
            captured_at_unix_ns: None,
        })
        .expect("append");
    }

    /// A real log whose retained range is genuinely truncated.
    ///
    /// `RETIRED` records are appended and flushed to segment files, then
    /// `KEPT` more are appended and flushed, and only then does the
    /// retention port advance the boundary. Retention is not simulated: the
    /// boundary move runs the backend's own crash-safe pass, which commits
    /// the watermark and reclaims the retired segment files. Returns the log,
    /// its directory, and the boundary the log now enforces.
    fn log_with_retired_prefix(
        retired: u64,
        kept: u64,
    ) -> (SessionExecutionLog, std::path::PathBuf, EventSeq) {
        let session = sid();
        let dir = std::env::temp_dir().join(format!(
            "virt_rec_c1_retention_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let log = SessionExecutionLog::create_for_tests(&dir, session).expect("log");

        for i in 0..retired {
            push_event(&log, i);
        }
        // Flush so the prefix is a durable segment. Without a flushed
        // segment the retention pass has nothing wholly retired to move the
        // boundary over, and would be a silent no-op.
        log.flush().expect("flush retired prefix");
        for i in retired..retired + kept {
            push_event(&log, i);
        }
        log.flush().expect("flush surviving suffix");

        let outcome = log
            .advance_retained_from(EventSeq::new(retired))
            .expect("retention must move the boundary forward");
        assert!(
            outcome.boundary_moved,
            "retention must actually move the boundary; a no-op would make \
             the rest of this test vacuous"
        );
        assert_eq!(
            log.retained_from(),
            EventSeq::new(retired),
            "the wrapper must report the new boundary"
        );
        (log, dir, EventSeq::new(retired))
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
        // authoritative log, not a serialized query.
        //
        // The contract is the WIRE SHAPE, so the wire shape is what this
        // test pins — in both directions, which is what the previous
        // roundtrip-only body failed to check:
        //
        //   1. `encode` emits exactly the four contract fields
        //      (`ecv1`, schema, session length, session id, next_seq) and
        //      nothing else. An offset/timestamp/filter/page-size field
        //      would be an extra field, so the field count fails.
        //   2. `decode` accepts nothing beyond those four, so query state
        //      cannot ride in from a hand-crafted cursor either. Without
        //      this, a decoder that ignored extra fields would let query
        //      state travel unobserved and (1) alone would be theatre.
        //   3. The decoded value EQUALS the original, so there is no state
        //      that is carried by neither the wire nor the decoder. A
        //      private field added to the struct without being encoded
        //      would default in `decode` and fail this equality.
        //
        // The session id is colon-free on purpose: the length prefix lets
        // real ids contain ':', but a split-based field count cannot see
        // past one, so this fixture keeps the count meaningful.
        let session = SessionId::new("rec-c1-02-value-object");
        let cursor = EventsCursorV1::start(session.clone());
        let encoded = cursor.encode();

        // (1) Wire shape: prefix + schema + length + session id + next_seq.
        let parts: Vec<&str> = encoded.split(':').collect();
        assert_eq!(
            parts.len(),
            5,
            "cursor wire value must carry exactly prefix+schema+len+session+next_seq \
             (got {parts:?}); an extra field is query state by definition"
        );
        assert_eq!(parts[0], "ecv1", "prefix must be the cursor version tag");
        assert_eq!(
            parts[1].parse::<u16>().expect("schema is numeric"),
            EVENTS_CURSOR_V1_SCHEMA
        );
        assert_eq!(
            parts[2]
                .parse::<usize>()
                .expect("session length is numeric"),
            session.as_str().len(),
            "the length prefix must describe the session id that follows it"
        );
        assert_eq!(parts[3], session.as_str());
        assert_eq!(
            parts[4].parse::<u64>().expect("next_seq is numeric"),
            cursor.next_seq().0,
            "the only position a cursor carries is next_seq"
        );

        // (2) Query state smuggled in from outside is rejected, not honoured.
        for smuggled in ["offset=10", "limit=50", "function_pattern=main"] {
            let with_state = format!("{encoded}:{smuggled}");
            let err = EventsCursorV1::decode(&with_state)
                .expect_err("a cursor carrying query state must not decode");
            assert!(
                matches!(err, EventsCursorError::Malformed { .. }),
                "expected a typed Malformed for {with_state:?}, got {err:?}"
            );
        }

        // (3) The wire value is the whole cursor: no hidden state anywhere.
        let decoded = EventsCursorV1::decode(&encoded).expect("decode");
        assert_eq!(cursor, decoded, "decoded cursor must equal the original");
        assert_eq!(cursor.next_seq(), decoded.next_seq());
        assert_eq!(decoded.encode(), encoded, "re-encoding must be stable");
    }

    #[test]
    fn rec_c1_03_advanced_to_rejects_backward() {
        // The cursor value-object contract (REC-C1.1/C1.2): advance is
        // monotonic, backward is refused. Named `rec_c1_03_` historically,
        // but UAT-REC-C1-03 is "gap truth" — the proof for that is real
        // retention in `events_log_read::rec_c1_3_retention_gap_tests`.
        // The test name is left alone deliberately: it describes exactly
        // what it asserts, and renaming it would lose the audit trail of
        // which UAT people have been reading this as covering.
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
    fn rec_c1_04_retention_stale_cursor_fails_explicitly_and_resumes_at_boundary() {
        // Per REC-C1.4: "Malformed, unknown-session or retention-stale
        // cursors fail explicitly; none silently restart from offset zero."
        //
        // This used to assert only that `encode` is deterministic, which
        // proved nothing about staleness. Real staleness is testable here,
        // so it is tested: the fixture RETIRES a real prefix (flushed
        // segments, then a retention pass that moves the boundary and
        // reclaims the files), and the evidence below the boundary is
        // genuinely gone. A cursor pointing into that retired range must
        // then:
        //
        //   - fail with the TYPED `CursorStale` carrying BOTH numbers,
        //   - NOT come back as a successful page claiming `complete`, and
        //   - NOT silently restart from offset zero.
        //
        // The recoverable half of the same claim: re-anchoring at the
        // boundary serves exactly the surviving records, from the boundary
        // and not from zero.
        let (log, _dir, boundary) = log_with_retired_prefix(RETIRED, KEPT);

        // A cursor from before the boundary is stale, and the failure is
        // typed with both numbers — the agent can tell WHICH seq it asked
        // for and WHERE the log can still start.
        let stale = EventsCursorV1::start(log.session_id().clone())
            .advanced_to(EventSeq::new(0))
            .expect("seq 0 is a legal position before retention moved");
        let err = read_page(&log, &stale, 64, &LogReadFilters::default())
            .expect_err("a cursor below the retention boundary must NOT read");
        match err {
            ServiceError::CursorStale {
                requested_next_seq,
                retained_from_seq,
            } => {
                assert_eq!(
                    requested_next_seq, 0,
                    "the typed error must echo the seq the caller asked for"
                );
                assert_eq!(
                    retained_from_seq, boundary.0,
                    "the typed error must report the real retention boundary"
                );
            }
            other => panic!("expected a typed CursorStale, got {other:?}"),
        }

        // Recoverable: re-anchor AT the boundary (not at zero) and the same
        // log serves exactly the records that survived. `from_seq` is what
        // rules out the silent restart — a read that quietly began at zero
        // would report the retired range as examined evidence.
        let re_anchored = EventsCursorV1::start(log.session_id().clone())
            .advanced_to(boundary)
            .expect("boundary is a legal position");
        let page = read_page(&log, &re_anchored, 64, &LogReadFilters::default())
            .expect("a cursor at the boundary must read");
        assert_eq!(
            page.records.len(),
            KEPT as usize,
            "only the surviving records may come back"
        );
        assert_eq!(
            page.records.first().map(|e| e.event_id),
            Some(RETIRED),
            "the first record served must be the first one retention kept"
        );
        assert_eq!(
            page.completeness.from_seq, boundary.0,
            "the examined range must start at the boundary, never at zero"
        );
        assert_eq!(
            page.retention.retained_from_seq, boundary.0,
            "the page must disclose the real retention boundary"
        );
        assert!(
            page.retention.history_truncated,
            "a non-zero boundary means history WAS truncated; the page must say so"
        );
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
        let mut budget = ReadBudget::start("summarize", &generous_limits(), 0);
        let summary = summarize_log(&log, &cursor, DEFAULT_BUCKET_SIZE_NS, &mut budget)
            .expect("a generous budget must not stop an empty-log summarize");
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
        let mut budget = ReadBudget::start("summarize", &generous_limits(), 0);
        let _ = summarize_log(&log, &cursor, 0, &mut budget);
    }

    #[test]
    fn rollup_log_on_empty_log_returns_empty_rollup() {
        use crate::session_log::SessionExecutionLog;
        use std::env;
        let session = sid();
        let tmp = env::temp_dir().join(format!(
            "virt_rollup_empty_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let log = SessionExecutionLog::create_for_tests(&tmp, session.clone()).expect("log");
        let cursor = EventsCursorV1::start(session);
        let mut budget = ReadBudget::start("rollup", &generous_limits(), 0);
        let rollup = rollup_log(&log, &cursor, &mut budget)
            .expect("a generous budget must not stop an empty-log rollup");
        assert_eq!(rollup.invocation_count, 0);
        assert_eq!(rollup.total_events, 0);
        assert_eq!(rollup.mean_per_invocation, 0);
        assert_eq!(rollup.max_per_invocation, 0);
    }

    // ======================================================================
    // The Silent Lie: a mid-walk read failure must not become a complete
    // aggregate.
    //
    // Both walks above used to be `while let Ok(page) = read_page(...)`.
    // That loop shape cannot distinguish "the log ended" from "the log became
    // unreadable on page 500", and it resolves both by leaving the loop and
    // returning `Ok`. The caller receives a summary over a prefix of the log
    // whose `total_events` undercounts it, with nothing marking the
    // difference. Under D2 that got worse, not better: the honest budget stop
    // had just been introduced, and it now sat next to a silent truncation
    // that looked identical from the outside.
    // ======================================================================

    /// A log whose records decode until `poison_at`, which carries a payload
    /// no decoder can interpret — the shape of a genuinely damaged segment,
    /// as opposed to a mock that fails on demand.
    ///
    /// `poison_at` must exceed one page (1024) for the failure to land on a
    /// *later* page, which is the whole point: the walk must already have
    /// folded real evidence into its aggregate when the read breaks.
    fn log_with_poisoned_record(tag: &str, valid: u64, poison_at: u64) -> SessionExecutionLog {
        use std::env;
        let session = sid();
        let tmp =
            env::temp_dir().join(format!("virt_lie_{tag}_{}", uuid::Uuid::new_v4().simple()));
        let log = SessionExecutionLog::create_for_tests(&tmp, session).expect("log");

        for i in 0..valid {
            let event = TraceEvent::new(
                i,
                MonotonicNs::from(i * 1_000_000),
                1,
                EventType::FunctionEntry,
                SourceLocation::from_address(0),
                EventData::Empty,
            );
            log.append(NewExecutionRecord {
                kind: ExecutionKind::Raw,
                session_id: log.session_id().clone(),
                monotonic_ns: i * 1_000_000,
                payload: ExecutionPayload::new(
                    serde_json::to_vec(&event).expect("encode"),
                    "trace_event",
                ),
                invocation_id: None,
                parent_invocation_id: None,
                symbol_id: None,
                captured_at_unix_ns: None,
            })
            .expect("append");
        }
        // The record that cannot be decoded, placed past the first page.
        log.append(NewExecutionRecord {
            kind: ExecutionKind::Raw,
            session_id: log.session_id().clone(),
            monotonic_ns: poison_at * 1_000_000,
            payload: ExecutionPayload::new(b"not-json-at-all".to_vec(), "unknown_producer"),
            invocation_id: None,
            parent_invocation_id: None,
            symbol_id: None,
            captured_at_unix_ns: None,
        })
        .expect("append");
        log.flush().ok();
        log
    }

    /// How many valid records the walk can absorb before the damaged one.
    /// Comfortably more than one page, so the aggregate is genuinely partial
    /// when the read breaks.
    const VALID_BEFORE_POISON: u64 = 3_000;
    /// The seq of the damaged record. Past the first page (1024) and inside
    /// the walk, so the failure is a mid-walk failure.
    const POISON_AT: u64 = 2_500;

    #[test]
    fn summarize_does_not_report_a_partial_aggregate_as_complete() {
        let log = log_with_poisoned_record("sum", VALID_BEFORE_POISON, POISON_AT);
        let cursor = EventsCursorV1::start(log.session_id().clone());
        let mut budget = ReadBudget::start("summarize", &generous_limits(), 0);

        let err = summarize_log(&log, &cursor, DEFAULT_BUCKET_SIZE_NS, &mut budget).expect_err(
            "a damaged record must stop the walk with an error, \
             not return the prefix that was aggregated before it",
        );
        assert!(
            matches!(err, AggregateError::Read { .. }),
            "the failure must be reported as a read failure: {err:?}"
        );
        // The reason survives: "it timed out" would be a different lie.
        let text = err.to_string();
        assert!(
            text.contains("decode") || text.contains("Decode"),
            "the decode failure must reach the caller: {text}"
        );
    }

    #[test]
    fn rollup_does_not_report_a_partial_aggregate_as_complete() {
        let log = log_with_poisoned_record("roll", VALID_BEFORE_POISON, POISON_AT);
        let cursor = EventsCursorV1::start(log.session_id().clone());
        let mut budget = ReadBudget::start("rollup", &generous_limits(), 0);

        let err = rollup_log(&log, &cursor, &mut budget)
            .expect_err("a damaged record must stop the walk with an error");
        assert!(
            matches!(err, AggregateError::Read { .. }),
            "the failure must be reported as a read failure: {err:?}"
        );
    }

    /// Non-vacuity, in the same shape as every other guard here: with the
    /// `Err` arm removed from the loop (i.e. the walk written as
    /// `while let Ok(page) = ...` again), BOTH tests above fail — the walk
    /// returns `Ok` over the 2,500 events it managed to read, and the
    /// caller cannot tell that record 2,500 was never seen.
    ///
    /// Kept as a live test rather than a note because the loop shape is
    /// exactly the kind of change a future refactor makes "for readability".
    #[test]
    fn a_read_failure_is_never_absorbed_into_an_ok_aggregate() {
        let log = log_with_poisoned_record("vac", VALID_BEFORE_POISON, POISON_AT);
        let cursor = EventsCursorV1::start(log.session_id().clone());
        let mut budget = ReadBudget::start("summarize", &generous_limits(), 0);

        match summarize_log(&log, &cursor, DEFAULT_BUCKET_SIZE_NS, &mut budget) {
            Err(AggregateError::Read { .. }) => {}
            Err(other) => panic!("expected a read failure, got {other:?}"),
            Ok(summary) => panic!(
                "a walk that hit a damaged record returned Ok over {} events — \
                 this is the Silent Lie, and the guard no longer discriminates",
                summary.total_events
            ),
        }
    }

    /// The two stops must stay distinguishable: a ceiling stop is resumable
    /// and carries an anchor; a read stop is not a policy decision. Before
    /// `AggregateError` they were the same `Ok`.
    #[test]
    fn a_budget_stop_and_a_read_stop_are_different_answers() {
        // A read stop, from a damaged record.
        let log = log_with_poisoned_record("distinguish_read", VALID_BEFORE_POISON, POISON_AT);
        let cursor = EventsCursorV1::start(log.session_id().clone());
        let mut budget = ReadBudget::start("summarize", &generous_limits(), 0);
        let read_stop = summarize_log(&log, &cursor, DEFAULT_BUCKET_SIZE_NS, &mut budget)
            .expect_err("damaged record");

        // A budget stop, from a clean log under a zero-second ceiling.
        use std::env;
        let session = sid();
        let tmp = env::temp_dir()
            .join(format!("virt_distinguish_budget_{}", uuid::Uuid::new_v4().simple()));
        let clean_log = SessionExecutionLog::create_for_tests(&tmp, session.clone()).expect("log");
        for i in 0..VALID_BEFORE_POISON {
            let event = TraceEvent::new(
                i,
                MonotonicNs::from(i * 1_000_000),
                1,
                EventType::FunctionEntry,
                SourceLocation::from_address(0),
                EventData::Empty,
            );
            clean_log
                .append(NewExecutionRecord {
                    kind: ExecutionKind::Raw,
                    session_id: session.clone(),
                    monotonic_ns: i * 1_000_000,
                    payload: ExecutionPayload::new(
                        serde_json::to_vec(&event).expect("encode"),
                        "trace_event",
                    ),
                    invocation_id: None,
                    parent_invocation_id: None,
                    symbol_id: None,
                    captured_at_unix_ns: None,
                })
                .expect("append");
        }
        clean_log.flush().ok();
        let cursor = EventsCursorV1::start(session);
        let mut budget = ReadBudget::start(
            "summarize",
            &ResourceLimits {
                max_events: usize::MAX,
                timeout_secs: 0,
            },
            0,
        );
        let budget_stop = summarize_log(&clean_log, &cursor, DEFAULT_BUCKET_SIZE_NS, &mut budget)
            .expect_err("zero-second ceiling");

        assert!(
            matches!(read_stop, AggregateError::Read { .. }),
            "the damaged-record walk is a read stop: {read_stop:?}"
        );
        match budget_stop {
            AggregateError::Budget(b) => {
                // The anchor is what makes a budget stop actionable, and it
                // is the thing a read stop cannot offer.
                assert!(b.next_seq > 0, "a budget stop must name its anchor");
            }
            other => panic!("expected a budget stop, got {other:?}"),
        }
    }
}
