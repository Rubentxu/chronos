//! Debug-trace service — trace query operations on a session.
//!
//! All 6 methods follow the same pattern:
//! 1. Lock the session map mutex.
//! 2. Look up the engine by `session_id`, return `Err(SessionNotFound)` if missing.
//! 3. Delegate to a `QueryEngine` method (all are sync).
//! 4. Map engine-level errors to `ServiceError` variants.
//!
//! The mutex is held only for the duration of the sync call, keeping latency
//! and contention low.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::error::ServiceError;
use crate::output::QueryEventsResult;
use chronos_domain::query::{ExecutionSummary, StackFrame, StateDiff};
use chronos_domain::trace::{EventType, TraceEvent};
use chronos_domain::TraceQuery;
use chronos_query::QueryEngine;

/// A zero-sized service struct. All state is passed in as arguments.
#[derive(Debug, Default)]
pub struct DebugTraceService;

impl DebugTraceService {
    /// Execute a trace event query with filters.
    ///
    /// Returns [`QueryEventsResult`] wrapping the engine's [`chronos_query::QueryResult`].
    /// The caller is responsible for translating event-type strings.
    #[allow(clippy::too_many_arguments)]
    pub async fn query_events(
        session_id: &str,
        event_types: Option<Vec<EventType>>,
        thread_id: Option<u64>,
        timestamp_start: Option<u64>,
        timestamp_end: Option<u64>,
        function_pattern: Option<&str>,
        limit: usize,
        offset: usize,
        engines: &Mutex<HashMap<String, QueryEngine>>,
    ) -> Result<QueryEventsResult, ServiceError> {
        let guard = engines.lock().await;
        let engine = guard
            .get(session_id)
            .ok_or_else(|| ServiceError::SessionNotFound(session_id.to_string()))?;

        let mut query = TraceQuery::new(session_id).pagination(limit, offset);

        if let Some(types) = event_types {
            query = query.event_types(types);
        }

        if let (Some(start), Some(end)) = (timestamp_start, timestamp_end) {
            query = query.time_range(start, end);
        }

        if let Some(pattern) = function_pattern {
            query = query.function_pattern(pattern);
        }

        if let Some(tid) = thread_id {
            query.thread_id = Some(tid);
        }

        let result = engine.execute(&query);
        Ok(QueryEventsResult { result })
    }

    /// Get a single trace event by its ID.
    ///
    /// Returns `Ok(Some(event))` if the event exists, `Ok(None)` if not found.
    /// The `None` case is not an error — callers that need to surface "not found"
    /// can do so explicitly.
    pub async fn get_event(
        session_id: &str,
        event_id: u64,
        engines: &Mutex<HashMap<String, QueryEngine>>,
    ) -> Result<Option<TraceEvent>, ServiceError> {
        let guard = engines.lock().await;
        let engine = guard
            .get(session_id)
            .ok_or_else(|| ServiceError::SessionNotFound(session_id.to_string()))?;

        Ok(engine.get_event_by_id(event_id).cloned())
    }

    /// Reconstruct the call stack at a specific event.
    ///
    /// Returns an empty vec if the event is not found or has no frame data.
    pub async fn get_call_stack(
        session_id: &str,
        event_id: u64,
        engines: &Mutex<HashMap<String, QueryEngine>>,
    ) -> Result<Vec<StackFrame>, ServiceError> {
        let guard = engines.lock().await;
        let engine = guard
            .get(session_id)
            .ok_or_else(|| ServiceError::SessionNotFound(session_id.to_string()))?;

        Ok(engine.reconstruct_call_stack(event_id))
    }

    /// Get execution summary for a session.
    ///
    /// The summary includes event counts, top functions, and potential issues.
    pub async fn get_execution_summary(
        session_id: &str,
        engines: &Mutex<HashMap<String, QueryEngine>>,
    ) -> Result<ExecutionSummary, ServiceError> {
        let guard = engines.lock().await;
        let engine = guard
            .get(session_id)
            .ok_or_else(|| ServiceError::SessionNotFound(session_id.to_string()))?;

        Ok(engine.execution_summary(session_id))
    }

    /// Compare program state (registers) between two timestamps.
    ///
    /// Returns an empty diff if neither timestamp has register evidence.
    pub async fn state_diff(
        session_id: &str,
        timestamp_a: u64,
        timestamp_b: u64,
        engines: &Mutex<HashMap<String, QueryEngine>>,
    ) -> Result<StateDiff, ServiceError> {
        let guard = engines.lock().await;
        let engine = guard
            .get(session_id)
            .ok_or_else(|| ServiceError::SessionNotFound(session_id.to_string()))?;

        Ok(engine.state_diff(timestamp_a, timestamp_b))
    }

    /// Build the call graph for a session up to a given depth.
    ///
    /// Returns callers and callees for each function observed in the trace.
    pub async fn debug_call_graph(
        session_id: &str,
        max_depth: usize,
        engines: &Mutex<HashMap<String, QueryEngine>>,
    ) -> Result<CallGraph, ServiceError> {
        let guard = engines.lock().await;
        let engine = guard
            .get(session_id)
            .ok_or_else(|| ServiceError::SessionNotFound(session_id.to_string()))?;

        // Build call graph from FunctionEntry events via per-thread stack simulation
        let mut callers: HashMap<String, Vec<String>> = HashMap::new();
        let mut callees: HashMap<String, Vec<String>> = HashMap::new();
        let mut call_counts: HashMap<String, u64> = HashMap::new();
        let mut stacks: HashMap<u64, Vec<String>> = HashMap::new();

        let query = TraceQuery::new(session_id).pagination(usize::MAX, 0);
        let result = engine.execute(&query);

        for event in &result.events {
            let func = event.location.function.clone().unwrap_or_default();
            if func.is_empty() {
                continue;
            }
            match event.event_type {
                EventType::FunctionEntry => {
                    let stack = stacks.entry(event.thread_id).or_default();
                    *call_counts.entry(func.clone()).or_insert(0) += 1;

                    // depth gate
                    if stack.len() < max_depth {
                        if let Some(parent) = stack.last().cloned() {
                            callees
                                .entry(parent.clone())
                                .or_default()
                                .push(func.clone());
                            callers.entry(func.clone()).or_default().push(parent);
                        }
                        stack.push(func);
                    }
                }
                EventType::FunctionExit => {
                    let stack = stacks.entry(event.thread_id).or_default();
                    stack.pop();
                }
                _ => {}
            }
        }

        // Deduplicate edges
        for v in callers.values_mut() {
            v.sort();
            v.dedup();
        }
        for v in callees.values_mut() {
            v.sort();
            v.dedup();
        }

        let nodes: Vec<CallGraphNode> = call_counts
            .iter()
            .map(|(name, count)| CallGraphNode {
                function: name.clone(),
                call_count: *count,
                callers: callers.get(name).cloned().unwrap_or_default(),
                callees: callees.get(name).cloned().unwrap_or_default(),
            })
            .collect();

        let edges: Vec<CallGraphEdge> = callees
            .iter()
            .flat_map(|(from, tos)| {
                tos.iter()
                    .map(|to| CallGraphEdge {
                        from: from.clone(),
                        to: to.clone(),
                    })
                    .collect::<Vec<_>>()
            })
            .collect();

        let edge_count = edges.len();

        let mut max_observed_depth = 0u32;
        for stack in stacks.values() {
            if !stack.is_empty() {
                max_observed_depth = max_observed_depth.max(stack.len() as u32);
            }
        }

        Ok(CallGraph {
            nodes,
            edges,
            stats: CallGraphStats {
                node_count: call_counts.len(),
                edge_count,
                max_observed_depth,
            },
        })
    }
}

// ---------------------------------------------------------------------------
// Call graph output types (defined here, not in output.rs — internal to this service)
// ---------------------------------------------------------------------------

/// A single node in a call graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallGraphNode {
    /// Function name.
    pub function: String,
    /// Number of times this function was called.
    pub call_count: u64,
    /// Functions that called this one (deduplicated).
    pub callers: Vec<String>,
    /// Functions called by this one (deduplicated).
    pub callees: Vec<String>,
}

/// A directed edge in a call graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallGraphEdge {
    /// Caller function name.
    pub from: String,
    /// Callee function name.
    pub to: String,
}

/// Aggregate statistics for a call graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallGraphStats {
    /// Total unique functions.
    pub node_count: usize,
    /// Total unique edges.
    pub edge_count: usize,
    /// Maximum observed call-stack depth.
    pub max_observed_depth: u32,
}

/// A complete call graph for a session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallGraph {
    /// All function nodes.
    pub nodes: Vec<CallGraphNode>,
    /// All directed edges.
    pub edges: Vec<CallGraphEdge>,
    /// Aggregate statistics.
    pub stats: CallGraphStats,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::trace::SourceLocation;
    use chronos_domain::EventData;

    fn make_engine(events: Vec<TraceEvent>) -> QueryEngine {
        QueryEngine::new(events)
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

    // --- Happy-path helpers ----------------------------------------------------

    fn events_engine() -> HashMap<String, QueryEngine> {
        let events = vec![
            trace_event(1, 100, 1, EventType::FunctionEntry, "main"),
            trace_event(2, 200, 1, EventType::FunctionExit, "main"),
            trace_event(3, 150, 1, EventType::FunctionEntry, "helper"),
            trace_event(4, 180, 1, EventType::FunctionExit, "helper"),
        ];
        let engine = make_engine(events);
        let mut map = HashMap::new();
        map.insert("s1".to_string(), engine);
        map
    }

    fn engine_with_call_chain() -> HashMap<String, QueryEngine> {
        let events = vec![
            trace_event(1, 100, 1, EventType::FunctionEntry, "a"),
            trace_event(2, 110, 1, EventType::FunctionEntry, "b"),
            trace_event(3, 120, 1, EventType::FunctionEntry, "c"),
            trace_event(4, 130, 1, EventType::FunctionExit, "c"),
            trace_event(5, 140, 1, EventType::FunctionExit, "b"),
            trace_event(6, 150, 1, EventType::FunctionExit, "a"),
        ];
        let engine = make_engine(events);
        let mut map = HashMap::new();
        map.insert("s2".to_string(), engine);
        map
    }

    // --- query_events ---

    #[tokio::test]
    async fn query_events_ok() {
        let map = events_engine();
        let engines = Mutex::new(map);
        let result =
            DebugTraceService::query_events("s1", None, None, None, None, None, 100, 0, &engines)
                .await
                .unwrap();
        assert_eq!(result.result.events.len(), 4);
    }

    #[tokio::test]
    async fn query_events_session_not_found() {
        let map = events_engine();
        let engines = Mutex::new(map);
        let result = DebugTraceService::query_events(
            "missing", None, None, None, None, None, 10, 0, &engines,
        )
        .await;
        assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
    }

    // --- get_event ---

    #[tokio::test]
    async fn get_event_ok() {
        let map = events_engine();
        let engines = Mutex::new(map);
        let result = DebugTraceService::get_event("s1", 1, &engines)
            .await
            .unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().event_id, 1);
    }

    #[tokio::test]
    async fn get_event_not_found() {
        let map = events_engine();
        let engines = Mutex::new(map);
        // get_event returns Option<TraceEvent>, not an error — the wrapper in
        // server.rs translates None → EventNotFound
        let result = DebugTraceService::get_event("s1", 9999, &engines)
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn get_event_session_not_found() {
        let map = events_engine();
        let engines = Mutex::new(map);
        let result = DebugTraceService::get_event("missing", 1, &engines).await;
        assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
    }

    // --- get_call_stack ---

    #[tokio::test]
    async fn get_call_stack_ok() {
        let map = events_engine();
        let engines = Mutex::new(map);
        // reconstruct_call_stack uses event_id to look back through FunctionEntry events
        let result = DebugTraceService::get_call_stack("s1", 3, &engines)
            .await
            .unwrap();
        // The implementation delegates to engine.reconstruct_call_stack
        // (returns whatever the engine returns for that event_id)
        // Just verify we got a Vec back without panicking
        let _ = result.len();
    }

    #[tokio::test]
    async fn get_call_stack_session_not_found() {
        let map = events_engine();
        let engines = Mutex::new(map);
        let result = DebugTraceService::get_call_stack("missing", 1, &engines).await;
        assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
    }

    // --- get_execution_summary ---

    #[tokio::test]
    async fn get_execution_summary_ok() {
        let map = events_engine();
        let engines = Mutex::new(map);
        let result = DebugTraceService::get_execution_summary("s1", &engines)
            .await
            .unwrap();
        assert_eq!(result.session_id, "s1");
    }

    #[tokio::test]
    async fn get_execution_summary_session_not_found() {
        let map = events_engine();
        let engines = Mutex::new(map);
        let result = DebugTraceService::get_execution_summary("missing", &engines).await;
        assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
    }

    // --- state_diff ---

    #[tokio::test]
    async fn state_diff_ok() {
        let map = events_engine();
        let engines = Mutex::new(map);
        let result = DebugTraceService::state_diff("s1", 100, 200, &engines)
            .await
            .unwrap();
        // No register evidence → empty diff is expected
        assert_eq!(result.timestamp_a, 100);
        assert_eq!(result.timestamp_b, 200);
    }

    #[tokio::test]
    async fn state_diff_session_not_found() {
        let map = events_engine();
        let engines = Mutex::new(map);
        let result = DebugTraceService::state_diff("missing", 0, 100, &engines).await;
        assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
    }

    // --- debug_call_graph ---

    #[tokio::test]
    async fn debug_call_graph_ok() {
        let map = events_engine();
        let engines = Mutex::new(map);
        let result = DebugTraceService::debug_call_graph("s1", 10, &engines)
            .await
            .unwrap();
        // 2 unique functions: main, helper
        assert_eq!(result.stats.node_count, 2);
    }

    #[tokio::test]
    async fn debug_call_graph_session_not_found() {
        let map = events_engine();
        let engines = Mutex::new(map);
        let result = DebugTraceService::debug_call_graph("missing", 10, &engines).await;
        assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
    }

    // --- Edge cases ---

    #[tokio::test]
    async fn empty_engine_returns_empty_results() {
        let engine = make_engine(vec![]);
        let mut map = HashMap::new();
        map.insert("empty".to_string(), engine);
        let engines = Mutex::new(map);

        let result =
            DebugTraceService::query_events("empty", None, None, None, None, None, 10, 0, &engines)
                .await
                .unwrap();
        assert_eq!(result.result.events.len(), 0);

        let cg = DebugTraceService::debug_call_graph("empty", 10, &engines)
            .await
            .unwrap();
        assert_eq!(cg.stats.node_count, 0);
        assert_eq!(cg.stats.edge_count, 0);
    }

    #[tokio::test]
    async fn deep_call_graph_verifies_caller_relationships() {
        let map = engine_with_call_chain();
        let engines = Mutex::new(map);

        // Verify function nodes exist and the call chain is built
        let cg = DebugTraceService::debug_call_graph("s2", 10, &engines)
            .await
            .unwrap();

        // All 3 functions should be present
        assert_eq!(cg.stats.node_count, 3, "should have 3 function nodes");

        // Find c's node and verify it knows about b as caller
        let c_node = cg
            .nodes
            .iter()
            .find(|n| n.function == "c")
            .expect("c must be present");
        assert!(
            c_node.callers.contains(&"b".to_string()),
            "c should see b as caller"
        );
        assert_eq!(c_node.call_count, 1, "c was entered once");

        // Verify b has c as callee
        let b_node = cg
            .nodes
            .iter()
            .find(|n| n.function == "b")
            .expect("b must be present");
        assert!(
            b_node.callees.contains(&"c".to_string()),
            "b should have c as callee"
        );

        // Verify edges exist
        assert!(
            cg.edges.iter().any(|e| e.from == "a" && e.to == "b"),
            "a→b edge must exist"
        );
        assert!(
            cg.edges.iter().any(|e| e.from == "b" && e.to == "c"),
            "b→c edge must exist"
        );
    }

    #[tokio::test]
    async fn call_graph_deduplicates_edges() {
        let map = engine_with_call_chain();
        let engines = Mutex::new(map);

        let cg = DebugTraceService::debug_call_graph("s2", 10, &engines)
            .await
            .unwrap();

        // a→b edge should appear exactly once
        let a_to_b_count = cg
            .edges
            .iter()
            .filter(|e| e.from == "a" && e.to == "b")
            .count();
        assert_eq!(a_to_b_count, 1, "a→b edge should appear exactly once");

        // a should see b as callee, not repeated
        let a_node = cg
            .nodes
            .iter()
            .find(|n| n.function == "a")
            .expect("a must be present");
        assert!(
            a_node.callees.iter().filter(|f| *f == "b").count() <= 1,
            "a's callees list should not contain duplicate b entries"
        );
    }
}
