//! Query service — read-only trace queries.

use std::collections::HashMap;

use tokio::sync::Mutex;

use crate::error::ServiceError;
use chronos_query::QueryEngine;

/// A service for querying trace data.
///
/// This struct is zero-sized — all state is passed in as arguments,
/// making it trivially `Send + Sync`.
#[derive(Debug, Default)]
pub struct QueryService;

impl QueryService {
    /// List all thread IDs for a given session.
    ///
    /// # Arguments
    ///
    /// * `session_id` — the session to look up
    /// * `engines`   — the shared engine map (held only for the duration of this call)
    pub async fn list_threads(
        session_id: &str,
        engines: &Mutex<HashMap<String, QueryEngine>>,
    ) -> Result<Vec<u64>, ServiceError> {
        let guard = engines.lock().await;
        let engine = guard
            .get(session_id)
            .ok_or_else(|| ServiceError::SessionNotFound(session_id.to_string()))?;
        Ok(engine.thread_ids())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::{EventData, EventType, SourceLocation};

    fn trace_event(
        event_id: u64,
        timestamp_ns: u64,
        thread_id: u64,
        event_type: EventType,
        data: EventData,
    ) -> chronos_domain::TraceEvent {
        chronos_domain::TraceEvent {
            event_id,
            timestamp_ns,
            thread_id,
            event_type,
            location: SourceLocation::default(),
            data,
        }
    }

    #[tokio::test]
    async fn list_threads_returns_thread_ids_from_engine() {
        // Build a minimal engine with 2 threads
        let events = vec![
            trace_event(
                0,
                0,
                10,
                EventType::FunctionEntry,
                EventData::Function {
                    name: "f".to_string(),
                    signature: None,
                    symbol_id: None,
                    invocation_id: None,
                    parent_invocation_id: None,
                },
            ),
            trace_event(
                1,
                1,
                20,
                EventType::FunctionEntry,
                EventData::Function {
                    name: "g".to_string(),
                    signature: None,
                    symbol_id: None,
                    invocation_id: None,
                    parent_invocation_id: None,
                },
            ),
            trace_event(
                2,
                2,
                10, // same as first
                EventType::FunctionExit,
                EventData::Function {
                    name: "f".to_string(),
                    signature: None,
                    symbol_id: None,
                    invocation_id: None,
                    parent_invocation_id: None,
                },
            ),
        ];
        let engine = QueryEngine::new(events);

        let mut map: HashMap<String, QueryEngine> = HashMap::new();
        map.insert("session-1".to_string(), engine);

        let engines = Mutex::new(map);
        let result = QueryService::list_threads("session-1", &engines).await;

        let threads = result.expect("list_threads should succeed");
        assert_eq!(threads.len(), 2, "expected exactly 2 unique thread IDs");
        assert!(threads.contains(&10), "thread 10 should be present");
        assert!(threads.contains(&20), "thread 20 should be present");
    }

    #[tokio::test]
    async fn list_threads_session_not_found() {
        let map: HashMap<String, QueryEngine> = HashMap::new();
        let engines = Mutex::new(map);

        let result = QueryService::list_threads("missing", &engines).await;
        assert!(matches!(result, Err(ServiceError::SessionNotFound(ref s)) if s == "missing"));
    }
}
