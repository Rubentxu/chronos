//! M7 — Events Read dispatcher (REC-C1.3 authoritative path).
//!
//! The v2 `events_read` tool is the single entry-point for agent-visible event
//! reads:
//!
//! | v1 tool | mode |
//! |---|---|
//! | `query_events` | `Query` |
//! | `get_event`    | `ById`  |
//!
//! ```text
//! events_read -> LiveProbeSession -> SessionExecutionLog -> EventSeq
//! ```
//!
//! `QueryEngine` and `DebugTraceService` no longer participate in this path:
//! the session-owned `ExecutionLog` is the authoritative source (REC-C1.2a)
//! and reads are stateless pages over an `EventSeq` position (REC-C1.3).
//!
//! ## Cursor semantics
//!
//! The cursor is opaque on the wire (`EventsCursorV1::encode`). A request
//! without a cursor starts at `seq#0`; the response carries `next_cursor` while
//! there is more evidence at or after the returned position.
//!
//! ## Completeness
//!
//! Reported as `"unknown"` until REC-C1.4 proves gap/completeness detection.
//! It is never `"complete"`: a completeness claim nothing backs would be a
//! Silent Lie.
//!
//! ## Offset/limit compatibility
//!
//! The deprecated `query_events` shim translates `offset` into a cursor
//! position at the MCP boundary and then calls this same dispatcher, so there
//! is exactly one real read implementation.

use chronos_domain::{EventType, TraceEvent};

use crate::error::ServiceError;
use crate::events_cursor::{EventsCursorError, EventsCursorV1};
use crate::events_log_read::{find_by_id, read_page, LogReadFilters};
use crate::output::{EventsReadKind, EventsReadOutput, EventsReadProvenance, QueryEventsResult};
use crate::session_log::{SessionExecutionLog, SessionExecutionLogRegistry};

/// Borrowed handle to the live-probe map (shared with the MCP server).
///
/// REC-C1.3: reads come from the session-owned log, so the context needs the
/// sessions — not the engine map.
pub struct EventsReadContext<'a> {
    /// REC-C1.3: reads resolve the log from the registry, which outlives the
    /// live-probe map. There is exactly one source; no live/finalized/engine
    /// chain to maintain.
    pub execution_logs: &'a SessionExecutionLogRegistry,
}

/// Input for [`ChronosEventsReadService::read`].
///
/// Carries the v2 `mode` discriminator plus the variant-specific
/// target fields. The dispatcher validates that the target field is
/// present when required by the mode.
#[derive(Debug, Clone)]
pub struct EventsReadInput {
    pub session_id: String,
    pub mode: EventsReadKind,
    // Query-mode fields
    pub event_types: Option<Vec<EventType>>,
    pub thread_id: Option<u64>,
    pub timestamp_start: Option<u64>,
    pub timestamp_end: Option<u64>,
    pub function_pattern: Option<String>,
    pub limit: usize,
    /// Opaque cursor for the next page (`EventsCursorV1::encode`). `None`
    /// starts at `seq#0`. A cursor minted for another session is refused.
    pub cursor: Option<String>,
    // ById-mode fields
    pub event_id: Option<u64>,
}

/// Stateless holder for the v2 events_read dispatcher.
///
/// The method is `async` only to acquire the engine-map lock; it releases
/// it as soon as the engine reference is in scope. Callers must drop the
/// returned outputs before any subsequent call that needs the map.
pub struct ChronosEventsReadService;

impl ChronosEventsReadService {
    /// Dispatch the v2 `events_read` request to the matching v1 algorithm.
    ///
    /// Required parameters per mode:
    /// - `ById`  → `event_id` (required)
    /// - `Query` → (no required target; filters are all optional)
    ///
    /// Returns [`ServiceError::InvalidInput`] when a required field is
    /// missing. The MCP wrapper forwards this verbatim to the agent.
    pub async fn read(
        ctx: &EventsReadContext<'_>,
        input: EventsReadInput,
    ) -> Result<EventsReadOutput, ServiceError> {
        match input.mode {
            EventsReadKind::Query => Self::query(ctx, input).await,
            EventsReadKind::ById => Self::by_id(ctx, input).await,
        }
    }

    fn session_log(
        ctx: &EventsReadContext<'_>,
        session_id: &str,
    ) -> Result<SessionExecutionLog, ServiceError> {
        ctx.execution_logs.get(session_id)
    }

    async fn query(
        ctx: &EventsReadContext<'_>,
        input: EventsReadInput,
    ) -> Result<EventsReadOutput, ServiceError> {
        let session_id = input.session_id.clone();
        let owned = Self::session_log(ctx, &session_id)?;

        // No cursor means "start at seq#0", inclusive.
        let cursor = match input.cursor.as_deref() {
            Some(raw) => EventsCursorV1::decode_for_session(raw, owned.session_id())
                .map_err(map_cursor_error)?,
            None => owned.cursor_start(),
        };

        let filters = LogReadFilters {
            event_types: input.event_types.clone(),
            thread_id: input.thread_id,
            timestamp_start: input.timestamp_start,
            timestamp_end: input.timestamp_end,
            function_pattern: input.function_pattern.clone(),
        };
        let page = read_page(&owned, &cursor, input.limit, &filters)?;

        // `next_cursor` is emitted while evidence may still exist at or after the
        // returned position; the log tail is the honest bound.
        let more = owned
            .handle()
            .tail_seq()
            .map(|tail| page.position_after.0 <= tail.0)
            .unwrap_or(false);
        let next_cursor = if more { Some(page.next.encode()) } else { None };

        // Raw gaps are reported as observed facts; interpreting them into
        // Partial/GapDetected/Unknown is REC-C1.4.
        let gap_summary: Vec<serde_json::Value> = page
            .gaps
            .iter()
            .map(|g| {
                serde_json::json!({
                    "first_missing": g.first_missing.0,
                    "last_missing": g.last_missing.0,
                    "reason": format!("{:?}", g.reason),
                    "source": g.source,
                })
            })
            .collect();

        let events = page.records;
        let result = QueryEventsResult {
            result: chronos_domain::query::QueryResult {
                // Page-scoped count. The authoritative completeness answer is
                // `completeness` (currently "unknown"); this legacy field must
                // not be read as a global total on the log-backed path.
                total_matching: events.len() as u64,
                events,
                next_offset: None,
            },
        };

        Ok(EventsReadOutput::Query {
            session_id,
            result,
            next_cursor,
            completeness: page.completeness,
            gap_summary: if gap_summary.is_empty() {
                None
            } else {
                Some(gap_summary)
            },
            provenance: EventsReadProvenance {
                source: "execution_log".into(),
                session_id: input.session_id,
            },
        })
    }

    async fn by_id(
        ctx: &EventsReadContext<'_>,
        input: EventsReadInput,
    ) -> Result<EventsReadOutput, ServiceError> {
        let event_id = input
            .event_id
            .ok_or_else(|| ServiceError::InvalidInput("event_id required for mode=by_id".into()))?;

        let session_id = input.session_id.clone();
        let owned = Self::session_log(ctx, &session_id)?;
        let event: Option<TraceEvent> = find_by_id(&owned, event_id)?;

        Ok(EventsReadOutput::ById {
            session_id: session_id.clone(),
            event,
            provenance: EventsReadProvenance {
                source: "execution_log".into(),
                session_id,
            },
        })
    }
}

/// Map a cursor validation failure onto the service error surface.
///
/// A cursor from another session is a caller bug, not a malformed payload, and
/// stays distinguishable.
fn map_cursor_error(err: EventsCursorError) -> ServiceError {
    match err {
        EventsCursorError::Malformed { .. } | EventsCursorError::UnsupportedVersion { .. } => {
            ServiceError::InvalidCursorPayload
        }
        EventsCursorError::WrongSession { expected, found } => ServiceError::InvalidInput(format!(
            "cursor belongs to session {found:?}, not {expected:?}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::EventsReadKind;
    use chronos_domain::{EventData, EventType, SourceLocation};
    use chronos_log::{ExecutionPayload, NewExecutionRecord, SessionId};
    struct Fixture {
        registry: SessionExecutionLogRegistry,
        session_id: String,
        _log: SessionExecutionLog,
    }

    impl Fixture {
        fn new(tag: &str, n: usize) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "rec-c1-3-dispatch-{tag}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let session_id = format!("sess-{tag}");
            let owned = crate::session_log::SessionExecutionLog::open(
                &dir,
                SessionId::new(session_id.clone()),
            )
            .expect("log");
            let handle = owned.handle();
            for i in 0..n {
                let event = TraceEvent::new(
                    i as u64,
                    i as u64 * 10,
                    1,
                    EventType::FunctionEntry,
                    SourceLocation {
                        file: Some("f.rs".into()),
                        line: Some(i as u32),
                        column: None,
                        function: Some(format!("fn_{i}")),
                        address: 0,
                    },
                    EventData::Empty,
                );
                handle
                    .append(NewExecutionRecord {
                        session_id: handle.session_id().clone(),
                        monotonic_ns: i as u64 * 10,
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

            let registry = SessionExecutionLogRegistry::new();
            registry.register(owned.clone()).expect("register");
            Self {
                registry,
                session_id,
                _log: owned,
            }
        }

        fn ctx(&self) -> EventsReadContext<'_> {
            EventsReadContext {
                execution_logs: &self.registry,
            }
        }

        fn input(&self, limit: usize, cursor: Option<String>) -> EventsReadInput {
            EventsReadInput {
                session_id: self.session_id.clone(),
                mode: EventsReadKind::Query,
                event_types: None,
                thread_id: None,
                timestamp_start: None,
                timestamp_end: None,
                function_pattern: None,
                limit,
                cursor,
                event_id: None,
            }
        }
    }

    #[tokio::test]
    async fn query_reads_from_the_session_log_with_an_opaque_cursor() {
        let fx = Fixture::new("q", 25);
        let out = ChronosEventsReadService::read(&fx.ctx(), fx.input(10, None))
            .await
            .expect("read");
        match out {
            EventsReadOutput::Query {
                result,
                next_cursor,
                completeness,
                provenance,
                ..
            } => {
                assert_eq!(result.result.events.len(), 10);
                assert_eq!(result.result.events[0].event_id, 0, "seq#0 included");
                assert_eq!(
                    completeness.status.as_str(),
                    "complete",
                    "a bounded examined range with no gap is provably complete"
                );
                assert_eq!(completeness.scope, "examined_range");
                assert_eq!(provenance.source, "execution_log");
                let cursor = next_cursor.expect("more evidence remains");
                assert!(
                    cursor.starts_with("ecv1:"),
                    "cursor must be opaque: {cursor}"
                );
            }
            other => panic!("expected Query, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn query_resumes_exactly_from_the_returned_cursor() {
        let fx = Fixture::new("resume", 25);
        let first = ChronosEventsReadService::read(&fx.ctx(), fx.input(10, None))
            .await
            .unwrap();
        let cursor = match first {
            EventsReadOutput::Query { next_cursor, .. } => next_cursor.unwrap(),
            _ => unreachable!(),
        };
        let second = ChronosEventsReadService::read(&fx.ctx(), fx.input(10, Some(cursor)))
            .await
            .unwrap();
        match second {
            EventsReadOutput::Query { result, .. } => {
                assert_eq!(result.result.events[0].event_id, 10);
                assert_eq!(result.result.events.len(), 10);
            }
            other => panic!("expected Query, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_cursor_from_another_session_is_refused() {
        let a = Fixture::new("a", 5);
        let b = Fixture::new("b", 5);
        let err =
            ChronosEventsReadService::read(&a.ctx(), a.input(10, Some("ecv1:1:5:b-xyz:0".into())))
                .await
                .unwrap_err();
        // Session "b-xyz" is not the session this context serves.
        assert!(matches!(err, ServiceError::InvalidInput(_)), "{err:?}");
        let _ = b;
    }

    #[tokio::test]
    async fn by_id_reads_from_the_log() {
        let fx = Fixture::new("byid", 5);
        let mut input = fx.input(10, None);
        input.mode = EventsReadKind::ById;
        input.event_id = Some(3);
        match ChronosEventsReadService::read(&fx.ctx(), input)
            .await
            .unwrap()
        {
            EventsReadOutput::ById {
                event, provenance, ..
            } => {
                assert_eq!(event.expect("found").event_id, 3);
                assert_eq!(provenance.source, "execution_log");
            }
            other => panic!("expected ById, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn unknown_session_reports_unavailable_not_a_fallback() {
        let fx = Fixture::new("missing", 3);
        let mut input = fx.input(10, None);
        input.session_id = "nope".into();
        let err = ChronosEventsReadService::read(&fx.ctx(), input)
            .await
            .unwrap_err();
        assert!(
            matches!(err, ServiceError::ExecutionLogUnavailable { .. }),
            "a missing log must never fall back to another source: {err:?}"
        );
    }
}
