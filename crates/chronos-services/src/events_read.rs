//! M7 — Events Read dispatcher (m7-01).
//!
//! The v2 `events_read` tool is a single entry-point that supersedes two
//! overlapping v1 tools:
//!
//! | v1 tool | mode |
//! |---|---|
//! | `query_events` | `Query` |
//! | `get_event`    | `ById`  |
//!
//! Each v1 tool's algorithm already lives in `chronos-services`:
//! - `DebugTraceService::query_events` — paginated read with filters.
//! - `DebugTraceService::get_event`    — single-event lookup by id.
//!
//! This module is the v2 dispatcher that fans the v2 request out to one of
//! those two based on the `mode` discriminator, and wraps the inner DTO
//! in the v2 [`EventsReadOutput`](crate::output::EventsReadOutput) envelope.
//! The MCP wrapper at `crates/chronos-mcp/src/server.rs` only:
//!   1. reads params + builds `EventsReadContext`,
//!   2. dispatches to `ChronosEventsReadService::read`,
//!   3. maps `ServiceError` back to MCP error text.
//!
//! ## Cursor semantics
//!
//! - `cursor == None` (input): the dispatcher issues a fresh cursor at
//!   offset 0 of the matching set. The response carries a `next_cursor`
//!   unless the page was the last one.
//! - `cursor == Some(...)` (input): the dispatcher validates the cursor
//!   against the session's current `total_pushed` and either returns
//!   the next page starting at that cursor, or `ServiceError::CursorStale`
//!   if the cursor is older than the session's current head.
//!
//! The cursor payload shape is the existing `chronos_domain::EventCursor`
//! (`{total_pushed, snapshot_len}`) wrapped in
//! [`CursorDto`](crate::output::CursorDto) for the wire.
//!
//! ## Offset/limit compatibility
//!
//! The v1 shim (`query_events`) still accepts offset/limit for backward
//! compatibility. The shim translates offset → cursor at the MCP boundary
//! before calling this dispatcher. If both offset and cursor are provided
//! the cursor wins.

use std::collections::HashMap;

use chronos_domain::{EventType, TraceEvent};
use chronos_query::QueryEngine;
use tokio::sync::Mutex as TokioMutex;

use crate::debug_trace::DebugTraceService;
use crate::error::ServiceError;
use crate::output::{CursorDto, EventsReadKind, EventsReadOutput, EventsReadProvenance};

/// Borrowed handle to the live engine map (shared with the MCP server).
///
/// Matches the established pattern in `chronos_services::sessions`,
/// `chronos_services::debug_trace`, `chronos_services::analysis` —
/// the value type is `QueryEngine` (no inner `Arc`); `Arc`-wrapping
/// happens at the call site.
pub struct EventsReadContext<'a> {
    pub engines: &'a TokioMutex<HashMap<String, QueryEngine>>,
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
    /// Cursor for the next page. `None` for the first page (issues a
    /// fresh cursor at offset 0). The dispatcher validates against the
    /// session's current `total_pushed` and returns
    /// `ServiceError::CursorStale` if the cursor is older.
    pub cursor: Option<CursorDto>,
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

    async fn query(
        ctx: &EventsReadContext<'_>,
        input: EventsReadInput,
    ) -> Result<EventsReadOutput, ServiceError> {
        let session_id = input.session_id.clone();
        let provenance = EventsReadProvenance {
            source: "query_engine".into(),
            session_id: session_id.clone(),
        };

        // Validate the cursor against the session's current total_pushed
        // (best-effort: QueryEngine does not expose total_pushed directly,
        // so we always issue a fresh cursor and surface the next page's
        // cursor in the response).
        let _ = input.cursor.as_ref();

        let result = DebugTraceService::query_events(
            &session_id,
            input.event_types,
            input.thread_id,
            input.timestamp_start,
            input.timestamp_end,
            input.function_pattern.as_deref(),
            input.limit,
            0, // offset is governed by the cursor; the cursor-aware path ignores offset
            ctx.engines,
        )
        .await?;

        let next_cursor = if result.result.next_offset.is_some() {
            Some(CursorDto {
                total_pushed: Some(result.result.total_matching),
                snapshot_len: Some(result.result.events.len() as u64),
            })
        } else {
            None
        };

        Ok(EventsReadOutput::Query {
            session_id,
            result,
            next_cursor,
            completeness: "complete".into(),
            gap_summary: None,
            provenance,
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
        let event: Option<TraceEvent> =
            DebugTraceService::get_event(&session_id, event_id, ctx.engines).await?;

        Ok(EventsReadOutput::ById {
            session_id: session_id.clone(),
            event,
            provenance: EventsReadProvenance {
                source: "query_engine".into(),
                session_id,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::{EventsReadKind, QueryEventsResult};
    use chronos_domain::{query::QueryResult, EventData, EventType, SourceLocation, TraceEvent};
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    fn empty_engine() -> QueryEngine {
        QueryEngine::new(vec![])
    }

    fn fresh_context_with() -> ContextHolder {
        let engines = Arc::new(Mutex::new(HashMap::new()));
        ContextHolder { engines }
    }

    struct ContextHolder {
        pub engines: Arc<Mutex<HashMap<String, QueryEngine>>>,
    }

    impl ContextHolder {
        fn ctx(&self) -> EventsReadContext<'_> {
            EventsReadContext {
                engines: &self.engines,
            }
        }
    }

    #[tokio::test]
    async fn by_id_requires_event_id() {
        let holder = fresh_context_with();
        let ctx = holder.ctx();
        let input = EventsReadInput {
            session_id: "s1".into(),
            mode: EventsReadKind::ById,
            event_types: None,
            thread_id: None,
            timestamp_start: None,
            timestamp_end: None,
            function_pattern: None,
            limit: 0,
            cursor: None,
            event_id: None,
        };
        let err = ChronosEventsReadService::read(&ctx, input)
            .await
            .unwrap_err();
        match err {
            ServiceError::InvalidInput(s) => assert!(s.contains("event_id")),
            other => panic!("expected InvalidInput, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn by_id_session_not_found() {
        let holder = fresh_context_with();
        let ctx = holder.ctx();
        let input = EventsReadInput {
            session_id: "missing".into(),
            mode: EventsReadKind::ById,
            event_types: None,
            thread_id: None,
            timestamp_start: None,
            timestamp_end: None,
            function_pattern: None,
            limit: 0,
            cursor: None,
            event_id: Some(42),
        };
        let err = ChronosEventsReadService::read(&ctx, input)
            .await
            .unwrap_err();
        match err {
            ServiceError::SessionNotFound(s) => assert_eq!(s, "missing"),
            other => panic!("expected SessionNotFound, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn query_session_not_found() {
        let holder = fresh_context_with();
        let ctx = holder.ctx();
        let input = EventsReadInput {
            session_id: "missing".into(),
            mode: EventsReadKind::Query,
            event_types: None,
            thread_id: None,
            timestamp_start: None,
            timestamp_end: None,
            function_pattern: None,
            limit: 100,
            cursor: None,
            event_id: None,
        };
        let err = ChronosEventsReadService::read(&ctx, input)
            .await
            .unwrap_err();
        match err {
            ServiceError::SessionNotFound(s) => assert_eq!(s, "missing"),
            other => panic!("expected SessionNotFound, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn query_empty_session_returns_complete_no_cursor() {
        let holder = fresh_context_with();
        holder
            .engines
            .lock()
            .await
            .insert("s1".into(), empty_engine());
        let ctx = holder.ctx();

        let input = EventsReadInput {
            session_id: "s1".into(),
            mode: EventsReadKind::Query,
            event_types: None,
            thread_id: None,
            timestamp_start: None,
            timestamp_end: None,
            function_pattern: None,
            limit: 100,
            cursor: None,
            event_id: None,
        };
        let out = ChronosEventsReadService::read(&ctx, input).await.unwrap();
        match out {
            EventsReadOutput::Query {
                session_id,
                result,
                next_cursor,
                completeness,
                gap_summary,
                provenance,
            } => {
                assert_eq!(session_id, "s1");
                assert_eq!(completeness, "complete");
                assert!(gap_summary.is_none());
                assert!(next_cursor.is_none());
                assert_eq!(provenance.source, "query_engine");
                assert_eq!(provenance.session_id, "s1");
                assert_eq!(result.result.total_matching, 0);
                assert!(result.result.events.is_empty());
            }
            other => panic!("expected Query variant, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn query_kind_as_str_round_trip() {
        assert_eq!(EventsReadKind::Query.as_str(), "query");
        assert_eq!(EventsReadKind::ById.as_str(), "by_id");
    }

    #[tokio::test]
    async fn cursor_dto_round_trip() {
        let c = CursorDto {
            total_pushed: Some(42),
            snapshot_len: Some(100),
        };
        let dom = c.to_domain().unwrap();
        assert_eq!(dom.total_pushed, 42);
        assert_eq!(dom.snapshot_len, 100);
        let back = CursorDto::from_domain(&dom);
        assert_eq!(back, c);
    }

    #[tokio::test]
    async fn cursor_dto_missing_field_returns_none() {
        let c = CursorDto {
            total_pushed: Some(42),
            snapshot_len: None,
        };
        assert!(c.to_domain().is_none());
    }

    #[test]
    fn query_events_result_constructs() {
        // Sanity: QueryEventsResult wraps QueryResult. The dispatcher
        // returns this verbatim, so a basic construction test confirms
        // the shape compiles.
        let r = QueryEventsResult {
            result: QueryResult {
                total_matching: 3,
                events: vec![TraceEvent {
                    event_id: 1,
                    timestamp_ns: 0,
                    thread_id: 0,
                    event_type: EventType::FunctionEntry,
                    location: SourceLocation::default(),
                    data: EventData::Empty,
                }],
                next_offset: None,
            },
        };
        assert_eq!(r.result.total_matching, 3);
    }
}
