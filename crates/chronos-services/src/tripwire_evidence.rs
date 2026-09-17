//! REC-C2.1 — durable tripwire-firing evidence.
//!
//! The rule this module implements:
//!
//! > **Persist first, derive second, fan-out last.** ExecutionLog owns
//! > occurrence; live channels only transport observations.
//!
//! A firing is derived **from an already-accepted source record** and is
//! itself accepted into the log. It therefore has its own authoritative
//! `ExecutionRecord.seq`, while
//! [`TripwireFiredEvidence::source_seq`] names the cause:
//!
//! ```text
//! seq 419  Raw             (the accepted source)
//! seq 421  TripwireFired   (derived; source_seq = 419)
//! ```

use std::sync::Arc;

use chronos_domain::tripwire::TripwireManager;
use chronos_domain::TraceEvent;
use chronos_log::{
    EventSeq, ExecutionKind, ExecutionRecord, NewExecutionRecord, SessionId, TripwireFiredEvidence,
};

use crate::error::ServiceError;
use crate::events_log_read::decode;
use crate::session_log::SessionExecutionLog;

/// One firing that could not be made durable.
///
/// The source evidence is **not** missing — a projection failed. This is
/// deliberately not modelled as a `Gap` (which would claim lost source
/// evidence) and is surfaced to the caller as an explicit signal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TripwirePersistenceFailure {
    /// The accepted source record this firing derives from.
    pub source_seq: EventSeq,
    pub tripwire_id: chronos_domain::TripwireId,
    pub reason: String,
}

/// The result of deriving firings from one accepted record.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DerivationReport {
    /// seq of each durable firing record, in append order.
    pub appended: Vec<EventSeq>,
    /// Derivations that could not be persisted (the source record stays).
    pub failures: Vec<TripwirePersistenceFailure>,
}

impl DerivationReport {
    pub fn is_empty(&self) -> bool {
        self.appended.is_empty() && self.failures.is_empty()
    }
}

/// Derive firings from an accepted record, dispatching on
/// [`ExecutionKind`].
///
/// **The recursion barrier lives here.** Only `ExecutionKind::Raw` is
/// evaluated. `GapMarker` and `TripwireFired` are inert, which makes
/// `TripwireFired -> evaluate -> TripwireFired -> ...` impossible by
/// construction rather than by a payload-tag string comparison.
pub fn derive_firings_from_record(
    log: &SessionExecutionLog,
    manager: &Arc<TripwireManager>,
    record: &ExecutionRecord,
) -> Result<DerivationReport, ServiceError> {
    // The barrier: a firing (or a gap) is never itself a source.
    if record.kind != ExecutionKind::Raw {
        return Ok(DerivationReport::default());
    }
    let Some(event) = decode(record) else {
        // A raw record whose payload cannot be decoded cannot be evaluated.
        // Not a persistence failure: there is nothing to derive.
        return Ok(DerivationReport::default());
    };
    derive_firings_from_event(log, manager, record.seq, &event)
}

/// Derive firings for one accepted source event at `source_seq`.
pub fn derive_firings_from_event(
    log: &SessionExecutionLog,
    manager: &Arc<TripwireManager>,
    source_seq: EventSeq,
    event: &TraceEvent,
) -> Result<DerivationReport, ServiceError> {
    let matches = manager.matching(event);
    if matches.is_empty() {
        return Ok(DerivationReport::default());
    }

    let session_id: SessionId = log.session_id().clone();
    let mut report = DerivationReport::default();

    for m in matches {
        let evidence = TripwireFiredEvidence {
            tripwire_id: m.id,
            source_seq,
            source_event_id: Some(event.event_id),
            condition: m.condition,
            label: m.label,
            source_timestamp_ns: event.timestamp_ns,
            source_thread_id: event.thread_id,
        };
        let payload = match evidence.to_payload() {
            Ok(p) => p,
            Err(e) => {
                report.failures.push(TripwirePersistenceFailure {
                    source_seq,
                    tripwire_id: m.id,
                    reason: format!("encode evidence: {e}"),
                });
                continue;
            }
        };
        let new = NewExecutionRecord {
            kind: ExecutionKind::TripwireFired,
            session_id: session_id.clone(),
            // The firing happens right after the source it derives from;
            // ordering stays consistent with the log's session-relative clock.
            monotonic_ns: event.timestamp_ns,
            payload,
            ..Default::default()
        };
        match log.handle().append(new) {
            Ok(seq) => report.appended.push(seq),
            Err(e) => report.failures.push(TripwirePersistenceFailure {
                source_seq,
                tripwire_id: m.id,
                reason: format!("append firing evidence: {e}"),
            }),
        }
    }

    if !report.appended.is_empty() {
        let _ = log.handle().flush();
    }
    Ok(report)
}

/// One page of firing evidence.
#[derive(Debug, Clone)]
pub struct FiringPage {
    /// Firings found in this page, in seq order.
    pub firings: Vec<(EventSeq, TripwireFiredEvidence)>,
    /// Position to use as the next cursor: **after the last record examined**,
    /// firing or not.
    pub position_after: EventSeq,
    /// How many records this page looked at.
    pub examined: u64,
    /// True when the scan reached the end of what the log currently holds.
    pub exhausted: bool,
}

/// Default cap on records examined per page.
///
/// A page is bounded by **two** budgets, not one. `max_firings` alone does not
/// bound cost: a sparse firing in a two-million-record log would let a request
/// for 10 firings scan to the end. Filtering must not turn a query into an
/// unbounded scan, so a page may legitimately return
/// `0 firings, 50_000 examined, advanced cursor, exhausted=false`.
pub const DEFAULT_SCAN_BUDGET: u64 = 50_000;

/// Read every durable firing recorded in a session's log.
pub fn read_firings(
    log: &SessionExecutionLog,
    from: EventSeq,
    limit: usize,
) -> Result<Vec<(EventSeq, TripwireFiredEvidence)>, ServiceError> {
    Ok(read_firings_page(log, from, limit, DEFAULT_SCAN_BUDGET)?.firings)
}

/// Paged firing read with the C1 cursor semantics.
///
/// **The returned position is after the last record _examined_, not after the
/// last firing _found_.** Filters change what you get back; they never change
/// what the position means (REC-C1.3). Advancing to the last firing's seq would
/// make a page that ends on a non-firing record either re-scan or skip.
///
/// Stops at whichever budget is hit first: `max_firings` collected or
/// `max_examined_records` looked at.
pub fn read_firings_page(
    log: &SessionExecutionLog,
    from: EventSeq,
    max_firings: usize,
    max_examined_records: u64,
) -> Result<FiringPage, ServiceError> {
    let mut page = FiringPage {
        firings: Vec::new(),
        position_after: from,
        examined: 0,
        exhausted: false,
    };
    let mut position = from;
    let mut remaining_pages = 64u64;

    while remaining_pages > 0 && page.examined < max_examined_records {
        remaining_pages -= 1;
        let read = log
            .handle()
            .read_from_seq(position, 512)
            .map_err(|e| ServiceError::DrainFailed(format!("read firings: {e}")))?;

        for record in &read.records {
            if page.examined >= max_examined_records {
                return Ok(page);
            }
            // Advance past every record we examine, firing or not.
            position = EventSeq::new(record.seq.0 + 1);
            page.examined += 1;
            if record.kind == ExecutionKind::TripwireFired {
                if let Ok(Some(ev)) = TripwireFiredEvidence::from_payload(&record.payload) {
                    page.firings.push((record.seq, ev));
                    page.position_after = position;
                    if page.firings.len() >= max_firings {
                        return Ok(page);
                    }
                }
            }
            page.position_after = position;
        }

        if read.exhausted {
            page.exhausted = true;
            break;
        }
        if read.position_after < position {
            break;
        }
        position = read.position_after;
        page.position_after = position;
    }

    Ok(page)
}

/// Count firings per subscription by scanning the log.
///
/// `fire_count` is derived evidence, not a mutable counter (REC-C2.1.5): a
/// counter that is never incremented reads 0 forever (CHAR-C2-04).
pub fn firing_counts(
    log: &SessionExecutionLog,
) -> Result<std::collections::HashMap<chronos_domain::TripwireId, u64>, ServiceError> {
    let mut counts: std::collections::HashMap<chronos_domain::TripwireId, u64> = Default::default();
    let mut position = log.retained_from();
    let mut remaining_pages = 64u64;
    while remaining_pages > 0 {
        remaining_pages -= 1;
        let page = log
            .handle()
            .read_from_seq(position, 512)
            .map_err(|e| ServiceError::DrainFailed(format!("count firings: {e}")))?;
        for record in &page.records {
            position = EventSeq::new(record.seq.0 + 1);
            if record.kind == ExecutionKind::TripwireFired {
                if let Ok(Some(ev)) = TripwireFiredEvidence::from_payload(&record.payload) {
                    *counts.entry(ev.tripwire_id).or_insert(0) += 1;
                }
            }
        }
        if page.exhausted || page.position_after < position {
            break;
        }
        position = page.position_after;
    }
    Ok(counts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::{
        EventData, EventType, SourceLocation, TraceEvent, TripwireCondition, TripwireManager,
    };
    use chronos_log::{
        ExecutionPayload, NewExecutionRecord, SegmentedConfig, SegmentedExecutionLog,
    };

    fn tempdir(tag: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!(
            "chronos-c21-derive-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("create tempdir");
        p
    }

    fn matching_event() -> TraceEvent {
        let location = SourceLocation {
            function: Some("main_work".to_string()),
            ..SourceLocation::default()
        };
        TraceEvent {
            event_id: 83,
            timestamp_ns: 419_000,
            thread_id: 1,
            event_type: EventType::FunctionEntry,
            location,
            data: EventData::Function {
                name: "main_work".into(),
                signature: None,
                symbol_id: None,
                invocation_id: None,
                parent_invocation_id: None,
            },
        }
    }

    fn non_matching_event(id: u64) -> TraceEvent {
        use chronos_domain::{EventData, EventType, SourceLocation};
        TraceEvent {
            event_id: id,
            timestamp_ns: id * 1000,
            thread_id: 1,
            event_type: EventType::FunctionEntry,
            location: SourceLocation {
                function: Some(format!("other_{id}")),
                ..SourceLocation::default()
            },
            data: EventData::Function {
                name: format!("other_{id}"),
                signature: None,
                symbol_id: None,
                invocation_id: None,
                parent_invocation_id: None,
            },
        }
    }

    fn manager_with_main() -> Arc<TripwireManager> {
        let mgr = Arc::new(TripwireManager::new());
        mgr.register(TripwireCondition::FunctionName {
            pattern: "main*".to_string(),
        });
        mgr
    }

    fn open_log(dir: &std::path::Path, session: &str) -> SessionExecutionLog {
        let log =
            SegmentedExecutionLog::open(SessionId::new(session), SegmentedConfig::with_dir(dir))
                .expect("open");
        SessionExecutionLog::try_adopt(
            Some(dir.to_path_buf()),
            SessionId::new(session),
            Arc::new(log),
        )
        .expect("adopt")
    }

    fn append_raw(log: &SessionExecutionLog, event: &TraceEvent) -> EventSeq {
        let payload =
            ExecutionPayload::new(serde_json::to_vec(event).expect("encode"), "trace_event");
        let seq = log
            .handle()
            .append(NewExecutionRecord {
                kind: ExecutionKind::Raw,
                session_id: log.session_id().clone(),
                monotonic_ns: event.timestamp_ns,
                payload,
                ..Default::default()
            })
            .expect("append raw");
        log.handle().flush().ok();
        seq
    }

    /// The firing gets its OWN seq, and its evidence names the cause.
    #[test]
    fn firing_is_derived_with_source_seq_pointing_at_the_raw_record() {
        let dir = tempdir("derive");
        let session = "rec-c2-1-derive";
        let log = open_log(&dir, session);
        let mgr = manager_with_main();

        let event = matching_event();
        let source_seq = append_raw(&log, &event);

        let record = log
            .handle()
            .read_from_seq(source_seq, 1)
            .expect("read")
            .records
            .into_iter()
            .next()
            .expect("source record");
        let report = derive_firings_from_record(&log, &mgr, &record).expect("derive");

        assert_eq!(report.appended.len(), 1, "one firing persisted");
        assert!(report.failures.is_empty(), "no persistence failures");
        let firing_seq = report.appended[0];
        assert_eq!(
            firing_seq,
            EventSeq::new(source_seq.0 + 1),
            "the firing is appended right after its source"
        );

        let firings = read_firings(&log, EventSeq::ZERO, 10).expect("read firings");
        assert_eq!(firings.len(), 1);
        let (seq, ev) = &firings[0];
        assert_eq!(*seq, firing_seq, "identity of the firing");
        assert_eq!(ev.source_seq, source_seq, "identity of the cause");
        assert_eq!(ev.source_event_id, Some(83));
        assert_eq!(ev.label, None);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A page is bounded by BOTH budgets: a sparse firing must not turn a
    /// small request into an unbounded scan.
    #[test]
    fn firing_page_is_bounded_by_the_scan_budget() {
        let dir = tempdir("budget");
        let session = "rec-c2-1-budget";
        let log = open_log(&dir, session);
        let mgr = manager_with_main();

        // 30 raw records that never match the tripwire.
        for i in 0..30u64 {
            append_raw(&log, &non_matching_event(i));
        }

        let page = read_firings_page(&log, EventSeq::ZERO, 10, 10).expect("page");
        assert!(page.firings.is_empty(), "nothing matched");
        assert_eq!(page.examined, 10, "the scan budget caps the work");
        assert_eq!(page.position_after, EventSeq::new(10), "cursor advanced");
        assert!(!page.exhausted, "there is still more log to read");
        assert!(
            mgr.matching(&non_matching_event(0)).is_empty(),
            "fixture sanity: non-matching events really do not match"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The recursion barrier: a firing record is never itself a source.
    #[test]
    fn barrier_does_not_derive_from_a_firing_record() {
        let dir = tempdir("barrier");
        let session = "rec-c2-1-barrier";
        let log = open_log(&dir, session);
        let mgr = manager_with_main();

        let source_seq = append_raw(&log, &matching_event());
        let source = log
            .handle()
            .read_from_seq(source_seq, 1)
            .expect("read")
            .records
            .into_iter()
            .next()
            .expect("source");
        derive_firings_from_record(&log, &mgr, &source).expect("derive");

        // Now feed the FIRING back in. Its payload is not a decodable
        // TraceEvent and, more importantly, its kind is TripwireFired.
        let firing = log
            .handle()
            .read_from_seq(EventSeq::new(source_seq.0 + 1), 1)
            .expect("read")
            .records
            .into_iter()
            .next()
            .expect("firing");
        assert_eq!(firing.kind, ExecutionKind::TripwireFired);
        let report = derive_firings_from_record(&log, &mgr, &firing).expect("derive");
        assert!(
            report.is_empty(),
            "a TripwireFired record must not produce another firing"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A failed derived append is a projection failure, not a gap: the source
    /// stays and no firing is claimed.
    #[test]
    fn failed_firing_append_is_reported_not_hidden() {
        let dir = tempdir("fail");
        let session = "rec-c2-1-fail";
        let log = open_log(&dir, session);
        let mgr = manager_with_main();

        let event = matching_event();
        let source_seq = append_raw(&log, &event);
        let source = log
            .handle()
            .read_from_seq(source_seq, 1)
            .expect("read")
            .records
            .into_iter()
            .next()
            .expect("source");

        // Seal the log so the derived append is refused.
        log.seal().expect("seal");
        let report = derive_firings_from_record(&log, &mgr, &source).expect("derive");

        assert!(report.appended.is_empty(), "nothing became durable");
        assert_eq!(report.failures.len(), 1, "the failure is surfaced");
        assert_eq!(report.failures[0].source_seq, source_seq);
        // The source evidence is untouched.
        let still = log
            .handle()
            .read_from_seq(source_seq, 1)
            .expect("read")
            .records;
        assert_eq!(still.len(), 1, "the accepted source is not reverted");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
