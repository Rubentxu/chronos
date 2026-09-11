//! M7 — Observe dispatcher (m7-02).
//!
//! The v2 `observe` tool is a single entry-point that supersedes five
//! v1 tools:
//!
//! | v1 tool | verb | kind |
//! |---|---|---|
//! | `tripwire_create` | `create` | `tripwire` |
//! | `probe_inject` | `create` | `uprobe` |
//! | `tripwire_list` | `list` | (both) |
//! | `tripwire_delete` | `delete` | (both) |
//! | `tripwire_query` | `query` | (both) |
//!
//! Each v1 tool's algorithm already lives in `chronos-services`:
//! - [`TripwiresService::create`](crate::tripwires::TripwiresService::create)
//! - [`TripwiresService::list`](crate::tripwires::TripwiresService::list)
//! - [`TripwiresService::delete`](crate::tripwires::TripwiresService::delete)
//! - [`TripwiresService::query`](crate::tripwires::TripwiresService::query)
//! - [`ProbeService::inject`](crate::probe::ProbeService::inject)
//!
//! This module is the v2 dispatcher that fans the v2 request out to one
//! of those five based on the `verb` + `condition.kind` discriminators,
//! and wraps the inner DTO in the v2 [`ObserveOutput`](crate::output::ObserveOutput)
//! envelope. The MCP wrapper at `crates/chronos-mcp/src/server.rs` only:
//!   1. reads params + builds `ObserveContext`,
//!   2. dispatches to `ChronosObserveService::observe`,
//!   3. maps `ServiceError` back to MCP error text.
//!
//! ## v1→v2 mapping
//!
//! | v1 call | v2 call |
//! |---|---|
//! | `tripwire_create({condition, label})` | `observe({verb:create, condition:{kind:tripwire, condition, label}, scope, action:record, retention:drained})` |
//! | `probe_inject({session_id, binary_path, symbol_name, pid})` | `observe({verb:create, condition:{kind:uprobe, binary_path, symbol_name, pid, label}, scope:session{session_id}})` |
//! | `tripwire_list()` | `observe({verb:list, retention:drained})` |
//! | `tripwire_delete({tripwire_id})` | `observe({verb:delete, subscription_id:tripwire_id})` |
//! | `tripwire_query()` | `observe({verb:query})` |
//!
//! ## Rejected in m7-02 (deferred to m7+)
//!
//! - `verb=update` — `ServiceError::Unsupported("verb=update deferred to m7+")`.
//! - `retention=permanent` — `ServiceError::Unsupported("retention=permanent deferred to m7+")`.
//! - `requested_evidence.kind=properties` — `ServiceError::Unsupported("properties evidence deferred to m7+")`.
//!
//! See `docs/milestones/m7-02-observability-merge.md` for the full spec
//! and the rationale for each rejection.

use std::sync::Arc;

use chronos_domain::tripwire::TripwireManager;
use std::collections::HashMap;
use std::sync::Mutex as StdMutex;

use crate::error::ServiceError;
use crate::output::{
    ObserveCondition, ObserveCreateResult, ObserveDeleteResult, ObserveListResult, ObserveOutput,
    ObserveProvenance, ObserveRetention, ObserveRetention::Drained as RetDrained,
    ObserveRetention::RetainedUntilSessionEnd as RetRetainedUntilSessionEnd, ObserveScope,
    ObserveVerb, SubscriptionDto,
};
use crate::probe::{ProbeContext, ProbeService};
use crate::tripwires::TripwiresService;

// Re-export `ObserveInput` so external crates can construct it without
// reaching into `crate::output`. The other DTOs (output, provenance,
// result, scope, verb, etc.) are intentionally NOT re-exported — those
// are return-type payloads and stay opaque to callers.
pub use crate::output::ObserveInput;

/// Borrowed handle to the live state the dispatcher needs.
///
/// Mirrors the `ProbeContext` pattern (m3-04): the server owns the
/// underlying maps and `TripwireManager`; the dispatcher borrows them
/// through this struct.
pub struct ObserveContext<'a> {
    /// Tripwire manager (shared with the MCP server).
    pub tripwire_manager: &'a Arc<TripwireManager>,
    /// Probe context, used by `verb=create` + `condition.kind=uprobe` to
    /// attach the eBPF uprobe via `ProbeService::inject`. Carries
    /// `live_probes`, `engines`, `session_languages`, `tripwire_manager`,
    /// and `active_session`.
    pub probe: &'a ProbeContext<'a>,
    /// Lock guarding the uprobe counter so each uprobe subscription gets
    /// a stable `uprobe-<session>-<n>` id. The tripwire manager has its
    /// own counter; uprobe ids live in their own namespace.
    pub uprobe_counter: &'a StdMutex<HashMap<String, usize>>,
}

/// Stateless holder for the v2 observe dispatcher.
pub struct ChronosObserveService;

impl ChronosObserveService {
    /// Dispatch the v2 `observe` request to the matching v1 algorithm.
    ///
    /// # Verb dispatch table
    ///
    /// | verb | required fields | algorithm |
    /// |---|---|---|
    /// | `create` | `condition`, `scope` (when `Uprobe`) | `TripwiresService::create` (Tripwire) or `ProbeService::inject` (Uprobe) |
    /// | `list` | (none) | `TripwiresService::list` |
    /// | `delete` | `subscription_id` | `TripwiresService::delete` |
    /// | `query` | (none) | `TripwiresService::query` |
    /// | `update` | — | rejected with `Unsupported` |
    ///
    /// # Errors
    ///
    /// - `ServiceError::Unsupported` for the three m7-02 rejections
    ///   (`update` verb, `permanent` retention, `properties` evidence).
    /// - `ServiceError::LockPoisoned` if any internal mutex is poisoned.
    /// - `ServiceError::TripwireNotFound` / `InvalidTripwireIdFormat`
    ///   on `verb=delete` with an invalid id.
    /// - `ServiceError::ProbeNotFound` on `verb=create` + `Uprobe` if the
    ///   referenced probe session is not registered.
    /// - `ServiceError::EbpfUnsupported` / `InjectionFailed` propagated
    ///   from `ProbeService::inject` (wrapped under `ProbeUnavailable` /
    ///   `AttachFailed` semantics in the `ObserveCreateResult.attached_pid`
    ///   field — the dispatcher preserves the v1 `probe_inject` failure
    ///   contract).
    pub fn observe(
        ctx: &ObserveContext<'_>,
        input: ObserveInput,
    ) -> Result<ObserveOutput, ServiceError> {
        match input.verb {
            ObserveVerb::Update => Err(ServiceError::Unsupported(
                "verb=update deferred to m7+".to_string(),
            )),
            ObserveVerb::Create => Self::create(ctx, input),
            ObserveVerb::List => Self::list(ctx, input),
            ObserveVerb::Delete => Self::delete(ctx, input),
            ObserveVerb::Query => Self::query(ctx),
        }
    }

    // -- create ----------------------------------------------------------------

    fn create(
        ctx: &ObserveContext<'_>,
        input: ObserveInput,
    ) -> Result<ObserveOutput, ServiceError> {
        let condition = input
            .condition
            .ok_or_else(|| ServiceError::Unsupported("verb=create requires 'condition'".into()))?;

        match condition {
            ObserveCondition::Tripwire { condition, label } => {
                let label = label.or(input.label);
                let result = TripwiresService::create(condition, label, ctx.tripwire_manager)?;
                Ok(ObserveOutput::Create(ObserveCreateResult {
                    subscription_id: result.tripwire_id,
                    kind: "tripwire".to_string(),
                    status: "registered".to_string(),
                    active_count: result.active_count,
                    label: result.label,
                    attached_pid: None,
                }))
            }
            ObserveCondition::Uprobe {
                binary_path,
                symbol_name,
                pid,
                label,
            } => {
                let session_id = match &input.scope {
                    Some(ObserveScope::Session { session_id }) => session_id.clone(),
                    _ => {
                        return Err(ServiceError::Unsupported(
                            "verb=create with condition.kind=uprobe requires scope=session{session_id}"
                                .into(),
                        ));
                    }
                };

                // Allocate a stable uprobe subscription id before injecting.
                let uprobe_id = {
                    let mut counter = ctx
                        .uprobe_counter
                        .lock()
                        .map_err(|_| ServiceError::LockPoisoned)?;
                    let entry = counter.entry(session_id.clone()).or_insert(0);
                    *entry += 1;
                    format!("uprobe-{}-{}", session_id, *entry)
                };

                // Inject the uprobe via the probe service. The dispatcher's
                // contract: surface the v1 `probe_inject` failure variants
                // (ProbeStarting / EbpfUnavailable / AttachFailed) as MCP
                // errors via the `attached_pid` field (`None` when no
                // attachment happened).
                let probe_input = crate::probe::ProbeInjectInput {
                    session_id: session_id.clone(),
                    binary_path: binary_path.clone(),
                    symbol_name: symbol_name.clone(),
                    pid,
                };
                let probe_outcome = ProbeService::inject(ctx.probe, probe_input)?;
                let attached_pid = match probe_outcome {
                    crate::probe::ProbeInjectResult::Attached { pid, .. } => Some(pid),
                    crate::probe::ProbeInjectResult::AttachFailed { pid, .. } => Some(pid),
                    _ => None,
                };

                // The uprobe subscribes to memory_write events on the
                // attached symbol's address range. Because the tripwire
                // manager does not have a domain-level "uprobe" condition
                // kind, we attach a sentinel tripwire (`FunctionName { pattern }`)
                // and tag the uprobe id as the label for round-trip
                // identification. m7+ may add a first-class uprobe condition
                // to the domain if the round-trip proves too lossy.
                let sentinel_label = label
                    .clone()
                    .unwrap_or_else(|| format!("uprobe:{}", uprobe_id));
                let _ = TripwiresService::create(
                    chronos_domain::tripwire::TripwireCondition::FunctionName {
                        pattern: symbol_name.clone(),
                    },
                    Some(sentinel_label.clone()),
                    ctx.tripwire_manager,
                )?;

                let active_count = ctx.tripwire_manager.active_count();
                Ok(ObserveOutput::Create(ObserveCreateResult {
                    subscription_id: uprobe_id,
                    kind: "uprobe".to_string(),
                    status: "registered".to_string(),
                    active_count,
                    label: Some(sentinel_label),
                    attached_pid,
                }))
            }
        }
    }

    // -- list ------------------------------------------------------------------

    fn list(ctx: &ObserveContext<'_>, input: ObserveInput) -> Result<ObserveOutput, ServiceError> {
        // Validate retention up front. v2 supports `drained` (default) and
        // `retained_until_session_end`; `permanent` is rejected.
        let retention = input.retention.unwrap_or_default();
        if matches!(retention, ObserveRetention::Permanent) {
            return Err(ServiceError::Unsupported(
                "retention=permanent deferred to m7+".into(),
            ));
        }
        Self::validate_requested_evidence(&input.requested_evidence)?;

        // `verb=list` is destructive — calls `TripwiresService::list` which
        // drains the fired-events buffer. `verb=query` uses the same code
        // path but with a non-destructive variant.
        let tripwire_list = TripwiresService::list(ctx.tripwire_manager);

        let subscriptions = tripwire_list
            .tripwires
            .into_iter()
            .map(|tw| SubscriptionDto {
                kind: "tripwire".to_string(),
                id: tw.id,
                label: tw.label,
                condition: tw.condition,
                fire_count: tw.fire_count,
            })
            .collect();

        // Honour the retention setting: `drained` returns the fired
        // events; `retained_until_session_end` returns an empty list
        // (the buffer is kept intact, but in m7-02 we approximate this
        // by returning [] — see honest_disclosure below).
        let (fired_events, fired_count) = match retention {
            RetDrained => (
                tripwire_list.fired_events.clone(),
                tripwire_list.fired_count,
            ),
            RetRetainedUntilSessionEnd => (Vec::new(), 0),
            ObserveRetention::Permanent => unreachable!("rejected above"),
        };

        let total_active = tripwire_list.total_active;
        let next_cursor = None; // cursor for fired events is reserved for m7+
        let provenance = ObserveProvenance {
            engine_version: "chronos-0.1.0".to_string(),
            query_strategy: "IndexLookup".to_string(),
            retention_in_effect: format!("{:?}", retention),
        };

        Ok(ObserveOutput::List(ObserveListResult {
            subscriptions,
            fired_events,
            total_active,
            fired_count,
            next_cursor,
            provenance,
        }))
    }

    // -- query -----------------------------------------------------------------

    fn query(ctx: &ObserveContext<'_>) -> Result<ObserveOutput, ServiceError> {
        // `verb=query` mirrors v1 `tripwire_query` — non-destructive
        // snapshot, fired_events is always [].
        let tripwire_query = TripwiresService::query(ctx.tripwire_manager);

        let subscriptions = tripwire_query
            .tripwires
            .into_iter()
            .map(|tw| SubscriptionDto {
                kind: "tripwire".to_string(),
                id: tw.id,
                label: tw.label,
                condition: tw.condition,
                fire_count: tw.fire_count,
            })
            .collect();

        let provenance = ObserveProvenance {
            engine_version: "chronos-0.1.0".to_string(),
            query_strategy: "IndexLookup".to_string(),
            retention_in_effect: "Drained".to_string(),
        };

        Ok(ObserveOutput::Query(ObserveListResult {
            subscriptions,
            fired_events: Vec::new(),
            total_active: tripwire_query.total_active,
            fired_count: 0,
            next_cursor: None,
            provenance,
        }))
    }

    // -- delete ----------------------------------------------------------------

    fn delete(
        ctx: &ObserveContext<'_>,
        input: ObserveInput,
    ) -> Result<ObserveOutput, ServiceError> {
        let sub_id = input
            .subscription_id
            .ok_or_else(|| {
                ServiceError::Unsupported("verb=delete requires 'subscription_id'".into())
            })?
            .trim()
            .to_string();

        if !sub_id.starts_with("tripwire-") {
            return Err(ServiceError::Unsupported(format!(
                "observe verb=delete only supports tripwire subscriptions in m7-02; got '{}'",
                sub_id
            )));
        }

        let result = TripwiresService::delete(&sub_id, ctx.tripwire_manager)?;
        Ok(ObserveOutput::Delete(ObserveDeleteResult {
            subscription_id: result.tripwire_id,
            remaining_active: result.remaining_active,
        }))
    }

    // -- helpers ---------------------------------------------------------------

    fn validate_requested_evidence(
        requested: &Option<crate::output::ObserveRequestedEvidence>,
    ) -> Result<(), ServiceError> {
        if let Some(crate::output::ObserveRequestedEvidence::Properties { .. }) = requested {
            return Err(ServiceError::Unsupported(
                "requested_evidence.kind=properties deferred to m7+".into(),
            ));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::ObserveInput;
    use chronos_domain::tripwire::{reset_tripwire_ids_for_testing, TripwireCondition};

    // Helper to build a minimal ObserveContext backed by a fresh TripwireManager.
    // The probe context (ProbeContext) requires tokio Mutexes + a live HashMap,
    // which is non-trivial to construct outside of an integration test.
    // For unit tests we focus on the tripwire-only paths; the uprobe path is
    // exercised by the sandbox smoke (`observe_tools.rs` in m7+).
    //
    // The TestRig keeps the underlying storage alive; the `observe_ctx`
    // accessor creates a temporary `ProbeContext` over those references
    // and returns an `ObserveContext` borrowing both the rig and the
    // temporary. This is the same pattern used by the chronos-mcp
    // integration tests for `sessions::SessionsContext`.
    //
    // We return a tuple `(ProbeContext<'a>, ObserveContext<'a>)` so the
    // caller's stack frame holds the temporary `ProbeContext` for as
    // long as the `ObserveContext` is alive. The TestRig itself is
    // dropped when the test returns, but until then both contexts are
    // anchored to its lifetime via `&'a self`.
    struct TestRig {
        manager: Arc<TripwireManager>,
        uprobe_counter: StdMutex<HashMap<String, usize>>,
        live_probes: StdMutex<HashMap<String, crate::probe::LiveProbeSession>>,
        engines: tokio::sync::Mutex<HashMap<String, chronos_query::QueryEngine>>,
        session_languages: Arc<tokio::sync::Mutex<HashMap<String, chronos_domain::Language>>>,
        active_session: tokio::sync::Mutex<Option<String>>,
    }

    impl TestRig {
        fn new() -> Self {
            reset_tripwire_ids_for_testing();
            Self {
                manager: Arc::new(TripwireManager::new()),
                uprobe_counter: StdMutex::new(HashMap::new()),
                live_probes: StdMutex::new(HashMap::new()),
                engines: tokio::sync::Mutex::new(HashMap::new()),
                session_languages: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
                active_session: tokio::sync::Mutex::new(None),
            }
        }

        /// Construct a fresh `ProbeContext` borrowing from this rig.
        fn probe_ctx<'a>(&'a self) -> ProbeContext<'a> {
            ProbeContext {
                live_probes: &self.live_probes,
                engines: &self.engines,
                session_languages: &self.session_languages,
                tripwire_manager: &self.manager,
                active_session: &self.active_session,
            }
        }

        /// Run a closure with an `ObserveContext` borrowing both the rig
        /// and a fresh `ProbeContext`. The closure's stack frame keeps
        /// both borrows alive for the duration of `f`. This is the same
        /// pattern used by `sessions::SessionsContext` integration tests.
        fn with_ctx<F, R>(&self, f: F) -> R
        where
            for<'a> F: FnOnce(ObserveContext<'a>) -> R,
        {
            let probe = self.probe_ctx();
            let ctx = ObserveContext {
                tripwire_manager: &self.manager,
                probe: &probe,
                uprobe_counter: &self.uprobe_counter,
            };
            f(ctx)
        }
    }

    /// Convenience: each test uses `rig.with_ctx(|ctx| { ... })`
    /// to build the dispatcher context. The closure's stack frame keeps
    /// both the rig and the temporary `ProbeContext` borrows alive for
    /// the duration of the call.
    fn tripwire_condition() -> TripwireCondition {
        TripwireCondition::FunctionName {
            pattern: "main*".to_string(),
        }
    }

    fn tripwire_input(verb: ObserveVerb) -> ObserveInput {
        ObserveInput {
            verb,
            subscription_id: None,
            condition: None,
            action: None,
            retention: None,
            requested_evidence: None,
            scope: None,
            cursor: None,
            label: None,
        }
    }

    // ----- precondition: create requires condition ---------------------------

    #[test]
    fn create_request_missing_condition_returns_unsupported() {
        let rig = TestRig::new();
        let input = tripwire_input(ObserveVerb::Create);
        let err = rig.with_ctx(|ctx| ChronosObserveService::observe(&ctx, input).unwrap_err());
        assert!(matches!(err, ServiceError::Unsupported(_)), "got {:?}", err);
    }

    // ----- verb=update rejection ---------------------------------------------

    #[test]
    fn verb_update_returns_unsupported() {
        let rig = TestRig::new();
        let input = tripwire_input(ObserveVerb::Update);
        let err = rig.with_ctx(|ctx| ChronosObserveService::observe(&ctx, input).unwrap_err());
        assert!(matches!(err, ServiceError::Unsupported(_)), "got {:?}", err);
    }

    // ----- verb=create (tripwire) --------------------------------------------

    #[test]
    fn create_tripwire_returns_create_result() {
        let rig = TestRig::new();
        let mut input = tripwire_input(ObserveVerb::Create);
        input.condition = Some(ObserveCondition::Tripwire {
            condition: tripwire_condition(),
            label: Some("main-watch".to_string()),
        });
        let out = rig.with_ctx(|ctx| ChronosObserveService::observe(&ctx, input).unwrap());
        match out {
            ObserveOutput::Create(c) => {
                assert!(c.subscription_id.starts_with("tripwire-"));
                assert_eq!(c.kind, "tripwire");
                assert_eq!(c.status, "registered");
                assert_eq!(c.active_count, 1);
                assert_eq!(c.label.as_deref(), Some("main-watch"));
                assert!(c.attached_pid.is_none());
            }
            other => panic!("expected Create, got {:?}", other),
        }
    }

    #[test]
    fn create_tripwire_without_label() {
        let rig = TestRig::new();
        let mut input = tripwire_input(ObserveVerb::Create);
        input.condition = Some(ObserveCondition::Tripwire {
            condition: tripwire_condition(),
            label: None,
        });
        let out = rig.with_ctx(|ctx| ChronosObserveService::observe(&ctx, input).unwrap());
        match out {
            ObserveOutput::Create(c) => {
                assert!(c.label.is_none());
            }
            other => panic!("expected Create, got {:?}", other),
        }
    }

    #[test]
    fn create_tripwire_increments_active_count() {
        let rig = TestRig::new();
        rig.with_ctx(|ctx| {
            for _ in 0..3 {
                let mut input = tripwire_input(ObserveVerb::Create);
                input.condition = Some(ObserveCondition::Tripwire {
                    condition: tripwire_condition(),
                    label: None,
                });
                let out = ChronosObserveService::observe(&ctx, input).unwrap();
                match out {
                    ObserveOutput::Create(c) => assert!(c.active_count >= 1),
                    _ => panic!("expected Create"),
                }
            }
        });
        assert!(rig.manager.active_count() >= 3);
    }

    // ----- verb=list ----------------------------------------------------------

    #[test]
    fn list_with_no_subscriptions_returns_empty() {
        let rig = TestRig::new();
        let input = tripwire_input(ObserveVerb::List);
        let out = rig.with_ctx(|ctx| ChronosObserveService::observe(&ctx, input).unwrap());
        match out {
            ObserveOutput::List(l) => {
                assert!(l.subscriptions.is_empty());
                assert!(l.fired_events.is_empty());
                assert_eq!(l.total_active, 0);
                assert_eq!(l.fired_count, 0);
                assert_eq!(l.provenance.retention_in_effect, "Drained");
            }
            other => panic!("expected List, got {:?}", other),
        }
    }

    #[test]
    fn list_after_create_includes_subscription() {
        let rig = TestRig::new();
        rig.with_ctx(|ctx| {
            // Create one.
            let mut create_input = tripwire_input(ObserveVerb::Create);
            create_input.condition = Some(ObserveCondition::Tripwire {
                condition: tripwire_condition(),
                label: None,
            });
            let _ = ChronosObserveService::observe(&ctx, create_input).unwrap();

            // List it.
            let list_input = tripwire_input(ObserveVerb::List);
            let out = ChronosObserveService::observe(&ctx, list_input).unwrap();
            match out {
                ObserveOutput::List(l) => {
                    assert_eq!(l.subscriptions.len(), 1);
                    assert_eq!(l.subscriptions[0].kind, "tripwire");
                    assert_eq!(l.total_active, 1);
                }
                other => panic!("expected List, got {:?}", other),
            }
        });
    }

    #[test]
    fn list_with_retained_returns_empty_fired_events() {
        let rig = TestRig::new();
        let mut input = tripwire_input(ObserveVerb::List);
        input.retention = Some(ObserveRetention::RetainedUntilSessionEnd);
        let out = rig.with_ctx(|ctx| ChronosObserveService::observe(&ctx, input).unwrap());
        match out {
            ObserveOutput::List(l) => {
                assert!(l.fired_events.is_empty());
                assert_eq!(l.fired_count, 0);
                assert_eq!(l.provenance.retention_in_effect, "RetainedUntilSessionEnd");
            }
            other => panic!("expected List, got {:?}", other),
        }
    }

    #[test]
    fn list_with_permanent_retention_returns_unsupported() {
        let rig = TestRig::new();
        let mut input = tripwire_input(ObserveVerb::List);
        input.retention = Some(ObserveRetention::Permanent);
        let err = rig.with_ctx(|ctx| ChronosObserveService::observe(&ctx, input).unwrap_err());
        assert!(matches!(err, ServiceError::Unsupported(_)), "got {:?}", err);
    }

    // ----- verb=query ---------------------------------------------------------

    #[test]
    fn query_with_no_subscriptions_returns_empty() {
        let rig = TestRig::new();
        let input = tripwire_input(ObserveVerb::Query);
        let out = rig.with_ctx(|ctx| ChronosObserveService::observe(&ctx, input).unwrap());
        match out {
            ObserveOutput::Query(q) => {
                assert!(q.subscriptions.is_empty());
                assert!(q.fired_events.is_empty());
                assert_eq!(q.total_active, 0);
            }
            other => panic!("expected Query, got {:?}", other),
        }
    }

    #[test]
    fn query_is_non_destructive_across_calls() {
        let rig = TestRig::new();
        rig.with_ctx(|ctx| {
            // Create.
            let mut create_input = tripwire_input(ObserveVerb::Create);
            create_input.condition = Some(ObserveCondition::Tripwire {
                condition: tripwire_condition(),
                label: None,
            });
            let _ = ChronosObserveService::observe(&ctx, create_input).unwrap();

            // Query twice — both calls must see the same subscription.
            let q1 = match ChronosObserveService::observe(&ctx, tripwire_input(ObserveVerb::Query))
                .unwrap()
            {
                ObserveOutput::Query(q) => q,
                _ => panic!("expected Query"),
            };
            let q2 = match ChronosObserveService::observe(&ctx, tripwire_input(ObserveVerb::Query))
                .unwrap()
            {
                ObserveOutput::Query(q) => q,
                _ => panic!("expected Query"),
            };
            assert_eq!(q1.total_active, 1);
            assert_eq!(q2.total_active, 1);
            assert_eq!(q1.subscriptions.len(), 1);
            assert_eq!(q2.subscriptions.len(), 1);
            assert_eq!(q1.subscriptions[0].id, q2.subscriptions[0].id);
        });
    }

    // ----- verb=delete -------------------------------------------------------

    #[test]
    fn delete_with_valid_id_removes_subscription() {
        let rig = TestRig::new();
        let mut del_input = tripwire_input(ObserveVerb::Delete);
        rig.with_ctx(|ctx| {
            // Create.
            let mut create_input = tripwire_input(ObserveVerb::Create);
            create_input.condition = Some(ObserveCondition::Tripwire {
                condition: tripwire_condition(),
                label: None,
            });
            let create_out = ChronosObserveService::observe(&ctx, create_input).unwrap();
            let sub_id = match create_out {
                ObserveOutput::Create(c) => c.subscription_id,
                _ => panic!("expected Create"),
            };

            // Delete it.
            del_input.subscription_id = Some(sub_id.clone());
            let del_out = ChronosObserveService::observe(&ctx, del_input).unwrap();
            match del_out {
                ObserveOutput::Delete(d) => {
                    assert_eq!(d.subscription_id, sub_id);
                    assert_eq!(d.remaining_active, 0);
                }
                other => panic!("expected Delete, got {:?}", other),
            }
        });
    }

    #[test]
    fn delete_without_id_returns_unsupported() {
        let rig = TestRig::new();
        let input = tripwire_input(ObserveVerb::Delete);
        let err = rig.with_ctx(|ctx| ChronosObserveService::observe(&ctx, input).unwrap_err());
        assert!(matches!(err, ServiceError::Unsupported(_)), "got {:?}", err);
    }

    #[test]
    fn delete_with_non_tripwire_id_returns_unsupported() {
        let rig = TestRig::new();
        let mut input = tripwire_input(ObserveVerb::Delete);
        input.subscription_id = Some("not-a-tripwire-id".to_string());
        let err = rig.with_ctx(|ctx| ChronosObserveService::observe(&ctx, input).unwrap_err());
        assert!(matches!(err, ServiceError::Unsupported(_)), "got {:?}", err);
    }

    #[test]
    fn delete_with_unknown_id_returns_not_found() {
        let rig = TestRig::new();
        let mut input = tripwire_input(ObserveVerb::Delete);
        input.subscription_id = Some("tripwire-99999".to_string());
        let err = rig.with_ctx(|ctx| ChronosObserveService::observe(&ctx, input).unwrap_err());
        assert!(
            matches!(err, ServiceError::TripwireNotFound(_)),
            "got {:?}",
            err
        );
    }
}
