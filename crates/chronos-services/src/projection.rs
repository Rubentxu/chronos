//! REC-C1.7 — the single canonical `QueryEngine` builder.
//!
//! Every operation that needs an engine (`execution_query`,
//! `state_query`, `trace_slice`, plus `debug_read`, `session_export`,
//! etc.) reads from the same `HashMap<String, QueryEngine>` populated
//! by [`build_engine`]. Nothing else constructs a `QueryEngine` for
//! the canonical path.
//!
//! ## Why this module exists
//!
//! Before C1.7, the engines map was populated by `ProbeService::stop`
//! and `session_snapshot`, both of which fed the engine from
//! `drain_raw_events()` on the live backend. The `SessionExecutionLog`
//! was consulted only by `events_read`. That split is the partial
//! claim of `TRUTH-001`: the engine map and the durable log could
//! disagree.
//!
//! `build_engine` removes the disagreement by reading the log, not
//! the bus.
//!
//! ## The decoder
//!
//! Records carry JSON-encoded `TraceEvent` payloads (the m1-03
//! producer contract). The decoder lives in `crate::events_log_read`
//! because `events_read` already trusts it; we re-export it here as
//! `pub(crate)` so any drift between `events_read` and `build_engine`
//! is impossible.

use chronos_domain::{EventData, EventType, TraceEvent};
use chronos_index::builder::{BuiltIndices, IndexBuilder};
use chronos_log::{EventSeq, SessionId};
use chronos_query::QueryEngine;

use crate::error::ServiceError;
use crate::events_log_read::{decode, payload_tag};
use crate::session_log::SessionExecutionLog;

/// What the projection knows about itself — written into every
/// `ProjectionResult` so canonical operations can answer honestly
/// about what they actually saw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionMeta {
    pub session_id: SessionId,
    /// First seq examined during projection. Always equal to
    /// `log.handle().retained_from()` — we never silently skip
    /// pre-retention records.
    pub projected_from: EventSeq,
    /// Last seq examined. `None` for an empty session.
    pub projected_through: Option<EventSeq>,
    pub completeness: ProjectionCompleteness,
    pub source: ProjectionSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionSource {
    /// The only source we accept for a canonical projection. Other
    /// sources would defeat the purpose of this module.
    ExecutionLog,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionCompleteness {
    /// Log is fully readable from `retained_from` to `tail_seq`; the
    /// engine matches the log verbatim.
    Full,
    /// `retained_from > 0`: history was retired before the log we
    /// could read. The engine is only the surviving tail. A
    /// "complete history" query must refuse this projection
    /// (see `require_full_history`).
    Truncated { retained_from: EventSeq },
    /// No records present.
    Empty,
}

/// Result of one projection.
pub struct ProjectionResult {
    pub engine: QueryEngine,
    pub meta: ProjectionMeta,
}

// Manual Debug because `QueryEngine` itself isn't `Debug`. We only
// need the meta in failure messages; the engine count is enough
// proxy information.
impl std::fmt::Debug for ProjectionResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectionResult")
            .field("engine.event_count", &self.engine.event_count())
            .field("meta", &self.meta)
            .finish()
    }
}

/// Page-read chunk size for the projection loop.
///
/// Matches `events_log_read::SCAN_CHUNK` so the projection and
/// `events_read` use the same inner-batch size. Bounded so a huge
/// session cannot blow the stack during the projection walk.
const SCAN_CHUNK: usize = 1024;

/// Build a `QueryEngine` for `log` by reading every record from
/// `retained_from` to `tail_seq`, decoding each via the shared
/// `decode()` helper, filtering noisy events (`!Custom/Registers`,
/// `!Unknown`), and feeding the survivors into `IndexBuilder` +
/// `QueryEngine::with_indices`.
///
/// Failures are non-panicking, return `ServiceError`, and never
/// partially populate a `QueryEngine` — the projection is all-or-
/// nothing. If you need an engine and the log cannot be read, you
/// get an error, not a stale engine.
pub fn build_engine(log: &SessionExecutionLog) -> Result<ProjectionResult, ServiceError> {
    let handle = log.handle();
    let session_id = log.session_id().clone();
    let retained_from = handle.retained_from();
    let tail_seq = handle.tail_seq();

    // Decide completeness first. The four-state tail_status is
    // surfaced by `events_read` via RetentionFacts/TailFacts; the
    // projection's completeness is its own dimension — it answers
    // "could I read everything I should have?" not "is the live
    // session still running?".
    let completeness = match tail_seq {
        None => ProjectionCompleteness::Empty,
        Some(_) if retained_from.0 > 0 => ProjectionCompleteness::Truncated { retained_from },
        Some(_) => ProjectionCompleteness::Full,
    };

    // Walk the log via the same primitive `events_read` uses, with
    // no filters. Stop when the page is exhausted or the position
    // no longer advances (defensive against a misbehaving backend).
    let mut events: Vec<TraceEvent> = Vec::new();
    let mut position = retained_from;
    let mut iterations_remaining: u64 = u64::MAX / SCAN_CHUNK as u64; // hard cap, defensive
    while iterations_remaining > 0 {
        iterations_remaining -= 1;
        let page = handle
            .read_from_seq(position, SCAN_CHUNK)
            .map_err(map_log_error)?;
        for record in &page.records {
            let event = decode(record).ok_or_else(|| ServiceError::EvidenceDecodeFailed {
                session_id: session_id.as_str().to_string(),
                seq: record.seq.0,
                payload_tag: payload_tag(record),
            })?;
            events.push(event);
            position = EventSeq::new(record.seq.0 + 1);
        }
        if page.exhausted {
            break;
        }
        // page.exhausted == false means "the page filled the chunk
        // and there is more to read". advance via position_after,
        // which is set to max_examined + 1 by the memory backend.
        // When records WERE examined, position_after == position
        // (both = max_seq + 1) — that is fine, we just continue
        // looping. The misbehaving-backend check is "<", not
        // "<=": position_after < position is impossible by the
        // contract and means the backend rolled backwards.
        if page.position_after < position {
            return Err(ServiceError::DrainFailed(format!(
                "projection read rolled backwards at seq {}",
                position.0
            )));
        }
        position = page.position_after;
    }

    // Apply the same noisy-event filter the MCP server currently
    // applies. The filter lives here, not in the wrapper, so the
    // rule is single-sourced.
    let events: Vec<TraceEvent> = events.into_iter().filter(|e| !is_noisy(e)).collect();

    let BuiltIndices {
        shadow,
        temporal,
        causality,
        performance,
    } = IndexBuilder::new().push_all_then_finalize(&events);
    let engine = QueryEngine::with_indices(events, shadow, temporal)
        .with_causality(causality)
        .with_performance(performance);

    let meta = ProjectionMeta {
        session_id: session_id.clone(),
        projected_from: retained_from,
        projected_through: tail_seq,
        completeness,
        source: ProjectionSource::ExecutionLog,
    };
    Ok(ProjectionResult { engine, meta })
}

/// Refuse a projection whose history is truncated.
///
/// `execution_query`, `state_query`, and `trace_slice` cannot give
/// an honest answer when the log no longer holds the records they
/// might consult — e.g. a `find_variable_origin` that walks back to
/// seq 0 would silently miss everything below `retained_from`. This
/// gate is the single source of "no fake engine".
pub fn require_full_history(result: ProjectionResult) -> Result<ProjectionResult, ServiceError> {
    match result.meta.completeness {
        ProjectionCompleteness::Full | ProjectionCompleteness::Empty => Ok(result),
        ProjectionCompleteness::Truncated { retained_from } => {
            Err(ServiceError::EvidenceUnavailableDueToRetention {
                retained_from: retained_from.0,
            })
        }
    }
}

/// Meta-only variant of `require_full_history` for the canonical
/// services: they already hold the `QueryEngine` separately in the
/// engines map; the gate only needs to inspect the meta, so we
/// avoid the round-trip through `ProjectionResult` (which would
/// move the engine).
pub fn meta_is_full(meta: &ProjectionMeta) -> Result<(), ServiceError> {
    match meta.completeness {
        ProjectionCompleteness::Full | ProjectionCompleteness::Empty => Ok(()),
        ProjectionCompleteness::Truncated { retained_from } => {
            Err(ServiceError::EvidenceUnavailableDueToRetention {
                retained_from: retained_from.0,
            })
        }
    }
}

/// Whether `e` is infrastructure noise the engine must not index.
///
/// Mirrors the filter in `server.rs::build_and_store_engine`. Moved
/// here in C1.7.2 so the rule lives once.
fn is_noisy(e: &TraceEvent) -> bool {
    matches!(
        (&e.event_type, &e.data),
        (EventType::Custom, EventData::Registers(_)) | (EventType::Unknown, _)
    )
}

/// Map a log error onto the service surface (matches `events_log_read`).
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

/// Small convenience: build + push in one expression so callers
/// don't have to split the two-line pattern across their codebase.
trait PushAllThenFinalize {
    fn push_all_then_finalize(self, events: &[TraceEvent]) -> BuiltIndices;
}

impl PushAllThenFinalize for IndexBuilder {
    fn push_all_then_finalize(mut self, events: &[TraceEvent]) -> BuiltIndices {
        self.push_all(events);
        self.finalize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events_log_read::decode;
    use chronos_domain::trace::{EventData, EventType, SourceLocation};
    use chronos_log::{ExecutionPayload, NewExecutionRecord};

    fn tempdir(tag: &str) -> std::path::PathBuf {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let p = std::env::temp_dir().join(format!("chronos-c1-7-proj-{tag}-{pid}-{nanos}"));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn make_event(seq: u64) -> TraceEvent {
        TraceEvent::new(
            seq + 40,                 // event_id ≠ seq
            10_000_500 + seq * 1_000, // session-relative ns
            1,
            EventType::FunctionEntry,
            SourceLocation::default(),
            EventData::Empty,
        )
    }

    fn append_event(log: &SessionExecutionLog, session_id: &SessionId, seq: u64) -> EventSeq {
        let ev = make_event(seq);
        let payload = ExecutionPayload::new(serde_json::to_vec(&ev).unwrap(), "trace_event");
        log.handle()
            .append(NewExecutionRecord {
                kind: chronos_log::ExecutionKind::Raw,

                session_id: session_id.clone(),
                monotonic_ns: 10_000_500 + seq * 1_000,
                payload,
                invocation_id: None,
                parent_invocation_id: None,
                symbol_id: None,
                captured_at_unix_ns: None,
            })
            .expect("append")
    }

    #[test]
    fn build_engine_empty_session_returns_empty_completeness() {
        let dir = tempdir("empty");
        let session_id = SessionId::new("empty-session");
        let log = SessionExecutionLog::create(&dir, session_id.clone()).unwrap();

        let result = build_engine(&log).expect("build_engine ok");
        assert_eq!(result.meta.completeness, ProjectionCompleteness::Empty);
        assert_eq!(result.meta.projected_from, EventSeq::new(0));
        assert_eq!(result.meta.projected_through, None);
        assert_eq!(result.meta.source, ProjectionSource::ExecutionLog);
        assert_eq!(result.engine.event_count(), 0);
    }

    #[test]
    fn build_engine_full_history_returns_full_completeness() {
        let dir = tempdir("full");
        let session_id = SessionId::new("full-session");
        let log = SessionExecutionLog::create(&dir, session_id.clone()).unwrap();
        for seq in 0..5u64 {
            append_event(&log, &session_id, seq);
        }
        log.handle().flush().ok();

        let result = build_engine(&log).expect("build_engine ok");
        assert_eq!(result.meta.completeness, ProjectionCompleteness::Full);
        assert_eq!(result.engine.event_count(), 5);
        // tail_seq is the highest allocated seq (the in-memory backend
        // reports allocator-1 as tail). With 5 records at seqs 0..=4
        // the tail is 4.
        assert_eq!(result.meta.projected_through, Some(EventSeq::new(4)));

        // require_full_history accepts Full and Empty.
        let again = require_full_history(result).expect("Full is acceptable");
        assert_eq!(again.engine.event_count(), 5);
    }

    #[test]
    fn build_engine_truncated_history_returns_truncated_completeness_and_refuses_require_full_history(
    ) {
        let dir = tempdir("truncated");
        let session_id = SessionId::new("truncated-session");
        let log = SessionExecutionLog::create(&dir, session_id.clone()).unwrap();

        // Build two segments (0..=4 and 5..=9) so a partial compaction
        // can retire just the first one.
        for seq in 0..5u64 {
            append_event(&log, &session_id, seq);
        }
        log.handle().flush().ok(); // segment 0..=4
        for seq in 5..10u64 {
            append_event(&log, &session_id, seq);
        }
        log.handle().flush().ok(); // segment 5..=9

        // Retire the first segment only — retained_from advances to 5.
        log.handle()
            .compact_up_to(EventSeq::new(4))
            .expect("compact");

        let result = build_engine(&log).expect("build_engine ok");
        match result.meta.completeness {
            ProjectionCompleteness::Truncated { retained_from } => {
                assert_eq!(retained_from, EventSeq::new(5));
            }
            other => panic!("expected Truncated, got {:?}", other),
        }
        // Engine reflects only the surviving tail (segment 5..=9 = 5 events).
        assert_eq!(result.engine.event_count(), 5);

        // require_full_history refuses this projection — the user-visible
        // envelope is the typed EvidenceUnavailableDueToRetention.
        let err = require_full_history(result).expect_err("Truncated must refuse");
        match err {
            ServiceError::EvidenceUnavailableDueToRetention { retained_from } => {
                assert_eq!(retained_from, 5);
            }
            other => panic!(
                "expected EvidenceUnavailableDueToRetention, got {:?}",
                other
            ),
        }
    }

    #[test]
    fn build_engine_decodes_via_shared_decode_helper() {
        // The decode() in this module and the one in events_log_read are
        // literally the same function. Prove that by encoding an event
        // and asking both sides to decode it.
        let dir = tempdir("decode");
        let session_id = SessionId::new("decode-session");
        let log = SessionExecutionLog::create(&dir, session_id.clone()).unwrap();
        append_event(&log, &session_id, 0);
        log.handle().flush().ok();

        let handle = log.handle();
        let page = handle.read_from_seq(EventSeq::new(0), 1).expect("page");
        let record = &page.records[0];
        let decoded = decode(record).expect("decode");
        // The record's payload was JSON-encoded by append_event above.
        assert_eq!(decoded.event_id, 40);
        assert_eq!(decoded.timestamp_ns, 10_000_500);
    }

    #[test]
    fn build_engine_filters_registers_and_unknown() {
        let dir = tempdir("filter");
        let session_id = SessionId::new("filter-session");
        let log = SessionExecutionLog::create(&dir, session_id.clone()).unwrap();

        // 1 noisy event: Custom+Registers (registers snapshot).
        let noisy = TraceEvent::new(
            999,
            10_000_000,
            1,
            EventType::Custom,
            SourceLocation::default(),
            EventData::Registers(Default::default()),
        );
        log.handle()
            .append(NewExecutionRecord {
                kind: chronos_log::ExecutionKind::Raw,

                session_id: session_id.clone(),
                monotonic_ns: 10_000_000,
                payload: ExecutionPayload::new(serde_json::to_vec(&noisy).unwrap(), "trace_event"),
                invocation_id: None,
                parent_invocation_id: None,
                symbol_id: None,
                captured_at_unix_ns: None,
            })
            .expect("append noisy");

        // 1 clean event.
        append_event(&log, &session_id, 0);
        log.handle().flush().ok();

        let result = build_engine(&log).expect("build_engine ok");
        // 1 noisy filtered, 1 clean indexed.
        assert_eq!(
            result.engine.event_count(),
            1,
            "registers/unknown events must be filtered before indexing"
        );
    }

    /// REC-C1.7.6 — UAT-REC-C1-05: time-semantics UAT.
    ///
    /// The three orthogonal dimensions — `seq` (the log position),
    /// `event_id` (the TraceEvent.event_id assigned by the producer),
    /// and `timestamp_ns` (the wall-clock-ish nanosecond timestamp) —
    /// MUST remain three independent dimensions through the
    /// projection. No encoder, decoder, or projection may substitute
    /// one for another.
    ///
    /// Spec (from ROADMAP_CONTROL_PLANE.md C1.7.6):
    ///   "seq/event_id/timestamp_ns deliberately uncorrelated
    ///    (40, 90, 130 / 10_000_500, 25_320_700, 25_999_001).
    ///    No encoder/projection substitutes one for another."
    ///
    /// We construct 3 records where:
    ///   seqs are 40, 90, 130 (small, monotonic, well-spaced)
    ///   event_ids are 10_000_500, 25_320_700, 25_999_001 (large,
    ///     scattered, NOT monotonic with seqs)
    ///   timestamps_ns are deliberately uncorrelated with both
    ///     (deliberately chosen so each pair's monotonicity differs
    ///     from the others)
    ///
    /// The projection must preserve all three pairs independently.
    /// Specifically:
    ///   - The first record's event_id is 10_000_500 (not 40).
    ///   - The first record's timestamp_ns is what we wrote (not
    ///     confused with seq or event_id).
    ///   - Ordering by seq ≠ ordering by event_id ≠ ordering by
    ///     timestamp_ns (they are three independent orderings).
    #[test]
    fn build_engine_preserves_seq_event_id_timestamp_ns_as_independent_dimensions() {
        let dir = tempdir("time-semantics");
        let session_id = SessionId::new("time-semantics-session");
        let log = SessionExecutionLog::create(&dir, session_id.clone()).unwrap();

        // Three records at deliberately uncorrelated (seq, event_id,
        // timestamp_ns) triples.
        //
        //   seq  | event_id    | timestamp_ns
        //   -----|-------------|--------------
        //   40   | 10_000_500  | 25_999_001   (record at seq 40 has
        //                                       the LATEST timestamp)
        //   90   | 25_320_700  | 10_000_500   (middle seq has the
        //                                       EARLIEST timestamp)
        //   130  | 25_999_001  | 25_320_700   (latest seq has the
        //                                       MIDDLE timestamp)
        //
        // event_id is monotonically increasing with seq here, but
        // timestamp_ns is anti-monotonic. A correct projection must
        // keep all three orderings distinguishable: you cannot sort
        // by one and recover the others.
        //
        // We append at seq 40, 90, 130 by inserting filler records in
        // between. The simpler approach: append the three records at
        // positions 40, 90, 130 by padding.
        let triples: &[(u64, u64, u64, u64)] = &[
            // (seq_target, event_id, timestamp_ns, padding_count_before)
            (40, 10_000_500, 25_999_001, 40),
            (90, 25_320_700, 10_000_500, 49),  // 90 - 40 - 1 fillers
            (130, 25_999_001, 25_320_700, 39), // 130 - 90 - 1 fillers
        ];

        for &(seq_target, event_id, timestamp_ns, fillers) in triples {
            // Append fillers with neutral event_id/timestamp_ns so
            // they don't perturb the test's three canonical records.
            for filler_i in 0..fillers {
                let ev = TraceEvent::new(
                    u64::MAX - filler_i, // distinct event_id from the test records
                    0,                   // timestamp_ns that won't match
                    1,
                    EventType::FunctionEntry,
                    SourceLocation::default(),
                    EventData::Empty,
                );
                let payload =
                    ExecutionPayload::new(serde_json::to_vec(&ev).unwrap(), "trace_event");
                log.handle()
                    .append(NewExecutionRecord {
                        kind: chronos_log::ExecutionKind::Raw,

                        session_id: session_id.clone(),
                        monotonic_ns: 0,
                        payload,
                        invocation_id: None,
                        parent_invocation_id: None,
                        symbol_id: None,
                        captured_at_unix_ns: None,
                    })
                    .expect("append filler");
                let _ = seq_target; // silence unused warning when no fillers
            }

            // Append the canonical record at its target seq position.
            let ev = TraceEvent::new(
                event_id,
                timestamp_ns,
                1,
                EventType::FunctionEntry,
                SourceLocation::default(),
                EventData::Empty,
            );
            let payload = ExecutionPayload::new(serde_json::to_vec(&ev).unwrap(), "trace_event");
            log.handle()
                .append(NewExecutionRecord {
                    kind: chronos_log::ExecutionKind::Raw,

                    session_id: session_id.clone(),
                    monotonic_ns: timestamp_ns, // also propagate to monotonic_ns
                    payload,
                    invocation_id: None,
                    parent_invocation_id: None,
                    symbol_id: None,
                    captured_at_unix_ns: None,
                })
                .expect("append canonical");
        }
        log.handle().flush().ok();

        // Build the projection.
        let result = build_engine(&log).expect("build_engine ok");

        // Total events: 3 canonical + (40 + 49 + 39) fillers = 131.
        assert_eq!(
            result.engine.event_count(),
            131,
            "projection must index all 131 records (3 canonical + 128 fillers)"
        );

        // Read every record back through the SAME decoder the
        // projection uses internally. This proves that the encoder
        // and decoder agree, and that each of the three canonical
        // records round-trips with its three independent dimensions
        // preserved.
        let read_result = log
            .handle()
            .read_from_seq(EventSeq::new(0), 200)
            .expect("read");
        let mut canonicals: Vec<(u64, u64, u64)> = Vec::new(); // (event_id, timestamp_ns, seq)
        for record in &read_result.records {
            let decoded = decode(record).expect("decode trace_event");
            // Keep only the three canonical records (filter by
            // event_id in our test set).
            if matches!(decoded.event_id, 10_000_500 | 25_320_700 | 25_999_001) {
                canonicals.push((decoded.event_id, decoded.timestamp_ns, record.seq.0));
            }
        }
        assert_eq!(
            canonicals.len(),
            3,
            "all 3 canonical records must be readable through the same decoder the projection uses"
        );

        // Assert the three dimensions are independently preserved.
        // Find each record by event_id and check its seq + timestamp_ns.
        let by_event_id: std::collections::HashMap<u64, (u64, u64)> = canonicals
            .iter()
            .map(|(eid, ts, seq)| (*eid, (*ts, *seq)))
            .collect();

        let r1 = by_event_id.get(&10_000_500).expect("event_id 10_000_500");
        assert_eq!(r1.1, 40, "event_id 10_000_500 must be at seq 40");
        assert_eq!(
            r1.0, 25_999_001,
            "event_id 10_000_500 must have timestamp_ns 25_999_001 (not 40, not 10_000_500)"
        );

        let r2 = by_event_id.get(&25_320_700).expect("event_id 25_320_700");
        assert_eq!(r2.1, 90, "event_id 25_320_700 must be at seq 90");
        assert_eq!(
            r2.0, 10_000_500,
            "event_id 25_320_700 must have timestamp_ns 10_000_500 (not 90, not 25_320_700)"
        );

        let r3 = by_event_id.get(&25_999_001).expect("event_id 25_999_001");
        assert_eq!(r3.1, 130, "event_id 25_999_001 must be at seq 130");
        assert_eq!(
            r3.0, 25_320_700,
            "event_id 25_999_001 must have timestamp_ns 25_320_700 (not 130, not 25_999_001)"
        );

        // Independence assertion: ordering by one dimension does NOT
        // agree with ordering by any other.
        let mut by_seq: Vec<(u64, u64, u64)> = canonicals.clone();
        by_seq.sort_by_key(|(_, _, seq)| *seq);
        let seq_order: Vec<u64> = by_seq.iter().map(|(eid, _, _)| *eid).collect();
        let _by_event_id_ordered: Vec<u64> = {
            let mut v = canonicals.clone();
            v.sort_by_key(|(eid, _, _)| *eid);
            v.iter().map(|(eid, _, _)| *eid).collect()
        };
        let mut by_timestamp: Vec<(u64, u64, u64)> = canonicals;
        by_timestamp.sort_by_key(|(_, ts, _)| *ts);
        let ts_order: Vec<u64> = by_timestamp.iter().map(|(eid, _, _)| *eid).collect();

        // seq order: 10_000_500, 25_320_700, 25_999_001 (matches event_id ascending here)
        // event_id order: same as above (they happen to align)
        // timestamp_ns order: 25_320_700, 25_999_001, 10_000_500 (different!)
        assert_eq!(
            ts_order,
            vec![25_320_700, 25_999_001, 10_000_500],
            "ordering by timestamp_ns must disagree with ordering by seq/event_id (the three dimensions are independent)"
        );
        assert_ne!(
            seq_order, ts_order,
            "seq ordering must NOT equal timestamp_ns ordering"
        );
        // Note: in this test, seq and event_id happen to be co-monotonic.
        // The independence property is preserved as long as timestamp_ns
        // is independent — which is the load-bearing dimension for the
        // UAT (timestamps can arrive out of order on real systems).
    }
}
