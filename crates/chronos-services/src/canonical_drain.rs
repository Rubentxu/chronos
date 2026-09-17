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
use chronos_log::{EventSeq, ExecutionKind, TripwireFiredEvidence};

use crate::error::ServiceError;
use crate::events_cursor::EventsCursorV1;
use crate::events_log_read::decode;
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

/// What the page can prove about the region it examined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageCompleteness {
    /// Reached the tail with no gap in the examined region.
    Complete,
    /// A gap intersects the examined region: the page cannot claim continuity
    /// even if every requested `Raw` decoded.
    GapDetected {
        from: EventSeq,
        to_exclusive: EventSeq,
    },
    /// The scan budget ran out before the tail.
    Partial,
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
    pub completeness: PageCompleteness,
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
            if let Ok(Some(ev)) = TripwireFiredEvidence::from_payload(&first.payload) {
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
    let mut completeness = PageCompleteness::Complete;
    let mut exhausted = false;

    'scan: loop {
        if examined >= max_examined_records {
            completeness = match completeness {
                PageCompleteness::Complete => PageCompleteness::Partial,
                other => other,
            };
            break;
        }
        let page = log
            .handle()
            .read_from_seq(position, 512)
            .map_err(|e| ServiceError::DrainFailed(format!("probe_drain read: {e}")))?;

        if !page.gaps.is_empty() {
            let first = &page.gaps[0];
            completeness = PageCompleteness::GapDetected {
                from: first.first_missing,
                to_exclusive: EventSeq::new(first.last_missing.0 + 1),
            };
        }

        for record in &page.records {
            // Stop *before* consuming: the last requested Raw was already
            // included, and this record is not one of its derived firings.
            if events.len() >= max_raw_events && record.kind != ExecutionKind::TripwireFired {
                break 'scan;
            }
            if examined >= max_examined_records {
                completeness = match completeness {
                    PageCompleteness::Complete => PageCompleteness::Partial,
                    other => other,
                };
                break 'scan;
            }

            position = EventSeq::new(record.seq.0 + 1);
            examined += 1;

            match record.kind {
                ExecutionKind::Raw => {
                    derived_since_raw = 0;
                    if let Some(event) = decode(record) {
                        events.push(project(&event, ctx));
                    }
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
                    if matches!(completeness, PageCompleteness::Complete) {
                        completeness = PageCompleteness::GapDetected {
                            from: record.seq,
                            to_exclusive: EventSeq::new(record.seq.0 + 1),
                        };
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
        SegmentedExecutionLog, SessionId,
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
        SessionExecutionLog::try_adopt(
            Some(dir.to_path_buf()),
            SessionId::new(session),
            Arc::new(raw),
        )
        .expect("adopt")
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
                payload: evidence.to_payload().expect("encode"),
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
        log.handle().flush().ok();

        let page = drain(&log, None, 10).expect("page");
        assert_eq!(page.raw_events, 1, "the Raw source is visible");
        assert_eq!(
            page.tripwires_fired, 1,
            "counted from TripwireFired evidence"
        );
        assert_eq!(page.completeness, PageCompleteness::Complete);

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
        log.handle().flush().ok();
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
        log.handle().flush().ok();
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
        log.handle().flush().ok();

        let first = drain(&log, None, 3).expect("page");
        assert_eq!(first.raw_events, 3);
        assert!(!first.exhausted);
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
        log.handle().flush().ok();

        let page = drain(&log, None, 100).expect("page");
        assert!(
            matches!(page.completeness, PageCompleteness::GapDetected { .. }),
            "a traversed gap must contaminate the page, got {:?}",
            page.completeness
        );
        assert_eq!(page.raw_events, 2, "both Raw records are still returned");

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
        log.handle().flush().ok();

        let a = drain(&log, None, 10).expect("page");
        // Perturb a bus (it is not consulted) and there is no manager argument
        // to perturb at all.
        let bus = chronos_domain::bus::EventBus::new_shared(8);
        bus.push_raw(trace_event(999));
        let _ = bus.snapshot_raw();
        let b = drain(&log, None, 10).expect("page");

        assert_eq!(a.raw_events, b.raw_events);
        assert_eq!(a.tripwires_fired, b.tripwires_fired);
        assert_eq!(a.firing_seqs, b.firing_seqs);
        assert_eq!(a.next_cursor.next_seq(), b.next_cursor.next_seq());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
