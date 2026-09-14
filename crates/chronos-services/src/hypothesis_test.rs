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

use std::collections::HashMap;

use chronos_domain::property::{ComparisonOp, PropertyValue};
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
    /// - `Invariant` -> `scope + comparison + constant`, plus `property_target`
    ///   when `scope == PropertyValue`.
    /// - `Existence` -> `predicate`.
    /// - `CallPath` -> `caller + callee`. `max_depth` defaults to 10
    ///   (cap for BFS expansion; same default as `debug_call_graph`).
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
            HypothesisKind::Invariant => {
                // Translate the services-layer HypothesisScope into the
                // domain-owned PropertyObservationSource (m9-80 T1). The
                // missing-property_target case must short-circuit BEFORE
                // calling the domain function so that the verdict reason
                // matches the original semantics exactly.
                let scope = input.scope.unwrap_or(HypothesisScope::EventCount);
                let observation = match scope {
                    HypothesisScope::EventCount => {
                        chronos_domain::property::PropertyObservationSource::EventCount
                    }
                    HypothesisScope::PropertyValue => match input.property_target.clone() {
                        Some(target) => {
                            chronos_domain::property::PropertyObservationSource::PropertyValue {
                                target,
                            }
                        }
                        None => {
                            // Missing target: short-circuit with the
                            // original "requires property_target" verdict
                            // before invoking the domain function.
                            return Ok(HypothesisOutput::Invariant {
                                verdict: HypothesisVerdict::Unsupported {
                                    reason: "scope=property_value requires property_target".into(),
                                },
                                support_event_ids: Vec::new(),
                                counter_event_ids: Vec::new(),
                                scope: HypothesisScope::PropertyValue,
                                summary: "scope=property_value requires property_target to be set"
                                    .into(),
                            });
                        }
                    },
                    HypothesisScope::LatencyMs => {
                        chronos_domain::property::PropertyObservationSource::LatencyMs
                    }
                };
                let outcome = chronos_domain::property::eval_invariant(
                    &events,
                    input.comparison.unwrap_or(ComparisonOp::Eq),
                    input.constant.clone().unwrap_or(PropertyValue::Number(0.0)),
                    observation,
                );
                Ok(HypothesisOutput::from(outcome))
            }
            HypothesisKind::Existence => {
                // Translate the services-layer ExistencePredicate into the
                // domain-owned PropertyExistencePredicate (m9-80 T2). The
                // mapping is direct for EventTypeEquals / ThreadEquals /
                // PropertyKeyEquals; anything else falls back to
                // EventTypeEquals function_entry (matches the original
                // default predicate when input.predicate is None).
                let domain_predicate = match input.predicate.clone().unwrap_or_else(|| {
                    ExistencePredicate::EventTypeEquals {
                        event_type: "function_entry".into(),
                    }
                }) {
                    ExistencePredicate::EventTypeEquals { event_type } => {
                        chronos_domain::property::PropertyExistencePredicate::EventTypeEquals {
                            event_type,
                        }
                    }
                    ExistencePredicate::ThreadEquals { thread_id } => {
                        chronos_domain::property::PropertyExistencePredicate::ThreadEquals {
                            thread_id,
                        }
                    }
                    ExistencePredicate::PropertyKeyEquals { target } => {
                        chronos_domain::property::PropertyExistencePredicate::PropertyKeyEquals {
                            target,
                        }
                    }
                };
                let outcome = chronos_domain::property::eval_existence(&events, domain_predicate);
                Ok(HypothesisOutput::from(outcome))
            }
            HypothesisKind::CallPath => {
                let caller = input.caller.clone().unwrap_or_default();
                let callee = input.callee.clone().unwrap_or_default();
                let max_depth = input.max_depth.unwrap_or(10);
                let outcome =
                    chronos_domain::property::eval_call_path(&events, caller, callee, max_depth);
                Ok(HypothesisOutput::from(outcome))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Invariant
// ---------------------------------------------------------------------------

// outcome_to_envelope removed: PropertyOutcome → HypothesisVerdict conversion
// is no longer used (T1 moved Invariant + its helpers to domain, and T2/T3
// similarly bypass this bridge by returning domain outcomes converted via
// From impls in output.rs).

// observe_property_target and parse_property_value live in
// chronos_domain::property (T1). services calls them indirectly via
// chronos_domain::property::eval_invariant.

// ---------------------------------------------------------------------------
// Existence
// ---------------------------------------------------------------------------

// fn eval_existence, fn scan_predicate, fn predicate_label, fn event_type_label
// all moved to chronos_domain::property in T2.

// ---------------------------------------------------------------------------
// CallPath
// ---------------------------------------------------------------------------

// fn eval_call_path and fn bfs_reach moved to chronos_domain::property in T3.

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::trace::{EventData, EventType, SourceLocation, TraceEvent};
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
            HypothesisOutput::Invariant {
                verdict,
                support_event_ids,
                ..
            } => {
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
            HypothesisOutput::Existence {
                verdict,
                support_event_ids,
                ..
            } => {
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
            HypothesisOutput::Existence {
                verdict,
                support_event_ids,
                ..
            } => {
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
            HypothesisOutput::CallPath {
                verdict,
                reachable_path,
                ..
            } => {
                assert_eq!(verdict, HypothesisVerdict::Pass);
                let path = reachable_path.expect("path on Pass");
                assert_eq!(
                    path,
                    vec!["main".to_string(), "a".to_string(), "b".to_string()]
                );
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
            HypothesisOutput::CallPath {
                verdict,
                reachable_path,
                ..
            } => {
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
