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
use chronos_log::{EventSeq, ExecutionRecord, Gap, LogPage, TailState};

use serde::{Deserialize, Serialize};

use crate::error::ServiceError;
use crate::events_cursor::EventsCursorV1;
use crate::session_log::SessionExecutionLog;

/// How complete the evidence in the examined range is.
///
/// ```text
/// Complete      the backend can demonstrate that no evidence is missing
///               inside the examined range
/// GapDetected   one or more explicit gaps intersect the examined range
/// Partial       loss is known but cannot be delimited to a range
/// Unknown       neither completeness nor loss can be demonstrated
/// Unsupported   reserved for a source that cannot evaluate completeness at all
/// ```
///
/// REC-C1.4 produces only `Complete`, `GapDetected` and `Unknown`. `Partial`
/// and `Unsupported` exist because future sources will need them, and inventing
/// a producer now just to justify the enum would be the opposite of the point.
///
/// ## The central rule
///
/// ```text
/// absence_of_known_gap != proof_of_completeness
/// ```
///
/// A range is `Complete` only when the log can demonstrate continuity for it.
/// For the ExecutionLog that proof is structural: `append` assigns dense seqs,
/// so inside the allocated range (`<= tail_seq`) a missing seq can only come
/// from a recorded gap. Outside the allocated range, or when the log cannot
/// report its own tail, there is no proof → `Unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Completeness {
    Complete,
    GapDetected,
    Partial,
    Unknown,
    Unsupported,
}

impl Completeness {
    pub fn as_str(self) -> &'static str {
        match self {
            Completeness::Complete => "complete",
            Completeness::GapDetected => "gap_detected",
            Completeness::Partial => "partial",
            Completeness::Unknown => "unknown",
            Completeness::Unsupported => "unsupported",
        }
    }
}

/// A completeness verdict, always scoped to an explicit range.
///
/// "Complete" without saying *of what* is ambiguous: a live session keeps
/// producing events, yet the range just examined can be fully observed. The
/// scope is therefore part of the answer, and pagination is orthogonal to it —
/// `Complete` with more evidence after the range is perfectly coherent, which is
/// what `next_cursor` expresses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletenessReport {
    pub status: Completeness,
    /// Always `"examined_range"` today; named so a future scope cannot be
    /// confused with this one.
    pub scope: &'static str,
    pub from_seq: u64,
    /// Exclusive upper bound of the examined range.
    pub to_seq_exclusive: u64,
}

impl CompletenessReport {
    pub const SCOPE_EXAMINED_RANGE: &'static str = "examined_range";

    pub fn is_complete(&self) -> bool {
        self.status == Completeness::Complete
    }

    /// The scope name this report refers to (serialized as `scope`).
    pub fn scope_name(&self) -> &'static str {
        self.scope
    }
}

impl<'de> serde::Deserialize<'de> for CompletenessReport {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        struct Raw {
            status: String,
            #[serde(default)]
            scope: String,
            from_seq: u64,
            to_seq_exclusive: u64,
        }
        let raw = Raw::deserialize(de)?;
        // An unknown scope must not be silently reinterpreted as this one.
        if !raw.scope.is_empty() && raw.scope != CompletenessReport::SCOPE_EXAMINED_RANGE {
            return Err(serde::de::Error::custom(format!(
                "unsupported completeness scope {:?}",
                raw.scope
            )));
        }
        let status = match raw.status.as_str() {
            "complete" => Completeness::Complete,
            "gap_detected" => Completeness::GapDetected,
            "partial" => Completeness::Partial,
            "unsupported" => Completeness::Unsupported,
            _ => Completeness::Unknown,
        };
        Ok(CompletenessReport {
            status,
            scope: CompletenessReport::SCOPE_EXAMINED_RANGE,
            from_seq: raw.from_seq,
            to_seq_exclusive: raw.to_seq_exclusive,
        })
    }
}

impl serde::Serialize for CompletenessReport {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = ser.serialize_struct("CompletenessReport", 4)?;
        st.serialize_field("status", self.status.as_str())?;
        st.serialize_field("scope", self.scope)?;
        st.serialize_field("from_seq", &self.from_seq)?;
        st.serialize_field("to_seq_exclusive", &self.to_seq_exclusive)?;
        st.end()
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
            if !types.contains(&r.event_type) {
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
    /// The next position, same vocabulary as `LogPage::position_after` and
    /// `EventsCursorV1::next_seq`: `next.next_seq() == position_after`. Named for
    /// what it is — the position AFTER the last examined record — not "highest
    /// examined", which was an off-by-one waiting to happen.
    pub position_after: EventSeq,
    /// Gaps the read passed over, for C1.4 to interpret.
    pub gaps: Vec<Gap>,
    /// Verdict for the range this read examined, with its scope.
    pub completeness: CompletenessReport,
    /// REC-C1.6: retention facts for this log. Always populated so the agent
    /// can reason about `retained_from_seq` without a separate call.
    pub retention: RetentionFacts,
    /// REC-C1.6: tail facts for this log. `state` is always populated; `tail_seq`
    /// is `None` iff `state == Unknown` (no inference, no fabrication).
    pub tail: TailFacts,
}

/// REC-C1.6: retention facts for a log, surfaced on every `events_read` page.
///
/// These are FACTS about the current durable boundary — not a retention policy.
/// Policy decisions (when to retire, what to retire) live outside the read
/// path; this struct tells the agent exactly what survives today.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionFacts {
    /// The earliest seq the log can serve. A cursor before this is `CursorStale`,
    /// not evidence loss.
    pub retained_from_seq: u64,
    /// True iff `retained_from_seq > 0`, i.e. some history was already retired.
    /// The agent can use this without subtracting from a zero baseline.
    pub history_truncated: bool,
}

impl RetentionFacts {
    /// Build from the log's authoritative retention boundary.
    pub fn from_retained_from(retained_from: EventSeq) -> Self {
        RetentionFacts {
            retained_from_seq: retained_from.0,
            history_truncated: retained_from.0 > 0,
        }
    }
}

/// REC-C1.6: tail facts for a log, surfaced on every `events_read` page.
///
/// Distinct from [`chronos_log::TailState`]: that enum carries lifecycle
/// provenance (sealed-at timestamps, unclean reasons). The wire form is
/// intentionally flat — agent reasoning needs the state NAME plus the tail
/// position; deeper provenance is recovered via the lifecycle APIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TailStateWire {
    /// The run is still in progress in this process.
    Open,
    /// The run ended through an explicit, durable seal.
    Sealed,
    /// Positive evidence that the previous run did not end cleanly.
    Unclean,
    /// No proof either way (legacy metadata, incomplete external metadata).
    Unknown,
}

impl TailStateWire {
    /// Flatten the underlying state name into the wire enum. State-specific
    /// fields (sealed-at, unclean reasons) are NOT carried here — they live in
    /// the lifecycle/provenance surface, not on every read page.
    pub fn from_log_state(state: &TailState) -> Self {
        match state {
            TailState::Open => TailStateWire::Open,
            TailState::Sealed { .. } => TailStateWire::Sealed,
            TailState::Unclean { .. } => TailStateWire::Unclean,
            TailState::Unknown { .. } => TailStateWire::Unknown,
        }
    }
}

/// REC-C1.6: tail facts for a log, surfaced on every `events_read` page.
///
/// `tail_seq` is `None` iff `state == Unknown`. We do not infer a seq we
/// cannot prove — that would invent a false tail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TailFacts {
    pub state: TailStateWire,
    pub tail_seq: Option<u64>,
}

impl TailFacts {
    /// Build from the log's authoritative tail state and tail seq.
    ///
    /// `tail_seq` is dropped to `None` when `state == Unknown`. The log may
    /// still report a tail seq for unknown-state runs (legacy metadata), but
    /// the wire form refuses to present an unproven tail as if it were proven.
    pub fn from_log_tail(state: &TailState, tail_seq: Option<EventSeq>) -> Self {
        let wire = TailStateWire::from_log_state(state);
        let tail_seq = match (wire, tail_seq) {
            // Honest `Unknown` → no tail_seq. Even if the log happens to know
            // a number for a legacy run, the wire form refuses to present it
            // as "the tail".
            (TailStateWire::Unknown, _) => None,
            (_, seq) => seq.map(|s| s.0),
        };
        TailFacts {
            state: wire,
            tail_seq,
        }
    }
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
    let session_id = log.session_id().as_str().to_string();
    read_page_with(
        &session_id,
        cursor,
        limit,
        filters,
        handle.retained_from(),
        handle.tail_seq(),
        &handle.tail_state(),
        |position, chunk| handle.read_from_seq(position, chunk).map_err(map_log_error),
    )
}

/// The read loop, generic over the batch reader so its contract can be tested
/// against a deliberately misbehaving backend.
///
/// Contract enforced here:
///
/// ```text
/// page.exhausted == true                                  -> stop normally
/// page.exhausted == false && position_after > start       -> continue
/// page.exhausted == false && position_after <= start      -> EvidenceReadStalled
/// ```
#[allow(clippy::too_many_arguments)]
pub(crate) fn read_page_with<F>(
    session_id: &str,
    cursor: &EventsCursorV1,
    limit: usize,
    filters: &LogReadFilters,
    retained_from: EventSeq,
    tail_seq: Option<EventSeq>,
    tail_state: &TailState,
    mut read: F,
) -> Result<LogReadPage, ServiceError>
where
    F: FnMut(EventSeq, usize) -> Result<LogPage, ServiceError>,
{
    let mut position = cursor.next_seq();
    let mut matched: Vec<TraceEvent> = Vec::new();
    let mut gaps: Vec<Gap> = Vec::new();

    while matched.len() < limit {
        let start_position = position;
        let page = read(position, SCAN_CHUNK)?;
        gaps.extend(page.gaps.iter().cloned());

        let mut stopped_early = false;
        for record in &page.records {
            if matched.len() >= limit {
                stopped_early = true;
                break;
            }
            // A record we cannot decode is NOT skippable: advancing past it would
            // drop evidence and hand the agent a cursor that pretends the read was
            // complete. Fail closed and emit no new cursor.
            let event = decode(record).ok_or_else(|| ServiceError::EvidenceDecodeFailed {
                session_id: session_id.to_string(),
                seq: record.seq.0,
                payload_tag: payload_tag(record),
            })?;
            // The position advances exactly to the last record actually examined,
            // never to the end of the inner chunk.
            position = EventSeq::new(record.seq.0 + 1);
            if filters.matches(&event) {
                matched.push(event);
            }
        }

        if stopped_early || matched.len() >= limit {
            break;
        }
        if page.exhausted {
            position = page.position_after;
            break;
        }
        if page.position_after > position {
            // Consumed the whole chunk: jump to its end. This is what carries the
            // reader over gaps, and keeps SCAN_CHUNK an internal detail.
            position = page.position_after;
        }
        if position <= start_position {
            // "More data" with no forward progress is a backend contract
            // violation. Returning the same cursor would loop forever.
            return Err(ServiceError::EvidenceReadStalled {
                session_id: session_id.to_string(),
                position: position.0,
            });
        }
    }

    // Only gaps the reader actually passed over belong to this page.
    gaps.retain(|g| g.last_missing.0 < position.0);
    gaps.sort_by_key(|g| g.first_missing.0);
    gaps.dedup_by_key(|g| g.first_missing.0);

    let next = cursor
        .clone()
        .advanced_to(position)
        .map_err(|_| ServiceError::InvalidCursorPayload)?;

    let from_seq = cursor.next_seq();
    let completeness = completeness_for(session_id, from_seq, position, &gaps, tail_seq);

    // REC-C1.6: retention/tail facts surfaced on every page.
    let retention = RetentionFacts::from_retained_from(retained_from);
    let tail = TailFacts::from_log_tail(tail_state, tail_seq);

    Ok(LogReadPage {
        records: matched,
        next,
        position_after: position,
        gaps,
        completeness,
        retention,
        tail,
    })
}

/// Decide the completeness verdict for `[from_seq, to_seq_exclusive)`.
///
/// Only gaps that INTERSECT the examined range contaminate it: a gap before the
/// cursor or one that starts after the position we stopped at says nothing about
/// the evidence actually used.
pub(crate) fn completeness_for(
    _session_id: &str,
    from_seq: EventSeq,
    to_seq_exclusive: EventSeq,
    gaps: &[Gap],
    tail: Option<EventSeq>,
) -> CompletenessReport {
    let report = |status: Completeness| CompletenessReport {
        status,
        scope: CompletenessReport::SCOPE_EXAMINED_RANGE,
        from_seq: from_seq.0,
        to_seq_exclusive: to_seq_exclusive.0,
    };

    // An empty examined range used no evidence, so no gap can contaminate it.
    let range_is_empty = to_seq_exclusive.0 <= from_seq.0;
    let intersects = |g: &Gap| {
        !range_is_empty && g.first_missing.0 < to_seq_exclusive.0 && g.last_missing.0 >= from_seq.0
    };
    if gaps.iter().any(intersects) {
        return report(Completeness::GapDetected);
    }

    // Continuity proof: inside the allocated range the seq space is dense by
    // construction, so the absence of a recorded gap IS the proof. Without a
    // known tail there is nothing to prove against.
    match tail {
        Some(tail) if to_seq_exclusive.0 <= tail.0 + 1 => report(Completeness::Complete),
        _ => report(Completeness::Unknown),
    }
}

/// The producer-declared payload tag, for diagnostics on decode failure.
pub(crate) fn payload_tag(record: &ExecutionRecord) -> String {
    record.payload.tag.clone()
}

/// Map a log error onto the service surface.
///
/// Retention is NOT evidence loss: a cursor before the boundary becomes a typed
/// `CursorStale` carrying both numbers, so the caller can re-anchor deliberately
/// instead of being silently moved.
fn map_log_error(e: chronos_log::LogError) -> ServiceError {
    match e {
        chronos_log::LogError::PositionBeforeRetention {
            requested_next_seq,
            retained_from,
        } => ServiceError::CursorStale {
            requested_next_seq: requested_next_seq.0,
            retained_from_seq: retained_from.0,
        },
        other => ServiceError::DrainFailed(format!("{other:?}")),
    }
}

/// Decode a log record's payload into a `TraceEvent`.
///
/// The ExecutionLog payload is JSON-encoded `TraceEvent` (the m1-03 producer
/// contract); a record that does not decode is counted separately rather than
/// silently dropped, so the bytes stay durable and the anomaly is visible.
///
/// `pub(crate)` so the projection builder (`crate::projection::build_engine`)
/// shares the same decoder and any drift between the two paths is impossible.
pub(crate) fn decode(record: &ExecutionRecord) -> Option<TraceEvent> {
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
    let session_id = log.session_id().as_str().to_string();
    // Scan only the RETAINED region: starting at 0 would be refused by the
    // watermark, and the retired range is not evidence we may return from.
    let result = find_by_id_with(
        &session_id,
        event_id,
        log.retained_from(),
        |position, chunk| handle.read_from_seq(position, chunk).map_err(map_log_error),
    )?;
    if result.is_some() {
        return Ok(result);
    }
    // Not found in the retained region. If history has been retired, the event
    // may have lived there, and `None` would claim the whole session was
    // searched. Refusing is the honest answer.
    let retained = log.retained_from();
    if retained > EventSeq::ZERO {
        return Err(ServiceError::EvidenceUnavailableDueToRetention {
            retained_from: retained.0,
        });
    }
    Ok(None)
}

/// Same batch contract as [`read_page_with`], applied to a by-id scan.
///
/// Without the stall check a misbehaving backend could make this loop forever.
pub(crate) fn find_by_id_with<F>(
    session_id: &str,
    event_id: u64,
    from_seq: EventSeq,
    mut read: F,
) -> Result<Option<TraceEvent>, ServiceError>
where
    F: FnMut(EventSeq, usize) -> Result<LogPage, ServiceError>,
{
    let mut position = from_seq;
    loop {
        let start_position = position;
        let page = read(position, SCAN_CHUNK)?;
        if page.records.is_empty() && page.exhausted {
            return Ok(None);
        }
        for record in &page.records {
            // Same fail-closed rule as `read_page`: a record we cannot interpret
            // is not the same as "the event is not here".
            let event = decode(record).ok_or_else(|| ServiceError::EvidenceDecodeFailed {
                session_id: session_id.to_string(),
                seq: record.seq.0,
                payload_tag: payload_tag(record),
            })?;
            if event.event_id == event_id {
                return Ok(Some(event));
            }
        }
        if page.exhausted {
            return Ok(None);
        }
        if page.position_after > position {
            position = page.position_after;
        }
        if position <= start_position {
            return Err(ServiceError::EvidenceReadStalled {
                session_id: session_id.to_string(),
                position: position.0,
            });
        }
    }
}

#[cfg(test)]
mod rec_c1_3_tests {
    //! Mandatory C1.3 properties: seq#0, exact page boundary, exact resume,
    //! independent readers, and filters that never move the position.

    use super::*;
    use chronos_domain::{EventType, SourceLocation};
    use chronos_log::{ExecutionPayload, NewExecutionRecord, SessionId};

    pub(super) fn log(tag: &str, events: &[(EventType, u64, u32)]) -> SessionExecutionLog {
        let dir = std::env::temp_dir().join(format!(
            "rec-c1-3-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let owned = SessionExecutionLog::create(&dir, SessionId::new(tag)).expect("log");
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
                    payload: ExecutionPayload::new(
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

    /// Append one decodable event at an explicit id/seq position.
    pub(super) fn push(
        handle: &std::sync::Arc<chronos_log::SegmentedExecutionLog>,
        i: u64,
        event_type: EventType,
    ) {
        let event = TraceEvent::new(
            i,
            i * 10,
            i,
            event_type,
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
                monotonic_ns: i * 10,
                payload: ExecutionPayload::new(serde_json::to_vec(&event).unwrap(), "trace_event"),
                invocation_id: None,
                parent_invocation_id: None,
                symbol_id: None,
            })
            .expect("append");
    }

    pub(super) fn tmpdir(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "rec-c1-4-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    /// Log with records 0, then a gap 1..=3, then record 4.
    pub(super) fn gappy_owned() -> (SessionExecutionLog, std::path::PathBuf) {
        use chronos_log::{Gap, GapReason};
        let dir = tmpdir("gappy");
        let owned = SessionExecutionLog::create(&dir, SessionId::new("gappy")).expect("log");
        let handle = owned.handle();
        push(&handle, 0, EventType::FunctionEntry);
        handle
            .record_gap(Gap::new(
                EventSeq::new(1),
                EventSeq::new(3),
                GapReason::AdapterBufferOverflow,
                "test",
            ))
            .expect("gap");
        push(&handle, 4, EventType::FunctionEntry);
        handle.flush().ok();
        (owned, dir)
    }

    pub(super) fn entries(n: usize, event_type: EventType) -> Vec<(EventType, u64, u32)> {
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
        // The whole log was examined and the log knows its tail (3 records, no
        // gaps), so this range is provably complete.
        assert_eq!(page.completeness.status, Completeness::Complete);
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
    fn t1_limit_larger_than_one_inner_chunk_is_honoured() {
        // SCAN_CHUNK is an internal detail (512). A caller asking for 1000 must
        // receive 1000, not one chunk's worth.
        let owned = log("t1", &entries(1200, EventType::FunctionEntry));
        let page = read_page(
            &owned,
            &owned.cursor_start(),
            1000,
            &LogReadFilters::default(),
        )
        .unwrap();
        assert_eq!(
            page.records.len(),
            1000,
            "limit must not be capped by SCAN_CHUNK"
        );
        assert_eq!(
            page.records
                .iter()
                .map(|e| e.event_id)
                .collect::<Vec<u64>>(),
            (0..1000).collect::<Vec<u64>>()
        );
        assert_eq!(page.next.next_seq(), EventSeq::new(1000));
        assert_eq!(page.position_after, EventSeq::new(1000));
    }

    #[test]
    fn t2_matches_beyond_the_first_chunk_are_found() {
        // Matches at 103 (chunk 1), 721 and 1147 (later chunks).
        let mut events = entries(1200, EventType::FunctionEntry);
        for idx in [103usize, 721, 1147] {
            events[idx].0 = EventType::FunctionExit;
        }
        let owned = log("t2", &events);
        let filters = LogReadFilters {
            event_types: Some(vec![EventType::FunctionExit]),
            ..Default::default()
        };
        let page = read_page(&owned, &owned.cursor_start(), 3, &filters).unwrap();
        assert_eq!(
            page.records
                .iter()
                .map(|e| e.event_id)
                .collect::<Vec<u64>>(),
            vec![103, 721, 1147],
            "the scan must continue past the first chunk"
        );
        assert_eq!(page.next.next_seq(), EventSeq::new(1148));
    }

    #[test]
    fn t3_scan_continues_after_a_gap_between_chunks() {
        use chronos_log::{Gap, GapReason};
        // Realistic gap shape: records up to 599, a recorded gap 600..=700, then
        // the producer continues at 701. (Appending a gap AFTER later records
        // would put it out of seq order in the entry list; real producers cannot
        // do that, and both this reader and `read_after` assume seq order.)
        let dir = std::env::temp_dir().join(format!(
            "rec-c1-3-t3-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let owned = SessionExecutionLog::create(&dir, SessionId::new("t3")).expect("log");
        let handle = owned.handle();
        for i in 0..600u64 {
            push(&handle, i, EventType::FunctionEntry);
        }
        handle
            .record_gap(Gap::new(
                EventSeq::new(600),
                EventSeq::new(700),
                GapReason::AdapterBufferOverflow,
                "test",
            ))
            .expect("gap");
        for i in 701..1400u64 {
            push(&handle, i, EventType::FunctionEntry);
        }
        handle.flush().ok();

        // Chunk 1 ends at 511, so both the gap and everything after it live in
        // later chunks: the scan must keep going.
        let page = read_page(
            &owned,
            &owned.cursor_start(),
            1000,
            &LogReadFilters::default(),
        )
        .unwrap();
        assert_eq!(
            page.records.len(),
            1000,
            "records after the gap are reachable"
        );
        assert!(
            page.position_after > EventSeq::new(700),
            "position must clear the gap, got {:?}",
            page.position_after
        );
        assert_eq!(page.gaps.len(), 1, "the gap is reported, not hidden");
        assert_eq!(page.gaps[0].last_missing, EventSeq::new(700));
    }

    #[test]
    fn an_undecodable_record_fails_closed_and_emits_no_cursor() {
        let mut events = entries(5, EventType::FunctionEntry);
        events[2].1 = 20;
        let owned = log("badpayload", &events);
        let handle = owned.handle();
        // Poison one record's payload.
        handle
            .append(NewExecutionRecord {
                session_id: handle.session_id().clone(),
                monotonic_ns: 999,
                payload: ExecutionPayload::new(b"not-json-at-all".to_vec(), "unknown_producer"),
                invocation_id: None,
                parent_invocation_id: None,
                symbol_id: None,
            })
            .expect("append");
        handle.flush().ok();

        let err = read_page(
            &owned,
            &owned.cursor_start(),
            50,
            &LogReadFilters::default(),
        )
        .unwrap_err();
        match err {
            ServiceError::EvidenceDecodeFailed {
                seq, payload_tag, ..
            } => {
                assert_eq!(seq, 5);
                assert_eq!(payload_tag, "unknown_producer");
            }
            other => panic!("expected EvidenceDecodeFailed, got {other:?}"),
        }

        // `find_by_id` follows the same rule: an unreadable record is not the
        // same as "the event is not here".
        let err = find_by_id(&owned, 9999).unwrap_err();
        assert!(matches!(err, ServiceError::EvidenceDecodeFailed { .. }));
    }

    #[test]
    fn by_id_reads_from_the_log_not_an_engine() {
        let owned = log("byid", &entries(5, EventType::FunctionEntry));
        let found = find_by_id(&owned, 3).unwrap().expect("event 3 exists");
        assert_eq!(found.event_id, 3);
        assert!(find_by_id(&owned, 99).unwrap().is_none());
    }

    #[test]
    fn c15_by_id_under_retention_never_claims_not_found() {
        use chronos_log::SessionId as LogSessionId;
        let dir = tmpdir("byid-retention");
        let owned = SessionExecutionLog::create(&dir, LogSessionId::new("byid-ret")).expect("log");
        let handle = owned.handle();
        // Two segments, so the first ten seqs can be retired as a whole unit.
        for i in 0..10u64 {
            push(&handle, i, EventType::FunctionEntry);
        }
        handle.flush().ok();
        for i in 10..20u64 {
            push(&handle, i, EventType::FunctionEntry);
        }
        handle.flush().ok();
        let retired = handle
            .retain_up_to(EventSeq::new(9))
            .expect("retire the first ten seqs");
        assert!(retired.retained_from > EventSeq::ZERO, "retention happened");

        // An id inside the retained range is found normally.
        assert!(find_by_id(&owned, 15).unwrap().is_some());
        // An id that could have lived in the retired range must NOT be reported
        // as absent: we cannot claim the whole session was searched.
        let err = find_by_id(&owned, 12345).unwrap_err();
        match err {
            ServiceError::EvidenceUnavailableDueToRetention { retained_from } => {
                assert_eq!(retained_from, owned.retained_from().0);
            }
            other => panic!("expected EvidenceUnavailableDueToRetention, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cursor_from_another_session_is_refused() {
        let a = log("sess-a", &entries(3, EventType::FunctionEntry));
        let b = log("sess-b", &entries(3, EventType::FunctionEntry));
        let err = read_page(&a, &b.cursor_start(), 10, &LogReadFilters::default()).unwrap_err();
        assert!(matches!(err, ServiceError::NoExecutionLog(_)), "{err:?}");
    }
}

/// REC-C1.3 — negative tests against a deliberately misbehaving backend.
///
/// The batch reader is injected so the reader's *contract* can be exercised:
/// `exhausted == false` with no forward progress must be an error, never a
/// normal page that hands the same cursor back (which would loop forever in
/// `find_by_id`).
#[cfg(test)]
mod rec_c1_3_stall_tests {
    use super::rec_c1_3_tests::{entries, gappy_owned, log, push, tmpdir};
    use super::*;
    use chronos_log::{LogPage, SessionId};

    fn faulty_reader() -> impl FnMut(EventSeq, usize) -> Result<LogPage, ServiceError> + Copy {
        |position, _chunk| {
            // Claims "more data" while never advancing the position.
            Ok(LogPage {
                records: Vec::new(),
                gaps: Vec::new(),
                position_after: position,
                exhausted: false,
            })
        }
    }

    // ---- C1.4 completeness DoD -------------------------------------------

    #[test]
    fn c14_continuous_range_is_complete() {
        let owned = log("c14-cont", &entries(10, EventType::FunctionEntry));
        let page = read_page(&owned, &owned.cursor_start(), 5, &LogReadFilters::default()).unwrap();
        assert_eq!(page.completeness.status, Completeness::Complete);
        assert_eq!(page.completeness.scope, "examined_range");
        assert_eq!(page.completeness.from_seq, 0);
        assert_eq!(page.completeness.to_seq_exclusive, 5);
        // Orthogonality: Complete while more evidence exists after the range.
        assert!(page.next.next_seq() < EventSeq::new(10));
    }

    #[test]
    fn c14_gap_inside_the_range_is_gap_detected_with_an_exact_range() {
        let (owned, _) = gappy_owned();
        let page = read_page(
            &owned,
            &owned.cursor_start(),
            10,
            &LogReadFilters::default(),
        )
        .unwrap();
        assert_eq!(page.completeness.status, Completeness::GapDetected);
        assert_eq!(page.completeness.from_seq, 0);
        assert_eq!(page.completeness.to_seq_exclusive, page.position_after.0);
        assert_eq!(page.gaps.len(), 1);
    }

    #[test]
    fn c14_gap_before_the_cursor_does_not_contaminate_the_page() {
        let (owned, _) = gappy_owned();
        // The gap is 1..=3; start well past it.
        let cursor = owned.cursor_start().advanced_to(EventSeq::new(4)).unwrap();
        let page = read_page(&owned, &cursor, 10, &LogReadFilters::default()).unwrap();
        assert_eq!(
            page.completeness.status,
            Completeness::Complete,
            "a gap already behind the cursor says nothing about this range"
        );
        assert!(page.gaps.is_empty());
    }

    #[test]
    fn c14_gap_after_the_stop_point_does_not_contaminate_yet() {
        let (owned, _) = gappy_owned();
        // Stop before reaching the gap (which lives at 1..=3): limit 1.
        let page = read_page(&owned, &owned.cursor_start(), 1, &LogReadFilters::default()).unwrap();
        assert_eq!(page.position_after, EventSeq::new(1));
        assert_eq!(
            page.completeness.status,
            Completeness::Complete,
            "the gap is ahead of the examined range"
        );
    }

    #[test]
    fn c14_filters_that_span_a_gap_are_gap_detected() {
        // Matches at seq 0 and 5, with a gap 1..=3 between them.
        let (owned, _) = gappy_owned();
        let filters = LogReadFilters::default();
        let page = read_page(&owned, &owned.cursor_start(), 2, &filters).unwrap();
        assert_eq!(
            page.records
                .iter()
                .map(|e| e.event_id)
                .collect::<Vec<u64>>(),
            vec![0, 4],
            "both requested results exist"
        );
        assert_eq!(
            page.completeness.status,
            Completeness::GapDetected,
            "getting `limit` results does NOT mean the evidence used was complete"
        );
    }

    #[test]
    fn c14_two_gaps_are_reported_ordered_and_not_collapsed() {
        use chronos_log::{Gap, GapReason};
        let dir = tmpdir("c14-two-gaps");
        let owned = SessionExecutionLog::create(&dir, SessionId::new("two-gaps")).expect("log");
        let handle = owned.handle();
        for i in 0..6u64 {
            push(&handle, i, EventType::FunctionEntry);
        }
        handle
            .record_gap(Gap::new(
                EventSeq::new(6),
                EventSeq::new(7),
                GapReason::AdapterBufferOverflow,
                "a",
            ))
            .unwrap();
        for i in 8..12u64 {
            push(&handle, i, EventType::FunctionEntry);
        }
        handle
            .record_gap(Gap::new(
                EventSeq::new(12),
                EventSeq::new(14),
                GapReason::KernelRingOverflow,
                "b",
            ))
            .unwrap();
        for i in 15..20u64 {
            push(&handle, i, EventType::FunctionEntry);
        }
        handle.flush().ok();

        let page = read_page(
            &owned,
            &owned.cursor_start(),
            100,
            &LogReadFilters::default(),
        )
        .unwrap();
        assert_eq!(page.gaps.len(), 2, "both gaps reported: {:?}", page.gaps);
        assert!(
            page.gaps[0].first_missing.0 < page.gaps[1].first_missing.0,
            "gaps ordered by position"
        );
        assert_eq!(page.completeness.status, Completeness::GapDetected);
    }

    #[test]
    fn c14_no_proof_available_is_unknown_never_complete() {
        // A log that cannot report its own tail cannot demonstrate continuity.
        let report = completeness_for("s", EventSeq::new(0), EventSeq::new(5), &[], None);
        assert_eq!(report.status, Completeness::Unknown);
    }

    #[test]
    fn c14_invariant_gaps_in_range_never_yield_complete() {
        use chronos_log::{Gap, GapReason};
        // Small exhaustive sweep of gap placements against every examined range.
        for gap_first in 0..6u64 {
            for gap_last in gap_first..6u64 {
                let gaps = vec![Gap::new(
                    EventSeq::new(gap_first),
                    EventSeq::new(gap_last),
                    GapReason::CorruptSegment,
                    "t",
                )];
                for from in 0..8u64 {
                    for to in from..8u64 {
                        let r = completeness_for(
                            "s",
                            EventSeq::new(from),
                            EventSeq::new(to),
                            &gaps,
                            Some(EventSeq::new(9)),
                        );
                        // Empty ranges intersect nothing, so the comparison must
                        // be driven by the intersection predicate itself.
                        let intersects = to > from && gap_first < to && gap_last >= from;
                        if intersects {
                            assert_ne!(
                                r.status,
                                Completeness::Complete,
                                "gap {gap_first}..={gap_last} intersects [{from},{to})"
                            );
                        }
                        assert_eq!(
                            r.status == Completeness::GapDetected,
                            intersects,
                            "verdict must match intersection for [{from},{to})"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn c14_only_complete_gapdetected_and_unknown_are_produced() {
        let owned = log("c14-set", &entries(3, EventType::FunctionEntry));
        let page = read_page(
            &owned,
            &owned.cursor_start(),
            10,
            &LogReadFilters::default(),
        )
        .unwrap();
        assert!(matches!(
            page.completeness.status,
            Completeness::Complete | Completeness::GapDetected | Completeness::Unknown
        ));
        assert_ne!(page.completeness.status, Completeness::Partial);
        assert_ne!(page.completeness.status, Completeness::Unsupported);
    }

    #[test]
    fn t4_read_page_stalls_loudly_and_emits_no_cursor() {
        let cursor = EventsCursorV1::start(chronos_log::SessionId::new("stall"));
        let err = read_page_with(
            "stall",
            &cursor,
            10,
            &LogReadFilters::default(),
            EventSeq::ZERO,
            Some(EventSeq::new(0)),
            &TailState::Open,
            faulty_reader(),
        )
        .unwrap_err();
        match err {
            ServiceError::EvidenceReadStalled {
                session_id,
                position,
            } => {
                assert_eq!(session_id, "stall");
                assert_eq!(position, 0);
            }
            other => panic!("expected EvidenceReadStalled, got {other:?}"),
        }
    }

    #[test]
    fn t5_find_by_id_stalls_loudly_and_never_loops() {
        let err = find_by_id_with("stall", 42, EventSeq::ZERO, faulty_reader()).unwrap_err();
        assert!(
            matches!(err, ServiceError::EvidenceReadStalled { .. }),
            "{err:?}"
        );
    }

    #[test]
    fn an_exhausted_backend_is_still_a_normal_empty_result() {
        // `exhausted == true` is the legitimate "caught up" answer and must not
        // be turned into an error.
        let cursor = EventsCursorV1::start(chronos_log::SessionId::new("ok"));
        let page = read_page_with(
            "ok",
            &cursor,
            10,
            &LogReadFilters::default(),
            EventSeq::ZERO,
            Some(EventSeq::new(0)),
            &TailState::Open,
            |position, _| Ok(LogPage::empty_at(position)),
        )
        .unwrap();
        assert!(page.records.is_empty());
        assert_eq!(page.next.next_seq(), EventSeq::ZERO);
    }
}

/// REC-C1.6 — `RetentionFacts` and `TailFacts` show up on every page.
///
/// These tests pin the wire invariants the agent relies on:
/// - `retained_from_seq` is the authoritative boundary, NOT a policy.
/// - `history_truncated == true` iff some seqs were already retired.
/// - `tail.state` is always populated.
/// - `tail.tail_seq` is `None` iff `state == Unknown` (no inference).
/// - `TailStateWire` serializes as snake_case strings: open / sealed /
///   unclean / unknown.
#[cfg(test)]
mod rec_c1_6_wire_facts_tests {
    use super::*;
    use chronos_log::{LogPage, SessionId};

    /// A reader that always returns the same empty exhausted page. Sufficient
    /// to exercise the retention/tail fact construction in `read_page_with`
    /// without depending on a real ExecutionLog.
    fn empty_exhausted_reader(
    ) -> impl FnMut(EventSeq, usize) -> Result<LogPage, ServiceError> + Copy {
        |position, _| Ok(LogPage::empty_at(position))
    }

    fn cursor_start() -> EventsCursorV1 {
        EventsCursorV1::start(SessionId::new("wire-facts"))
    }

    // ---- WIRE-RET-1 / 2: retention facts -------------------------------

    #[test]
    fn wire_ret_1_zero_retained_from_is_not_truncated() {
        let page = read_page_with(
            "wire-facts",
            &cursor_start(),
            10,
            &LogReadFilters::default(),
            EventSeq::ZERO,
            Some(EventSeq::new(0)),
            &TailState::Open,
            empty_exhausted_reader(),
        )
        .unwrap();
        assert_eq!(page.retention.retained_from_seq, 0);
        assert!(!page.retention.history_truncated);
    }

    #[test]
    fn wire_ret_2_nonzero_retained_from_marks_truncated() {
        // A log whose first 5 seqs were retired. The cursor points past the
        // boundary; the read returns no records, but the wire form MUST show
        // that the boundary is non-zero, so the agent can tell why a fresh
        // read from seq#0 would be refused with CursorStale.
        let page = read_page_with(
            "wire-facts",
            &cursor_start(),
            10,
            &LogReadFilters::default(),
            EventSeq::new(5),
            Some(EventSeq::new(5)),
            &TailState::Open,
            empty_exhausted_reader(),
        )
        .unwrap();
        assert_eq!(page.retention.retained_from_seq, 5);
        assert!(page.retention.history_truncated);
    }

    // ---- WIRE-TAIL-1 / 2 / 3: tail facts -------------------------------

    #[test]
    fn wire_tail_1_open_state_carries_tail_seq() {
        let page = read_page_with(
            "wire-facts",
            &cursor_start(),
            10,
            &LogReadFilters::default(),
            EventSeq::ZERO,
            Some(EventSeq::new(42)),
            &TailState::Open,
            empty_exhausted_reader(),
        )
        .unwrap();
        assert_eq!(page.tail.state, TailStateWire::Open);
        assert_eq!(page.tail.tail_seq, Some(42));
    }

    #[test]
    fn wire_tail_2_sealed_state_carries_tail_seq() {
        let page = read_page_with(
            "wire-facts",
            &cursor_start(),
            10,
            &LogReadFilters::default(),
            EventSeq::ZERO,
            Some(EventSeq::new(99)),
            &TailState::Sealed {
                tail_seq: Some(99),
                sealed_at_unix_ms: 1_700_000_000_000,
            },
            empty_exhausted_reader(),
        )
        .unwrap();
        assert_eq!(page.tail.state, TailStateWire::Sealed);
        assert_eq!(page.tail.tail_seq, Some(99));
    }

    #[test]
    fn wire_tail_3_unknown_state_drops_tail_seq_even_if_log_has_one() {
        // The honest answer: Unknown → tail_seq is None. We do NOT present an
        // unproven number as if it were a fact about the tail, even when the
        // log (for legacy metadata reasons) could report one.
        let page = read_page_with(
            "wire-facts",
            &cursor_start(),
            10,
            &LogReadFilters::default(),
            EventSeq::ZERO,
            Some(EventSeq::new(123)),
            &TailState::Unknown {
                reason: "legacy".to_string(),
            },
            empty_exhausted_reader(),
        )
        .unwrap();
        assert_eq!(page.tail.state, TailStateWire::Unknown);
        assert_eq!(
            page.tail.tail_seq, None,
            "Unknown state refuses to present an unproven tail"
        );
    }

    // ---- SERDE shape: snake_case enum and additive fields --------------

    #[test]
    fn wire_tail_state_serializes_as_snake_case() {
        // The wire form is intentionally flat: state as a snake_case string,
        // tail_seq as a number-or-null. No "kind"/"version" envelope.
        let json = serde_json::to_value(TailFacts {
            state: TailStateWire::Sealed,
            tail_seq: Some(42),
        })
        .unwrap();
        assert_eq!(json["state"], "sealed");
        assert_eq!(json["tail_seq"], 42);

        let json = serde_json::to_value(TailFacts {
            state: TailStateWire::Unclean,
            tail_seq: None,
        })
        .unwrap();
        assert_eq!(json["state"], "unclean");
        assert_eq!(json["tail_seq"], serde_json::Value::Null);
    }

    #[test]
    fn wire_retention_serializes_with_two_flat_fields() {
        let json = serde_json::to_value(RetentionFacts {
            retained_from_seq: 5,
            history_truncated: true,
        })
        .unwrap();
        assert_eq!(json["retained_from_seq"], 5);
        assert_eq!(json["history_truncated"], true);
    }
}
