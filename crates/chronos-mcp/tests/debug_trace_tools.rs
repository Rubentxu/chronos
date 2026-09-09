//! Integration tests for the 6 debug-trace MCP tools.
//!
//! These tests call `DebugTraceService::*` methods directly, bypassing the MCP
//! transport layer. They verify the service layer without requiring a running
//! chronos-mcp binary.

use chronos_domain::trace::{EventType, SourceLocation};
use chronos_domain::{EventData, TraceEvent};
use chronos_services::debug_trace::DebugTraceService;
use chronos_services::error::ServiceError;
use std::collections::HashMap;
use tokio::sync::Mutex;

fn make_engine(events: Vec<TraceEvent>) -> chronos_query::QueryEngine {
    chronos_query::QueryEngine::new(events)
}

fn trace_event(
    event_id: u64,
    timestamp_ns: u64,
    thread_id: u64,
    event_type: EventType,
    function: &str,
) -> TraceEvent {
    TraceEvent {
        event_id,
        timestamp_ns,
        thread_id,
        event_type,
        location: SourceLocation {
            function: Some(function.to_string()),
            ..SourceLocation::default()
        },
        data: EventData::Empty,
    }
}

fn engines_with_session() -> Mutex<HashMap<String, chronos_query::QueryEngine>> {
    let events = vec![
        trace_event(1, 100, 1, EventType::FunctionEntry, "main"),
        trace_event(2, 110, 1, EventType::FunctionEntry, "helper"),
        trace_event(3, 120, 1, EventType::FunctionExit, "helper"),
        trace_event(4, 130, 1, EventType::FunctionExit, "main"),
        trace_event(5, 200, 2, EventType::FunctionEntry, "worker"),
        trace_event(6, 300, 2, EventType::FunctionExit, "worker"),
    ];
    let engine = make_engine(events);
    let mut map = HashMap::new();
    map.insert("test-session".to_string(), engine);
    Mutex::new(map)
}

fn empty_engines() -> Mutex<HashMap<String, chronos_query::QueryEngine>> {
    Mutex::new(HashMap::new())
}

// ---------------------------------------------------------------------------
// query_events
// ---------------------------------------------------------------------------

#[tokio::test]
async fn query_events_returns_events() {
    let engines = engines_with_session();
    let result = DebugTraceService::query_events(
        "test-session",
        None,
        None,
        None,
        None,
        None,
        100,
        0,
        &engines,
    )
    .await
    .unwrap();
    assert_eq!(result.result.events.len(), 6);
    assert_eq!(result.result.total_matching, 6);
}

#[tokio::test]
async fn query_events_session_not_found() {
    let engines = empty_engines();
    let result = DebugTraceService::query_events(
        "no-such-session",
        None,
        None,
        None,
        None,
        None,
        10,
        0,
        &engines,
    )
    .await;
    assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
}

// ---------------------------------------------------------------------------
// get_event
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_event_found() {
    let engines = engines_with_session();
    let result = DebugTraceService::get_event("test-session", 1, &engines)
        .await
        .unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap().event_id, 1);
}

#[tokio::test]
async fn get_event_not_found_returns_none() {
    let engines = engines_with_session();
    let result = DebugTraceService::get_event("test-session", 9999, &engines)
        .await
        .unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn get_event_session_not_found() {
    let engines = empty_engines();
    let result = DebugTraceService::get_event("no-such-session", 1, &engines).await;
    assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
}

// ---------------------------------------------------------------------------
// get_call_stack
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_call_stack_session_not_found() {
    let engines = empty_engines();
    let result = DebugTraceService::get_call_stack("no-such-session", 1, &engines).await;
    assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
}

// ---------------------------------------------------------------------------
// get_execution_summary
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_execution_summary_session_not_found() {
    let engines = empty_engines();
    let result = DebugTraceService::get_execution_summary("no-such-session", &engines).await;
    assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
}

// ---------------------------------------------------------------------------
// state_diff
// ---------------------------------------------------------------------------

#[tokio::test]
async fn state_diff_session_not_found() {
    let engines = empty_engines();
    let result = DebugTraceService::state_diff("no-such-session", 0, 100, &engines).await;
    assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
}

// ---------------------------------------------------------------------------
// debug_call_graph
// ---------------------------------------------------------------------------

#[tokio::test]
async fn debug_call_graph_returns_graph() {
    let engines = engines_with_session();
    let result = DebugTraceService::debug_call_graph("test-session", 10, &engines)
        .await
        .unwrap();
    // 3 unique functions: main, helper, worker
    assert_eq!(result.stats.node_count, 3);
    // No register evidence expected
    assert_eq!(result.stats.max_observed_depth, 0);
}

#[tokio::test]
async fn debug_call_graph_session_not_found() {
    let engines = empty_engines();
    let result = DebugTraceService::debug_call_graph("no-such-session", 10, &engines).await;
    assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
}
