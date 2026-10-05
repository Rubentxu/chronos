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

/// Marker `native_adapter` writes when the tracee returned from `main`.
const PROCESS_EXIT_MARKER: &str = "process_exit";

/// The signal the tracer sends its own tracee on the way out, and so the one
/// death that is not the program's own doing. The live-probe loop cleanup,
/// the clone reaping and the frame-capture failure path in
/// `probe_backend.rs` and `capture_runner.rs` all `kill`/`waitpid` the traced
/// pid, so a normal session ends in a SIGKILL that means nothing about the
/// tracee.
pub(crate) const TRACER_TEARDOWN_SIGNAL: &str = "SIGKILL";

/// Signals that end a process, whether or not the tracer sent them.
///
/// Membership says nothing about death on its own — a tracee with a
/// `sigaction` handler can survive any of these. What makes them worth
/// reporting is that the tracer does not manufacture them, so a death from
/// one of them is the program's own doing.
pub(crate) const FATAL_SIGNALS: [&str; 5] = ["SIGSEGV", "SIGABRT", "SIGBUS", "SIGILL", "SIGFPE"];

/// What the log says about how the tracee ended, kept apart from what it
/// merely received.
///
/// This split is the whole point: a `SignalDelivered` event records the
/// *delivery* of a signal, and a tracee that installs a `sigaction` handler
/// and then `raise()`s the signal records a delivery and can still exit 0.
/// A crash verdict that rests on deliveries cannot tell those two apart, and
/// so cannot be wrong in only one direction.
///
/// Each fact carries the event that proves it, so a verdict can point at the
/// place the tracee died rather than at the first signal that happened to be
/// fatal.
#[derive(Debug, Default)]
pub(crate) struct TerminationFacts<'a> {
    /// Signals the tracee actually died from, in the order they appear.
    deaths: Vec<(&'a TraceEvent, &'a str)>,
    /// The exit event, when the tracee returned normally.
    exit: Option<(&'a TraceEvent, i32)>,
    /// Signals the tracee received and kept running through.
    survived: Vec<(&'a TraceEvent, &'a str)>,
}

impl TerminationFacts<'_> {
    /// A signal from `fatal_signals` that this tracee received, whether or not
    /// it lived through it.
    ///
    /// A fact about delivery, not about death — but "delivery" is the whole
    /// of it. A signal that killed the tracee was also delivered to it, and
    /// narrowing this to the survivors changes what a caller already reads:
    /// `session_explain`'s `FactsBundle::signal_delivered` reported the fatal
    /// signal that ended a crashing session, and a bundle that answers
    /// `crash_detected: true` with no signal next to it says less than the
    /// one it replaces.
    ///
    /// The verdict is a separate question with a separate answer; see
    /// `decide_crash`. Kept here so no consumer re-reads `deaths` and
    /// `survived` to work out which signals a tracee saw.
    pub(crate) fn first_fatal_seen(&self, fatal_signals: &[&str]) -> Option<&str> {
        self.deaths
            .iter()
            .map(|(_, name)| *name)
            .chain(self.survived.iter().map(|(_, name)| *name))
            .find(|name| fatal_signals.contains(name))
    }
}

#[derive(Debug)]
pub(crate) enum CrashVerdict<'a> {
    /// The tracee died from a signal the tracer did not send.
    Crashed {
        event: &'a TraceEvent,
        signal: &'a str,
    },
    /// The tracee died, and the only signal is the one chronos sends while
    /// tearing the session down.
    KilledByTeardown {
        event: &'a TraceEvent,
        signal: &'a str,
    },
    /// The tracee did not die from a signal.
    Survived { reason: String },
}

/// Decide the crash verdict from what the log proves, not from what it merely
/// contains.
///
/// The rules, in order of how much they prove:
///
/// 1. A death the tracer did not send is a crash. Nothing overrides it.
/// 2. A death by the teardown signal is a death, but not attributable: it may
///    be chronos killing its own tracee at cleanup, or something external such
///    as the OOM killer. Reported, with the ambiguity declared.
/// 3. No death, but a normal exit: the tracee finished on its own. Fatal
///    signals it survived are then evidence *against* a crash, not for one
///    -- but only if the exit came *after* them. A `process_exit` recorded
///    before a later fatal signal belongs to a different thread leaving, not
///    to the process finishing, and treating it as one invents the sentence
///    "it handled SIGSEGV and returned 0" about a tracee that did not.
/// 4. No death and no exit: the log carries no termination record, so the
///    older signal-splitting rule runs as a fallback. That is the only case
///    where a delivery is treated as evidence, and the verdict says so.
///
/// Rule 3 is what removes the false positive. A program that handles SIGSEGV
/// and returns 0 delivers a SIGSEGV and does not die from it; calling that a
/// crash is a claim the trace cannot support.
///
/// Rule 3's ordering condition is what keeps the fix from introducing the
/// opposite error. `ptrace` reports `Exited` per thread group, and
/// `native_adapter` writes a `process_exit` marker for each, so a session
/// with worker threads carries several. On a log recorded before
/// `terminated_tracee` existed, a thread leaving normally and the main thread
/// then dying reads as "exit, then SIGSEGV" -- and calling that "handled"
/// would turn a crash into a clean run, which is the worse direction to
/// fail.
pub(crate) fn decide_crash<'a>(
    facts: &TerminationFacts<'a>,
    teardown_signal: &str,
    fatal_signals: &[&str],
) -> CrashVerdict<'a> {
    if let Some((event, signal)) = facts
        .deaths
        .iter()
        .find(|(_, name)| *name != teardown_signal)
    {
        return CrashVerdict::Crashed { event, signal };
    }

    if let Some((event, signal)) = facts
        .deaths
        .iter()
        .find(|(_, name)| *name == teardown_signal)
    {
        return CrashVerdict::KilledByTeardown { event, signal };
    }

    if let Some((exit_event, code)) = facts.exit {
        // An exit only says "the process finished on its own terms" if
        // nothing fatal came after it. Ordering is by event id, which is
        // monotonic in the log and does not depend on clock resolution
        // between the capture thread and the traced process.
        let fatal_after_exit = facts.survived.iter().any(|(event, name)| {
            fatal_signals.contains(name) && event.event_id > exit_event.event_id
        });
        if !fatal_after_exit {
            let survived_fatal = facts
                .survived
                .iter()
                .map(|(_, name)| *name)
                .filter(|name| fatal_signals.contains(name))
                .collect::<Vec<_>>();

            let reason = if survived_fatal.is_empty() {
                format!("The tracee returned normally with exit code {code}: no signal killed it")
            } else {
                format!(
                    "The tracee received {} and still returned with exit code {}: \
                     it handled the signal and did not die from it",
                    survived_fatal.join(", "),
                    code
                )
            };
            return CrashVerdict::Survived { reason };
        }
        // A fatal signal after the exit means that exit was not this
        // process finishing. Fall through to rule 4, which reads the
        // delivery as evidence because the log no longer proves survival.
    }

    // No termination record at all. Keep the old behaviour so that traces
    // captured before the tracer recorded terminations still resolve, but say
    // out loud that the verdict rests on deliveries.
    if let Some((event, signal)) = facts
        .survived
        .iter()
        .find(|(_, name)| *name != teardown_signal && fatal_signals.contains(name))
    {
        return CrashVerdict::Crashed { event, signal };
    }
    if let Some((event, name)) = facts
        .survived
        .iter()
        .find(|(_, name)| *name == teardown_signal)
    {
        return CrashVerdict::KilledByTeardown {
            event,
            signal: name,
        };
    }

    CrashVerdict::Survived {
        reason: "No fatal signal found in the trace".to_string(),
    }
}

/// Split a trace into what it proves about the tracee's end: the signals it
/// died from, the exit it returned with, and the signals it walked away from.
///
/// The single reader of both facts. Every consumer of a crash verdict in this
/// crate builds its `TerminationFacts` here, so a session cannot be scored
/// one way by `find_crash` and another by `session_explain`.
pub(crate) fn facts_from(events: &[TraceEvent]) -> TerminationFacts<'_> {
    let mut facts = TerminationFacts::default();
    for event in events {
        match &event.data {
            EventData::Signal {
                signal_name,
                terminated_tracee,
                ..
            } => {
                if *terminated_tracee {
                    facts.deaths.push((event, signal_name.as_str()));
                } else {
                    facts.survived.push((event, signal_name.as_str()));
                }
            }
            EventData::Custom { name, data_json }
                if name == PROCESS_EXIT_MARKER && facts.exit.is_none() =>
            {
                facts.exit = parse_exit_code(data_json).map(|code| (event, code));
            }
            _ => {}
        }
    }
    facts
}

/// Read the exit code out of the `process_exit` marker.
///
/// `native_adapter` writes it as `{"exit_code": N}`. A marker that does not
/// parse is not a termination record: returning `None` keeps a malformed
/// marker from silently becoming proof of a clean exit.
fn parse_exit_code(data_json: &str) -> Option<i32> {
    let value: serde_json::Value = serde_json::from_str(data_json).ok()?;
    value.get("exit_code")?.as_i64().map(|code| code as i32)
}

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

    /// Identify the crash point in a trace and reconstruct the call stack
    /// where the tracee actually died.
    ///
    /// The verdict rests on the tracee's **termination**, not on the signals
    /// it received. The tracer records a `SignalDelivered` both when a signal
    /// is delivered and when the signal kills the tracee, and the two look
    /// identical in the log unless you ask for the difference; a tracee that
    /// handles `SIGSEGV` and returns 0 is the case that a delivery-based
    /// verdict gets wrong. `native_adapter` therefore marks the death, and
    /// this method prefers that marker over any signal it can see.
    ///
    /// Returns `crash_found = false` when the tracee is shown to have ended
    /// without dying from a signal, which now includes the case where it
    /// received a fatal signal and survived it.
    ///
    /// A `SIGKILL` verdict carries a `note` declaring the ambiguity: chronos
    /// `SIGKILL`s its own tracee during session teardown, so that death may be
    /// the tracer's own doing rather than a crash of the program, and the
    /// trace alone cannot tell the two apart.
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
        // session.
        //
        // `TRACER_TEARDOWN_SIGNAL` and `FATAL_SIGNALS` are declared at module
        // scope: `session_explain` asks the same question of the same traces,
        // and a second copy of the signal set is how the two answers drift.

        // Two event kinds, because the two facts live in different ones: a
        // signal that ended the tracee is a `SignalDelivered` carrying
        // `terminated_tracee`, and a normal return is a `process_exit` marker.
        // Querying only signals would miss a program that lived long enough to
        // be exonerated, which is the case the verdict most needs to catch.
        let query = TraceQuery::new(session_id)
            .event_types(vec![EventType::SignalDelivered, EventType::Custom])
            .pagination(usize::MAX, 0);
        let result = engine.execute(&query);

        let mut facts = TerminationFacts::default();

        for event in &result.events {
            match &event.data {
                EventData::Signal {
                    signal_name,
                    terminated_tracee,
                    ..
                } => {
                    if *terminated_tracee {
                        facts.deaths.push((event, signal_name.as_str()));
                    } else {
                        facts.survived.push((event, signal_name.as_str()));
                    }
                }
                // First readable exit wins: a later marker cannot un-end the
                // process. A marker that does not parse leaves the slot open,
                // so one unreadable marker does not hide a readable one.
                EventData::Custom { name, data_json }
                    if name == PROCESS_EXIT_MARKER && facts.exit.is_none() =>
                {
                    facts.exit = parse_exit_code(data_json).map(|code| (event, code));
                }
                _ => {}
            }
        }

        match decide_crash(&facts, TRACER_TEARDOWN_SIGNAL, &FATAL_SIGNALS) {
            CrashVerdict::Crashed { event, signal } => {
                let stack = engine.reconstruct_call_stack(event.event_id);
                Ok(CrashPoint {
                    session_id: session_id.to_string(),
                    crash_found: true,
                    signal: signal.to_string(),
                    event_id: event.event_id,
                    timestamp_ns: event.timestamp_ns.get(),
                    thread_id: event.thread_id,
                    call_stack_depth: stack.len(),
                    call_stack: stack.into_iter().map(CrashStackFrame::from).collect(),
                    // A signal the tracer cannot fabricate needs no hedging.
                    note: None,
                })
            }
            CrashVerdict::KilledByTeardown { event, signal } => {
                let stack = engine.reconstruct_call_stack(event.event_id);
                Ok(CrashPoint {
                    session_id: session_id.to_string(),
                    crash_found: true,
                    signal: signal.to_string(),
                    event_id: event.event_id,
                    timestamp_ns: event.timestamp_ns.get(),
                    thread_id: event.thread_id,
                    call_stack_depth: stack.len(),
                    call_stack: stack.into_iter().map(CrashStackFrame::from).collect(),
                    note: Some(
                        "The tracee died by the only signal chronos sends on its own, and the \
                         trace cannot say whether that was the tracer's teardown kill (chronos \
                         SIGKILLs its tracee during session cleanup) or something external such \
                         as the OOM killer."
                            .to_string(),
                    ),
                })
            }
            CrashVerdict::Survived { reason } => {
                // Event coordinates stay at zero, as they have for every
                // no-crash answer: there is no crash point to point at, and
                // inventing one from the exit marker would claim a call site
                // the trace never captured.
                Ok(CrashPoint {
                    session_id: session_id.to_string(),
                    crash_found: false,
                    signal: String::new(),
                    event_id: 0,
                    timestamp_ns: 0,
                    thread_id: 0,
                    call_stack_depth: 0,
                    call_stack: vec![],
                    note: Some(reason),
                })
            }
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
    /// and, when the capture had hardware counters, CPU cycles.
    ///
    /// The "when" is load-bearing. Nothing in the capture path records cycles:
    /// `IndexBuilder` is the only production caller of `record_call` and passes
    /// `cycles: None` with the comment "Cycle counts not available from trace
    /// events directly". So `FunctionPerf::total_cycles` keeps the `0` that
    /// `FunctionPerf::new` starts it with, forever, and publishing that `0`
    /// as `Some(0)` states that zero cycles were measured across N calls —
    /// which is not the same claim as "no cycles were measured".
    ///
    /// The counters that *were* read are discarded, so the report now says
    /// which of the two it is instead of implying the first.
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

        // One query for the whole set, because `counters_available` is a
        // property of the session and not of one function: asking per function
        // and taking `.next()` discarded the only field that says whether the
        // numbers mean anything.
        let perf = engine.query_perf(&PerfQuery {
            session_id: session_id.to_string(),
            function_filter: None,
            sort_by: PerfSortBy::Cycles,
            limit: usize::MAX,
        });
        let cycles_measured = perf.as_ref().is_some_and(|p| p.counters_available);

        let mut hotspot_functions = Vec::new();
        for f in summary.top_functions.iter().take(top_n) {
            let perf_entry = perf.as_ref().and_then(|r| {
                r.functions
                    .iter()
                    .find(|e| e.name.as_deref() == Some(f.name.as_str()))
            });

            hotspot_functions.push(HotspotEntry {
                function: f.name.clone(),
                call_count: f.call_count,
                total_cycles: match perf_entry {
                    // Measured, so the number is a measurement. `Some(0)` for
                    // a function that was genuinely never scheduled is a real
                    // zero and stays one.
                    Some(e) if cycles_measured => Some(e.total_cycles),
                    // Not measured. `None` means "unknown", which is the truth;
                    // `Some(0)` would mean "we watched it burn no cycles",
                    // which nobody watched.
                    _ => None,
                },
                avg_cycles_per_call: match perf_entry {
                    Some(e) if cycles_measured => Some(e.avg_cycles),
                    _ => None,
                },
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
            hint: Some({
                let base = "Use debug_call_graph for full call graph or query_events to drill into specific functions";
                if cycles_measured {
                    base.to_string()
                } else {
                    format!(
                        "{base}. NOTE: total_cycles and avg_cycles_per_call are null because this \
                         capture has no hardware performance counters; the ranking is by call count, \
                         which is the only cost signal in this session."
                    )
                }
            }),
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

        // Ask for cycles only when there are counters to have measured them
        // with. Sorting a set whose cycles are all zero by cycles is an
        // arbitrary order that looks like a ranking.
        let counters = engine
            .query_perf(&PerfQuery {
                session_id: session_id.to_string(),
                function_filter: None,
                sort_by: PerfSortBy::Cycles,
                limit: 1,
            })
            .map(|p| p.counters_available)
            .unwrap_or(false);

        let perf_result = engine.query_perf(&PerfQuery {
            session_id: session_id.to_string(),
            function_filter: None,
            sort_by: if counters {
                PerfSortBy::Cycles
            } else {
                PerfSortBy::CallCount
            },
            limit,
        });

        // Whether cycles were measured at all, taken from the one field that
        // says so. It used to be discarded and the code read `perf.functions`
        // unconditionally, so in production `total_cycles` was always 0, the
        // sum was always 0, and every function scored 0.0 — a flat ranking
        // presented as a measurement. The `else` below, the only branch that
        // emitted the disclosure the type already defines, was unreachable.
        let perf_by_cycles = if counters { perf_result.clone() } else { None };
        let scores: Vec<SaliencyScore> = if let Some(perf) = perf_by_cycles {
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
            // No cycles were measured. Score by call count, which IS in the
            // log, and say so — `cycles: Some(())` is the disclosure the
            // `SaliencyScore` type has always declared for exactly this case
            // and which production could never emit.
            //
            // The denominator is the sum over the same set being ranked, not
            // the call total of the trace: the score answers "which share of
            // the analysed calls does this function account for?", and using
            // the trace total would measure the analysed set's share of the
            // session instead, which is not what a per-function rank means.
            //
            // The set comes from the perf index when there is one and from
            // `summary.top_functions` when there is not, so the two paths
            // agree on WHICH functions are listed; they differ only in
            // whether the per-function cycle columns exist.
            let entries: Vec<(String, u64)> = match perf_result {
                Some(perf) => perf
                    .functions
                    .into_iter()
                    .map(|e| {
                        (
                            e.name.unwrap_or_else(|| "<unknown>".to_string()),
                            e.call_count,
                        )
                    })
                    .collect(),
                None => summary
                    .top_functions
                    .iter()
                    .map(|f| (f.name.clone(), f.call_count))
                    .collect(),
            };
            let analyzed_calls: u64 = entries.iter().map(|(_, calls)| *calls).sum();
            entries
                .into_iter()
                .take(limit)
                .map(|(name, call_count)| {
                    let score = if analyzed_calls > 0 {
                        call_count as f64 / analyzed_calls as f64
                    } else {
                        0.0
                    };
                    SaliencyScore {
                        function: name,
                        saliency_score: (score * 10000.0).round() / 10000.0,
                        call_count,
                        total_cycles: None,
                        cycles: Some(()),
                    }
                })
                .collect()
        };

        // The unit is the claim. Saying "near 1.0 means it dominated CPU time"
        // unconditionally is what made an unmeasured ranking dangerous: a
        // reader could not tell a flat ranking from a flat *measurement*.
        let hint = if counters {
            "saliency_score near 1.0 means this function dominated CPU time. Use debug_expand_hotspot to zoom in."
        } else {
            "This session has no hardware performance counters, so saliency_score ranks by CALL COUNT, not CPU time: near 1.0 means this function took that share of the calls analysed, and says nothing about how much time it took. No cycles were measured. Use debug_expand_hotspot to zoom in."
        };

        Ok(SaliencyScoreResult {
            session_id: session_id.to_string(),
            scored_functions: scores.len(),
            scores,
            hint: Some(hint.to_string()),
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
                terminated_tracee: false,
            },
        }
    }

    /// A signal that actually ended the tracee, which is a different fact from
    /// a signal the tracee received and survived.
    fn signal_termination_event(
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
                terminated_tracee: true,
            },
        }
    }

    /// The tracee running to completion and returning `code`, which
    /// `native_adapter` records as a `process_exit` marker.
    fn process_exit_event(
        event_id: u64,
        timestamp_ns: u64,
        thread_id: u64,
        code: i32,
    ) -> TraceEvent {
        TraceEvent {
            event_id,
            timestamp_ns: MonotonicNs::from(timestamp_ns),
            thread_id,
            event_type: EventType::Custom,
            location: SourceLocation::default(),
            data: EventData::Custom {
                name: "process_exit".to_string(),
                data_json: format!(r#"{{"exit_code": {}}}"#, code),
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

    /// The shape a real abort produces today: the program dies from `SIGABRT`
    /// and the tracer's teardown `SIGKILL` lands afterwards. The verdict must
    /// name the signal that actually killed the tracee.
    #[tokio::test]
    async fn find_crash_reports_the_signal_that_killed_the_tracee() {
        let events = vec![
            trace_event(1, 100, 1, EventType::FunctionEntry, "main"),
            signal_termination_event(2, 200, 1, 6, "SIGABRT"),
            // Tracer teardown kill, strictly after the abort.
            signal_termination_event(3, 300, 1, 9, "SIGKILL"),
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

    /// The false positive this change exists to remove.
    ///
    /// A program that installs a `sigaction` handler and then `raise()`s a
    /// fatal signal delivers that signal and returns 0. The trace records the
    /// delivery *and* the normal exit, and the verdict has to say there was no
    /// crash: it has the proof. A rule that treats any fatal delivery as a
    /// crash reports a crash that never happened, and there is no way for a
    /// reader to tell that from a real one.
    ///
    /// Under the delivery-based rule this fixture came back
    /// `crash_found = true, signal = "SIGSEGV"`.
    #[tokio::test]
    async fn find_crash_a_handled_fatal_signal_is_not_a_crash() {
        let events = vec![
            trace_event(1, 100, 1, EventType::FunctionEntry, "main"),
            // The tracee handles the signal, keeps running, and returns 0.
            signal_event(2, 200, 1, 11, "SIGSEGV"),
            process_exit_event(3, 300, 1, 0),
        ];
        let engines = engines_with_session("s1", events);
        let result = DebugTraceSpecializedService::find_crash("s1", &engines)
            .await
            .unwrap();
        assert!(
            !result.crash_found,
            "a delivered signal the tracee survived is not a crash"
        );
        assert_eq!(result.signal, "");
        let note = result.note.expect("an exoneration must say why");
        assert!(
            note.contains("SIGSEGV") && note.contains("handled"),
            "the note must name the signal and the fact it was handled: {note}"
        );
    }

    /// A normal exit with no signal at all: the ordinary "no crash" answer,
    /// now with the exit code to back it instead of an absence of evidence.
    #[tokio::test]
    async fn find_crash_normal_exit_without_signals_is_not_a_crash() {
        let events = vec![
            trace_event(1, 100, 1, EventType::FunctionEntry, "main"),
            process_exit_event(2, 200, 1, 0),
        ];
        let engines = engines_with_session("s1", events);
        let result = DebugTraceSpecializedService::find_crash("s1", &engines)
            .await
            .unwrap();
        assert!(!result.crash_found);
        assert_eq!(result.signal, "");
        assert!(result.note.unwrap().contains("exit code 0"));
    }

    /// A death outranks a later exit marker, and a teardown kill recorded
    /// first must not hide the real cause. The ordering is the point: the
    /// log order alone must not decide which signal the verdict names.
    #[tokio::test]
    async fn find_crash_a_death_outranks_a_later_normal_exit() {
        let events = vec![
            signal_termination_event(1, 100, 1, 9, "SIGKILL"),
            signal_termination_event(2, 200, 1, 8, "SIGFPE"),
            process_exit_event(3, 300, 1, 0),
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

    /// Traces captured before the tracer recorded terminations carry neither a
    /// death nor an exit. They still have to resolve, so the old
    /// delivery-splitting rule runs as a fallback — and the verdict rests on
    /// deliveries alone, which is exactly the weakness this change removes
    /// everywhere else. Pinned so the fallback cannot rot into a different
    /// answer by accident.
    #[tokio::test]
    async fn find_crash_without_termination_records_falls_back_to_deliveries() {
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

    /// The one case the rule cannot settle: a lone teardown `SIGKILL`. It is
    /// still reported as a crash because the OOM killer killing a user's
    /// process is real, but the note must not let a reader take it as proof.
    #[tokio::test]
    async fn find_crash_only_sigkill_still_detected_with_teardown_note() {
        let events = vec![
            trace_event(1, 100, 1, EventType::FunctionEntry, "main"),
            signal_termination_event(2, 200, 1, 9, "SIGKILL"),
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

    // --- decide_crash: the rule on its own, with no engine behind it ---

    /// The rule-3 ordering condition.
    ///
    /// `ptrace` reports `Exited` per thread group, so a multi-threaded session
    /// carries several `process_exit` markers. A worker leaving before the
    /// main thread then dies reads as "exit, then SIGSEGV", and calling that
    /// "handled the signal" turns a crash into a clean run.
    #[test]
    fn a_fatal_signal_after_the_exit_is_not_a_handled_signal() {
        // exit first (event_id 1), then the fatal signal (event_id 2), which a
        // log recorded before `terminated_tracee` existed cannot distinguish
        // from a handled delivery.
        let events: Vec<TraceEvent> = vec![
            process_exit_event(1, 100, 1, 0),
            signal_event(2, 200, 1, 11, "SIGSEGV"),
        ];
        let facts = facts_from(&events);
        match decide_crash(&facts, TRACER_TEARDOWN_SIGNAL, &FATAL_SIGNALS) {
            CrashVerdict::Crashed { signal, .. } => assert_eq!(signal, "SIGSEGV"),
            other => panic!("a fatal signal after the exit is still a death: {other:?}"),
        }
    }

    /// The same two events in the other order: that one really is a handled
    /// signal, and the test pair is what keeps the condition from becoming a
    /// blanket "an exit never proves survival".
    #[test]
    fn a_fatal_signal_before_the_exit_is_a_handled_signal() {
        let events: Vec<TraceEvent> = vec![
            signal_event(1, 100, 1, 11, "SIGSEGV"),
            process_exit_event(2, 200, 1, 0),
        ];
        let facts = facts_from(&events);
        match decide_crash(&facts, TRACER_TEARDOWN_SIGNAL, &FATAL_SIGNALS) {
            CrashVerdict::Survived { reason } => assert!(
                reason.contains("handled the signal"),
                "expected the handled reading, got: {reason}"
            ),
            other => panic!("signal then exit 0 is a handled signal: {other:?}"),
        }
    }

    /// Two fatal deaths: rule 1 takes the first one in log order, which is
    /// the one the user has to debug. Which one is "the" crash is only
    /// defined by the order, so the test states it rather than leaving it.
    #[test]
    fn the_first_of_two_fatal_deaths_is_the_crash_point() {
        let events: Vec<TraceEvent> = vec![
            signal_termination_event(1, 100, 1, 11, "SIGSEGV"),
            signal_termination_event(2, 200, 1, 6, "SIGABRT"),
        ];
        let facts = facts_from(&events);
        match decide_crash(&facts, TRACER_TEARDOWN_SIGNAL, &FATAL_SIGNALS) {
            CrashVerdict::Crashed { event, signal } => {
                assert_eq!(signal, "SIGSEGV");
                assert_eq!(event.event_id, 1);
            }
            other => panic!("a fatal death is a crash: {other:?}"),
        }
    }

    /// A fatal delivery that the teardown SIGKILL then ended.
    ///
    /// `decide_crash` reports this as a death by teardown rather than a
    /// crash, because the only thing that killed the process was chronos
    /// cleaning up. `session_explain` still answers `true` (a bool has
    /// nowhere to put the distinction) and this test pins that, so the loss
    /// of detail is declared rather than accidental.
    #[test]
    fn a_fatal_delivery_ended_by_teardown_is_a_death_not_a_crash() {
        let events: Vec<TraceEvent> = vec![
            signal_event(1, 100, 1, 11, "SIGSEGV"),
            signal_termination_event(2, 200, 1, 9, "SIGKILL"),
        ];
        let facts = facts_from(&events);
        match decide_crash(&facts, TRACER_TEARDOWN_SIGNAL, &FATAL_SIGNALS) {
            CrashVerdict::KilledByTeardown { signal, .. } => assert_eq!(signal, "SIGKILL"),
            other => panic!("only the teardown signal killed it: {other:?}"),
        }
    }

    /// A `process_exit` whose payload does not parse is not a termination
    /// record.
    ///
    /// Reading it as one would let a malformed marker stand as proof of a
    /// clean exit; refusing to read it falls through to the delivery rule,
    /// which fails toward reporting a crash on an unproven trace.
    #[test]
    fn a_malformed_exit_marker_is_not_a_termination_record() {
        let malformed = TraceEvent {
            event_id: 1,
            timestamp_ns: MonotonicNs::from(100),
            thread_id: 1,
            event_type: EventType::Custom,
            location: SourceLocation::default(),
            data: EventData::Custom {
                name: PROCESS_EXIT_MARKER.to_string(),
                data_json: "not json at all".to_string(),
            },
        };
        let events: Vec<TraceEvent> = vec![malformed];
        let facts = facts_from(&events);
        assert!(
            facts.exit.is_none(),
            "a marker that does not parse cannot prove a clean exit"
        );
        match decide_crash(&facts, TRACER_TEARDOWN_SIGNAL, &FATAL_SIGNALS) {
            CrashVerdict::Survived { .. } => {}
            other => panic!("no record at all is not a crash: {other:?}"),
        }
    }

    /// Build a session whose functions were called, the way the capture path
    /// builds one: through the events, with `cycles: None`.
    fn engine_with_function_events() -> QueryEngine {
        make_engine(vec![
            function_event(1, 100, 1, "hot_fn"),
            function_event(2, 200, 1, "hot_fn"),
            function_event(3, 300, 1, "cold_fn"),
        ])
    }

    /// The name goes in `location.function`, which is where the engine reads
    /// it from to build `top_functions` and the perf index. Putting it only in
    /// `EventData::Function::name` produced a session the engine could not see
    /// a single function in, which is a quieter version of the same mistake
    /// this cycle is about.
    fn function_event(id: u64, ts: u64, addr: u64, name: &str) -> TraceEvent {
        TraceEvent {
            event_id: id,
            timestamp_ns: MonotonicNs::from(ts),
            thread_id: 1,
            event_type: EventType::FunctionEntry,
            location: SourceLocation::new("test.rs", 10, name, addr),
            data: EventData::Function {
                name: name.to_string(),
                signature: None,
                symbol_id: None,
                invocation_id: None,
                parent_invocation_id: None,
            },
        }
    }

    /// The production case: a capture with no hardware counters.
    ///
    /// `hot_fn` was called twice and `cold_fn` once, so a call-count ranking
    /// separates them. Before this fix the ranking was flat at 0.0 for both,
    /// and `total_cycles` said `Some(0)` — a measurement of zero cycles for
    /// functions nobody ever watched run.
    #[tokio::test]
    async fn saliency_without_counters_ranks_by_calls_and_says_cycles_were_not_measured() {
        let mut engines: HashMap<String, QueryEngine> = HashMap::new();
        engines.insert("s".to_string(), engine_with_function_events());

        let out = DebugTraceSpecializedService::get_saliency_scores("s", 10, &Mutex::new(engines))
            .await
            .expect("saliency fallo");

        assert_eq!(
            out.scores.len(),
            2,
            "both functions should be listed: {out:?}"
        );

        let hot = out
            .scores
            .iter()
            .find(|s| s.function == "hot_fn")
            .expect("hot_fn missing");
        let cold = out
            .scores
            .iter()
            .find(|s| s.function == "cold_fn")
            .expect("cold_fn missing");

        assert!(
            hot.saliency_score > cold.saliency_score,
            "a call-count ranking has to separate twice-called from once-called: \
             hot={} cold={}",
            hot.saliency_score,
            cold.saliency_score
        );
        assert_eq!(hot.call_count, 2);
        assert!(
            out.scores.iter().all(|s| s.total_cycles.is_none()),
            "no cycles were measured, so none may be reported: {:?}",
            out.scores
        );
        assert!(
            out.scores.iter().all(|s| s.cycles.is_some()),
            "the disclosure field must be present exactly when counters are absent: {:?}",
            out.scores
        );

        let hint = out.hint.clone().unwrap_or_default();
        assert!(
            hint.contains("CALL COUNT") && hint.contains("No cycles were measured"),
            "the hint names the unit, because the unit is the claim: {hint}"
        );
        assert!(
            !hint.contains("dominated CPU time"),
            "the hint must not promise CPU time it does not have: {hint}"
        );
    }

    /// The case that is easy to break while fixing the one above: a session
    /// that *did* read counters still gets a cycle ranking.
    #[tokio::test]
    async fn saliency_with_counters_still_ranks_by_cycles() {
        use chronos_domain::{PerfCounters, PerformanceIndex};

        let mut perf = PerformanceIndex::new();
        perf.record_call(0x1000, Some("hot_fn".to_string()), Some(9000));
        perf.record_call(0x1000, Some("hot_fn".to_string()), Some(1000));
        perf.record_call(0x2000, Some("cold_fn".to_string()), Some(200));
        perf.set_counters(PerfCounters {
            cycles: Some(10200),
            instructions: Some(40000),
            cache_misses: None,
            cache_references: None,
        });

        let mut engines: HashMap<String, QueryEngine> = HashMap::new();
        engines.insert(
            "s".to_string(),
            engine_with_function_events().with_performance(perf),
        );

        let out = DebugTraceSpecializedService::get_saliency_scores("s", 10, &Mutex::new(engines))
            .await
            .expect("saliency fallo");

        let hot = out.scores.iter().find(|s| s.function == "hot_fn").unwrap();
        let cold = out.scores.iter().find(|s| s.function == "cold_fn").unwrap();
        assert_eq!(hot.total_cycles, Some(10000));
        assert_eq!(cold.total_cycles, Some(200));
        assert!(
            hot.saliency_score > cold.saliency_score,
            "a cycle ranking has to separate them too"
        );
        assert!(
            out.scores.iter().all(|s| s.cycles.is_none()),
            "the disclosure field is for the absence of counters only"
        );
        assert!(
            out.hint.unwrap_or_default().contains("dominated CPU time"),
            "with counters, the original hint is the correct one"
        );
    }

    /// `expand_hotspot` carried the same claim in a field with no disclosure
    /// at all: `Some(0)` for every function, every session.
    #[tokio::test]
    async fn hotspot_report_leaves_cycles_null_when_none_were_measured() {
        let mut engines: HashMap<String, QueryEngine> = HashMap::new();
        engines.insert("s".to_string(), engine_with_function_events());

        let out = DebugTraceSpecializedService::expand_hotspot("s", 10, &Mutex::new(engines))
            .await
            .expect("expand_hotspot fallo");

        assert!(!out.hotspot_functions.is_empty());
        for entry in &out.hotspot_functions {
            assert_eq!(
                entry.total_cycles, None,
                "cycles were never measured for {}",
                entry.function
            );
            assert_eq!(
                entry.avg_cycles_per_call, None,
                "an average over an unmeasured total is not zero, it is unknown: {}",
                entry.function
            );
        }
        let hint = out.hint.clone().unwrap_or_default();
        assert!(
            hint.contains("no hardware performance counters"),
            "the report says why the cycle columns are null: {hint}"
        );
    }

    #[test]
    fn decide_crash_an_empty_trace_is_not_a_crash() {
        let events: Vec<TraceEvent> = vec![];
        let facts = facts_from(&events);
        match decide_crash(&facts, TRACER_TEARDOWN_SIGNAL, &FATAL_SIGNALS) {
            CrashVerdict::Survived { reason } => {
                assert_eq!(reason, "No fatal signal found in the trace")
            }
            other => panic!("an empty trace is not a crash: {other:?}"),
        }
    }

    #[test]
    fn decide_crash_a_handled_fatal_signal_with_a_normal_exit_is_not_a_crash() {
        let events = vec![
            signal_event(1, 100, 1, 11, "SIGSEGV"),
            process_exit_event(2, 200, 1, 0),
        ];
        let facts = facts_from(&events);
        match decide_crash(&facts, TRACER_TEARDOWN_SIGNAL, &FATAL_SIGNALS) {
            CrashVerdict::Survived { reason } => {
                assert!(reason.contains("SIGSEGV"), "{reason}");
                assert!(reason.contains("handled"), "{reason}");
            }
            other => panic!("a survived signal is not a crash: {other:?}"),
        }
    }

    #[test]
    fn decide_crash_a_death_wins_even_with_a_normal_exit_recorded() {
        let events = vec![
            signal_termination_event(1, 100, 1, 11, "SIGSEGV"),
            process_exit_event(2, 200, 1, 0),
        ];
        let facts = facts_from(&events);
        match decide_crash(&facts, TRACER_TEARDOWN_SIGNAL, &FATAL_SIGNALS) {
            CrashVerdict::Crashed { signal, .. } => assert_eq!(signal, "SIGSEGV"),
            other => panic!("a death is a death: {other:?}"),
        }
    }

    #[test]
    fn decide_crash_a_non_teardown_death_wins_over_a_teardown_death() {
        let events = vec![
            signal_termination_event(1, 100, 1, 9, "SIGKILL"),
            signal_termination_event(2, 200, 1, 6, "SIGABRT"),
        ];
        let facts = facts_from(&events);
        match decide_crash(&facts, TRACER_TEARDOWN_SIGNAL, &FATAL_SIGNALS) {
            CrashVerdict::Crashed { signal, .. } => assert_eq!(signal, "SIGABRT"),
            other => panic!("the program's own death wins: {other:?}"),
        }
    }

    #[test]
    fn decide_crash_a_lone_teardown_death_is_hedged() {
        let events = vec![signal_termination_event(1, 100, 1, 9, "SIGKILL")];
        let facts = facts_from(&events);
        match decide_crash(&facts, TRACER_TEARDOWN_SIGNAL, &FATAL_SIGNALS) {
            CrashVerdict::KilledByTeardown { signal, .. } => assert_eq!(signal, "SIGKILL"),
            other => panic!("a teardown death is still a death: {other:?}"),
        }
    }

    #[test]
    fn decide_crash_without_a_termination_record_treats_a_delivery_as_a_crash() {
        // The fallback, pinned: no death and no exit, so the old rule runs and
        // a delivered fatal signal is reported as a crash again.
        let events = vec![signal_event(1, 100, 1, 11, "SIGSEGV")];
        let facts = facts_from(&events);
        match decide_crash(&facts, TRACER_TEARDOWN_SIGNAL, &FATAL_SIGNALS) {
            CrashVerdict::Crashed { signal, .. } => assert_eq!(signal, "SIGSEGV"),
            other => panic!("the fallback must keep the old answer: {other:?}"),
        }
    }

    #[test]
    fn decide_crash_a_normal_exit_overrides_a_delivered_teardown_kill() {
        // The teardown kill is a delivery here, not a death, so a normal exit
        // already settles it: the program finished before anything killed it.
        let events = vec![
            signal_event(1, 100, 1, 9, "SIGKILL"),
            process_exit_event(2, 200, 1, 0),
        ];
        let facts = facts_from(&events);
        match decide_crash(&facts, TRACER_TEARDOWN_SIGNAL, &FATAL_SIGNALS) {
            CrashVerdict::Survived { .. } => {}
            other => panic!("a normal exit beats an undelivered-death claim: {other:?}"),
        }
    }

    // --- parse_exit_code ---

    #[test]
    fn parse_exit_code_reads_the_code() {
        assert_eq!(parse_exit_code(r#"{"exit_code": 0}"#), Some(0));
        assert_eq!(parse_exit_code(r#"{"exit_code": -6}"#), Some(-6));
    }

    #[test]
    fn parse_exit_code_rejects_anything_it_cannot_read() {
        // A marker that does not parse must not become proof of a clean exit.
        assert_eq!(parse_exit_code("not json"), None);
        assert_eq!(parse_exit_code(r#"{"other": 1}"#), None);
        assert_eq!(parse_exit_code(r#"{"exit_code": "zero"}"#), None);
        assert_eq!(parse_exit_code(""), None);
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
