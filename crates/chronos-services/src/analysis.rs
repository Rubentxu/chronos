//! M3 algorithms extracted from `chronos-mcp::server`: `mutation_lens` and
//! `causal_slice`.
//!
//! Both tools still own the public surface; this module owns the algorithm.
//! The MCP wrappers at `crates/chronos-mcp/src/server.rs` only:
//!   1. read params + build `AnalysisContext`,
//!   2. dispatch to `ChronosAnalysisService::*`,
//!   3. map `ServiceError` back to MCP error text.

use std::collections::HashMap;

use chronos_domain::{
    causal_slice::{slice_from, CausalEdge, EvidenceNodeId},
    property::{PropertyValue, StateTransition},
    EventData, VariableInfo,
};
use chronos_query::QueryEngine;
use tokio::sync::Mutex as TokioMutex;

use crate::error::ServiceError;
use crate::output::{CausalSliceOutput, MutationLensOutput};

/// Borrowed handle to the live engine map (shared with the MCP server).
///
/// The map is owned by `chronos-mcp::Server`; the service holds a reference
/// for the duration of the call. No locking past the await point.
///
/// Note: matches the established pattern in `chronos_services::sessions` and
/// `chronos_services::debug_trace` — the value type is `QueryEngine` (no
/// inner `Arc`); `Arc`-wrapping happens at the call site.
pub struct AnalysisContext<'a> {
    pub engines: &'a TokioMutex<HashMap<String, QueryEngine>>,
}

#[derive(Debug, Clone)]
pub struct MutationLensInput {
    pub session_id: String,
    pub target: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct CausalSliceInput {
    pub session_id: String,
    pub sink_event_id: u64,
}

/// Stateless holder for analysis algorithms.
///
/// The methods are `async` only to acquire the engine-map lock; they release
/// it as soon as the engine reference is in scope. Callers must drop the
/// returned outputs before any subsequent call that needs the map.
pub struct ChronosAnalysisService;

impl ChronosAnalysisService {
    /// Collect consecutive `EventData::Variable` writes per target, emitting
    /// one `StateTransition` per (prev, current) pair.
    ///
    /// Behaviour matches the legacy tool wrapper:
    ///   - `target` filter is optional (None == all variables).
    ///   - Default `limit` is 100; clamped after collection.
    ///   - A transition is only emitted when there is a *previous* write for
    ///     the same variable; the first write per variable is stored but not
    ///     emitted (no "before" available).
    pub async fn mutation_lens(
        ctx: &AnalysisContext<'_>,
        input: MutationLensInput,
    ) -> Result<MutationLensOutput, ServiceError> {
        let engines = ctx.engines.lock().await;
        let engine = engines
            .get(&input.session_id)
            .ok_or_else(|| ServiceError::SessionNotFound(input.session_id.clone()))?;

        let limit = input.limit.unwrap_or(100);

        let events = engine.events();
        let mut pending: HashMap<String, (VariableInfo, usize)> = HashMap::new();
        let mut transitions: Vec<StateTransition> = Vec::new();

        for (idx, event) in events.iter().enumerate() {
            if let EventData::Variable(var) = &event.data {
                let target_filter = input.target.as_ref();
                let matches = target_filter.is_none_or(|t| &var.name == t);
                if !matches {
                    continue;
                }

                if let Some((prev, _)) = pending.remove(&var.name) {
                    transitions.push(StateTransition {
                        target: prev.name.clone(),
                        before: Some(PropertyValue::Text(prev.value.clone())),
                        after: PropertyValue::Text(var.value.clone()),
                        index: idx,
                        actor: None,
                    });
                }
                pending.insert(var.name.clone(), (var.clone(), idx));
            }
        }

        if transitions.len() > limit {
            transitions.truncate(limit);
        }

        Ok(MutationLensOutput {
            session_id: input.session_id,
            count: transitions.len(),
            transitions,
        })
    }

    /// Build thread-consecutive `CausalEdge`s + observed map, then delegate to
    /// `chronos_domain::causal_slice::slice_from`.
    ///
    /// Returns:
    ///   - `included`: backward-reachable event ids (including sink itself).
    ///   - `missing`: included ids whose evidence is unobserved (never silently
    ///     dropped — surfaced to the caller).
    ///   - `depth`: `included.len()` (legacy semantic).
    pub async fn causal_slice(
        ctx: &AnalysisContext<'_>,
        input: CausalSliceInput,
    ) -> Result<CausalSliceOutput, ServiceError> {
        let engines = ctx.engines.lock().await;
        let engine = engines
            .get(&input.session_id)
            .ok_or_else(|| ServiceError::SessionNotFound(input.session_id.clone()))?;

        if engine.get_event_by_id(input.sink_event_id).is_none() {
            return Err(ServiceError::EventNotFound {
                event_id: input.sink_event_id,
            });
        }

        // Edges: consecutive events on the same thread.
        let mut prev_per_thread: HashMap<u64, u64> = HashMap::new();
        let mut edges: Vec<CausalEdge> = Vec::new();
        for ev in engine.events() {
            let curr_id = ev.event_id;
            if let Some(&prev_id) = prev_per_thread.get(&ev.thread_id) {
                edges.push(CausalEdge {
                    from: EvidenceNodeId(prev_id),
                    to: EvidenceNodeId(curr_id),
                });
            }
            prev_per_thread.insert(ev.thread_id, curr_id);
        }

        // All events observed.
        let mut observed: HashMap<EvidenceNodeId, bool> = HashMap::new();
        for ev in engine.events() {
            observed.insert(EvidenceNodeId(ev.event_id), true);
        }

        let slice = slice_from(&edges, &observed, EvidenceNodeId(input.sink_event_id));

        let included: Vec<u64> = slice.included.iter().map(|id| id.0).collect();
        let missing: Vec<u64> = slice.missing.iter().map(|id| id.0).collect();

        Ok(CausalSliceOutput {
            session_id: input.session_id,
            sink: input.sink_event_id,
            included,
            missing,
            depth: slice.included.len(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::{
        property::PropertyValue, EventData, EventType, SourceLocation, TraceEvent,
    };
    use chronos_query::QueryEngine;
    use std::collections::HashMap;

    fn dummy_event(id: u64, thread: u64, name: &str, value: &str) -> TraceEvent {
        TraceEvent {
            event_id: id,
            timestamp_ns: id * 1000,
            thread_id: thread,
            event_type: EventType::VariableWrite,
            location: SourceLocation::default(),
            data: EventData::Variable(VariableInfo {
                name: name.to_string(),
                value: value.to_string(),
                type_name: "String".to_string(),
                address: 0,
                scope: chronos_domain::VariableScope::Local,
            }),
        }
    }

    fn engine_with(events: Vec<TraceEvent>) -> QueryEngine {
        QueryEngine::new(events)
    }

    #[tokio::test]
    async fn mutation_lens_returns_session_not_found() {
        let map: HashMap<String, QueryEngine> = HashMap::new();
        let mutex = TokioMutex::new(map);
        let ctx = AnalysisContext { engines: &mutex };

        let result = ChronosAnalysisService::mutation_lens(
            &ctx,
            MutationLensInput {
                session_id: "absent".into(),
                target: None,
                limit: None,
            },
        )
        .await;

        assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
    }

    #[tokio::test]
    async fn mutation_lens_emits_transitions_for_consecutive_writes() {
        let engine = engine_with(vec![
            dummy_event(1, 1, "x", "1"),
            dummy_event(2, 1, "x", "2"),
            dummy_event(3, 1, "y", "9"),
            dummy_event(4, 1, "x", "3"),
        ]);

        let mut map: HashMap<String, QueryEngine> = HashMap::new();
        map.insert("s".to_string(), engine);
        let mutex = TokioMutex::new(map);
        let ctx = AnalysisContext { engines: &mutex };

        let result = ChronosAnalysisService::mutation_lens(
            &ctx,
            MutationLensInput {
                session_id: "s".into(),
                target: None,
                limit: None,
            },
        )
        .await
        .unwrap();

        // x: (1->2), (2->3); y: (none -> 9, no transition)
        assert_eq!(result.count, 2);
        assert_eq!(result.transitions[0].target, "x");
        assert_eq!(result.transitions[0].before, Some(PropertyValue::Text("1".into())));
        assert_eq!(result.transitions[0].after, PropertyValue::Text("2".into()));
        assert_eq!(result.transitions[1].target, "x");
        assert_eq!(result.transitions[1].before, Some(PropertyValue::Text("2".into())));
        assert_eq!(result.transitions[1].after, PropertyValue::Text("3".into()));
    }

    #[tokio::test]
    async fn mutation_lens_target_filter_isolates_one_variable() {
        let engine = engine_with(vec![
            dummy_event(1, 1, "x", "1"),
            dummy_event(2, 1, "y", "2"),
            dummy_event(3, 1, "x", "3"),
        ]);

        let mut map: HashMap<String, QueryEngine> = HashMap::new();
        map.insert("s".to_string(), engine);
        let mutex = TokioMutex::new(map);
        let ctx = AnalysisContext { engines: &mutex };

        let result = ChronosAnalysisService::mutation_lens(
            &ctx,
            MutationLensInput {
                session_id: "s".into(),
                target: Some("x".into()),
                limit: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(result.count, 1);
        assert_eq!(result.transitions[0].target, "x");
    }

    #[tokio::test]
    async fn mutation_lens_truncates_to_limit() {
        let events: Vec<TraceEvent> = (0..10u64)
            .map(|i| dummy_event(i, 1, "x", &format!("v{i}")))
            .collect();
        let engine = engine_with(events);

        let mut map: HashMap<String, QueryEngine> = HashMap::new();
        map.insert("s".to_string(), engine);
        let mutex = TokioMutex::new(map);
        let ctx = AnalysisContext { engines: &mutex };

        let result = ChronosAnalysisService::mutation_lens(
            &ctx,
            MutationLensInput {
                session_id: "s".into(),
                target: None,
                limit: Some(3),
            },
        )
        .await
        .unwrap();

        assert_eq!(result.count, 3);
        assert_eq!(result.transitions.len(), 3);
    }

    #[tokio::test]
    async fn causal_slice_returns_event_not_found() {
        let engine = engine_with(vec![dummy_event(1, 1, "x", "1")]);

        let mut map: HashMap<String, QueryEngine> = HashMap::new();
        map.insert("s".to_string(), engine);
        let mutex = TokioMutex::new(map);
        let ctx = AnalysisContext { engines: &mutex };

        let result = ChronosAnalysisService::causal_slice(
            &ctx,
            CausalSliceInput {
                session_id: "s".into(),
                sink_event_id: 999,
            },
        )
        .await;

        match result {
            Err(ServiceError::EventNotFound { event_id }) => assert_eq!(event_id, 999),
            other => panic!("expected EventNotFound, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn causal_slice_includes_thread_consecutive_events() {
        let engine = engine_with(vec![
            dummy_event(1, 1, "x", "1"),
            dummy_event(2, 2, "y", "2"),
            dummy_event(3, 1, "x", "3"),
            dummy_event(4, 2, "y", "4"),
        ]);

        let mut map: HashMap<String, QueryEngine> = HashMap::new();
        map.insert("s".to_string(), engine);
        let mutex = TokioMutex::new(map);
        let ctx = AnalysisContext { engines: &mutex };

        let result = ChronosAnalysisService::causal_slice(
            &ctx,
            CausalSliceInput {
                session_id: "s".into(),
                sink_event_id: 4,
            },
        )
        .await
        .unwrap();

        // Thread 2 edges: 2->4. Backward from 4: {2, 4}.
        assert_eq!(result.sink, 4);
        assert_eq!(result.depth, 2);
        assert!(result.included.contains(&2));
        assert!(result.included.contains(&4));
        assert!(result.missing.is_empty());
    }

    #[tokio::test]
    async fn causal_slice_includes_only_sink_when_no_thread_history() {
        let engine = engine_with(vec![dummy_event(1, 1, "x", "1")]);

        let mut map: HashMap<String, QueryEngine> = HashMap::new();
        map.insert("s".to_string(), engine);
        let mutex = TokioMutex::new(map);
        let ctx = AnalysisContext { engines: &mutex };

        let result = ChronosAnalysisService::causal_slice(
            &ctx,
            CausalSliceInput {
                session_id: "s".into(),
                sink_event_id: 1,
            },
        )
        .await
        .unwrap();

        assert_eq!(result.depth, 1);
        assert_eq!(result.included, vec![1]);
    }
}
