//! Debug-trace specialized service — 6 tools that combine engine queries
//! into higher-level diagnostic results.
//!
//! All 6 methods follow the same pattern:
//! 1. Lock the session map mutex.
//! 2. Look up the engine by `session_id`, return `Err(SessionNotFound)` if missing.
//! 3. Execute one or more engine queries.
//! 4. Map to the output type and return.

use std::collections::HashMap;

use tokio::sync::Mutex;

#[cfg(test)]
use chronos_domain::trace::TraceEvent;

use crate::error::ServiceError;
use crate::output::{
    CausalityReport, CrashPoint, CrashStackFrame, HotspotEntry, HotspotReport, LineageEntry,
    RaceReport, SaliencyScore, SaliencyScoreResult, VariableOriginResult,
};
use chronos_domain::query::{CausalityQuery, PerfQuery, PerfSortBy, RaceDetectionQuery};
use chronos_domain::trace::{EventData, EventType};
use chronos_domain::TraceQuery;
use chronos_query::QueryEngine;

/// A zero-sized service struct. All state is passed in as arguments.
#[derive(Debug, Default)]
pub struct DebugTraceSpecializedService;

impl DebugTraceSpecializedService {
    /// Trace the origin of a variable: find all write mutations to it.
    ///
    /// Returns a result that serializes to the same JSON shape as the original
    /// `debug_find_variable_origin` tool.
    pub async fn find_variable_origin(
        session_id: &str,
        variable_name: &str,
        limit: usize,
        engines: &Mutex<HashMap<String, QueryEngine>>,
    ) -> Result<VariableOriginResult, ServiceError> {
        let guard = engines.lock().await;
        let engine = guard
            .get(session_id)
            .ok_or_else(|| ServiceError::SessionNotFound(session_id.to_string()))?;

        let query = CausalityQuery {
            session_id: session_id.to_string(),
            address: None,
            variable_name: Some(variable_name.to_string()),
            before_timestamp: None,
            full_lineage: true,
        };

        match engine.query_causality(&query) {
            Some(result) => {
                let mut mutations: Vec<LineageEntry> = result
                    .mutations
                    .into_iter()
                    .map(LineageEntry::from)
                    .collect();
                mutations.truncate(limit);
                Ok(VariableOriginResult {
                    session_id: session_id.to_string(),
                    variable_name: variable_name.to_string(),
                    mutation_count: mutations.len(),
                    mutations,
                    note: None,
                })
            }
            None => Ok(VariableOriginResult {
                session_id: session_id.to_string(),
                variable_name: variable_name.to_string(),
                mutation_count: 0,
                mutations: vec![],
                note: Some("No causality index or no writes to this variable found".to_string()),
            }),
        }
    }

    /// Identify the crash point in a trace: find the last event before a fatal
    /// signal and reconstruct the call stack.
    ///
    /// Returns `Ok(CrashPoint)` with `crash_found = false` when no fatal
    /// signal is present.
    pub async fn find_crash(
        session_id: &str,
        engines: &Mutex<HashMap<String, QueryEngine>>,
    ) -> Result<CrashPoint, ServiceError> {
        let guard = engines.lock().await;
        let engine = guard
            .get(session_id)
            .ok_or_else(|| ServiceError::SessionNotFound(session_id.to_string()))?;

        let fatal_signals = [
            "SIGSEGV", "SIGABRT", "SIGBUS", "SIGILL", "SIGFPE", "SIGKILL",
        ];

        let query = TraceQuery::new(session_id)
            .event_types(vec![EventType::SignalDelivered])
            .pagination(usize::MAX, 0);
        let result = engine.execute(&query);

        let crash_event = result.events.iter().find(|e| {
            if let EventData::Signal { signal_name, .. } = &e.data {
                fatal_signals.contains(&signal_name.as_str())
            } else {
                false
            }
        });

        match crash_event {
            Some(ev) => {
                let stack = engine.reconstruct_call_stack(ev.event_id);
                let signal_name = if let EventData::Signal { signal_name, .. } = &ev.data {
                    signal_name.clone()
                } else {
                    "unknown".to_string()
                };

                Ok(CrashPoint {
                    session_id: session_id.to_string(),
                    crash_found: true,
                    signal: signal_name,
                    event_id: ev.event_id,
                    timestamp_ns: ev.timestamp_ns,
                    thread_id: ev.thread_id,
                    call_stack_depth: stack.len(),
                    call_stack: stack.into_iter().map(CrashStackFrame::from).collect(),
                    note: None,
                })
            }
            None => Ok(CrashPoint {
                session_id: session_id.to_string(),
                crash_found: false,
                signal: String::new(),
                event_id: 0,
                timestamp_ns: 0,
                thread_id: 0,
                call_stack_depth: 0,
                call_stack: vec![],
                note: Some("No fatal signal found in the trace".to_string()),
            }),
        }
    }

    /// Detect suspicious concurrent accesses — two writes to the same address
    /// from different threads within `threshold_ns` nanoseconds.
    ///
    /// This is a triage signal, not a proven data race (no happens-before
    /// analysis is performed).
    pub async fn detect_races(
        session_id: &str,
        threshold_ns: u64,
        engines: &Mutex<HashMap<String, QueryEngine>>,
    ) -> Result<RaceReport, ServiceError> {
        let guard = engines.lock().await;
        let engine = guard
            .get(session_id)
            .ok_or_else(|| ServiceError::SessionNotFound(session_id.to_string()))?;

        let query = RaceDetectionQuery {
            session_id: session_id.to_string(),
            time_range: None,
            threshold_ns,
        };
        let result = engine.detect_concurrent_access(&query);

        let total_writes = result.addresses_checked;

        // Build a list of (function_a, function_b) pairs that are suspicious
        let suspicious_pairs: Vec<(String, String)> = result
            .accesses
            .iter()
            .map(|a| (a.write_a.function.clone(), a.write_b.function.clone()))
            .collect();

        let summary = if result.accesses.is_empty() {
            "No suspicious concurrent accesses found within the threshold".to_string()
        } else {
            format!(
                "Found {} suspicious concurrent accesses across {} addresses",
                result.accesses.len(),
                total_writes
            )
        };

        Ok(RaceReport {
            session_id: session_id.to_string(),
            threshold_ns,
            access_count: result.accesses.len(),
            accesses: result.accesses,
            total_writes,
            suspicious_pairs,
            summary,
        })
    }

    /// Inspect the full causal history of a memory address.
    pub async fn inspect_causality(
        session_id: &str,
        address: u64,
        limit: usize,
        engines: &Mutex<HashMap<String, QueryEngine>>,
    ) -> Result<CausalityReport, ServiceError> {
        let guard = engines.lock().await;
        let engine = guard
            .get(session_id)
            .ok_or_else(|| ServiceError::SessionNotFound(session_id.to_string()))?;

        let query = CausalityQuery {
            session_id: session_id.to_string(),
            address: Some(address),
            variable_name: None,
            before_timestamp: None,
            full_lineage: true,
        };

        match engine.query_causality(&query) {
            Some(result) => {
                let mut mutations: Vec<LineageEntry> = result
                    .mutations
                    .into_iter()
                    .map(LineageEntry::from)
                    .collect();
                mutations.truncate(limit);
                Ok(CausalityReport {
                    session_id: session_id.to_string(),
                    address,
                    mutation_count: mutations.len(),
                    mutations,
                    note: None,
                })
            }
            None => Ok(CausalityReport {
                session_id: session_id.to_string(),
                address,
                mutation_count: 0,
                mutations: vec![],
                note: Some("No causality index or no writes to this address found".to_string()),
            }),
        }
    }

    /// Semantic compression Level 1 — top-N hottest functions by call count
    /// and CPU cycles.
    pub async fn expand_hotspot(
        session_id: &str,
        top_n: usize,
        engines: &Mutex<HashMap<String, QueryEngine>>,
    ) -> Result<HotspotReport, ServiceError> {
        let guard = engines.lock().await;
        let engine = guard
            .get(session_id)
            .ok_or_else(|| ServiceError::SessionNotFound(session_id.to_string()))?;

        let summary = engine.execution_summary(session_id);

        let mut hotspot_functions = Vec::new();
        for f in summary.top_functions.iter().take(top_n) {
            let perf_entry = engine
                .query_perf(&PerfQuery {
                    session_id: session_id.to_string(),
                    function_filter: Some(f.name.clone()),
                    sort_by: PerfSortBy::Cycles,
                    limit: 1,
                })
                .and_then(|r| r.functions.into_iter().next());

            hotspot_functions.push(HotspotEntry {
                function: f.name.clone(),
                call_count: f.call_count,
                total_cycles: perf_entry.as_ref().map(|p| p.total_cycles),
                avg_cycles_per_call: perf_entry.as_ref().map(|p| p.avg_cycles),
            });
        }

        let total_calls: u64 = summary.top_functions.iter().map(|f| f.call_count).sum();

        Ok(HotspotReport {
            session_id: session_id.to_string(),
            compression_level: "hotspot".to_string(),
            top_n,
            total_calls_in_trace: total_calls,
            hotspot_functions,
            hint: Some(
                "Use debug_call_graph for full call graph or query_events to drill into specific functions"
                    .to_string(),
            ),
        })
    }

    /// Compute saliency scores [0.0–1.0] for functions: a high score means the
    /// function consumed a disproportionate share of CPU cycles.
    pub async fn get_saliency_scores(
        session_id: &str,
        limit: usize,
        engines: &Mutex<HashMap<String, QueryEngine>>,
    ) -> Result<SaliencyScoreResult, ServiceError> {
        let guard = engines.lock().await;
        let engine = guard
            .get(session_id)
            .ok_or_else(|| ServiceError::SessionNotFound(session_id.to_string()))?;

        let summary = engine.execution_summary(session_id);

        let perf_result = engine.query_perf(&PerfQuery {
            session_id: session_id.to_string(),
            function_filter: None,
            sort_by: PerfSortBy::Cycles,
            limit,
        });

        let scores: Vec<SaliencyScore> = if let Some(perf) = perf_result {
            let total_cycles: u64 = perf.functions.iter().map(|e| e.total_cycles).sum();

            perf.functions
                .iter()
                .take(limit)
                .map(|entry| {
                    let score = if total_cycles > 0 {
                        entry.total_cycles as f64 / total_cycles as f64
                    } else {
                        0.0
                    };
                    SaliencyScore {
                        function: entry
                            .name
                            .clone()
                            .unwrap_or_else(|| "<unknown>".to_string()),
                        saliency_score: (score * 10000.0).round() / 10000.0,
                        call_count: entry.call_count,
                        total_cycles: Some(entry.total_cycles),
                        cycles: None,
                    }
                })
                .collect()
        } else {
            let total_calls: u64 = summary.top_functions.iter().map(|f| f.call_count).sum();
            summary
                .top_functions
                .iter()
                .take(limit)
                .map(|f| {
                    let score = if total_calls > 0 {
                        f.call_count as f64 / total_calls as f64
                    } else {
                        0.0
                    };
                    SaliencyScore {
                        function: f.name.clone(),
                        saliency_score: (score * 10000.0).round() / 10000.0,
                        call_count: f.call_count,
                        total_cycles: None,
                        cycles: Some(()),
                    }
                })
                .collect()
        };

        Ok(SaliencyScoreResult {
            session_id: session_id.to_string(),
            scored_functions: scores.len(),
            scores,
            hint: Some(
                "saliency_score near 1.0 means this function dominated CPU time. Use debug_expand_hotspot to zoom in."
                    .to_string(),
            ),
        })
    }
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

    // --- Helpers ---

    fn engines_with_session(
        session_id: &str,
        events: Vec<TraceEvent>,
    ) -> Mutex<HashMap<String, QueryEngine>> {
        let engine = make_engine(events);
        let mut map = HashMap::new();
        map.insert(session_id.to_string(), engine);
        Mutex::new(map)
    }

    // --- find_variable_origin ---

    #[tokio::test]
    async fn find_variable_origin_ok() {
        let events = vec![
            trace_event(1, 100, 1, EventType::FunctionEntry, "main"),
            trace_event(2, 200, 1, EventType::FunctionExit, "main"),
        ];
        let engines = engines_with_session("s1", events);
        let result = DebugTraceSpecializedService::find_variable_origin("s1", "x", 10, &engines)
            .await
            .unwrap();
        assert_eq!(result.session_id, "s1");
        assert_eq!(result.variable_name, "x");
    }

    #[tokio::test]
    async fn find_variable_origin_session_not_found() {
        let events = vec![];
        let engines = engines_with_session("s1", events);
        let result =
            DebugTraceSpecializedService::find_variable_origin("missing", "x", 10, &engines).await;
        assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
    }

    #[tokio::test]
    async fn find_variable_origin_empty_engine() {
        let events = vec![];
        let engines = engines_with_session("s1", events);
        let result = DebugTraceSpecializedService::find_variable_origin("s1", "x", 10, &engines)
            .await
            .unwrap();
        assert_eq!(result.mutation_count, 0);
        assert!(result.note.is_some());
    }

    // --- find_crash ---

    #[tokio::test]
    async fn find_crash_ok() {
        let events = vec![
            trace_event(1, 100, 1, EventType::FunctionEntry, "main"),
            signal_event(2, 200, 1, 11, "SIGSEGV"),
        ];
        let engines = engines_with_session("s1", events);
        let result = DebugTraceSpecializedService::find_crash("s1", &engines)
            .await
            .unwrap();
        assert!(result.crash_found);
        assert_eq!(result.signal, "SIGSEGV");
        assert_eq!(result.event_id, 2);
    }

    #[tokio::test]
    async fn find_crash_session_not_found() {
        let events = vec![];
        let engines = engines_with_session("s1", events);
        let result = DebugTraceSpecializedService::find_crash("missing", &engines).await;
        assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
    }

    #[tokio::test]
    async fn find_crash_no_signal() {
        let events = vec![
            trace_event(1, 100, 1, EventType::FunctionEntry, "main"),
            trace_event(2, 200, 1, EventType::FunctionExit, "main"),
        ];
        let engines = engines_with_session("s1", events);
        let result = DebugTraceSpecializedService::find_crash("s1", &engines)
            .await
            .unwrap();
        assert!(!result.crash_found);
    }

    // --- detect_races ---

    #[tokio::test]
    async fn detect_races_ok() {
        let events = vec![];
        let engines = engines_with_session("s1", events);
        let result = DebugTraceSpecializedService::detect_races("s1", 100, &engines)
            .await
            .unwrap();
        assert_eq!(result.session_id, "s1");
        assert_eq!(result.threshold_ns, 100);
        assert_eq!(result.access_count, 0);
    }

    #[tokio::test]
    async fn detect_races_session_not_found() {
        let events = vec![];
        let engines = engines_with_session("s1", events);
        let result = DebugTraceSpecializedService::detect_races("missing", 100, &engines).await;
        assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
    }

    #[tokio::test]
    async fn detect_races_empty_engine() {
        let events = vec![];
        let engines = engines_with_session("s1", events);
        let result = DebugTraceSpecializedService::detect_races("s1", 100, &engines)
            .await
            .unwrap();
        assert_eq!(result.access_count, 0);
        assert!(result.summary.contains("No suspicious"));
    }

    // --- inspect_causality ---

    #[tokio::test]
    async fn inspect_causality_ok() {
        let events = vec![];
        let engines = engines_with_session("s1", events);
        let result = DebugTraceSpecializedService::inspect_causality("s1", 0x1000, 10, &engines)
            .await
            .unwrap();
        assert_eq!(result.session_id, "s1");
        assert_eq!(result.address, 0x1000);
    }

    #[tokio::test]
    async fn inspect_causality_session_not_found() {
        let events = vec![];
        let engines = engines_with_session("s1", events);
        let result =
            DebugTraceSpecializedService::inspect_causality("missing", 0x1000, 10, &engines).await;
        assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
    }

    #[tokio::test]
    async fn inspect_causality_empty_engine() {
        let events = vec![];
        let engines = engines_with_session("s1", events);
        let result = DebugTraceSpecializedService::inspect_causality("s1", 0x1000, 10, &engines)
            .await
            .unwrap();
        assert_eq!(result.mutation_count, 0);
    }

    // --- expand_hotspot ---

    #[tokio::test]
    async fn expand_hotspot_ok() {
        let events = vec![
            trace_event(1, 100, 1, EventType::FunctionEntry, "main"),
            trace_event(2, 200, 1, EventType::FunctionExit, "main"),
        ];
        let engines = engines_with_session("s1", events);
        let result = DebugTraceSpecializedService::expand_hotspot("s1", 5, &engines)
            .await
            .unwrap();
        assert_eq!(result.session_id, "s1");
        assert_eq!(result.compression_level, "hotspot");
        assert_eq!(result.top_n, 5);
    }

    #[tokio::test]
    async fn expand_hotspot_session_not_found() {
        let events = vec![];
        let engines = engines_with_session("s1", events);
        let result = DebugTraceSpecializedService::expand_hotspot("missing", 5, &engines).await;
        assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
    }

    #[tokio::test]
    async fn expand_hotspot_empty_engine() {
        let events = vec![];
        let engines = engines_with_session("s1", events);
        let result = DebugTraceSpecializedService::expand_hotspot("s1", 5, &engines)
            .await
            .unwrap();
        assert_eq!(result.hotspot_functions.len(), 0);
        assert_eq!(result.total_calls_in_trace, 0);
    }

    // --- get_saliency_scores ---

    #[tokio::test]
    async fn get_saliency_scores_ok() {
        let events = vec![
            trace_event(1, 100, 1, EventType::FunctionEntry, "main"),
            trace_event(2, 200, 1, EventType::FunctionExit, "main"),
        ];
        let engines = engines_with_session("s1", events);
        let result = DebugTraceSpecializedService::get_saliency_scores("s1", 20, &engines)
            .await
            .unwrap();
        assert_eq!(result.session_id, "s1");
        assert_eq!(result.scored_functions, 1);
    }

    #[tokio::test]
    async fn get_saliency_scores_session_not_found() {
        let events = vec![];
        let engines = engines_with_session("s1", events);
        let result =
            DebugTraceSpecializedService::get_saliency_scores("missing", 20, &engines).await;
        assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
    }

    #[tokio::test]
    async fn get_saliency_scores_empty_engine() {
        let events = vec![];
        let engines = engines_with_session("s1", events);
        let result = DebugTraceSpecializedService::get_saliency_scores("s1", 20, &engines)
            .await
            .unwrap();
        assert_eq!(result.scored_functions, 0);
        assert!(result.hint.is_some());
    }
}
