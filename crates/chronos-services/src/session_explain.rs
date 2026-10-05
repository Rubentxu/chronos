//! M7-03 — `session_explain` v2 dispatcher (net-new).
//!
//! Produces a four-tier structured bundle for a single session:
//! - `facts`    — direct observations (total_events, distinct types/
//!   functions, duration, thread_count, crash_detected,
//!   signal_delivered).
//! - `derived`  — projections computed from facts (hotspots +
//!   call_graph_summary). `regressions_vs_none` is `None` in m7-03
//!   because regression detection requires a baseline (that flow
//!   lives in `session_compare{kind=regression}`).
//! - `inferred` — best-effort characterisations: CrashDetected,
//!   IoHeavy (>50% IO syscalls), CpuBound (one function >70% of
//!   the trace's function calls), SingleThreaded (thread_count==1),
//!   or Unknown.
//! - `hypothesis` — typed `HypothesisTestPlan` the agent can execute
//!   via the m6-04 `hypothesis_test` tool. `session_explain` does
//!   NOT execute the plan; it only plans.
//!
//! See `docs/milestones/m7-03-session-compare-explain-merge.md` for
//! the full spec + algorithm details.

use std::collections::HashSet;
use std::sync::Arc;

use crate::debug_trace_specialized::{
    decide_crash, facts_from, CrashVerdict, TerminationFacts, FATAL_SIGNALS as CRASH_SIGNALS,
    TRACER_TEARDOWN_SIGNAL,
};
use crate::error::ServiceError;
use crate::output::{
    CallGraphSummary, DerivedBundle, FactsBundle, FunctionHotspot, HypothesisBundle,
    HypothesisTestKind, HypothesisTestPlan, InferredBundle, InferredTag, SessionExplainInput,
    SessionExplainKind, SessionExplainOutput, SessionExplainProvenance,
};
use chronos_domain::ports::session_reader::{SessionReader, SessionReaderError};
use chronos_query::QueryEngine;

/// Borrowed handle to the live `SessionReader` port (REC-C3.5-B.2).
///
/// The adapter behind the port is owned by `chronos-mcp::Server`; the
/// service holds an `Arc<dyn SessionReader>` for the duration of the
/// call. No locking past the await point.
pub struct SessionExplainContext {
    pub reader: Arc<dyn SessionReader>,
}

/// Threshold for the IoHeavy inference (share of events that are
/// IO-syscall events). 0.50 = 50%.
const IO_HEAVY_THRESHOLD: f64 = 0.50;

/// Threshold for the CpuBound inference (one function's share of the
/// trace's function calls). 0.70 = 70%.
///
/// The denominator is `ExecutionSummary::total_function_calls`, the
/// whole-trace call census -- NOT the sum over the engine's truncated
/// `top_functions` list, which is a subtotal of the 20 hottest functions.
const CPU_BOUND_THRESHOLD: f64 = 0.70;

/// Names of syscalls considered "IO" for the IoHeavy inference.
const IO_SYSCALLS: &[&str] = &[
    "read", "write", "recv", "send", "recvfrom", "sendto", "accept", "accept4", "connect", "close",
];

// `CRASH_SIGNALS` is `debug_trace_specialized::FATAL_SIGNALS`, aliased at the
// import above rather than repeated: two copies of "what counts as fatal" is
// how two callers end up disagreeing about the same trace.

/// Stateless holder for the v2 `session_explain` dispatcher.
pub struct ChronosSessionExplainService;

impl ChronosSessionExplainService {
    /// v2 `session_explain` dispatcher entrypoint.
    ///
    /// Routes by `kind`:
    /// - `Facts`      → reads session + builds FactsBundle
    /// - `Derived`    → reads session + builds DerivedBundle (hotspots + call graph summary)
    /// - `Inferred`   → reads session + applies heuristic tags
    /// - `Hypothesis` → returns a typed plan the agent can execute
    pub fn explain(
        ctx: &SessionExplainContext,
        input: SessionExplainInput,
    ) -> Result<SessionExplainOutput, ServiceError> {
        let (meta, events) = load_session(ctx, &input.session_id)?;
        let engine = QueryEngine::new(events.clone());
        let summary = engine.execution_summary(&input.session_id);
        let provenance = SessionExplainProvenance {
            engine_version: "chronos-0.1.0".to_string(),
            source: format!("session_explain:{}", kind_source(&input.kind)),
        };

        match input.kind {
            SessionExplainKind::Facts => Ok(SessionExplainOutput::Facts {
                bundle: build_facts(&meta, &summary, &events),
                provenance,
            }),
            SessionExplainKind::Derived => Ok(SessionExplainOutput::Derived {
                bundle: build_derived(&input.session_id, &summary),
                provenance,
            }),
            SessionExplainKind::Inferred => Ok(SessionExplainOutput::Inferred {
                bundle: build_inferred(&input.session_id, &summary, &events),
                provenance,
            }),
            SessionExplainKind::Hypothesis => {
                let hypothesis_kind = input.hypothesis_kind.ok_or_else(|| {
                    ServiceError::InvalidInput(
                        "session_explain{kind=hypothesis} requires `hypothesis_kind`".to_string(),
                    )
                })?;
                Ok(SessionExplainOutput::Hypothesis {
                    bundle: build_hypothesis(&input.session_id, &summary, hypothesis_kind),
                    provenance,
                })
            }
        }
    }
}

fn kind_source(kind: &SessionExplainKind) -> &'static str {
    match kind {
        SessionExplainKind::Facts => "facts",
        SessionExplainKind::Derived => "derived",
        SessionExplainKind::Inferred => "inferred",
        SessionExplainKind::Hypothesis => "hypothesis",
    }
}

fn load_session(
    ctx: &SessionExplainContext,
    session_id: &str,
) -> Result<
    (
        chronos_domain::SessionMetadata,
        Vec<chronos_domain::TraceEvent>,
    ),
    ServiceError,
> {
    ctx.reader.load_session(session_id).map_err(map_load_error)
}

fn map_load_error(e: SessionReaderError) -> ServiceError {
    match e {
        SessionReaderError::NotFound(s) => ServiceError::SessionNotFound(s),
        SessionReaderError::InvalidId(s) => {
            ServiceError::InvalidInput(format!("invalid session id: '{}'", s))
        }
        other => ServiceError::LoadFailed(format!("session_explain: store error: {}", other)),
    }
}

/// Did this session end in a crash?
///
/// Decided from the tracee's TERMINATION, not from the signals it received.
/// This used to be true for any `SignalDelivered` whose name was in
/// `CRASH_SIGNALS`, which is a statement about delivery: a tracee that
/// installs a `sigaction` handler and raises SIGSEGV receives SIGSEGV and
/// exits 0, and this reported a crash for it.
///
/// The verdict is `decide_crash`'s, not a second opinion written here.
/// `find_crash` already answers "did this trace end in a crash" from the same
/// facts, and two answers to one question is how one tool tells a user the
/// session crashed while the other says it did not.
///
/// `summary` contributes one channel the events cannot: a producer that
/// reports a crash directly, with `issue_type == "crash"` and no signal
/// behind it. What is deliberately NOT counted is `issue_type == "signal"` —
/// the engine raises it for every signal it sees, handled or not, so counting
/// it reinstates the very claim this replaces. The engine's own "Signal
/// killed the tracee: X" description does count, because that string only
/// exists for a death.
fn crash_detected_for(
    facts: &TerminationFacts<'_>,
    summary: &chronos_domain::query::ExecutionSummary,
) -> bool {
    let from_trace = match decide_crash(facts, TRACER_TEARDOWN_SIGNAL, &CRASH_SIGNALS) {
        CrashVerdict::Crashed { .. } | CrashVerdict::KilledByTeardown { .. } => true,
        CrashVerdict::Survived { .. } => false,
    };
    let from_engine = summary
        .potential_issues
        .iter()
        .any(|issue| issue.issue_type == "crash" || issue.description.starts_with("Signal killed"));

    from_trace || from_engine
}

fn build_facts(
    meta: &chronos_domain::SessionMetadata,
    summary: &chronos_domain::query::ExecutionSummary,
    events: &[chronos_domain::TraceEvent],
) -> FactsBundle {
    let mut distinct_event_types: HashSet<String> = HashSet::new();
    let mut distinct_functions: HashSet<String> = HashSet::new();
    for ev in events {
        distinct_event_types.insert(format!("{:?}", ev.event_type));
        if let chronos_domain::EventData::Function { name, .. } = &ev.data {
            distinct_functions.insert(name.clone());
        }
    }
    // `crash_detected` and `CrashDetected` are one question asked twice, by two
    // output kinds. They must not be able to answer differently, so both go
    // through `crash_detected_for`.
    let facts = facts_from(events);
    let crash_detected = crash_detected_for(&facts, summary);

    // The last fatal signal the tracee received and lived through. Reported
    // as a fact about delivery, separate from the verdict above: a tracee can
    // handle SIGSEGV and still be worth telling the user it saw one.
    let signal_delivered = facts
        .first_survived_fatal(&CRASH_SIGNALS)
        .map(|name| name.to_string());

    let _ = meta; // metadata fields could be threaded in m7+
    FactsBundle {
        session_id: summary.session_id.clone(),
        total_events: summary.total_events as usize,
        distinct_event_types: {
            let mut v: Vec<String> = distinct_event_types.into_iter().collect();
            v.sort();
            v
        },
        distinct_functions: {
            let mut v: Vec<String> = distinct_functions.into_iter().collect();
            v.sort();
            v
        },
        duration_ms: Some(summary.duration_ns / 1_000_000),
        thread_count: summary.thread_count as usize,
        crash_detected,
        signal_delivered,
    }
}

fn build_derived(
    session_id: &str,
    summary: &chronos_domain::query::ExecutionSummary,
) -> DerivedBundle {
    // The denominator is the call census of the WHOLE trace, not the sum over
    // `top_functions`: that list is truncated to its 20 hottest entries, so
    // summing it understates the trace and inflates every listed share. The
    // census is exactly what `ExecutionSummary::total_function_calls` holds.
    let total_calls: u64 = summary.total_function_calls;
    let mut hotspots: Vec<FunctionHotspot> = summary
        .top_functions
        .iter()
        .map(|f| FunctionHotspot {
            function: f.name.clone(),
            call_count: f.call_count,
            share_pct: if total_calls > 0 {
                (f.call_count as f64 / total_calls as f64) * 100.0
            } else {
                0.0
            },
        })
        .collect();
    // Sort by call count descending.
    hotspots.sort_by_key(|h| std::cmp::Reverse(h.call_count));
    // Cardinality of the ANALYSED hotspot set, not a callee census: it
    // counts callers as well, and it saturates at the engine's top-20
    // cutoff. See the `CallGraphSummary::distinct_callees` doc for why the
    // name is kept; the honest graph cardinality is `execution_query`
    // `kind = "call_graph"`.
    let distinct_callees = hotspots.len();
    DerivedBundle {
        session_id: session_id.to_string(),
        hotspots,
        call_graph_summary: CallGraphSummary {
            total_calls,
            distinct_callees,
            max_depth: 0, // depth isn't exposed by ExecutionSummary; m7+ can derive
        },
        regressions_vs_none: None,
    }
}

fn build_inferred(
    session_id: &str,
    summary: &chronos_domain::query::ExecutionSummary,
    events: &[chronos_domain::TraceEvent],
) -> InferredBundle {
    let mut inferences: Vec<InferredTag> = Vec::new();

    // 1. CrashDetected: the same question `build_facts` answers in
    //    `FactsBundle::crash_detected`, asked for the `inferred` output kind.
    //    It used to be its own rule — "any potential_issue of type crash or
    //    signal" — which read every signal as a crash and could contradict
    //    the `facts` bundle about the same session. One function, one answer.
    if crash_detected_for(&facts_from(events), summary) {
        inferences.push(InferredTag::CrashDetected);
    }

    // 2. IoHeavy: >50% of events are syscall_enter/exit for IO syscalls.
    let total_events: u64 = summary.event_counts_by_type.iter().map(|(_, c)| *c).sum();
    let syscall_total: u64 = summary
        .event_counts_by_type
        .iter()
        .filter(|(name, _)| name == "syscall_enter" || name == "syscall_exit")
        .map(|(_, c)| *c)
        .sum();
    // Heuristic: if the syscall share > threshold and we can't filter by
    // syscall number (we only have counts here), assume IO heavy.
    if total_events > 0 {
        let syscall_share = syscall_total as f64 / total_events as f64;
        if syscall_share > IO_HEAVY_THRESHOLD {
            inferences.push(InferredTag::IoHeavy);
        }
    }

    // 3. CpuBound: one function dominates >70% of the trace's function calls.
    //    The denominator is the whole-trace call census
    //    (`total_function_calls`), NOT the sum over `top_functions`: that list
    //    is truncated to the 20 hottest functions, so a top-20 denominator
    //    inflates the leader's share for exactly the traces with the most
    //    distinct functions, and fires this tag on sessions where no
    //    function holds a majority. A false CpuBound is not cosmetic -- it
    //    also suppresses the typed `Unknown` fallback below, so the agent
    //    loses the "nothing characterises this session" signal.
    let total_calls: u64 = summary.total_function_calls;
    if total_calls > 0 {
        let max_share = summary
            .top_functions
            .iter()
            .map(|f| f.call_count as f64 / total_calls as f64)
            .fold(0.0_f64, f64::max);
        if max_share > CPU_BOUND_THRESHOLD {
            inferences.push(InferredTag::CpuBound);
        }
    }

    // 4. SingleThreaded: thread_count == 1.
    if summary.thread_count == 1 {
        inferences.push(InferredTag::SingleThreaded);
    }

    // Typed fallback: if no inferences apply, push Unknown.
    if inferences.is_empty() {
        inferences.push(InferredTag::Unknown);
    }

    InferredBundle {
        session_id: session_id.to_string(),
        inferences,
    }
}

fn build_hypothesis(
    session_id: &str,
    summary: &chronos_domain::query::ExecutionSummary,
    kind: HypothesisTestKind,
) -> HypothesisBundle {
    match kind {
        HypothesisTestKind::CrashInvariant => HypothesisBundle {
            session_id: session_id.to_string(),
            plan: HypothesisTestPlan::CrashInvariant {
                session_id: session_id.to_string(),
                comparison: chronos_domain::property::ComparisonOp::Eq,
            },
            hint: format!(
                "Plan: check whether this session's exit_status violates Equal(0). \
                 Session summary flagged {} potential issues.",
                summary.potential_issues.len()
            ),
        },
        HypothesisTestKind::DominantFunctionCallPath => {
            let dominant = summary.top_functions.first().cloned();
            let (function, at_least_n) = dominant
                .map(|f| (f.name.clone(), f.call_count))
                .unwrap_or_else(|| ("unknown".to_string(), 0));
            HypothesisBundle {
                session_id: session_id.to_string(),
                plan: HypothesisTestPlan::DominantFunctionCallPath {
                    session_id: session_id.to_string(),
                    function,
                    at_least_n,
                },
                hint: "Plan: check whether the dominant function was called at least N times. \
                     Use hypothesis_test{kind=call_path} to verify. \
                     Dominant function from top_functions[0]."
                    .to_string(),
            }
        }
    }
}

// Mark IO_SYSCALLS as intentionally available for m7+ use; the
// constant is referenced here so the import doesn't go stale.
#[allow(dead_code)]
const _IO_SYSCALLS_REF: &[&str] = IO_SYSCALLS;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::MonotonicNs;
    use chronos_domain::{EventData, EventType, SourceLocation, TraceEvent};
    use chronos_store::{SessionMetadata, SessionStore};

    fn make_event(id: u64, func: &str) -> TraceEvent {
        let loc = SourceLocation::new("test.rs", 1, func.to_string(), 0x1000 + id);
        TraceEvent::new(
            id,
            MonotonicNs::from(id * 100),
            1,
            EventType::FunctionEntry,
            loc,
            EventData::Function {
                name: func.to_string(),
                signature: None,
                symbol_id: None,
                invocation_id: None,
                parent_invocation_id: None,
            },
        )
    }

    fn save_session(store: &SessionStore, id: &str, funcs: &[&str]) {
        let events: Vec<TraceEvent> = funcs
            .iter()
            .enumerate()
            .map(|(i, f)| make_event(i as u64, f))
            .collect();
        let meta = SessionMetadata {
            session_id: id.to_string(),
            created_at: 0,
            language: "native".to_string(),
            target: "/bin/test".to_string(),
            event_count: events.len(),
            duration_ms: 100,
            tail_sealed: false,
            sealed_at: None,
        };
        store.save_session(meta, &events).unwrap();
    }

    fn empty_arc_store() -> std::sync::Arc<SessionStore> {
        std::sync::Arc::new(SessionStore::in_memory().unwrap())
    }

    fn make_ctx(store: std::sync::Arc<SessionStore>) -> SessionExplainContext {
        use chronos_store::session_reader_adapter::SessionStoreBackedSessionReader;
        let adapter = std::sync::Arc::new(SessionStoreBackedSessionReader::new(store))
            as std::sync::Arc<dyn SessionReader>;
        SessionExplainContext { reader: adapter }
    }

    /// A `SignalDelivered` event, the way the native tracer writes one.
    ///
    /// `terminated` is the R6.10 field: false is a signal the tracee received
    /// and lived through, true is the signal that ended it.
    fn signal_event(sig: &str, terminated: bool) -> TraceEvent {
        TraceEvent::new(
            1,
            MonotonicNs::from(100),
            1,
            EventType::SignalDelivered,
            SourceLocation::new("test.rs", 1, "main", 0x2000),
            EventData::Signal {
                signal_number: 11,
                signal_name: sig.to_string(),
                terminated_tracee: terminated,
            },
        )
    }

    /// The `process_exit` marker `native_adapter` writes when the tracee
    /// returns from `main`.
    fn process_exit_event(code: i32) -> TraceEvent {
        TraceEvent::new(
            2,
            MonotonicNs::from(200),
            1,
            EventType::Custom,
            SourceLocation::from_address(0),
            EventData::Custom {
                name: "process_exit".to_string(),
                data_json: format!("{{\"exit_code\": {}}}", code),
            },
        )
    }

    /// A session whose ending is spelled out by `ending`.
    ///
    /// The three shapes are the three states a crash verdict can be in: the
    /// tracee died, the tracee exited on its own terms, or the log never says.
    fn session_ending(ending: &[TraceEvent]) -> (std::sync::Arc<SessionStore>, String) {
        let store = empty_arc_store();
        let mut events = vec![make_event(0, "main")];
        events.extend_from_slice(ending);
        let meta = SessionMetadata {
            session_id: "sess".to_string(),
            created_at: 0,
            language: "native".to_string(),
            target: "/bin/test".to_string(),
            event_count: events.len(),
            duration_ms: 100,
            tail_sealed: false,
            sealed_at: None,
        };
        store.save_session(meta, &events).unwrap();
        (store, "sess".to_string())
    }

    /// Run one `session_explain` kind against a store, panicking on the wrong
    /// output variant.
    fn explain_one(
        store: &std::sync::Arc<SessionStore>,
        kind: SessionExplainKind,
    ) -> SessionExplainOutput {
        let ctx = make_ctx(std::sync::Arc::clone(store));
        ChronosSessionExplainService::explain(
            &ctx,
            SessionExplainInput {
                kind,
                session_id: "sess".to_string(),
                hypothesis_kind: None,
            },
        )
        .unwrap()
    }

    #[test]
    fn explain_facts_returns_facts_bundle() {
        let store = empty_arc_store();
        save_session(&store, "s1", &["main", "helper", "main"]);
        let ctx = make_ctx(std::sync::Arc::clone(&store));
        let input = SessionExplainInput {
            kind: SessionExplainKind::Facts,
            session_id: "s1".to_string(),
            hypothesis_kind: None,
        };
        let out = ChronosSessionExplainService::explain(&ctx, input).unwrap();
        match out {
            SessionExplainOutput::Facts { bundle, provenance } => {
                assert_eq!(bundle.session_id, "s1");
                assert_eq!(bundle.total_events, 3);
                assert_eq!(provenance.source, "session_explain:facts");
                assert!(!provenance.engine_version.is_empty());
            }
            other => panic!("expected Facts, got {:?}", other),
        }
    }

    #[test]
    fn explain_derived_returns_derived_bundle() {
        let store = empty_arc_store();
        save_session(&store, "s1", &["main", "helper", "main"]);
        let ctx = make_ctx(std::sync::Arc::clone(&store));
        let input = SessionExplainInput {
            kind: SessionExplainKind::Derived,
            session_id: "s1".to_string(),
            hypothesis_kind: None,
        };
        let out = ChronosSessionExplainService::explain(&ctx, input).unwrap();
        match out {
            SessionExplainOutput::Derived { bundle, provenance } => {
                assert_eq!(bundle.session_id, "s1");
                assert_eq!(bundle.hotspots.len(), 2);
                assert_eq!(bundle.regressions_vs_none, None);
                assert_eq!(provenance.source, "session_explain:derived");
            }
            other => panic!("expected Derived, got {:?}", other),
        }
    }

    /// The tracee died on SIGSEGV: a crash, on both output kinds.
    #[test]
    fn crash_detected_when_the_tracee_died_from_a_fatal_signal() {
        let (store, _) = session_ending(&[signal_event("SIGSEGV", true)]);

        match explain_one(&store, SessionExplainKind::Facts) {
            SessionExplainOutput::Facts { bundle, .. } => {
                assert!(bundle.crash_detected, "a death is a crash")
            }
            other => panic!("expected Facts, got {:?}", other),
        }
        match explain_one(&store, SessionExplainKind::Inferred) {
            SessionExplainOutput::Inferred { bundle, .. } => assert!(
                bundle.inferences.contains(&InferredTag::CrashDetected),
                "expected CrashDetected in {:?}",
                bundle.inferences
            ),
            other => panic!("expected Inferred, got {:?}", other),
        }
    }

    /// The tracee handled SIGSEGV and returned 0: NOT a crash.
    ///
    /// This is the case the delivery-based rule could not express. Both
    /// events are present and both are real — a SIGSEGV was delivered, and
    /// the process exited normally — so the only thing separating them from a
    /// crash is which of the two the tracee survived.
    #[test]
    fn no_crash_when_the_tracee_handled_a_fatal_signal_and_exited_zero() {
        let (store, _) = session_ending(&[signal_event("SIGSEGV", false), process_exit_event(0)]);

        match explain_one(&store, SessionExplainKind::Facts) {
            SessionExplainOutput::Facts { bundle, .. } => {
                assert!(
                    !bundle.crash_detected,
                    "a tracee that handled SIGSEGV and exited 0 did not crash"
                );
                // The delivery is still reported: it is a fact about the
                // session, kept apart from the verdict about its ending.
                assert_eq!(
                    bundle.signal_delivered.as_deref(),
                    Some("SIGSEGV"),
                    "the handled signal is still reported as delivered"
                );
            }
            other => panic!("expected Facts, got {:?}", other),
        }
        match explain_one(&store, SessionExplainKind::Inferred) {
            SessionExplainOutput::Inferred { bundle, .. } => assert!(
                !bundle.inferences.contains(&InferredTag::CrashDetected),
                "a handled SIGSEGV must not infer a crash, got {:?}",
                bundle.inferences
            ),
            other => panic!("expected Inferred, got {:?}", other),
        }
    }

    /// `facts` and `inferred` are one question; they must not answer
    /// differently about the same session.
    #[test]
    fn facts_and_inferred_agree_on_every_way_a_session_can_end() {
        for (label, ending, expect_crash) in [
            ("died on SIGSEGV", vec![signal_event("SIGSEGV", true)], true),
            (
                "handled SIGSEGV, exited 0",
                vec![signal_event("SIGSEGV", false), process_exit_event(0)],
                false,
            ),
            (
                "handled SIGSEGV, exited 3",
                vec![signal_event("SIGABRT", false), process_exit_event(3)],
                false,
            ),
            ("no signals at all", vec![], false),
        ] {
            let (store, _) = session_ending(&ending);

            let facts = match explain_one(&store, SessionExplainKind::Facts) {
                SessionExplainOutput::Facts { bundle, .. } => bundle.crash_detected,
                other => panic!("expected Facts for {}, got {:?}", label, other),
            };
            let inferred = match explain_one(&store, SessionExplainKind::Inferred) {
                SessionExplainOutput::Inferred { bundle, .. } => {
                    bundle.inferences.contains(&InferredTag::CrashDetected)
                }
                other => panic!("expected Inferred for {}, got {:?}", label, other),
            };

            assert_eq!(facts, expect_crash, "facts verdict wrong for {label}");
            assert_eq!(inferred, expect_crash, "inferred verdict wrong for {label}");
            assert_eq!(facts, inferred, "the two kinds disagree for {label}");
        }
    }

    /// A log that never says how the tracee ended falls back to the old rule.
    ///
    /// Declared, not endorsed: the fallback is what keeps a session truncated
    /// mid-trace from resolving to "no crash" by default, and the tracee in
    /// that state cannot be shown to have survived anything. The three tests
    /// above are what make the fallback narrow — it only runs when the log
    /// offers no termination record at all.
    #[test]
    fn a_log_without_a_termination_record_falls_back_to_delivery() {
        let (store, _) = session_ending(&[signal_event("SIGSEGV", false)]);
        match explain_one(&store, SessionExplainKind::Inferred) {
            SessionExplainOutput::Inferred { bundle, .. } => assert!(
                bundle.inferences.contains(&InferredTag::CrashDetected),
                "the fallback treats a lone fatal delivery as a crash, got {:?}",
                bundle.inferences
            ),
            other => panic!("expected Inferred, got {:?}", other),
        }
    }

    #[test]
    fn explain_inferred_single_threaded_when_thread_count_one() {
        let store = empty_arc_store();
        save_session(&store, "s1", &["main"]);
        let ctx = make_ctx(std::sync::Arc::clone(&store));
        let input = SessionExplainInput {
            kind: SessionExplainKind::Inferred,
            session_id: "s1".to_string(),
            hypothesis_kind: None,
        };
        let out = ChronosSessionExplainService::explain(&ctx, input).unwrap();
        match out {
            SessionExplainOutput::Inferred { bundle, .. } => {
                // Default thread_count for a single-thread session is 1,
                // so SingleThreaded should be inferred.
                assert!(
                    bundle.inferences.contains(&InferredTag::SingleThreaded),
                    "expected SingleThreaded in {:?}",
                    bundle.inferences
                );
            }
            other => panic!("expected Inferred, got {:?}", other),
        }
    }

    #[test]
    fn explain_inferred_unknown_when_no_characterisation_applies() {
        // A session with multiple threads, no signals, no IO syscalls,
        // and a perfectly even function distribution so CpuBound does
        // not fire. With thread_count > 1 and no dominant function, only
        // the typed fallback Unknown should remain.
        let store = empty_arc_store();
        // 12 events across 4 functions (3 each), 4 distinct threads.
        let events: Vec<TraceEvent> = (0..12)
            .map(|i| {
                let func_name = format!("func{}", i % 4);
                let loc = SourceLocation::new("test.rs", 1, func_name.clone(), 0x1000 + i);
                TraceEvent::new(
                    i,
                    MonotonicNs::from(i * 100),
                    i % 4, // threads 0..3
                    EventType::FunctionEntry,
                    loc,
                    EventData::Function {
                        name: func_name,
                        signature: None,
                        symbol_id: None,
                        invocation_id: None,
                        parent_invocation_id: None,
                    },
                )
            })
            .collect();
        let meta = SessionMetadata {
            session_id: "s1".to_string(),
            created_at: 0,
            language: "native".to_string(),
            target: "/bin/test".to_string(),
            event_count: events.len(),
            duration_ms: 100,
            tail_sealed: false,
            sealed_at: None,
        };
        store.save_session(meta, &events).unwrap();

        let ctx = make_ctx(std::sync::Arc::clone(&store));
        let input = SessionExplainInput {
            kind: SessionExplainKind::Inferred,
            session_id: "s1".to_string(),
            hypothesis_kind: None,
        };
        let out = ChronosSessionExplainService::explain(&ctx, input).unwrap();
        match out {
            SessionExplainOutput::Inferred { bundle, .. } => {
                // 4 distinct threads => NOT SingleThreaded.
                assert!(
                    !bundle.inferences.contains(&InferredTag::SingleThreaded),
                    "expected NOT SingleThreaded in {:?}",
                    bundle.inferences
                );
                // Perfectly even distribution => NOT CpuBound (max share = 25%).
                assert!(
                    !bundle.inferences.contains(&InferredTag::CpuBound),
                    "expected NOT CpuBound in {:?}",
                    bundle.inferences
                );
                // No crashes, no syscall events => NOT CrashDetected, NOT IoHeavy.
                assert!(
                    !bundle.inferences.contains(&InferredTag::CrashDetected),
                    "expected NOT CrashDetected in {:?}",
                    bundle.inferences
                );
                assert!(
                    !bundle.inferences.contains(&InferredTag::IoHeavy),
                    "expected NOT IoHeavy in {:?}",
                    bundle.inferences
                );
                // Typed fallback should be present.
                assert!(
                    bundle.inferences.contains(&InferredTag::Unknown),
                    "expected Unknown fallback in {:?}",
                    bundle.inferences
                );
            }
            other => panic!("expected Inferred, got {:?}", other),
        }
    }

    /// Session shape that makes the engine's top-20 subtotal and the
    /// whole-trace call census differ: 100 distinct functions, 199 calls.
    ///
    /// `hot` is called 100 times, each `cold_NN` once. The engine ranks 20
    /// entries, so `top_functions` is `hot` plus 19 cold ones: a subtotal of
    /// 119 against a real total of 199. Two threads are used so that
    /// `SingleThreaded` cannot fire and the `Unknown` fallback stays
    /// reachable -- the whole point of the `CpuBound` regression is that a
    /// false tag must not eat that signal.
    const LOP_HOT_CALLS: u64 = 100;
    const LOP_TAIL_FUNCTIONS: u64 = 99;
    const LOP_TRACE_TOTAL: u64 = 199;
    const LOP_ENGINE_TOP_N: usize = 20;
    const LOP_SUBTOTAL: u64 = 119;

    fn lopsided_session() -> (std::sync::Arc<SessionStore>, String) {
        let store = empty_arc_store();
        let mut events: Vec<TraceEvent> = Vec::new();
        let mut id = 0u64;
        for _ in 0..LOP_HOT_CALLS {
            events.push(make_event(id, "hot"));
            id += 1;
        }
        for tail in 0..LOP_TAIL_FUNCTIONS {
            events.push(make_event(id, &format!("cold_{tail:02}")));
            id += 1;
        }
        for (i, ev) in events.iter_mut().enumerate() {
            ev.thread_id = (i % 2) as u64;
        }
        let meta = SessionMetadata {
            session_id: "lopsided".to_string(),
            created_at: 0,
            language: "native".to_string(),
            target: "/bin/test".to_string(),
            event_count: events.len(),
            duration_ms: 100,
            tail_sealed: false,
            sealed_at: None,
        };
        store.save_session(meta, &events).unwrap();
        (store, "lopsided".to_string())
    }

    fn explain_lopsided(kind: SessionExplainKind) -> SessionExplainOutput {
        let (store, sid) = lopsided_session();
        let ctx = make_ctx(store);
        let input = SessionExplainInput {
            kind,
            session_id: sid,
            hypothesis_kind: None,
        };
        ChronosSessionExplainService::explain(&ctx, input).unwrap()
    }

    /// Regression: a session with more distinct functions than the engine's
    /// top-20 cutoff must NOT be characterised as `CpuBound` merely because
    /// the cutoff removed the cold tail from the denominator.
    ///
    /// The leader holds 100 of 199 calls = 50.3% of the trace, which is below
    /// the 70% threshold. Measured over the top-20 subtotal of 119 the very
    /// same leader reads 84.0% and fires the tag. That is a wrong verdict, not
    /// a mislabelled field, and it used to travel with the loss of the
    /// `Unknown` fallback -- telling the agent "this is CPU bound" about a
    /// session where no function even holds a majority.
    #[test]
    fn explain_inferred_does_not_fire_cpu_bound_over_a_truncated_denominator() {
        // Fixture sanity, computed here so the test's own premise is checked
        // independently of the production arithmetic.
        let true_share = LOP_HOT_CALLS as f64 / LOP_TRACE_TOTAL as f64; // 0.5025
        let subtotal_share = LOP_HOT_CALLS as f64 / LOP_SUBTOTAL as f64; // 0.8403
        assert!(
            true_share < CPU_BOUND_THRESHOLD,
            "fixture is broken: the true share {} already exceeds the threshold",
            true_share
        );
        assert!(
            subtotal_share > CPU_BOUND_THRESHOLD,
            "fixture no longer discriminates: the subtotal share {} no longer \
             crosses the threshold, so the test would pass with or without the fix",
            subtotal_share
        );

        let out = explain_lopsided(SessionExplainKind::Inferred);
        match out {
            SessionExplainOutput::Inferred { bundle, .. } => {
                assert!(
                    !bundle.inferences.contains(&InferredTag::CpuBound),
                    "CpuBound must not fire: the hottest function holds {}/{} = \
                     {:.2}% of the trace's function calls, under the {:.0}% threshold. \
                     Inferences: {:?}",
                    LOP_HOT_CALLS,
                    LOP_TRACE_TOTAL,
                    true_share * 100.0,
                    CPU_BOUND_THRESHOLD * 100.0,
                    bundle.inferences
                );
                // The honest "nothing characterises this session" signal must
                // survive: a false CpuBound used to suppress it by leaving
                // `inferences` non-empty.
                assert_eq!(
                    bundle.inferences,
                    vec![InferredTag::Unknown],
                    "with no crash, no IO, 2 threads and no dominant function, \
                     Unknown is the only correct inference"
                );
            }
            other => panic!("expected Inferred, got {:?}", other),
        }
    }

    /// `CallGraphSummary::total_calls` and `FunctionHotspot.share_pct` are
    /// denominated over the whole trace (199 calls), not over the engine's
    /// top-20 subtotal (119). Exact values, no tolerance.
    #[test]
    fn explain_derived_total_calls_and_share_use_the_whole_trace_census() {
        let out = explain_lopsided(SessionExplainKind::Derived);
        match out {
            SessionExplainOutput::Derived { bundle, .. } => {
                assert_eq!(
                    bundle.call_graph_summary.total_calls,
                    LOP_TRACE_TOTAL,
                    "total_calls must be the census of the whole trace, not the \
                     subtotal over the 20 hottest functions ({}); the gap is the \
                     {} calls of the truncated tail",
                    LOP_SUBTOTAL,
                    LOP_TRACE_TOTAL - LOP_SUBTOTAL
                );

                // The ranking itself is still capped, and the leader is first.
                assert_eq!(bundle.hotspots.len(), LOP_ENGINE_TOP_N);
                let leader = &bundle.hotspots[0];
                assert_eq!(leader.function, "hot");
                assert_eq!(leader.call_count, LOP_HOT_CALLS);

                // Exact: 100/199 of the trace, as a percentage.
                let expected_share = (LOP_HOT_CALLS as f64 / LOP_TRACE_TOTAL as f64) * 100.0;
                assert_eq!(
                    leader.share_pct, expected_share,
                    "leader share must be 100/199 = 50.2512...%, not 100/119 = \
                     84.0336...% from the truncated subtotal"
                );
                // Same value pinned at two decimals, so a reader does not have
                // to trust the float expression above.
                assert_eq!((leader.share_pct * 100.0).round() / 100.0, 50.25);

                // The listed hotspots cover 119 of the 199 calls, so their
                // shares sum to well under 100%: the tail is real work and the
                // denominator now shows it.
                let share_sum: f64 = bundle.hotspots.iter().map(|h| h.share_pct).sum();
                assert!(
                    share_sum < 100.0,
                    "hotspot shares must sum below 100%, got {}",
                    share_sum
                );
                assert_eq!((share_sum * 100.0).round() / 100.0, 59.8);
            }
            other => panic!("expected Derived, got {:?}", other),
        }
    }

    /// `distinct_callees` is the cardinality of the ANALYSED hotspot set and
    /// saturates at the engine's top-20 cutoff. The field name is a published
    /// wire key and stays as it is; this test pins the value that the
    /// `CallGraphSummary::distinct_callees` doc describes, so nobody reads
    /// `20` as "this trace had 20 callees".
    #[test]
    fn distinct_callees_is_the_analysed_set_cardinality_not_a_callee_census() {
        let out = explain_lopsided(SessionExplainKind::Derived);
        match out {
            SessionExplainOutput::Derived { bundle, .. } => {
                assert_eq!(
                    bundle.call_graph_summary.distinct_callees,
                    LOP_ENGINE_TOP_N,
                    "the value is the analysed hotspot set, capped at {}; the trace \
                     called {} distinct functions. The honest graph cardinality is \
                     `execution_query{{kind=\"call_graph\"}}`, not this field.",
                    LOP_ENGINE_TOP_N,
                    LOP_HOT_CALLS + LOP_TAIL_FUNCTIONS
                );
                // It is not a callee count either: `hot` is a leaf here, called
                // 100 times and never calling anything, and it is still
                // counted.
                let leaf_only = &bundle.hotspots[0];
                assert_eq!(leaf_only.function, "hot");
                assert_eq!(bundle.call_graph_summary.max_depth, 0);
            }
            other => panic!("expected Derived, got {:?}", other),
        }
    }

    #[test]
    fn explain_hypothesis_crash_invariant_returns_plan() {
        let store = empty_arc_store();
        save_session(&store, "s1", &["main"]);
        let ctx = make_ctx(std::sync::Arc::clone(&store));
        let input = SessionExplainInput {
            kind: SessionExplainKind::Hypothesis,
            session_id: "s1".to_string(),
            hypothesis_kind: Some(HypothesisTestKind::CrashInvariant),
        };
        let out = ChronosSessionExplainService::explain(&ctx, input).unwrap();
        match out {
            SessionExplainOutput::Hypothesis { bundle, provenance } => {
                assert_eq!(provenance.source, "session_explain:hypothesis");
                match bundle.plan {
                    HypothesisTestPlan::CrashInvariant { session_id, .. } => {
                        assert_eq!(session_id, "s1");
                    }
                    other => panic!("expected CrashInvariant plan, got {:?}", other),
                }
                assert!(!bundle.hint.is_empty());
            }
            other => panic!("expected Hypothesis, got {:?}", other),
        }
    }

    #[test]
    fn explain_hypothesis_dominant_callpath_returns_plan() {
        let store = empty_arc_store();
        save_session(&store, "s1", &["main", "main", "main", "helper"]);
        let ctx = make_ctx(std::sync::Arc::clone(&store));
        let input = SessionExplainInput {
            kind: SessionExplainKind::Hypothesis,
            session_id: "s1".to_string(),
            hypothesis_kind: Some(HypothesisTestKind::DominantFunctionCallPath),
        };
        let out = ChronosSessionExplainService::explain(&ctx, input).unwrap();
        match out {
            SessionExplainOutput::Hypothesis { bundle, .. } => match bundle.plan {
                HypothesisTestPlan::DominantFunctionCallPath {
                    session_id,
                    function,
                    at_least_n,
                } => {
                    assert_eq!(session_id, "s1");
                    assert_eq!(function, "main");
                    assert!(at_least_n >= 1);
                }
                other => panic!("expected DominantFunctionCallPath plan, got {:?}", other),
            },
            other => panic!("expected Hypothesis, got {:?}", other),
        }
    }

    #[test]
    fn explain_hypothesis_missing_kind_returns_invalid_input() {
        let store = empty_arc_store();
        save_session(&store, "s1", &["main"]);
        let ctx = make_ctx(std::sync::Arc::clone(&store));
        let input = SessionExplainInput {
            kind: SessionExplainKind::Hypothesis,
            session_id: "s1".to_string(),
            hypothesis_kind: None,
        };
        let err = ChronosSessionExplainService::explain(&ctx, input).unwrap_err();
        assert!(
            matches!(err, ServiceError::InvalidInput(_)),
            "expected InvalidInput, got {:?}",
            err
        );
    }

    #[test]
    fn explain_session_not_found() {
        let store = empty_arc_store();
        let ctx = make_ctx(std::sync::Arc::clone(&store));
        let input = SessionExplainInput {
            kind: SessionExplainKind::Facts,
            session_id: "missing".to_string(),
            hypothesis_kind: None,
        };
        let err = ChronosSessionExplainService::explain(&ctx, input).unwrap_err();
        assert!(matches!(err, ServiceError::SessionNotFound(_)));
    }

    #[test]
    fn explain_provenance_present_on_all_kinds() {
        let store = empty_arc_store();
        save_session(&store, "s1", &["main", "helper"]);
        let ctx = make_ctx(std::sync::Arc::clone(&store));

        for kind in [
            SessionExplainKind::Facts,
            SessionExplainKind::Derived,
            SessionExplainKind::Inferred,
        ] {
            let input = SessionExplainInput {
                kind,
                session_id: "s1".to_string(),
                hypothesis_kind: None,
            };
            let out = ChronosSessionExplainService::explain(&ctx, input).unwrap();
            let source = match &out {
                SessionExplainOutput::Facts { provenance, .. } => &provenance.source,
                SessionExplainOutput::Derived { provenance, .. } => &provenance.source,
                SessionExplainOutput::Inferred { provenance, .. } => &provenance.source,
                SessionExplainOutput::Hypothesis { provenance, .. } => &provenance.source,
            };
            assert!(source.starts_with("session_explain:"));
            assert!(!source.is_empty());
        }
    }
}
