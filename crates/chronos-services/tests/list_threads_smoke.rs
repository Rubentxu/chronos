//! Smoke test for `QueryService::list_threads` via direct service call.

use std::collections::HashMap;

use tokio::sync::Mutex;

use chronos_domain::{EventData, EventType, SourceLocation};
use chronos_query::QueryEngine;
use chronos_services::error::ServiceError;
use chronos_services::query_service::QueryService;

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
async fn list_threads_smoke() {
    // Build a minimal engine with events on 2 threads
    let events = vec![
        trace_event(
            0,
            0,
            7,
            EventType::FunctionEntry,
            EventData::Function {
                name: "foo".to_string(),
                signature: None,
                symbol_id: None,
                invocation_id: None,
                parent_invocation_id: None,
            },
        ),
        trace_event(
            1,
            1,
            13,
            EventType::FunctionEntry,
            EventData::Function {
                name: "bar".to_string(),
                signature: None,
                symbol_id: None,
                invocation_id: None,
                parent_invocation_id: None,
            },
        ),
        trace_event(
            2,
            2,
            7, // duplicate thread
            EventType::FunctionExit,
            EventData::Function {
                name: "foo".to_string(),
                signature: None,
                symbol_id: None,
                invocation_id: None,
                parent_invocation_id: None,
            },
        ),
    ];
    let engine = QueryEngine::new(events);

    let mut map: HashMap<String, QueryEngine> = HashMap::new();
    map.insert("test-session".to_string(), engine);

    let engines = Mutex::new(map);

    let threads = QueryService::list_threads("test-session", &engines)
        .await
        .expect("should succeed");

    assert_eq!(threads.len(), 2, "exactly 2 unique thread IDs expected");
    assert!(threads.contains(&7), "thread 7 must be present");
    assert!(threads.contains(&13), "thread 13 must be present");
}

#[tokio::test]
async fn list_threads_unknown_session() {
    let engines = Mutex::new(HashMap::new());

    let result = QueryService::list_threads("does-not-exist", &engines).await;

    assert!(
        matches!(result, Err(ServiceError::SessionNotFound(ref s)) if s == "does-not-exist"),
        "expected SessionNotFound error, got {result:?}"
    );
}
