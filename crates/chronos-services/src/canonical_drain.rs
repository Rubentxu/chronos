//! REC-C2.2.2 — the canonical probe-drain reader.
//!
//! One scan over the session's `ExecutionLog`, one truth per record kind:
//!
//! ```text
//! Raw           -> decode TraceEvent -> project_semantic -> wire event
//! TripwireFired -> tripwires_fired += 1
//! Gap           -> page completeness becomes GapDetected
//! ```
//!
//! `TripwireManager` is **not** a parameter. Changing runtime subscriptions
//! must not retroactively change how many firings a page says it observed:
//! `tripwires_fired` is a count of durable `TripwireFired` evidence, not a
//! second evaluation.
//!
//! The EventBus is not consulted either; it may transport the same view live,
//! but it is not this reader's backing store.

use chronos_domain::semantic::{ResolveContext, SemanticEvent};
use chronos_domain::TraceEvent;
use chronos_log::{tripwire_evidence_codec as codec, EventSeq, ExecutionKind};

use crate::error::ServiceError;
use crate::events_cursor::EventsCursorV1;
use crate::events_log_read::{decode, payload_tag, Completeness, CompletenessReport};
use crate::session_log::SessionExecutionLog;

/// Default number of `Raw` events a canonical drain page returns.
pub const DEFAULT_MAX_RAW_EVENTS: usize = 512;
/// Default cap on records examined per page (cost bound, not a truth bound).
pub const DEFAULT_MAX_EXAMINED_RECORDS: u64 = 50_000;
/// Default cap on derived firings per source `Raw`.
///
/// A pathological cluster fails explicitly instead of making the "Raw plus its
/// derived records" unit unbounded.
pub const DEFAULT_MAX_DERIVED_PER_SOURCE: usize = 4_096;

/// Every durable `Raw` record of a session, decoded, in log order.
///
/// REC-C2.2.3: the canonical replacement for the destructive
/// `ProbeBackend::drain_raw_events()` (an `EventBus` ring snapshot). Three
/// differences matter, and all three are the point:
///
/// * **non-destructive** — the ring consumed what it returned, so a second
///   consumer saw nothing; the log is read, not drained.
/// * **complete within the retained range** — the ring silently dropped the
///   oldest events once at capacity, so `probe_stop`'s `total_events` was a
///   statement about a bounded buffer, not about the capture.
/// * **fail-closed** — an undecodable record is a hard error, never a silent
///   skip (same policy as `events_read` and `read_canonical_drain_page`).
#[derive(Debug)]
pub struct CanonicalRawScan {
    pub events: Vec<TraceEvent>,
    /// Completeness of the range that was read, in the shared C1 vocabulary.
    pub completeness: CompletenessReport,
    /// ExecutionRecords examined, including derived and marker records.
    pub examined_records: u64,
}

/// Read every durable `Raw` record of `log`, decoded, in order.
///
/// No `EventBus`, no `TripwireManager`: the log is the only input.
pub fn read_all_raw_events(log: &SessionExecutionLog) -> Result<CanonicalRawScan, ServiceError> {
    let from = log.retained_from();
    let mut events = Vec::new();
    let mut position = from;
    let mut examined: u64 = 0;
    let mut gap_seen: Option<(EventSeq, EventSeq)> = None;

    loop {
        let page = log
            .handle()
            .read_from_seq(position, 1024)
            .map_err(|e| ServiceError::DrainFailed(format!("canonical raw scan: {e}")))?;

        if gap_seen.is_none() {
            if let Some(first) = page.gaps.first() {
                gap_seen = Some((first.first_missing, EventSeq::new(first.last_missing.0 + 1)));
            }
        }

        if page.records.is_empty() {
            break;
        }

        for record in &page.records {
            position = EventSeq::new(record.seq.0 + 1);
            examined += 1;
            match record.kind {
                ExecutionKind::Raw => {
                    let event =
                        decode(record).ok_or_else(|| ServiceError::EvidenceDecodeFailed {
                            session_id: log.session_id().as_str().to_string(),
                            seq: record.seq.0,
                            payload_tag: payload_tag(record),
                        })?;
                    events.push(event);
                }
                ExecutionKind::GapMarker => {
                    if gap_seen.is_none() {
                        gap_seen = Some((record.seq, EventSeq::new(record.seq.0 + 1)));
                    }
                }
                // Derived firings are evidence about a Raw, not a Raw. The
                // caller asked for events; they are counted by the drain
                // reader, not returned as if they were occurrences.
                ExecutionKind::TripwireFired => {}
            }
        }
    }

    Ok(CanonicalRawScan {
        events,
        completeness: CompletenessReport {
            status: match gap_seen {
                Some(_) => Completeness::GapDetected,
                None => Completeness::Complete,
            },
            scope: CompletenessReport::SCOPE_EXAMINED_RANGE,
            from_seq: from.0,
            to_seq_exclusive: position.0,
        },
        examined_records: examined,
    })
}

/// One canonical probe-drain page.
#[derive(Debug, Clone)]
pub struct CanonicalDrainPage {
    /// Semantic projection of the `Raw` records in this page.
    pub events: Vec<SemanticEvent>,
    /// Number of `TripwireFired` evidence records examined in this page.
    pub tripwires_fired: usize,
    /// The positions of those firings, in order (identity, for callers that
    /// want it).
    pub firing_seqs: Vec<EventSeq>,
    /// Cursor to hand back: **after the last examined record**.
    pub next_cursor: EventsCursorV1,
    /// Records examined (Raw + firings + anything else).
    pub examined_records: u64,
    /// `Raw` events returned.
    pub raw_events: usize,
    /// Completeness of the **examined range**, using the same vocabulary as
    /// `events_read` (REC-C1.3/C1.4). This is evidence truth, not pagination:
    /// a page that stopped at `max_raw_events` can still be `Complete` for the
    /// range it examined. `exhausted` is the pagination/tail fact.
    pub completeness: CompletenessReport,
    /// True when the scan reached the end of what the log currently holds.
    pub exhausted: bool,
}

/// Read one canonical page.
///
/// Cursor laws (identical to `events_read` / `observe`):
/// * `from` must sit at a logical boundary: a `Raw`, a gap, or the tail.
///   Reading from inside a derived cluster would let a page report firings
///   whose source it does not return, so it is a typed error and **never** a
///   silent re-anchor.
/// * the cursor advances after the last record **examined**.
/// * a `Raw` and the `TripwireFired` records that immediately follow it are one
///   logical read unit: once the last requested `Raw` is included, its derived
///   firings are consumed too.
pub fn read_canonical_drain_page(
    log: &SessionExecutionLog,
    cursor: Option<&EventsCursorV1>,
    max_raw_events: usize,
    max_examined_records: u64,
    max_derived_per_source: usize,
    ctx: &ResolveContext,
    project: &dyn Fn(&TraceEvent, &ResolveContext) -> SemanticEvent,
) -> Result<CanonicalDrainPage, ServiceError> {
    let from = match cursor {
        Some(c) => c.next_seq(),
        None => log.retained_from(),
    };

    // --- logical-boundary check ---
    let head = log
        .handle()
        .read_from_seq(from, 1)
        .map_err(|e| ServiceError::DrainFailed(format!("probe_drain head read: {e}")))?;
    if let Some(first) = head.records.first() {
        if first.kind == ExecutionKind::TripwireFired {
            if let Ok(Some(ev)) = codec::decode(&first.payload) {
                if ev.source_seq < from {
                    return Err(ServiceError::InvalidInput(format!(
                        "probe_drain cursor is not at a logical Raw boundary: seq {} is a \
                         derived firing of source {}; resume from {} or from a Raw boundary",
                        from.0, ev.source_seq.0, ev.source_seq.0
                    )));
                }
            }
        }
    }

    let mut events = Vec::new();
    let mut firing_seqs = Vec::new();
    let mut examined: u64 = 0;
    let mut position = from;
    let mut derived_since_raw = 0usize;
    let mut gap_seen: Option<(EventSeq, EventSeq)> = None;
    let mut exhausted = false;

    'scan: loop {
        // Budget exhaustion stops the page; it says nothing about the
        // completeness of the range already examined.
        if examined >= max_examined_records {
            break;
        }
        let page = log
            .handle()
            .read_from_seq(position, 512)
            .map_err(|e| ServiceError::DrainFailed(format!("probe_drain read: {e}")))?;

        if gap_seen.is_none() {
            if let Some(first) = page.gaps.first() {
                gap_seen = Some((first.first_missing, EventSeq::new(first.last_missing.0 + 1)));
            }
        }

        for record in &page.records {
            // Stop *before* consuming: the last requested Raw was already
            // included, and this record is not one of its derived firings.
            if events.len() >= max_raw_events && record.kind != ExecutionKind::TripwireFired {
                break 'scan;
            }
            if examined >= max_examined_records {
                break 'scan;
            }

            position = EventSeq::new(record.seq.0 + 1);
            examined += 1;

            match record.kind {
                ExecutionKind::Raw => {
                    derived_since_raw = 0;
                    // Fail closed: an undecodable Raw must not be skipped while
                    // its derived firings are still counted, and no cursor may
                    // advance over evidence we could not read. Same policy as
                    // `events_read`.
                    let event =
                        decode(record).ok_or_else(|| ServiceError::EvidenceDecodeFailed {
                            session_id: log.session_id().as_str().to_string(),
                            seq: record.seq.0,
                            payload_tag: payload_tag(record),
                        })?;
                    events.push(project(&event, ctx));
                }
                ExecutionKind::TripwireFired => {
                    derived_since_raw += 1;
                    if derived_since_raw > max_derived_per_source {
                        return Err(ServiceError::InvalidInput(format!(
                            "probe_drain found more than {max_derived_per_source} derived firings \
                             for one source at seq {}",
                            record.seq.0
                        )));
                    }
                    firing_seqs.push(record.seq);
                }
                // A producer-recorded gap marker is gap evidence too.
                ExecutionKind::GapMarker => {
                    if gap_seen.is_none() {
                        gap_seen = Some((record.seq, EventSeq::new(record.seq.0 + 1)));
                    }
                }
            }
        }

        if page.exhausted {
            exhausted = true;
            break;
        }
        if page.position_after < position {
            return Err(ServiceError::EvidenceReadStalled {
                session_id: log.session_id().as_str().to_string(),
                position: position.0,
            });
        }
        position = page.position_after.max(position);
    }

    let next_cursor = EventsCursorV1::start(log.session_id().clone())
        .advanced_to(position)
        .map_err(|e| ServiceError::InvalidInput(format!("probe_drain cursor advance: {e}")))?;

    // Completeness describes the EXAMINED range, exactly like events_read.
    // Stopping at a budget is pagination, not evidence loss.
    let completeness = CompletenessReport {
        status: match gap_seen {
            Some(_) => Completeness::GapDetected,
            None => Completeness::Complete,
        },
        scope: CompletenessReport::SCOPE_EXAMINED_RANGE,
        from_seq: from.0,
        to_seq_exclusive: position.0,
    };

    Ok(CanonicalDrainPage {
        raw_events: events.len(),
        tripwires_fired: firing_seqs.len(),
        firing_seqs,
        events,
        next_cursor,
        examined_records: examined,
        completeness,
        exhausted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::semantic::{SemanticEvent, SemanticEventKind};
    use chronos_domain::{EventData, EventType, Language, SourceLocation, TraceEvent};
    use chronos_log::{
        ExecutionPayload, Gap, GapReason, NewExecutionRecord, SegmentedConfig,
        SegmentedExecutionLog, SessionId, TripwireFiredEvidence,
    };
    use std::sync::Arc;

    fn tempdir(tag: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!(
            "chronos-c22-drain-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("tempdir");
        p
    }

    fn open_log(dir: &std::path::Path, session: &str) -> SessionExecutionLog {
        let raw =
            SegmentedExecutionLog::open(SessionId::new(session), SegmentedConfig::with_dir(dir))
                .expect("open");
        SessionExecutionLog::from_segmented_log(
            SessionId::new(session),
            Arc::new(raw),
            Some(dir.to_path_buf()),
        )
    }

    fn trace_event(id: u64) -> TraceEvent {
        TraceEvent {
            event_id: id,
            timestamp_ns: id * 1000,
            thread_id: 1,
            event_type: EventType::FunctionEntry,
            location: SourceLocation {
                function: Some(format!("fn_{id}")),
                ..SourceLocation::default()
            },
            data: EventData::Function {
                name: format!("fn_{id}"),
                signature: None,
                symbol_id: None,
                invocation_id: None,
                parent_invocation_id: None,
            },
        }
    }

    fn append_raw(log: &SessionExecutionLog, id: u64) -> EventSeq {
        log.handle()
            .append(NewExecutionRecord {
                session_id: log.session_id().clone(),
                kind: ExecutionKind::Raw,
                monotonic_ns: id * 1000,
                payload: ExecutionPayload::new(
                    serde_json::to_vec(&trace_event(id)).expect("encode"),
                    "trace_event",
                ),
                ..Default::default()
            })
            .expect("append raw")
    }

    fn append_firing(log: &SessionExecutionLog, source: EventSeq) -> EventSeq {
        let evidence = TripwireFiredEvidence {
            tripwire_id: chronos_domain::TripwireId(9),
            source_seq: source,
            source_event_id: Some(source.0),
            condition: chronos_domain::TripwireCondition::FunctionName {
                pattern: "*".to_string(),
            },
            label: None,
            source_timestamp_ns: source.0 * 1000,
            source_thread_id: 1,
        };
        log.handle()
            .append(NewExecutionRecord {
                session_id: log.session_id().clone(),
                kind: ExecutionKind::TripwireFired,
                monotonic_ns: source.0 * 1000,
                payload: codec::encode(&evidence).expect("encode"),
                ..Default::default()
            })
            .expect("append firing")
    }

    fn trivial_project(event: &TraceEvent, _ctx: &ResolveContext) -> SemanticEvent {
        SemanticEvent {
            source_event_id: event.event_id,
            timestamp_ns: event.timestamp_ns,
            thread_id: event.thread_id,
            language: Language::Unknown,
            kind: SemanticEventKind::Unresolved,
            description: format!("raw-{}", event.event_id),
        }
    }

    fn ctx() -> ResolveContext {
        ResolveContext {
            pid: 0,
            binary_path: None,
        }
    }

    fn drain(
        log: &SessionExecutionLog,
        cursor: Option<&EventsCursorV1>,
        max_raw: usize,
    ) -> Result<CanonicalDrainPage, ServiceError> {
        read_canonical_drain_page(
            log,
            cursor,
            max_raw,
            100_000,
            DEFAULT_MAX_DERIVED_PER_SOURCE,
            &ctx(),
            &trivial_project,
        )
    }

    /// DRAIN-2: `tripwires_fired` is counted from evidence, not from runtime
    /// configuration. No manager exists anywhere in this reader.
    #[test]
    fn drain_2_firings_come_from_evidence_not_configuration() {
        let dir = tempdir("drain2");
        let log = open_log(&dir, "drain2");
        let s = append_raw(&log, 1);
        append_firing(&log, s);
        log.flush().ok();

        let page = drain(&log, None, 10).expect("page");
        assert_eq!(page.raw_events, 1, "the Raw source is visible");
        assert_eq!(
            page.tripwires_fired, 1,
            "counted from TripwireFired evidence"
        );
        assert_eq!(page.completeness.status, Completeness::Complete);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// DRAIN-3: a Raw and its derived firings are one logical unit.
    ///
    /// Layout (seqs are assigned by the log): `Raw0 F1 F2 Raw3`. With
    /// `max_raw = 1` the page returns the Raw, BOTH of its derived firings,
    /// and leaves the cursor at 3 — never a page with firings but no source.
    #[test]
    fn drain_3_raw_and_its_derived_firings_are_one_unit() {
        let dir = tempdir("drain3");
        let log = open_log(&dir, "drain3");
        let first_raw = append_raw(&log, 100);
        append_firing(&log, first_raw);
        append_firing(&log, first_raw);
        let next_raw = append_raw(&log, 103);
        log.flush().ok();
        assert_eq!(first_raw, EventSeq::new(0), "seqs are log-assigned");
        assert_eq!(next_raw, EventSeq::new(3));

        let page = drain(&log, None, 1).expect("page");
        assert_eq!(page.raw_events, 1, "only the requested Raw");
        assert_eq!(page.tripwires_fired, 2, "both derived firings came along");
        assert_eq!(
            page.next_cursor.next_seq(),
            EventSeq::new(3),
            "cursor sits at the next Raw boundary, after the cluster"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// DRAIN-4: a cursor inside a derived cluster is a typed error, never a
    /// silent re-anchor.
    #[test]
    fn drain_4_cursor_inside_a_derived_cluster_is_rejected() {
        let dir = tempdir("drain4");
        let log = open_log(&dir, "drain4");
        let source = append_raw(&log, 100);
        let firing = append_firing(&log, source);
        log.flush().ok();
        assert_eq!(source, EventSeq::new(0));
        assert_eq!(firing, EventSeq::new(1), "the derived firing's own seq");

        // Point the cursor AT the derived firing, not at its Raw boundary.
        let inside = EventsCursorV1::start(log.session_id().clone())
            .advanced_to(firing)
            .expect("cursor");
        let err = drain(&log, Some(&inside), 10).expect_err("must refuse");
        assert!(
            matches!(err, ServiceError::InvalidInput(_)),
            "expected a logical-boundary error, got {err:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// DRAIN-5: a page that stops early still advances the cursor.
    #[test]
    fn drain_5_page_progresses_when_it_stops_early() {
        let dir = tempdir("drain5");
        let log = open_log(&dir, "drain5");
        for i in 0..10u64 {
            append_raw(&log, i);
        }
        log.flush().ok();

        let first = drain(&log, None, 3).expect("page");
        assert_eq!(first.raw_events, 3);
        assert!(
            !first.exhausted,
            "there is more, so the scan did not reach the tail"
        );
        assert_eq!(
            first.completeness.status,
            Completeness::Complete,
            "stopping at a budget is pagination, not evidence loss: the EXAMINED range is complete"
        );
        assert_eq!(first.next_cursor.next_seq(), EventSeq::new(3));

        let second = drain(&log, Some(&first.next_cursor), 100).expect("page");
        assert_eq!(second.raw_events, 7, "the rest, no re-read");
        assert_eq!(second.next_cursor.next_seq(), EventSeq::new(10));
        assert!(second.exhausted);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// DRAIN-6: a gap inside the examined region contaminates the page.
    #[test]
    fn drain_6_gap_makes_completeness_explicit() {
        let dir = tempdir("drain6");
        let log = open_log(&dir, "drain6");
        append_raw(&log, 0);
        log.handle()
            .record_gap(Gap::new(
                EventSeq::new(1),
                EventSeq::new(5),
                GapReason::AdapterBufferOverflow,
                "drain6",
            ))
            .expect("gap");
        append_raw(&log, 6);
        log.flush().ok();

        let page = drain(&log, None, 100).expect("page");
        assert_eq!(
            page.completeness.status,
            Completeness::GapDetected,
            "a traversed gap must contaminate the page, got {:?}",
            page.completeness
        );
        assert_eq!(page.completeness.scope, "examined_range");
        assert_eq!(page.raw_events, 2, "both Raw records are still returned");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// DRAIN-7: an undecodable `Raw` fails closed. It must not be skipped
    /// while its derived firings are still counted, and no cursor may advance
    /// over evidence we could not read.
    #[test]
    fn drain_7_undecodable_raw_fails_closed() {
        let dir = tempdir("drain7");
        let log = open_log(&dir, "drain7");
        // A Raw whose payload is not a TraceEvent.
        let bad = log
            .handle()
            .append(NewExecutionRecord {
                session_id: log.session_id().clone(),
                kind: ExecutionKind::Raw,
                monotonic_ns: 1,
                payload: ExecutionPayload::new(b"not-json".to_vec(), "broken"),
                ..Default::default()
            })
            .expect("append undecodable raw");
        append_firing(&log, bad);
        log.flush().ok();

        let err = drain(&log, None, 10).expect_err("must fail closed");
        match err {
            ServiceError::EvidenceDecodeFailed { seq, .. } => {
                assert_eq!(seq, bad.0, "the error names the undecodable record");
            }
            other => panic!("expected EvidenceDecodeFailed, got {other:?}"),
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ---- REC-C2.2.3: the canonical raw reader ---------------------------

    /// STOP-1: reading every durable Raw is a READ, not a drain. The retired
    /// `drain_raw_events()` consumed the EventBus ring, so a second consumer
    /// saw nothing.
    #[test]
    fn stop_1_reading_all_raw_is_repeatable() {
        let dir = tempdir("stop1");
        let log = open_log(&dir, "stop1");
        for i in 0..5 {
            append_raw(&log, i);
        }
        log.flush().ok();

        let first = read_all_raw_events(&log).expect("first read");
        let second = read_all_raw_events(&log).expect("second read");

        assert_eq!(first.events.len(), 5);
        assert_eq!(
            second.events.len(),
            5,
            "a read must not consume: the retired ring drain saw 0 on the second call"
        );
        assert_eq!(
            first.events.iter().map(|e| e.event_id).collect::<Vec<_>>(),
            second.events.iter().map(|e| e.event_id).collect::<Vec<_>>()
        );
        assert_eq!(first.completeness.status, Completeness::Complete);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// STOP-2: derived firings are not occurrences. A `TripwireFired` record is
    /// evidence ABOUT a Raw and must never be returned as if the program had
    /// emitted it.
    #[test]
    fn stop_2_derived_firings_are_not_returned_as_events() {
        let dir = tempdir("stop2");
        let log = open_log(&dir, "stop2");
        let raw = append_raw(&log, 0);
        append_firing(&log, raw);
        log.flush().ok();

        let scan = read_all_raw_events(&log).expect("scan");
        assert_eq!(scan.events.len(), 1, "one Raw, one event");
        assert_eq!(scan.events[0].event_id, 0);
        assert!(
            scan.examined_records >= 2,
            "the firing was still examined: {}",
            scan.examined_records
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// STOP-3: a gap makes the shortfall explicit instead of silently
    /// shortening the snapshot.
    #[test]
    fn stop_3_gap_makes_completeness_explicit() {
        let dir = tempdir("stop3");
        let log = open_log(&dir, "stop3");
        append_raw(&log, 0);
        log.handle()
            .record_gap(Gap::new(
                EventSeq::new(1),
                EventSeq::new(3),
                GapReason::AdapterBufferOverflow,
                "stop3",
            ))
            .expect("gap");
        append_raw(&log, 4);
        log.flush().ok();

        let scan = read_all_raw_events(&log).expect("scan");
        assert_eq!(scan.events.len(), 2);
        assert_eq!(scan.completeness.status, Completeness::GapDetected);
        assert_eq!(scan.completeness.scope, "examined_range");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// STOP-4: the same fail-closed policy as the page reader. Skipping an
    /// unreadable record would make `total_events` a smaller-than-truth number
    /// presented as a fact.
    #[test]
    fn stop_4_undecodable_raw_fails_closed() {
        let dir = tempdir("stop4");
        let log = open_log(&dir, "stop4");
        append_raw(&log, 0);
        log.handle()
            .append(NewExecutionRecord {
                session_id: log.session_id().clone(),
                kind: ExecutionKind::Raw,
                monotonic_ns: 1,
                payload: ExecutionPayload::new(b"not-json".to_vec(), "broken"),
                ..Default::default()
            })
            .expect("append broken");
        log.flush().ok();

        let err = read_all_raw_events(&log).expect_err("must fail closed");
        assert!(matches!(err, ServiceError::EvidenceDecodeFailed { .. }));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The falsification: neither transport nor runtime policy is authority.
    /// The reader has no bus and no manager, so two identical reads over an
    /// unchanged log are byte-identical, and nothing about their result can be
    /// changed by draining a bus or editing subscriptions.
    #[test]
    fn drain_output_cannot_be_influenced_by_transport_or_policy() {
        let dir = tempdir("drainfalsify");
        let log = open_log(&dir, "drainfalsify");
        let s = append_raw(&log, 1);
        append_firing(&log, s);
        log.flush().ok();

        let a = drain(&log, None, 10).expect("page");
        // Perturb nothing: there is no bus to consult, no manager to perturb.
        // The drain MUST give the same answer regardless of unrelated state
        // mutations.
        let b = drain(&log, None, 10).expect("page");

        assert_eq!(a.raw_events, b.raw_events);
        assert_eq!(a.tripwires_fired, b.tripwires_fired);
        assert_eq!(a.firing_seqs, b.firing_seqs);
        assert_eq!(a.next_cursor.next_seq(), b.next_cursor.next_seq());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
