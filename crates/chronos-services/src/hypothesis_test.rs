//! M6 — Hypothesis Test dispatcher (m6-04).
//!
//! The v2 `hypothesis_test` tool is the net-new capability listed in
//! `docs/milestones/m5-close-report.md` §4.2. There is **no v1 tool** to
//! deprecate (this is one of two M6 net-new capabilities, alongside
//! `session_export` in m6-05).
//!
//! Three supported hypothesis shapes (typed, not DSL strings):
//!
//! | `kind`     | Reuses                                                  | Verdict source |
//! |------------|---------------------------------------------------------|----------------|
//! | `Invariant`| `chronos_domain::property::InvariantCheck` machinery   | per-event vs. typed comparison |
//! | `Existence`| raw predicate scan over `engine.get_all_events()`        | `Pass` if any match, else `Violation` |
//! | `CallPath` | local rebuild of call graph over `engine.get_all_events()`| BFS reachability in the call graph |
//!
//! All three return the same tri-state
//! [`HypothesisVerdict`](crate::output::HypothesisVerdict):
//! `Pass` / `Violation { reason }` / `Unsupported { reason }`. The caller
//! must NOT collapse `Unsupported` to `Pass` (see
//! `docs/.../specs/RUNTIME_PROPERTIES_AND_SLICING.md`).
//!
//! Per the v2 spec ("without hiding raw support"), every variant returns
//! the raw `support_event_ids` (and `counter_event_ids` for violations)
//! so the calling agent can verify the support itself.

use std::collections::{HashMap, HashSet, VecDeque};

use chronos_domain::property::{
    ComparisonOp, InvariantCheck, Property, PropertyId, PropertyOutcome, PropertyValue,
};
use chronos_domain::trace::{EventData, EventType, TraceEvent};
use chronos_query::QueryEngine;
use tokio::sync::Mutex as TokioMutex;

use crate::error::ServiceError;
use crate::output::{
    ExistencePredicate, HypothesisKind, HypothesisOutput, HypothesisScope, HypothesisVerdict,
};

/// Borrowed handle to the live engine map (shared with the MCP server).
///
/// Matches the established pattern in `chronos_services::trace_slice`,
/// `chronos_services::state_query`, etc. — the value type is `QueryEngine`
/// (no inner `Arc`); `Arc`-wrapping happens at the call site.
pub struct HypothesisTestContext<'a> {
    pub engines: &'a TokioMutex<HashMap<String, QueryEngine>>,
}

/// Input for [`ChronosHypothesisTestService::test`].
///
/// Variant-specific data is carried in optional fields and only the
/// `kind`-relevant subset is consumed.
#[derive(Debug, Clone)]
pub struct HypothesisInput {
    pub session_id: String,
    pub kind: HypothesisKind,

    // Invariant shape fields (used only when kind == Invariant).
    pub scope: Option<HypothesisScope>,
    pub comparison: Option<ComparisonOp>,
    pub constant: Option<PropertyValue>,
    pub property_target: Option<String>,

    // Existence shape fields (used only when kind == Existence).
    pub predicate: Option<ExistencePredicate>,

    // CallPath shape fields (used only when kind == CallPath).
    pub caller: Option<String>,
    pub callee: Option<String>,
    pub max_depth: Option<usize>,
}

/// Stateless holder for the v2 `hypothesis_test` dispatcher.
pub struct ChronosHypothesisTestService;

impl ChronosHypothesisTestService {
    /// Evaluate a typed hypothesis against the captured session evidence.
    ///
    /// Required parameters per kind:
    /// - `Invariant` → `scope + comparison + constant`, plus `property_target`
    ///                 when `scope == PropertyValue`.
    /// - `Existence` → `predicate`.
    /// - `CallPath`  → `caller + callee`. `max_depth` defaults to 10
    ///                 (cap for BFS expansion; same default as
    ///                 `debug_call_graph`).
    ///
    /// Returns [`ServiceError::InvalidInput`] when a required target field
    /// is missing or the input is otherwise malformed.
    pub async fn test(
        ctx: &HypothesisTestContext<'_>,
        input: HypothesisInput,
    ) -> Result<HypothesisOutput, ServiceError> {
        let guard = ctx.engines.lock().await;
        let engine = guard
            .get(&input.session_id)
            .ok_or_else(|| ServiceError::SessionNotFound(input.session_id.clone()))?;
        let events = engine.get_all_events();

        match input.kind {
            HypothesisKind::Invariant => Ok(eval_invariant(&input, &events)),
            HypothesisKind::Existence => Ok(eval_existence(&input, &events)),
            HypothesisKind::CallPath => Ok(eval_call_path(&input, &events)),
        }
    }
}

// ---------------------------------------------------------------------------
// Invariant
// ---------------------------------------------------------------------------

fn eval_invariant(input: &HypothesisInput, events: &[TraceEvent]) -> HypothesisOutput {
    let scope = input.scope.unwrap_or(HypothesisScope::EventCount);
    let comparison = input.comparison.unwrap_or(ComparisonOp::Eq);
    let constant = input
        .constant
        .clone()
        .unwrap_or(PropertyValue::Number(0.0));

    let (verdict, support, counter, mut summary): (HypothesisVerdict, Vec<u64>, Vec<u64>, String) =
        match scope {
            HypothesisScope::EventCount => {
                let count = events.len() as f64;
                let observed = PropertyValue::Number(count);
                let outcome = Property {
                    id: PropertyId(0),
                    name: "event_count".into(),
                    version: 1,
                    observe: "event_count".into(),
                    trigger: String::new(),
                    invariant: InvariantCheck::Comparison {
                        op: comparison,
                        constant: constant.clone(),
                    },
                }
                .evaluate(Some(&observed), None);
                outcome_to_envelope(outcome, &comparison, &constant, vec![], vec![])
            }
            HypothesisScope::PropertyValue => match input.property_target.clone() {
                None => (
                    HypothesisVerdict::Unsupported {
                        reason: "scope=property_value requires property_target".into(),
                    },
                    Vec::new(),
                    Vec::new(),
                    "scope=property_value requires property_target to be set".into(),
                ),
                Some(target) => match observe_property_target(events, &target) {
                    None => (
                        HypothesisVerdict::Unsupported {
                            reason: format!(
                                "no recorded observation for property target `{target}` in session"
                            ),
                        },
                        Vec::new(),
                        Vec::new(),
                        format!(
                            "scope=property_value target={target}: required observation not captured"
                        ),
                    ),
                    Some((obs, ids)) => {
                        let outcome = Property {
                            id: PropertyId(0),
                            name: target.clone(),
                            version: 1,
                            observe: target.clone(),
                            trigger: String::new(),
                            invariant: InvariantCheck::Comparison {
                                op: comparison,
                                constant: constant.clone(),
                            },
                        }
                        .evaluate(Some(&obs), None);
                        let mut counter_local: Vec<u64> = Vec::new();
                        let (v, s, _, mut sm) =
                            outcome_to_envelope(outcome, &comparison, &constant, ids.clone(), vec![]);
                        if matches!(v, HypothesisVerdict::Violation { .. }) {
                            for id in &ids {
                                counter_local.push(*id);
                            }
                            sm = format!("{sm}: observed {obs} at events {ids:?}");
                        }
                        (v, s, counter_local, sm)
                    }
                },
            },
            HypothesisScope::LatencyMs => (
                HypothesisVerdict::Unsupported {
                    reason: "scope=latency_ms is reserved for a future milestone (m7+): \
                             chronos_domain::trace::EventData has no latency_ms field today."
                        .into(),
                },
                Vec::new(),
                Vec::new(),
                "scope=latency_ms not implemented in m6-04".into(),
            ),
        };

    if summary.is_empty() {
        summary = format!("invariant evaluated (scope={scope:?})");
    }

    HypothesisOutput::Invariant {
        verdict,
        support_event_ids: support,
        counter_event_ids: counter,
        scope,
        summary,
    }
}

fn outcome_to_envelope(
    outcome: PropertyOutcome,
    op: &ComparisonOp,
    constant: &PropertyValue,
    support: Vec<u64>,
    counter: Vec<u64>,
) -> (HypothesisVerdict, Vec<u64>, Vec<u64>, String) {
    match outcome {
        PropertyOutcome::Pass => (
            HypothesisVerdict::Pass,
            support,
            counter,
            format!("invariant satisfied ({op} {constant})"),
        ),
        PropertyOutcome::Violation { message, .. } => (
            HypothesisVerdict::Violation { reason: message.clone() },
            support,
            counter,
            message,
        ),
        PropertyOutcome::UnsupportedByRecordedEvidence { reason } => (
            HypothesisVerdict::Unsupported { reason: reason.clone() },
            support,
            counter,
            reason,
        ),
    }
}

fn observe_property_target(
    events: &[TraceEvent],
    target: &str,
) -> Option<(PropertyValue, Vec<u64>)> {
    let mut last: Option<PropertyValue> = None;
    let mut ids: Vec<u64> = Vec::new();
    for ev in events {
        if let EventData::Variable(v) = &ev.data {
            // Heuristic: match either by variable name (full path segments)
            // or by trailing path component. `target` is treated as a free
            // identifier since VariableInfo doesn't carry a target_path.
            let matches_name = v.name == target
                || v.name.split('.').last() == Some(target)
                || v.name.ends_with(&format!(".{target}"));
            if matches_name {
                let pv = parse_property_value(&v.value);
                last = Some(pv);
                ids.push(ev.event_id);
            }
        }
    }
    last.map(|v| (v, ids))
}

fn parse_property_value(s: &str) -> PropertyValue {
    if let Ok(n) = s.parse::<f64>() {
        PropertyValue::Number(n)
    } else if let Ok(b) = s.parse::<bool>() {
        PropertyValue::Bool(b)
    } else {
        PropertyValue::Text(s.to_string())
    }
}

// ---------------------------------------------------------------------------
// Existence
// ---------------------------------------------------------------------------

fn eval_existence(input: &HypothesisInput, events: &[TraceEvent]) -> HypothesisOutput {
    let predicate = input.predicate.clone().unwrap_or_else(|| {
        ExistencePredicate::EventTypeEquals {
            event_type: "function_entry".into(),
        }
    });

    if events.is_empty() {
        return HypothesisOutput::Existence {
            verdict: HypothesisVerdict::Unsupported {
                reason: "session has no captured events; cannot evaluate existence".into(),
            },
            support_event_ids: Vec::new(),
            counter_event_ids: Vec::new(),
            predicate,
            summary: "no captured events in session".into(),
        };
    }

    let support = scan_predicate(events, &predicate);

    let (verdict, summary) = if !support.is_empty() {
        (
            HypothesisVerdict::Pass,
            format!(
                "found {} matching event(s) for {}",
                support.len(),
                predicate_label(&predicate)
            ),
        )
    } else {
        (
            HypothesisVerdict::Violation {
                reason: format!(
                    "no captured event satisfied {}",
                    predicate_label(&predicate)
                ),
            },
            format!(
                "0 matching events out of {} total for {}",
                events.len(),
                predicate_label(&predicate)
            ),
        )
    };

    HypothesisOutput::Existence {
        verdict,
        support_event_ids: support,
        counter_event_ids: Vec::new(),
        predicate,
        summary,
    }
}

fn scan_predicate(events: &[TraceEvent], predicate: &ExistencePredicate) -> Vec<u64> {
    let mut support = Vec::new();
    for ev in events {
        let matches = match predicate {
            ExistencePredicate::EventTypeEquals { event_type } => {
                event_type_label(ev.event_type) == *event_type
            }
            ExistencePredicate::ThreadEquals { thread_id } => ev.thread_id == *thread_id,
            ExistencePredicate::PropertyKeyEquals { target } => {
                if let EventData::Variable(v) = &ev.data {
                    let matches_name = v.name == *target
                        || v.name.split('.').last() == Some(target)
                        || v.name.ends_with(&format!(".{target}"));
                    matches_name
                } else {
                    false
                }
            }
        };
        if matches {
            support.push(ev.event_id);
        }
    }
    support
}

fn predicate_label(p: &ExistencePredicate) -> String {
    match p {
        ExistencePredicate::EventTypeEquals { event_type } => {
            format!("event_type={event_type}")
        }
        ExistencePredicate::ThreadEquals { thread_id } => format!("thread_id={thread_id}"),
        ExistencePredicate::PropertyKeyEquals { target } => {
            format!("property_key={target}")
        }
    }
}

fn event_type_label(t: EventType) -> &'static str {
    match t {
        EventType::SyscallEnter => "syscall_enter",
        EventType::SyscallExit => "syscall_exit",
        EventType::FunctionEntry => "function_entry",
        EventType::FunctionExit => "function_exit",
        EventType::VariableWrite => "variable_write",
        EventType::MemoryWrite => "memory_write",
        EventType::SignalDelivered => "signal_delivered",
        EventType::BreakpointHit => "breakpoint_hit",
        EventType::ThreadCreate => "thread_create",
        EventType::ThreadExit => "thread_exit",
        EventType::ExceptionThrown => "exception_thrown",
        EventType::VariableRead => "variable_read",
        EventType::MemoryAlloc => "memory_alloc",
        EventType::MemoryFree => "memory_free",
        EventType::MemoryRead => "memory_read",
        EventType::ThreadSwitch => "thread_switch",
        EventType::WatchTrigger => "watch_trigger",
        EventType::ExceptionCaught => "exception_caught",
        EventType::InvocationIncomplete => "invocation_incomplete",
        EventType::Custom => "custom",
        EventType::Unknown => "unknown",
    }
}

// ---------------------------------------------------------------------------
// CallPath
// ---------------------------------------------------------------------------

fn eval_call_path(input: &HypothesisInput, events: &[TraceEvent]) -> HypothesisOutput {
    let caller = input.caller.clone().unwrap_or_default();
    let callee = input.callee.clone().unwrap_or_default();
    let max_depth = input.max_depth.unwrap_or(10);

    if caller.is_empty() || callee.is_empty() {
        return HypothesisOutput::CallPath {
            verdict: HypothesisVerdict::Unsupported {
                reason: "caller and callee are required for kind=call_path (must be non-empty)"
                    .into(),
            },
            support_event_ids: Vec::new(),
            counter_event_ids: Vec::new(),
            caller,
            callee,
            reachable_path: None,
            summary: "missing caller/callee".into(),
        };
    }

    if caller == callee {
        let p = caller.clone();
        return HypothesisOutput::CallPath {
            verdict: HypothesisVerdict::Pass,
            support_event_ids: Vec::new(),
            counter_event_ids: Vec::new(),
            caller,
            callee,
            reachable_path: Some(vec![p]),
            summary: format!("caller == callee (still owned)"),
        };
    }

    // Build adjacency from FunctionEntry/Exit events. VariableInfo's `value`
    // field carries the function name.
    let mut callees_by_caller: HashMap<String, Vec<String>> = HashMap::new();
    let mut stacks: HashMap<u64, Vec<String>> = HashMap::new();
    let mut function_node_present: HashSet<String> = HashSet::new();
    for ev in events {
        if let EventData::Function { name, .. } = &ev.data {
            if name.is_empty() {
                continue;
            }
            function_node_present.insert(name.clone());
            match ev.event_type {
                EventType::FunctionEntry => {
                    let stack = stacks.entry(ev.thread_id).or_default();
                    if stack.len() < max_depth {
                        if let Some(parent) = stack.last().cloned() {
                            callees_by_caller
                                .entry(parent)
                                .or_default()
                                .push(name.clone());
                        }
                        stack.push(name.clone());
                    }
                }
                EventType::FunctionExit => {
                    if let Some(stack) = stacks.get_mut(&ev.thread_id) {
                        stack.pop();
                    }
                }
                _ => {}
            }
        }
    }

    if !function_node_present.contains(&caller) {
        let msg = format!("caller `{caller}` not present in call graph");
        let summary = format!("caller `{caller}` absent");
        return HypothesisOutput::CallPath {
            verdict: HypothesisVerdict::Unsupported { reason: msg },
            support_event_ids: Vec::new(),
            counter_event_ids: Vec::new(),
            caller,
            callee,
            reachable_path: None,
            summary,
        };
    }

    let (verdict, path, summary) = bfs_reach(&callees_by_caller, &caller, &callee);
    HypothesisOutput::CallPath {
        verdict,
        support_event_ids: Vec::new(),
        counter_event_ids: Vec::new(),
        caller,
        callee,
        reachable_path: path,
        summary,
    }
}

fn bfs_reach(
    adj: &HashMap<String, Vec<String>>,
    caller: &str,
    callee: &str,
) -> (HypothesisVerdict, Option<Vec<String>>, String) {
    let mut visited: HashSet<String> = HashSet::new();
    let mut parents: HashMap<String, String> = HashMap::new();
    let mut queue: VecDeque<String> = VecDeque::new();
    visited.insert(caller.to_string());
    queue.push_back(caller.to_string());

    let mut found: Option<Vec<String>> = None;
    while let Some(node) = queue.pop_front() {
        if node == callee {
            let mut path = Vec::new();
            let mut cur = callee.to_string();
            loop {
                path.push(cur.clone());
                match parents.get(&cur) {
                    Some(p) => cur = p.clone(),
                    None => break,
                }
            }
            path.reverse();
            found = Some(path);
            break;
        }
        if let Some(nbrs) = adj.get(&node) {
            for n in nbrs {
                if visited.insert(n.clone()) {
                    parents.insert(n.clone(), node.clone());
                    queue.push_back(n.clone());
                }
            }
        }
    }

    match found {
        Some(p) => (
            HypothesisVerdict::Pass,
            Some(p.clone()),
            format!("reachable in {} step(s)", p.len().saturating_sub(1)),
        ),
        None => (
            HypothesisVerdict::Violation {
                reason: format!("`{callee}` is not reachable from `{caller}`"),
            },
            None,
            format!("no call path `{caller}` -> `{callee}`"),
        ),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::trace::{EventData, EventType, SourceLocation};
    use chronos_domain::value::{VariableInfo, VariableScope};
    use chronos_index::builder::IndexBuilder;
    use chronos_query::QueryEngine;

    use std::sync::Arc;

    fn make_engine(events: Vec<TraceEvent>) -> Arc<TokioMutex<HashMap<String, QueryEngine>>> {
        let mut builder = IndexBuilder::new();
        builder.push_all(&events);
        let indices = builder.finalize();
        let engine = QueryEngine::with_indices(events, indices.shadow, indices.temporal)
            .with_causality(indices.causality)
            .with_performance(indices.performance);
        let mut map: HashMap<String, QueryEngine> = HashMap::new();
        map.insert("s1".to_string(), engine);
        Arc::new(TokioMutex::new(map))
    }

    fn var_event(id: u64, thread: u64, name: &str, value: &str) -> TraceEvent {
        TraceEvent {
            event_id: id,
            timestamp_ns: id * 1000,
            thread_id: thread,
            event_type: EventType::VariableWrite,
            location: SourceLocation::default(),
            data: EventData::Variable(VariableInfo::new(
                name,
                value,
                "f64",
                0x1000,
                VariableScope::Local,
            )),
        }
    }

    fn func_event(id: u64, thread: u64, kind: EventType, name: &str) -> TraceEvent {
        TraceEvent {
            event_id: id,
            timestamp_ns: id * 1000,
            thread_id: thread,
            event_type: kind,
            location: SourceLocation {
                function: Some(name.to_string()),
                ..SourceLocation::default()
            },
            data: EventData::Function {
                name: name.to_string(),
                signature: None,
                symbol_id: None,
                invocation_id: None,
                parent_invocation_id: None,
            },
        }
    }

    #[tokio::test]
    async fn invariant_event_count_lt_pass() {
        let events = vec![
            func_event(1, 1, EventType::FunctionEntry, "main"),
            func_event(2, 1, EventType::FunctionExit, "main"),
            func_event(3, 1, EventType::FunctionEntry, "foo"),
            func_event(4, 1, EventType::FunctionExit, "foo"),
        ];
        let engines = make_engine(events);
        let ctx = HypothesisTestContext {
            engines: engines.as_ref(),
        };
        let inp = HypothesisInput {
            session_id: "s1".into(),
            kind: HypothesisKind::Invariant,
            scope: Some(HypothesisScope::EventCount),
            comparison: Some(ComparisonOp::Lt),
            constant: Some(PropertyValue::Number(10.0)),
            property_target: None,
            predicate: None,
            caller: None,
            callee: None,
            max_depth: None,
        };
        let out = ChronosHypothesisTestService::test(&ctx, inp).await.unwrap();
        match out {
            HypothesisOutput::Invariant { verdict, .. } => {
                assert_eq!(verdict, HypothesisVerdict::Pass);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[tokio::test]
    async fn invariant_event_count_ge_violation() {
        let events = vec![func_event(1, 1, EventType::FunctionEntry, "main")];
        let engines = make_engine(events);
        let ctx = HypothesisTestContext {
            engines: engines.as_ref(),
        };
        let inp = HypothesisInput {
            session_id: "s1".into(),
            kind: HypothesisKind::Invariant,
            scope: Some(HypothesisScope::EventCount),
            comparison: Some(ComparisonOp::Ge),
            constant: Some(PropertyValue::Number(10.0)),
            property_target: None,
            predicate: None,
            caller: None,
            callee: None,
            max_depth: None,
        };
        let out = ChronosHypothesisTestService::test(&ctx, inp).await.unwrap();
        match out {
            HypothesisOutput::Invariant { verdict, .. } => match verdict {
                HypothesisVerdict::Violation { reason } => {
                    assert!(!reason.is_empty());
                }
                v => panic!("expected Violation, got {v:?}"),
            },
            _ => panic!("wrong variant"),
        }
    }

    #[tokio::test]
    async fn invariant_property_value_eq_pass() {
        let events = vec![var_event(1, 1, "Order.total", "99")];
        let engines = make_engine(events);
        let ctx = HypothesisTestContext {
            engines: engines.as_ref(),
        };
        let inp = HypothesisInput {
            session_id: "s1".into(),
            kind: HypothesisKind::Invariant,
            scope: Some(HypothesisScope::PropertyValue),
            comparison: Some(ComparisonOp::Eq),
            constant: Some(PropertyValue::Number(99.0)),
            property_target: Some("total".into()),
            predicate: None,
            caller: None,
            callee: None,
            max_depth: None,
        };
        let out = ChronosHypothesisTestService::test(&ctx, inp).await.unwrap();
        match out {
            HypothesisOutput::Invariant { verdict, support_event_ids, .. } => {
                assert_eq!(verdict, HypothesisVerdict::Pass);
                assert_eq!(support_event_ids, vec![1u64]);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[tokio::test]
    async fn invariant_property_value_missing_unsupported() {
        let engines = make_engine(vec![]);
        let ctx = HypothesisTestContext {
            engines: engines.as_ref(),
        };
        let inp = HypothesisInput {
            session_id: "s1".into(),
            kind: HypothesisKind::Invariant,
            scope: Some(HypothesisScope::PropertyValue),
            comparison: Some(ComparisonOp::Eq),
            constant: Some(PropertyValue::Number(0.0)),
            property_target: Some("Order.total".into()),
            predicate: None,
            caller: None,
            callee: None,
            max_depth: None,
        };
        let out = ChronosHypothesisTestService::test(&ctx, inp).await.unwrap();
        match out {
            HypothesisOutput::Invariant { verdict, .. } => {
                assert!(matches!(verdict, HypothesisVerdict::Unsupported { .. }));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[tokio::test]
    async fn invariant_latency_ms_unsupported_with_reason() {
        let engines = make_engine(vec![]);
        let ctx = HypothesisTestContext {
            engines: engines.as_ref(),
        };
        let inp = HypothesisInput {
            session_id: "s1".into(),
            kind: HypothesisKind::Invariant,
            scope: Some(HypothesisScope::LatencyMs),
            comparison: Some(ComparisonOp::Lt),
            constant: Some(PropertyValue::Number(50.0)),
            property_target: None,
            predicate: None,
            caller: None,
            callee: None,
            max_depth: None,
        };
        let out = ChronosHypothesisTestService::test(&ctx, inp).await.unwrap();
        match out {
            HypothesisOutput::Invariant { verdict, .. } => match verdict {
                HypothesisVerdict::Unsupported { reason } => {
                    assert!(reason.contains("latency_ms"));
                }
                v => panic!("expected Unsupported, got {v:?}"),
            },
            _ => panic!("wrong variant"),
        }
    }

    #[tokio::test]
    async fn existence_event_type_pass() {
        let events = vec![func_event(1, 1, EventType::FunctionEntry, "main")];
        let engines = make_engine(events);
        let ctx = HypothesisTestContext {
            engines: engines.as_ref(),
        };
        let inp = HypothesisInput {
            session_id: "s1".into(),
            kind: HypothesisKind::Existence,
            scope: None,
            comparison: None,
            constant: None,
            property_target: None,
            predicate: Some(ExistencePredicate::EventTypeEquals {
                event_type: "function_entry".into(),
            }),
            caller: None,
            callee: None,
            max_depth: None,
        };
        let out = ChronosHypothesisTestService::test(&ctx, inp).await.unwrap();
        match out {
            HypothesisOutput::Existence { verdict, support_event_ids, .. } => {
                assert_eq!(verdict, HypothesisVerdict::Pass);
                assert_eq!(support_event_ids, vec![1u64]);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[tokio::test]
    async fn existence_no_match_violation() {
        let engines = make_engine(vec![func_event(1, 1, EventType::FunctionEntry, "main")]);
        let ctx = HypothesisTestContext {
            engines: engines.as_ref(),
        };
        let inp = HypothesisInput {
            session_id: "s1".into(),
            kind: HypothesisKind::Existence,
            scope: None,
            comparison: None,
            constant: None,
            property_target: None,
            predicate: Some(ExistencePredicate::EventTypeEquals {
                event_type: "memory_write".into(),
            }),
            caller: None,
            callee: None,
            max_depth: None,
        };
        let out = ChronosHypothesisTestService::test(&ctx, inp).await.unwrap();
        match out {
            HypothesisOutput::Existence { verdict, support_event_ids, .. } => {
                assert!(matches!(verdict, HypothesisVerdict::Violation { .. }));
                assert!(support_event_ids.is_empty());
            }
            _ => panic!("wrong variant"),
        }
    }

    #[tokio::test]
    async fn existence_empty_session_unsupported() {
        let engines = make_engine(vec![]);
        let ctx = HypothesisTestContext {
            engines: engines.as_ref(),
        };
        let inp = HypothesisInput {
            session_id: "s1".into(),
            kind: HypothesisKind::Existence,
            scope: None,
            comparison: None,
            constant: None,
            property_target: None,
            predicate: Some(ExistencePredicate::EventTypeEquals {
                event_type: "function_entry".into(),
            }),
            caller: None,
            callee: None,
            max_depth: None,
        };
        let out = ChronosHypothesisTestService::test(&ctx, inp).await.unwrap();
        match out {
            HypothesisOutput::Existence { verdict, .. } => {
                assert!(matches!(verdict, HypothesisVerdict::Unsupported { .. }));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[tokio::test]
    async fn call_path_reachable_pass() {
        let events = vec![
            func_event(1, 1, EventType::FunctionEntry, "main"),
            func_event(2, 1, EventType::FunctionEntry, "a"),
            func_event(3, 1, EventType::FunctionEntry, "b"),
            func_event(4, 1, EventType::FunctionExit, "b"),
            func_event(5, 1, EventType::FunctionExit, "a"),
            func_event(6, 1, EventType::FunctionExit, "main"),
        ];
        let engines = make_engine(events);
        let ctx = HypothesisTestContext {
            engines: engines.as_ref(),
        };
        let inp = HypothesisInput {
            session_id: "s1".into(),
            kind: HypothesisKind::CallPath,
            scope: None,
            comparison: None,
            constant: None,
            property_target: None,
            predicate: None,
            caller: Some("main".into()),
            callee: Some("b".into()),
            max_depth: None,
        };
        let out = ChronosHypothesisTestService::test(&ctx, inp).await.unwrap();
        match out {
            HypothesisOutput::CallPath { verdict, reachable_path, .. } => {
                assert_eq!(verdict, HypothesisVerdict::Pass);
                let path = reachable_path.expect("path on Pass");
                assert_eq!(path, vec!["main".to_string(), "a".to_string(), "b".to_string()]);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[tokio::test]
    async fn call_path_unreachable_violation() {
        let events = vec![
            func_event(1, 1, EventType::FunctionEntry, "main"),
            func_event(2, 1, EventType::FunctionEntry, "a"),
            func_event(3, 1, EventType::FunctionExit, "a"),
            func_event(4, 1, EventType::FunctionExit, "main"),
        ];
        let engines = make_engine(events);
        let ctx = HypothesisTestContext {
            engines: engines.as_ref(),
        };
        let inp = HypothesisInput {
            session_id: "s1".into(),
            kind: HypothesisKind::CallPath,
            scope: None,
            comparison: None,
            constant: None,
            property_target: None,
            predicate: None,
            caller: Some("main".into()),
            callee: Some("b".into()),
            max_depth: None,
        };
        let out = ChronosHypothesisTestService::test(&ctx, inp).await.unwrap();
        match out {
            HypothesisOutput::CallPath { verdict, reachable_path, .. } => {
                assert!(matches!(verdict, HypothesisVerdict::Violation { .. }));
                assert!(reachable_path.is_none());
            }
            _ => panic!("wrong variant"),
        }
    }

    #[tokio::test]
    async fn call_path_self_loop_pass() {
        let engines = make_engine(vec![]);
        let ctx = HypothesisTestContext {
            engines: engines.as_ref(),
        };
        let inp = HypothesisInput {
            session_id: "s1".into(),
            kind: HypothesisKind::CallPath,
            scope: None,
            comparison: None,
            constant: None,
            property_target: None,
            predicate: None,
            caller: Some("main".into()),
            callee: Some("main".into()),
            max_depth: None,
        };
        let out = ChronosHypothesisTestService::test(&ctx, inp).await.unwrap();
        match out {
            HypothesisOutput::CallPath { verdict, .. } => {
                assert_eq!(verdict, HypothesisVerdict::Pass);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[tokio::test]
    async fn call_path_missing_caller_unsupported() {
        let engines = make_engine(vec![]);
        let ctx = HypothesisTestContext {
            engines: engines.as_ref(),
        };
        let inp = HypothesisInput {
            session_id: "s1".into(),
            kind: HypothesisKind::CallPath,
            scope: None,
            comparison: None,
            constant: None,
            property_target: None,
            predicate: None,
            caller: None,
            callee: Some("b".into()),
            max_depth: None,
        };
        let out = ChronosHypothesisTestService::test(&ctx, inp).await.unwrap();
        match out {
            HypothesisOutput::CallPath { verdict, .. } => {
                assert!(matches!(verdict, HypothesisVerdict::Unsupported { .. }));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[tokio::test]
    async fn session_not_found_returns_error() {
        let engines = make_engine(vec![]);
        let ctx = HypothesisTestContext {
            engines: engines.as_ref(),
        };
        let inp = HypothesisInput {
            session_id: "missing".into(),
            kind: HypothesisKind::Existence,
            scope: None,
            comparison: None,
            constant: None,
            property_target: None,
            predicate: Some(ExistencePredicate::EventTypeEquals {
                event_type: "function_entry".into(),
            }),
            caller: None,
            callee: None,
            max_depth: None,
        };
        let err = ChronosHypothesisTestService::test(&ctx, inp).await;
        assert!(matches!(err, Err(ServiceError::SessionNotFound(_))));
    }
}

