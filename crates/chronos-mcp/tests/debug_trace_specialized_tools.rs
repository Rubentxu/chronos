//! Integration tests for the 6 debug-trace specialized tools.
//!
//! These tests call `DebugTraceSpecializedService` methods directly with
//! real engine fixtures, verifying the service layer in isolation from
//! the MCP transport layer.

use chronos_domain::trace::{EventData, EventType, SourceLocation, TraceEvent};
use chronos_query::QueryEngine;
use chronos_services::debug_trace_specialized::DebugTraceSpecializedService;
use std::collections::HashMap;
use tokio::sync::Mutex;

fn make_engine(events: Vec<TraceEvent>) -> QueryEngine {
    QueryEngine::new(events)
}

fn engines_map(session_id: &str, events: Vec<TraceEvent>) -> Mutex<HashMap<String, QueryEngine>> {
    let mut map = HashMap::new();
    map.insert(session_id.to_string(), make_engine(events));
    Mutex::new(map)
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

fn signal_event(
    event_id: u64,
    timestamp_ns: u64,
    thread_id: u64,
    signal_number: i32,
    signal_name: &str,
) -> TraceEvent {
    TraceEvent {
        event_id,
        timestamp_ns,
        thread_id,
        event_type: EventType::SignalDelivered,
        location: SourceLocation::default(),
        data: EventData::Signal {
            signal_number,
            signal_name: signal_name.to_string(),
        },
    }
}

// ─── debug_find_variable_origin ──────────────────────────────────────────────

#[tokio::test]
async fn integration_find_variable_origin_ok() {
    let events = vec![
        trace_event(1, 100, 1, EventType::FunctionEntry, "main"),
        trace_event(2, 200, 1, EventType::FunctionExit, "main"),
    ];
    let engines = engines_map("s1", events);
    let result = DebugTraceSpecializedService::find_variable_origin("s1", "x", 10, &engines)
        .await
        .unwrap();
    assert_eq!(result.session_id, "s1");
    assert_eq!(result.variable_name, "x");
}

#[tokio::test]
async fn integration_find_variable_origin_session_not_found() {
    let events = vec![];
    let engines = engines_map("s1", events);
    let result =
        DebugTraceSpecializedService::find_variable_origin("missing", "x", 10, &engines).await;
    assert!(result.is_err());
}

// ─── debug_find_crash ────────────────────────────────────────────────────────

#[tokio::test]
async fn integration_find_crash_found() {
    let events = vec![
        trace_event(1, 100, 1, EventType::FunctionEntry, "main"),
        signal_event(2, 200, 1, 11, "SIGSEGV"),
    ];
    let engines = engines_map("s1", events);
    let result = DebugTraceSpecializedService::find_crash("s1", &engines)
        .await
        .unwrap();
    assert!(result.crash_found);
    assert_eq!(result.signal, "SIGSEGV");
    assert_eq!(result.event_id, 2);
    assert!(result.note.is_none());
}

#[tokio::test]
async fn integration_find_crash_not_found() {
    let events = vec![
        trace_event(1, 100, 1, EventType::FunctionEntry, "main"),
        trace_event(2, 200, 1, EventType::FunctionExit, "main"),
    ];
    let engines = engines_map("s1", events);
    let result = DebugTraceSpecializedService::find_crash("s1", &engines)
        .await
        .unwrap();
    assert!(!result.crash_found);
    assert!(result.note.is_some());
}

#[tokio::test]
async fn integration_find_crash_session_not_found() {
    let events = vec![];
    let engines = engines_map("s1", events);
    let result = DebugTraceSpecializedService::find_crash("missing", &engines).await;
    assert!(result.is_err());
}

// ─── debug_detect_races ─────────────────────────────────────────────────────

#[tokio::test]
async fn integration_detect_races_ok() {
    let events = vec![];
    let engines = engines_map("s1", events);
    let result = DebugTraceSpecializedService::detect_races("s1", 100, &engines)
        .await
        .unwrap();
    assert_eq!(result.session_id, "s1");
    assert_eq!(result.threshold_ns, 100);
}

#[tokio::test]
async fn integration_detect_races_session_not_found() {
    let events = vec![];
    let engines = engines_map("s1", events);
    let result = DebugTraceSpecializedService::detect_races("missing", 100, &engines).await;
    assert!(result.is_err());
}

// ─── inspect_causality ──────────────────────────────────────────────────────

#[tokio::test]
async fn integration_inspect_causality_ok() {
    let events = vec![];
    let engines = engines_map("s1", events);
    let result = DebugTraceSpecializedService::inspect_causality("s1", 0x1000, 10, &engines)
        .await
        .unwrap();
    assert_eq!(result.session_id, "s1");
    assert_eq!(result.address, 0x1000);
}

#[tokio::test]
async fn integration_inspect_causality_session_not_found() {
    let events = vec![];
    let engines = engines_map("s1", events);
    let result =
        DebugTraceSpecializedService::inspect_causality("missing", 0x1000, 10, &engines).await;
    assert!(result.is_err());
}

// ─── debug_expand_hotspot ──────────────────────────────────────────────────

#[tokio::test]
async fn integration_expand_hotspot_ok() {
    let events = vec![
        trace_event(1, 100, 1, EventType::FunctionEntry, "main"),
        trace_event(2, 200, 1, EventType::FunctionExit, "main"),
    ];
    let engines = engines_map("s1", events);
    let result = DebugTraceSpecializedService::expand_hotspot("s1", 5, &engines)
        .await
        .unwrap();
    assert_eq!(result.session_id, "s1");
    assert_eq!(result.compression_level, "hotspot");
    assert_eq!(result.top_n, 5);
}

#[tokio::test]
async fn integration_expand_hotspot_session_not_found() {
    let events = vec![];
    let engines = engines_map("s1", events);
    let result = DebugTraceSpecializedService::expand_hotspot("missing", 5, &engines).await;
    assert!(result.is_err());
}

// ─── debug_get_saliency_scores ─────────────────────────────────────────────

#[tokio::test]
async fn integration_get_saliency_scores_ok() {
    let events = vec![
        trace_event(1, 100, 1, EventType::FunctionEntry, "main"),
        trace_event(2, 200, 1, EventType::FunctionExit, "main"),
    ];
    let engines = engines_map("s1", events);
    let result = DebugTraceSpecializedService::get_saliency_scores("s1", 20, &engines)
        .await
        .unwrap();
    assert_eq!(result.session_id, "s1");
}

#[tokio::test]
async fn integration_get_saliency_scores_session_not_found() {
    let events = vec![];
    let engines = engines_map("s1", events);
    let result = DebugTraceSpecializedService::get_saliency_scores("missing", 20, &engines).await;
    assert!(result.is_err());
}
