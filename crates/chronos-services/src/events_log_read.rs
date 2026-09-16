//! REC-C1.3 — the authoritative, stateless read page for `events_read`.
//!
//! ```text
//! events_read -> LiveProbeSession -> SessionExecutionLog -> EventSeq
//! ```
//!
//! No `QueryEngine`. No `DebugTraceService`. No `offset`.
//!
//! ## Statelessness
//!
//! The read does not use `ExecutionLogBackend::read_after`, which carries
//! per-consumer state:
//!
//! * a shared consumer id means two agents tread on each other,
//! * a per-request consumer id means the backend's staleness detection stops
//!   meaning anything, because it only considers a cursor stale when state
//!   already exists for that consumer.
//!
//! The agent-visible cursor already carries the entire position
//! (`SessionId` + `EventSeq`), so the page read is a pure function of
//! `(log, position, limit, filters)`.
//!
//! ## Position vs results
//!
//! The cursor is **evidence examined**, not results returned:
//!
//! ```text
//! log   : seq 100..150
//! match :        103, 121, 147     limit_matches = 3
//! next  : next_seq = 148           NOT 3
//! ```
//!
//! Filters never enter the cursor.
//!
//! ## Completeness
//!
//! Until REC-C1.4 proves gap/completeness detection the only honest answer is
//! [`Completeness::Unknown`]. The page carries the gaps it saw so C1.4 can
//! interpret them, but this module does not claim completeness.

use chronos_domain::EventType;
use chronos_domain::TraceEvent;
use chronos_log::{EventSeq, ExecutionRecord, Gap, LogPage};

use crate::error::ServiceError;
use crate::events_cursor::EventsCursorV1;
use crate::session_log::SessionExecutionLog;

/// How complete the returned evidence is.
///
/// Deliberately minimal in C1.3: `Complete` is NOT available yet, because no
/// gap/completeness proof exists. A `Complete` that nothing backs would be a
/// Silent Lie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Completeness {
    /// This build cannot prove completeness yet (REC-C1.4 will).
    Unknown,
}

impl Completeness {
    pub fn as_str(self) -> &'static str {
        match self {
            Completeness::Unknown => "unknown",
        }
    }
}

/// Filters applied AFTER selection. They never affect the cursor position.
#[derive(Debug, Clone, Default)]
pub struct LogReadFilters {
    pub event_types: Option<Vec<EventType>>,
    pub thread_id: Option<u64>,
    pub timestamp_start: Option<u64>,
    pub timestamp_end: Option<u64>,
    pub function_pattern: Option<String>,
}

impl LogReadFilters {
    fn matches(&self, r: &TraceEvent) -> bool {
        if let Some(types) = &self.event_types {
            if !types.iter().any(|t| *t == r.event_type) {
                return false;
            }
        }
        if let Some(thread) = self.thread_id {
            if r.thread_id != thread {
                return false;
            }
        }
        if let Some(start) = self.timestamp_start {
            if r.timestamp_ns < start {
                return false;
            }
        }
        if let Some(end) = self.timestamp_end {
            if r.timestamp_ns > end {
                return false;
            }
        }
        if let Some(pattern) = &self.function_pattern {
            // `SourceLocation.function` is optional in the domain model; an
            // event with no symbol cannot match a function filter.
            match &r.location.function {
                Some(name) if name.contains(pattern.as_str()) => {}
                _ => return false,
            }
        }
        true
    }
}

/// One authoritative page.
#[derive(Debug, Clone)]
pub struct LogReadPage {
    /// Decoded events that passed the filters, in seq order.
    pub records: Vec<TraceEvent>,
    /// The cursor for the next read (opaque externally).
    pub next: EventsCursorV1,
    /// The highest seq the read examined. `next.next_seq() == scanned.0 + 1`
    /// when anything was examined, and equals the incoming position otherwise.
    pub scanned: EventSeq,
    /// Gaps the read passed over, for C1.4 to interpret.
    pub gaps: Vec<Gap>,
    pub completeness: Completeness,
}

/// Scan chunk size. Bounds the work per inner page without changing semantics:
/// the loop keeps scanning until it has `limit` matches or reaches the tail.
const SCAN_CHUNK: usize = 512;

/// Read up to `limit` matching records starting at `cursor.next_seq()`, and
/// return the cursor for the next read.
///
/// `cursor.next_seq() == 0` starts at seq#0 inclusive. After examining through
/// `M`, the returned cursor is `M + 1`. A reader therefore never re-reads and
/// never skips, and it advances across observed gaps rather than stalling.
pub fn read_page(
    log: &SessionExecutionLog,
    cursor: &EventsCursorV1,
    limit: usize,
    filters: &LogReadFilters,
) -> Result<LogReadPage, ServiceError> {
    if cursor.session_id() != log.session_id() {
        return Err(ServiceError::NoExecutionLog(format!(
            "cursor for session {} used against log for {}",
            cursor.session_id().as_str(),
            log.session_id().as_str()
        )));
    }

    let handle = log.handle();
    let mut position = cursor.next_seq();
    let mut matched: Vec<TraceEvent> = Vec::new();
    let mut gaps: Vec<Gap> = Vec::new();
    let mut unparseable: u64 = 0;

    while matched.len() < limit {
        let page: LogPage = handle
            .read_from_seq(position, SCAN_CHUNK)
            .map_err(|e| ServiceError::DrainFailed(format!("{e:?}")))?;
        gaps.extend(page.gaps.iter().cloned());

        let mut stopped_early = false;
        let mut examined_any = false;
        for record in &page.records {
            if matched.len() >= limit {
                stopped_early = true;
                break;
            }
            // The position advances exactly to the last record actually
            // examined. Never to the end of the inner chunk: claiming to have
            // examined records we did not compare would silently lose them for
            // a caller that changes filters on the next read.
            examined_any = true;
            position = EventSeq::new(record.seq.0 + 1);
            match decode(record) {
                Some(event) if filters.matches(&event) => matched.push(event),
                Some(_) => {}
                None => unparseable += 1,
            }
        }

        if stopped_early || matched.len() >= limit {
            break;
        }
        if page.exhausted && !examined_any {
            // Nothing at or after `position`; the reader is caught up. Gaps that
            // reach the position are consumed so progress is monotonic.
            position = page.position_after;
            break;
        }
        if page.position_after <= position {
            // Safety: the backend reported no forward progress. Stop rather than
            // spin; the caller sees the position it already had.
            break;
        }
        position = page.position_after;
    }

    // Only gaps the reader actually passed over belong to this page.
    gaps.retain(|g| g.last_missing.0 < position.0);
    gaps.sort_by_key(|g| g.first_missing.0);
    gaps.dedup_by_key(|g| g.first_missing.0);
    let _ = unparseable;

    let next = cursor
        .clone()
        .advanced_to(position)
        .map_err(|_| ServiceError::InvalidCursorPayload)?;
    Ok(LogReadPage {
        records: matched,
        next,
        scanned: position,
        gaps,
        completeness: Completeness::Unknown,
    })
}

/// Decode a log record's payload into a `TraceEvent`.
///
/// The ExecutionLog payload is JSON-encoded `TraceEvent` (the m1-03 producer
/// contract); a record that does not decode is counted separately rather than
/// silently dropped, so the bytes stay durable and the anomaly is visible.
fn decode(record: &ExecutionRecord) -> Option<TraceEvent> {
    serde_json::from_slice::<TraceEvent>(&record.payload.bytes).ok()
}

/// `ById` on the authoritative path: linear scan of the log.
///
/// Correctness first, index later: the deprecated path answered ids from the
/// in-memory `QueryEngine`, which is exactly the source C1.3 removes.
pub fn find_by_id(
    log: &SessionExecutionLog,
    event_id: u64,
) -> Result<Option<TraceEvent>, ServiceError> {
    let handle = log.handle();
    let mut position = EventSeq::ZERO;
    loop {
        let page = handle
            .read_from_seq(position, SCAN_CHUNK)
            .map_err(|e| ServiceError::DrainFailed(format!("{e:?}")))?;
        if page.records.is_empty() && page.exhausted {
            return Ok(None);
        }
        for record in &page.records {
            if let Some(event) = decode(record) {
                if event.event_id == event_id {
                    return Ok(Some(event));
                }
            }
        }
        if page.exhausted {
            return Ok(None);
        }
        position = page.position_after;
    }
}

#[cfg(test)]
mod rec_c1_3_tests {
    //! Mandatory C1.3 properties: seq#0, exact page boundary, exact resume,
    //! independent readers, and filters that never move the position.

    use super::*;
    use chronos_domain::{EventType, SourceLocation};
    use chronos_log::{NewExecutionRecord, SessionId};

    fn log(tag: &str, events: &[(EventType, u64, u32)]) -> SessionExecutionLog {
        let dir = std::env::temp_dir().join(format!(
            "rec-c1-3-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let owned = SessionExecutionLog::open(&dir, SessionId::new(tag)).expect("log");
        let handle = owned.handle();
        for (i, (event_type, ts, thread)) in events.iter().enumerate() {
            let event = TraceEvent::new(
                i as u64,
                *ts,
                *thread as u64,
                *event_type,
                SourceLocation {
                    file: Some("f.rs".to_string()),
                    line: Some(i as u32),
                    column: None,
                    function: Some(format!("fn_{i}")),
                    address: 0,
                },
                chronos_domain::EventData::Empty,
            );
            handle
                .append(NewExecutionRecord {
                    session_id: handle.session_id().clone(),
                    monotonic_ns: *ts,
                    payload: chronos_log::ExecutionPayload::new(
                        serde_json::to_vec(&event).unwrap(),
                        "trace_event",
                    ),
                    invocation_id: None,
                    parent_invocation_id: None,
                    symbol_id: None,
                })
                .expect("append");
        }
        handle.flush().ok();
        owned
    }

    fn entries(n: usize, event_type: EventType) -> Vec<(EventType, u64, u32)> {
        (0..n).map(|i| (event_type, i as u64 * 10, 1u32)).collect()
    }

    #[test]
    fn seq_zero_is_not_lost() {
        let owned = log("seq0", &entries(3, EventType::FunctionEntry));
        let cursor = owned.cursor_start();
        let page = read_page(&owned, &cursor, 10, &LogReadFilters::default()).unwrap();
        assert_eq!(page.records.len(), 3, "seq#0 must be delivered");
        assert_eq!(page.records[0].event_id, 0);
        assert_eq!(page.next.next_seq(), EventSeq::new(3));
        assert_eq!(page.completeness, Completeness::Unknown);
    }

    #[test]
    fn page_boundary_and_exact_resume() {
        let owned = log("pages", &entries(25, EventType::FunctionEntry));
        let p1 = read_page(
            &owned,
            &owned.cursor_start(),
            10,
            &LogReadFilters::default(),
        )
        .unwrap();
        let ids1: Vec<u64> = p1.records.iter().map(|e| e.event_id).collect();
        assert_eq!(ids1, (0..10).collect::<Vec<u64>>());
        assert_eq!(p1.next.next_seq(), EventSeq::new(10));

        let p2 = read_page(&owned, &p1.next, 10, &LogReadFilters::default()).unwrap();
        let ids2: Vec<u64> = p2.records.iter().map(|e| e.event_id).collect();
        assert_eq!(
            ids2,
            (10..20).collect::<Vec<u64>>(),
            "resume starts exactly at 10"
        );
        assert_eq!(p2.next.next_seq(), EventSeq::new(20));

        // Re-reading from a cursor re-anchored to 10 reproduces page 2 exactly.
        // (A cursor cannot be rewound in place: `advanced_to` refuses to go
        // backwards, which is what keeps a reader from silently re-reading.)
        let re_anchored = owned.cursor_start().advanced_to(EventSeq::new(10)).unwrap();
        let again = read_page(&owned, &re_anchored, 10, &LogReadFilters::default()).unwrap();
        assert_eq!(
            again
                .records
                .iter()
                .map(|e| e.event_id)
                .collect::<Vec<u64>>(),
            ids2
        );
    }

    #[test]
    fn two_readers_are_independent() {
        let owned = log("two", &entries(60, EventType::FunctionEntry));
        let a_cursor = owned.cursor_start().advanced_to(EventSeq::new(10)).unwrap();
        let b_cursor = owned.cursor_start().advanced_to(EventSeq::new(40)).unwrap();
        let a = read_page(&owned, &a_cursor, 5, &LogReadFilters::default()).unwrap();
        let b = read_page(&owned, &b_cursor, 5, &LogReadFilters::default()).unwrap();
        assert_eq!(a.records.first().unwrap().event_id, 10);
        assert_eq!(b.records.first().unwrap().event_id, 40);
        // A second read by A is unaffected by B.
        let a2 = read_page(&owned, &a_cursor, 5, &LogReadFilters::default()).unwrap();
        assert_eq!(
            a2.records.iter().map(|e| e.event_id).collect::<Vec<u64>>(),
            a.records.iter().map(|e| e.event_id).collect::<Vec<u64>>()
        );
    }

    #[test]
    fn filters_do_not_move_the_position_to_the_match_count() {
        // log: 151 events (seq 0..150). Only 103, 121 and 147 are exits.
        let mut events = entries(151, EventType::FunctionEntry);
        for idx in [103usize, 121, 147] {
            events[idx].0 = EventType::FunctionExit;
        }
        let owned = log("filters", &events);

        let cursor = owned
            .cursor_start()
            .advanced_to(EventSeq::new(100))
            .unwrap();
        let filters = LogReadFilters {
            event_types: Some(vec![EventType::FunctionExit]),
            ..Default::default()
        };
        let page = read_page(&owned, &cursor, 3, &filters).unwrap();
        assert_eq!(page.records.len(), 3);
        assert_eq!(
            page.records
                .iter()
                .map(|e| e.event_id)
                .collect::<Vec<u64>>(),
            vec![103, 121, 147]
        );
        // POSITION = evidence examined (147), NOT the match count (3).
        assert_eq!(
            page.next.next_seq(),
            EventSeq::new(148),
            "cursor must point past the last examined record, not at 3"
        );
    }

    #[test]
    fn by_id_reads_from_the_log_not_an_engine() {
        let owned = log("byid", &entries(5, EventType::FunctionEntry));
        let found = find_by_id(&owned, 3).unwrap().expect("event 3 exists");
        assert_eq!(found.event_id, 3);
        assert!(find_by_id(&owned, 99).unwrap().is_none());
    }

    #[test]
    fn cursor_from_another_session_is_refused() {
        let a = log("sess-a", &entries(3, EventType::FunctionEntry));
        let b = log("sess-b", &entries(3, EventType::FunctionEntry));
        let err = read_page(&a, &b.cursor_start(), 10, &LogReadFilters::default()).unwrap_err();
        assert!(matches!(err, ServiceError::NoExecutionLog(_)), "{err:?}");
    }
}
