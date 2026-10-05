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

use crate::error::ServiceError;
use crate::output::{
    CausalityReport, CrashPoint, CrashStackFrame, HotspotEntry, HotspotReport, LineageEntry,
    RaceReport, SaliencyScore, SaliencyScoreResult, VariableOriginResult,
};
use chronos_domain::query::{CausalityQuery, PerfQuery, PerfSortBy, RaceDetectionQuery};
use chronos_domain::trace::{EventData, EventType, TraceEvent};
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

    /// Identify the crash point in a trace: find the fatal signal that ended
    /// the tracee and reconstruct the call stack at that point.
    ///
    /// Returns `Ok(CrashPoint)` with `crash_found = false` when no fatal
    /// signal is present.
    ///
    /// The verdict prefers a fatal signal the tracer could not have sent and
    /// only falls back to `SIGKILL` when none exists, because chronos
    /// `SIGKILL`s its own tracee during session teardown. A `SIGKILL` verdict
    /// therefore carries a `note`: it may be that teardown kill rather than a
    /// crash of the program, and the trace alone cannot tell them apart.
    pub async fn find_crash(
        session_id: &str,
        engines: &Mutex<HashMap<String, QueryEngine>>,
    ) -> Result<CrashPoint, ServiceError> {
        let guard = engines.lock().await;
        let engine = guard
            .get(session_id)
            .ok_or_else(|| ServiceError::SessionNotFound(session_id.to_string()))?;

        // The one signal chronos itself delivers to its own tracee. The
        // teardown paths in `probe_backend.rs` (live-probe loop cleanup,
        // clone reaping) and in `capture_runner.rs` call `kill`/`waitpid` on
        // the traced pid, and the frame-capture helper failure path does the
        // same, so the tracer SIGKILLs its own tracee at the end of a normal
        // session. That kill is recorded as a plain `SignalDelivered`, which
        // is what made the previous "first fatal signal wins" rule report
        // `crash_found = true, signal = "SIGKILL"` for programs that never
        // crashed: the verdict blamed the program for a kill chronos sent.
        const TRACER_TEARDOWN_SIGNAL: &str = "SIGKILL";

        // Fatal signals the tracer cannot fabricate. Every one of them is
        // delivered by the tracee itself, so its presence in the log is proof
        // that the tracee died on it.
        let fatal_signals = ["SIGSEGV", "SIGABRT", "SIGBUS", "SIGILL", "SIGFPE"];

        let is_fatal = |name: &str| name == TRACER_TEARDOWN_SIGNAL || fatal_signals.contains(&name);

        let query = TraceQuery::new(session_id)
            .event_types(vec![EventType::SignalDelivered])
            .pagination(usize::MAX, 0);
        let result = engine.execute(&query);

        let fatal_events: Vec<(&TraceEvent, &str)> = result
            .events
            .iter()
            .filter_map(|e| match &e.data {
                EventData::Signal { signal_name, .. } if is_fatal(signal_name) => {
                    Some((e, signal_name.as_str()))
                }
                _ => None,
            })
            .collect();

        // Classification rule: prefer the first fatal signal the tracer could
        // not have sent, and fall back to the first SIGKILL only when there is
        // none.
        //
        // The ordering property of the teardown kill is what makes this
        // decidable rather than heuristic: chronos SIGKILLs its tracee while
        // tearing the session down, so that event is always the last one in
        // the log and always comes after whatever actually happened to the
        // tracee. Therefore any non-SIGKILL fatal signal in the log, whether
        // it precedes or follows the teardown kill, describes the real death
        // and outranks every SIGKILL. A SIGKILL is only the verdict when it
        // is the sole fatal signal, and that case stays genuinely ambiguous:
        // the OOM killer killing a user's process is a real debugging
        // scenario that must not be dropped, so the verdict is reported with
        // a `note` stating that it may be the tracer's own teardown kill.
        let crash_event = fatal_events
            .iter()
            .find(|(_, name)| *name != TRACER_TEARDOWN_SIGNAL)
            .or_else(|| fatal_events.first())
            .map(|(ev, name)| (*ev, *name));

        match crash_event {
            Some((ev, signal_name)) => {
                let stack = engine.reconstruct_call_stack(ev.event_id);

                let note = if signal_name == TRACER_TEARDOWN_SIGNAL {
                    Some(
                        "SIGKILL is the only fatal signal in the trace: the crash may be the \
                         tracer's own teardown kill (chronos SIGKILLs its tracee during session \
                         cleanup) or a genuine external kill such as the OOM killer, and the \
                         trace alone cannot tell them apart"
                            .to_string(),
                    )
                } else {
                    None
                };

                Ok(CrashPoint {
                    session_id: session_id.to_string(),
                    crash_found: true,
                    signal: signal_name.to_string(),
                    event_id: ev.event_id,
                    timestamp_ns: ev.timestamp_ns.get(),
                    thread_id: ev.thread_id,
                    call_stack_depth: stack.len(),
                    call_stack: stack.into_iter().map(CrashStackFrame::from).collect(),
                    note,
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

        // Distinct addresses the detector inspected, NOT the number of write
        // events. The engine groups writes by address and counts one per
        // address, so this is an address count. The `total_writes` field it
        // feeds is a published wire key and keeps its name (see
        // `RaceReport::total_writes`); the local name says what it holds.
        let addresses_checked = result.addresses_checked;

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
                addresses_checked
            )
        };

        Ok(RaceReport {
            session_id: session_id.to_string(),
            threshold_ns,
            access_count: result.accesses.len(),
            accesses: result.accesses,
            total_writes: addresses_checked,
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

        // Sum over the functions the engine analysed (its `top_functions` list
        // is truncated to the 20 hottest), which is why this is NOT the call
        // total of the trace despite what `total_calls_in_trace` suggests.
        // The name here is local, so it states the truth: this is the
        // denominator of a quota *within the analysed set*.
        let analyzed_calls: u64 = summary.top_functions.iter().map(|f| f.call_count).sum();

        Ok(HotspotReport {
            session_id: session_id.to_string(),
            compression_level: "hotspot".to_string(),
            top_n,
            total_calls_in_trace: analyzed_calls,
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
            // Fallback when no perf samples exist: score by call count.
            //
            // The denominator is the sum of the call counts of the analysed set
            // (`top_functions`, truncated to the 20 hottest), NOT the call
            // total of the trace. That is the right choice here: the score
            // answers "which share of the calls *of the analysed set* does this
            // function account for?", and numerator and denominator cover the
            // same set, so comparing two functions is consistent. Using the
            // real trace total would instead measure the analysed set's share
            // of the whole session, which is not what saliency ranks.
            let analyzed_calls: u64 = summary.top_functions.iter().map(|f| f.call_count).sum();
            summary
                .top_functions
                .iter()
                .take(limit)
                .map(|f| {
                    let score = if analyzed_calls > 0 {
                        f.call_count as f64 / analyzed_calls as f64
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
    use chronos_domain::MonotonicNs;

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
            timestamp_ns: MonotonicNs::from(timestamp_ns),
            thread_id,
            event_type,
            location: SourceLocation {
                function: Some(function.to_string()),
                ..SourceLocation::default()
            },
            data: EventData::Empty,
        }
    }

    /// A `MemoryWrite` event anchored at a concrete address. The race detector
    /// groups writes by `location.address`, not by `EventData::Memory`.
    fn memory_write_event(
        event_id: u64,
        timestamp_ns: u64,
        thread_id: u64,
        address: u64,
    ) -> TraceEvent {
        TraceEvent {
            event_id,
            timestamp_ns: MonotonicNs::from(timestamp_ns),
            thread_id,
            event_type: EventType::MemoryWrite,
            location: SourceLocation {
                address,
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
            timestamp_ns: MonotonicNs::from(timestamp_ns),
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
        assert_eq!(
            result.note.as_deref(),
            Some("No fatal signal found in the trace")
        );
    }

    /// A delivered signal that is not fatal must not become a crash verdict.
    /// Pins the "no fatal signal at all" branch, whose `note` is a published
    /// value of the tool contract.
    #[tokio::test]
    async fn find_crash_only_non_fatal_signal_is_not_a_crash() {
        let events = vec![
            trace_event(1, 100, 1, EventType::FunctionEntry, "main"),
            signal_event(2, 200, 1, 10, "SIGUSR1"),
        ];
        let engines = engines_with_session("s1", events);
        let result = DebugTraceSpecializedService::find_crash("s1", &engines)
            .await
            .unwrap();
        assert!(!result.crash_found);
        assert_eq!(result.signal, "");
        assert_eq!(
            result.note.as_deref(),
            Some("No fatal signal found in the trace")
        );
    }

    /// The real-world shape measured on the `test_abort` fixture: the program
    /// really aborts, and the tracer's teardown SIGKILL lands afterwards as a
    /// plain `SignalDelivered`. The verdict must name the program's own signal,
    /// not the teardown kill.
    #[tokio::test]
    async fn find_crash_prefers_real_fatal_signal_over_teardown_sigkill() {
        let events = vec![
            trace_event(1, 100, 1, EventType::FunctionEntry, "main"),
            signal_event(2, 200, 1, 6, "SIGABRT"),
            // Tracer teardown kill, strictly after the abort.
            signal_event(3, 300, 1, 9, "SIGKILL"),
        ];
        let engines = engines_with_session("s1", events);
        let result = DebugTraceSpecializedService::find_crash("s1", &engines)
            .await
            .unwrap();
        assert!(result.crash_found);
        assert_eq!(result.signal, "SIGABRT");
        assert_eq!(result.event_id, 2);
        assert_eq!(result.thread_id, 1);
        // A signal the tracer cannot fabricate needs no hedging note.
        assert_eq!(result.note, None);
    }

    /// The rule is stated on the signal, not on the position: a non-SIGKILL
    /// fatal signal outranks a SIGKILL even when the SIGKILL is logged first.
    /// This is the only ordering where the previous "first fatal signal wins"
    /// rule and the current rule disagree, so it is what pins the fix: the
    /// log order alone must not decide which signal the verdict names.
    #[tokio::test]
    async fn find_crash_prefers_real_fatal_signal_when_sigkill_is_earlier() {
        let events = vec![
            signal_event(1, 100, 1, 9, "SIGKILL"),
            signal_event(2, 200, 1, 8, "SIGFPE"),
        ];
        let engines = engines_with_session("s1", events);
        let result = DebugTraceSpecializedService::find_crash("s1", &engines)
            .await
            .unwrap();
        assert!(result.crash_found);
        assert_eq!(result.signal, "SIGFPE");
        assert_eq!(result.event_id, 2);
        assert_eq!(result.note, None);
    }

    /// The one case the rule cannot settle: a lone SIGKILL. It is still
    /// reported as a crash because the OOM killer killing a user's process is
    /// real, but the note must not let a reader take it as proof.
    #[tokio::test]
    async fn find_crash_only_sigkill_still_detected_with_teardown_note() {
        let events = vec![
            trace_event(1, 100, 1, EventType::FunctionEntry, "main"),
            signal_event(2, 200, 1, 9, "SIGKILL"),
        ];
        let engines = engines_with_session("s1", events);
        let result = DebugTraceSpecializedService::find_crash("s1", &engines)
            .await
            .unwrap();
        assert!(result.crash_found);
        assert_eq!(result.signal, "SIGKILL");
        assert_eq!(result.event_id, 2);
        let note = result.note.expect("a lone SIGKILL must be hedged");
        assert!(
            note.contains("teardown"),
            "note must name the teardown kill: {note}"
        );
        assert!(
            note.contains("SIGKILL"),
            "note must name the signal: {note}"
        );
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

    // --- unit/label contract: the value is right, the name overstates it ---

    /// `total_calls_in_trace` is the sum over the functions the engine
    /// *analysed* (its top-20 hottest list), not the call total of the trace.
    /// With more than 20 distinct functions the two differ, and the field must
    /// be the smaller one. The field name promises the larger one, so this test
    /// pins the real relationship: if the value were ever "corrected" into the
    /// true trace total, the strict inequality below fails.
    #[tokio::test]
    async fn expand_hotspot_total_calls_is_analysed_sum_not_trace_total() {
        // 25 distinct functions x 4 entries each = 100 real calls in the trace.
        const FUNCTIONS: usize = 25;
        const CALLS_PER_FUNCTION: u64 = 4;
        const ENGINE_TOP_N: usize = 20;

        let mut events = Vec::new();
        let mut event_id = 1u64;
        for f in 0..FUNCTIONS {
            for _ in 0..CALLS_PER_FUNCTION {
                events.push(trace_event(
                    event_id,
                    100 + event_id,
                    1,
                    EventType::FunctionEntry,
                    &format!("fn_{f:02}"),
                ));
                event_id += 1;
            }
        }
        let trace_total_calls: u64 = FUNCTIONS as u64 * CALLS_PER_FUNCTION;
        assert_eq!(trace_total_calls, 100, "fixture sanity: 25 x 4 real calls");

        let engines = engines_with_session("s1", events);
        // top_n above 20 so the report is limited by the engine's truncation,
        // not by the caller's window.
        let result = DebugTraceSpecializedService::expand_hotspot("s1", FUNCTIONS, &engines)
            .await
            .unwrap();

        // Only the engine's top 20 survive `truncate(20)`.
        assert_eq!(result.hotspot_functions.len(), ENGINE_TOP_N);
        assert_eq!(
            result.total_calls_in_trace,
            ENGINE_TOP_N as u64 * CALLS_PER_FUNCTION,
            "total_calls_in_trace must be the sum over the analysed top-20 set"
        );
        assert!(
            result.total_calls_in_trace < trace_total_calls,
            "total_calls_in_trace ({}) must stay below the real trace call total ({}): \
             the engine analyses only the 20 hottest functions, so the field is an \
             analysed-set sum and not the trace total its name suggests",
            result.total_calls_in_trace,
            trace_total_calls
        );
    }

    /// `RaceReport.total_writes` counts the distinct ADDRESSES the detector
    /// inspected, not the write events. Two writes to one address must report
    /// 1, not 2. If the value were ever changed into a write-event count, the
    /// equality below fails.
    #[tokio::test]
    async fn detect_races_total_writes_counts_addresses_not_write_events() {
        // Two write events, different threads, SAME address, 10ns apart.
        let events = vec![
            memory_write_event(1, 100, 1, 0x1000),
            memory_write_event(2, 110, 2, 0x1000),
        ];
        // The detector needs a causality index configured, but it may be empty:
        // the write grouping walks the events, and the index is only a lookup
        // with an event-derived fallback.
        let engine = make_engine(events).with_causality(chronos_domain::CausalityIndex::new());
        let mut map = HashMap::new();
        map.insert("s1".to_string(), engine);
        let engines = Mutex::new(map);

        let result = DebugTraceSpecializedService::detect_races("s1", 100, &engines)
            .await
            .unwrap();

        assert_eq!(
            result.total_writes, 1,
            "total_writes counts distinct addresses checked, not write events: \
             2 writes to 0x1000 is 1 address"
        );
        // The access itself is still reported, so the count above is not a
        // vacuous 0 from an unpopulated detector.
        assert_eq!(
            result.access_count, 1,
            "one cross-thread pair, got {result:?}"
        );
        assert!(
            result.summary.contains("across 1 address"),
            "the human summary must state the address unit too, got {:?}",
            result.summary
        );
    }
}
