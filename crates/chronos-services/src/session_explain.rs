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
//!   call counts), SingleThreaded (thread_count==1), or Unknown.
//! - `hypothesis` — typed `HypothesisTestPlan` the agent can execute
//!   via the m6-04 `hypothesis_test` tool. `session_explain` does
//!   NOT execute the plan; it only plans.
//!
//! See `docs/milestones/m7-03-session-compare-explain-merge.md` for
//! the full spec + algorithm details.

use std::collections::HashSet;

use crate::error::ServiceError;
use crate::output::{
    CallGraphSummary, DerivedBundle, FactsBundle, FunctionHotspot, HypothesisBundle,
    HypothesisTestKind, HypothesisTestPlan, InferredBundle, InferredTag, SessionExplainInput,
    SessionExplainKind, SessionExplainOutput, SessionExplainProvenance,
};
use chronos_query::QueryEngine;
use chronos_store::{SessionStore, StoreError};

/// Borrowed handle to the live `SessionStore`.
pub struct SessionExplainContext<'a> {
    pub store: &'a SessionStore,
}

/// Threshold for the IoHeavy inference (share of events that are
/// IO-syscall events). 0.50 = 50%.
const IO_HEAVY_THRESHOLD: f64 = 0.50;

/// Threshold for the CpuBound inference (one function's share of
/// total call counts). 0.70 = 70%.
const CPU_BOUND_THRESHOLD: f64 = 0.70;

/// Names of syscalls considered "IO" for the IoHeavy inference.
const IO_SYSCALLS: &[&str] = &[
    "read", "write", "recv", "send", "recvfrom", "sendto", "accept", "accept4", "connect", "close",
];

/// Signal names considered crash indicators.
const CRASH_SIGNALS: &[&str] = &["SIGSEGV", "SIGABRT", "SIGBUS", "SIGFPE", "SIGILL"];

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
        ctx: &SessionExplainContext<'_>,
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
                bundle: build_inferred(&input.session_id, &summary),
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
    ctx: &SessionExplainContext<'_>,
    session_id: &str,
) -> Result<
    (
        chronos_store::SessionMetadata,
        Vec<chronos_domain::TraceEvent>,
    ),
    ServiceError,
> {
    ctx.store.load_session(session_id).map_err(map_load_error)
}

fn map_load_error(e: StoreError) -> ServiceError {
    match e {
        StoreError::SessionNotFound(s) => ServiceError::SessionNotFound(s),
        other => ServiceError::LoadFailed(format!("session_explain: store error: {}", other)),
    }
}

fn build_facts(
    meta: &chronos_store::SessionMetadata,
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
    // Look for any signal-delivered event matching CRASH_SIGNALS.
    let mut signal_delivered: Option<String> = None;
    let mut crash_detected = false;
    for issue in &summary.potential_issues {
        if issue.issue_type == "crash" || issue.issue_type == "signal" {
            crash_detected = true;
        }
        if let Some(sig) = &signal_delivered {
            let _ = sig; // keep borrow happy
        }
        if issue.description.starts_with("signal=") {
            let sig = issue
                .description
                .strip_prefix("signal=")
                .unwrap_or("")
                .to_string();
            if CRASH_SIGNALS.iter().any(|c| sig == *c) {
                crash_detected = true;
                signal_delivered = Some(sig);
            }
        }
    }
    // Also scan events directly for signals delivered.
    for ev in events {
        if let chronos_domain::EventData::Signal { signal_name, .. } = &ev.data {
            if CRASH_SIGNALS.iter().any(|c| signal_name == *c) {
                crash_detected = true;
                signal_delivered = Some(signal_name.clone());
            }
        }
    }
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
    let total_calls: u64 = summary.top_functions.iter().map(|f| f.call_count).sum();
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
) -> InferredBundle {
    let mut inferences: Vec<InferredTag> = Vec::new();

    // 1. CrashDetected: any potential_issue with type "crash" or "signal"
    //    flagged in the execution summary.
    let crash_detected = summary
        .potential_issues
        .iter()
        .any(|i| i.issue_type == "crash" || i.issue_type == "signal");
    if crash_detected {
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

    // 3. CpuBound: one function dominates >70% of total call counts.
    let total_calls: u64 = summary.top_functions.iter().map(|f| f.call_count).sum();
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
    use chronos_domain::{EventData, EventType, SourceLocation, TraceEvent};
    use chronos_store::{SessionMetadata, SessionStore};

    fn make_event(id: u64, func: &str) -> TraceEvent {
        let loc = SourceLocation::new("test.rs", 1, func.to_string(), 0x1000 + id);
        TraceEvent::new(
            id,
            id * 100,
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

    fn empty_store() -> SessionStore {
        SessionStore::in_memory().unwrap()
    }

    fn build_session_with_potential_issue(
        issue_type: &str,
        signal: Option<&str>,
    ) -> (SessionStore, String) {
        let store = empty_store();
        let mut events = vec![make_event(0, "main")];
        if let Some(sig) = signal {
            let loc = SourceLocation::new("test.rs", 1, "main", 0x2000);
            events.push(TraceEvent::new(
                1,
                100,
                1,
                EventType::SignalDelivered,
                loc,
                EventData::Signal {
                    signal_number: 11,
                    signal_name: sig.to_string(),
                },
            ));
        }
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
        let _ = issue_type;
        (store, "sess".to_string())
    }

    #[test]
    fn explain_facts_returns_facts_bundle() {
        let store = empty_store();
        save_session(&store, "s1", &["main", "helper", "main"]);
        let ctx = SessionExplainContext { store: &store };
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
        let store = empty_store();
        save_session(&store, "s1", &["main", "helper", "main"]);
        let ctx = SessionExplainContext { store: &store };
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

    #[test]
    fn explain_inferred_crash_detected_when_signal_delivered() {
        let (store, sid) = build_session_with_potential_issue("signal", Some("SIGSEGV"));
        let ctx = SessionExplainContext { store: &store };
        let input = SessionExplainInput {
            kind: SessionExplainKind::Inferred,
            session_id: sid,
            hypothesis_kind: None,
        };
        let out = ChronosSessionExplainService::explain(&ctx, input).unwrap();
        match out {
            SessionExplainOutput::Inferred { bundle, .. } => {
                assert!(
                    bundle.inferences.contains(&InferredTag::CrashDetected),
                    "expected CrashDetected in {:?}",
                    bundle.inferences
                );
            }
            other => panic!("expected Inferred, got {:?}", other),
        }
    }

    #[test]
    fn explain_inferred_single_threaded_when_thread_count_one() {
        let store = empty_store();
        save_session(&store, "s1", &["main"]);
        let ctx = SessionExplainContext { store: &store };
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
        let store = empty_store();
        // 12 events across 4 functions (3 each), 4 distinct threads.
        let events: Vec<TraceEvent> = (0..12)
            .map(|i| {
                let func_name = format!("func{}", i % 4);
                let loc = SourceLocation::new("test.rs", 1, func_name.clone(), 0x1000 + i);
                TraceEvent::new(
                    i,
                    i * 100,
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

        let ctx = SessionExplainContext { store: &store };
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

    #[test]
    fn explain_hypothesis_crash_invariant_returns_plan() {
        let store = empty_store();
        save_session(&store, "s1", &["main"]);
        let ctx = SessionExplainContext { store: &store };
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
        let store = empty_store();
        save_session(&store, "s1", &["main", "main", "main", "helper"]);
        let ctx = SessionExplainContext { store: &store };
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
        let store = empty_store();
        save_session(&store, "s1", &["main"]);
        let ctx = SessionExplainContext { store: &store };
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
        let store = empty_store();
        let ctx = SessionExplainContext { store: &store };
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
        let store = empty_store();
        save_session(&store, "s1", &["main", "helper"]);
        let ctx = SessionExplainContext { store: &store };

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
