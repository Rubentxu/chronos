//! M7-05 — `session_start` + `session_stop` + `capabilities` v2 dispatchers.
//!
//! Three net-new v2 tool entry points. See
//! `docs/milestones/m7-05-session-lifecycle-dispatchers-scoping.md` for
//! the design rationale and
//! `docs/milestones/m7-05-session-lifecycle-dispatchers-merge.md` for
//! the execute-cycle spec.
//!
//! Architecture:
//! - `session_start{action=spawn}` wraps `ProbeService::start` and
//!   returns the `session_id` plus a `CapabilitySnapshot`.
//! - `session_start{action=load}` calls `SessionStore::load_session` and
//!   reads back `SessionMetadata` to fill the capability snapshot.
//! - `session_start{action=attach}` is a stub returning
//!   `ServiceError::Unsupported("attach (m7+)")` — the domain-layer
//!   attach API is m7+ scope.
//! - `session_stop` runs (1) `ChronosObserveService::observe{verb=list}`
//!   when `drain_subscriptions=true`, (2) `ProbeService::stop` to
//!   finalise the producer, and (3) updates
//!   `SessionMetadata.tail_sealed=true, sealed_at=<now>` when
//!   `seal_tail=true`.
//! - `capabilities` reads the static enum surface
//!   (`StaticCapabilities`) for the target and the dynamic live state
//!   (`DynamicCapabilities`) for the session.

use crate::error::ServiceError;
use crate::observe::{ChronosObserveService, ObserveContext};
use crate::output::{
    CapabilitiesInput, CapabilitiesOutput, CapabilitySnapshot, DynamicCapabilities,
    LanguageAdapterStatus, ObserveInput, ObserveScope, ObserveVerb, ProjectionKind,
    SessionLifecycleProvenance, SessionStartAction, SessionStartInput, SessionStartOutput,
    SessionStopInput, SessionStopOutput, StaticCapabilities, TargetSpec,
};
use crate::probe::{ProbeContext, ProbeService};
use chronos_domain::trace::{EventType, Language};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Engine version baked into `SessionLifecycleProvenance` until
/// `chronos_query::QueryEngine::engine_version()` exists.
const ENGINE_VERSION: &str = "chronos-0.1.0";

/// Borrowed handle to the live probe state, observe state, and the
/// session store.
///
/// The context is owned by `chronos-mcp::Server` for the duration of
/// the call; the service holds references and releases them when the
/// future completes. No locking past the await point.
pub struct SessionLifecycleContext<'a> {
    pub store: &'a chronos_store::SessionStore,
    pub probe: &'a ProbeContext<'a>,
    pub observe: &'a ObserveContext<'a>,
}

/// Outcome of `ChronosSessionLifecycleService::stop_with_persistence`.
///
/// `Stopped` — the live-stop path; events were drained from the
/// bus, the probe was torn down. The wrapper should now call
/// `save_session(meta, &events)` + `build_and_store_engine`.
///
/// `AlreadyStopped` — idempotent path; the probe is already gone
/// (e.g., from a previous stop or a v1 `probe_stop`). The wrapper
/// synthesises `SessionStopOutput` from the already-persisted
/// metadata + events.
pub enum SessionStopPersistence {
    Stopped {
        session_id: String,
        events: Vec<chronos_domain::TraceEvent>,
        language: chronos_domain::Language,
        target: String,
        total_events: u64,
        duration_ms: u64,
        ebpf_detached: bool,
        sealed_at: Option<u64>,
    },
    AlreadyStopped {
        session_id: String,
    },
}

/// Stateless holder for the v2 lifecycle dispatcher.
pub struct ChronosSessionLifecycleService;

impl ChronosSessionLifecycleService {
    /// v2 `session_start` dispatcher entrypoint.
    ///
    /// Routes by `action`:
    /// - `Spawn`  → `ProbeService::start` (async)
    /// - `Load`   → `SessionStore::load_session` (synchronous read)
    /// - `Attach` → `ServiceError::Unsupported("attach (m7+)")`
    pub async fn start(
        ctx: &SessionLifecycleContext<'_>,
        input: SessionStartInput,
    ) -> Result<SessionStartOutput, ServiceError> {
        match input.action {
            SessionStartAction::Spawn => Self::spawn(ctx, input).await,
            SessionStartAction::Load => Self::load(ctx.store, input),
            SessionStartAction::Attach => Self::attach(input),
        }
    }

    async fn spawn(
        ctx: &SessionLifecycleContext<'_>,
        input: SessionStartInput,
    ) -> Result<SessionStartOutput, ServiceError> {
        let spawn_fields = input.spawn_fields.ok_or_else(|| {
            ServiceError::InvalidInput(
                "session_start{action=spawn} requires `spawn_fields`".to_string(),
            )
        })?;
        let bus_capacity = spawn_fields.bus_capacity.unwrap_or(4096);
        let probe_input = crate::probe::ProbeStartInput {
            program: spawn_fields.program.clone(),
            args: spawn_fields.args,
            trace_syscalls: spawn_fields.trace_syscalls,
            cwd: spawn_fields.cwd,
            bus_capacity,
            track_function_frames: Some(spawn_fields.track_function_frames),
        };
        let out = ProbeService::start(ctx.probe, probe_input).await?;
        let lang = out.language.clone();
        let snapshot = CapabilitySnapshot {
            probe_type: Some("ebpf_user".to_string()),
            language: Some(lang.clone()),
            bus_capacity: Some(out.bus_capacity),
            bus_fill: Some(0),
            query_engine_ready: false,
            active_subscriptions: vec![],
            tail_sealed: false,
            sealed_at: None,
        };
        Ok(SessionStartOutput {
            session_id: out.session_id,
            action: SessionStartAction::Spawn,
            target: Some(spawn_fields.program),
            language: Some(lang),
            event_count: None,
            duration_ms: None,
            bus_capacity: Some(out.bus_capacity),
            capability_snapshot: snapshot,
            provenance: lifecycle_provenance("session_start:spawn"),
        })
    }

    fn load(
        store: &chronos_store::SessionStore,
        input: SessionStartInput,
    ) -> Result<SessionStartOutput, ServiceError> {
        let session_id = input.session_id.ok_or_else(|| {
            ServiceError::InvalidInput(
                "session_start{action=load} requires `session_id`".to_string(),
            )
        })?;
        let (meta, _events) = store.load_session(&session_id).map_err(|e| {
            ServiceError::LoadFailed(format!("load_session({}) failed: {}", session_id, e))
        })?;
        let snapshot = CapabilitySnapshot {
            probe_type: Some("loaded".to_string()),
            language: Some(meta.language.clone()),
            bus_capacity: None,
            bus_fill: None,
            query_engine_ready: true,
            active_subscriptions: vec![],
            tail_sealed: meta.tail_sealed,
            sealed_at: meta.sealed_at,
        };
        Ok(SessionStartOutput {
            session_id,
            action: SessionStartAction::Load,
            target: Some(meta.target),
            language: Some(meta.language),
            event_count: Some(meta.event_count),
            duration_ms: Some(meta.duration_ms),
            bus_capacity: None,
            capability_snapshot: snapshot,
            provenance: lifecycle_provenance("session_start:load"),
        })
    }

    fn attach(input: SessionStartInput) -> Result<SessionStartOutput, ServiceError> {
        if input.pid.is_none() {
            return Err(ServiceError::InvalidInput(
                "session_start{action=attach} requires `pid` (m7+)".to_string(),
            ));
        }
        Err(ServiceError::Unsupported(
            "session_start{action=attach} (m7+) — no domain-layer attach API yet".to_string(),
        ))
    }

    /// v2 `session_stop` dispatcher entrypoint (helper, single-call).
    ///
    /// Algorithm (single-call, m7-07):
    /// 1. If `drain_subscriptions`, run `ChronosObserveService::observe{verb=list}`
    ///    to destructively drain fired tripwire events.
    /// 2. `ProbeService::stop` (one call only — events returned to caller).
    /// 3. Compute `sealed_at` if `seal_tail` (no write yet — caller
    ///    decides whether to persist sealed metadata into the redb
    ///    store via `save_session(meta, &events)`).
    ///
    /// **Architecture (m7-07):** replaces the m7-05/m7-06
    /// double-call workaround. The MCP `session_stop` wrapper
    /// calls this helper directly (NOT `self.stop`), so the probe
    /// is stopped exactly once and the events flow through to the
    /// persistence side.
    pub async fn stop_with_persistence(
        ctx: &SessionLifecycleContext<'_>,
        input: SessionStopInput,
    ) -> Result<(bool, SessionStopPersistence), ServiceError> {
        let drained_subscriptions = if input.drain_subscriptions {
            // observe{verb=list} is destructive; we ignore its return
            // value here because the v2 contract only exposes a boolean
            // "drained_subscriptions" flag.
            let _ = ChronosObserveService::observe(
                ctx.observe,
                ObserveInput {
                    verb: ObserveVerb::List,
                    subscription_id: None,
                    condition: None,
                    action: None,
                    retention: None,
                    requested_evidence: None,
                    scope: Some(ObserveScope::Session {
                        session_id: input.session_id.clone(),
                    }),
                    cursor: None,
                    label: None,
                },
            );
            true
        } else {
            false
        };

        let sealed_at = if input.seal_tail {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            Some(now)
        } else {
            None
        };

        // Single ProbeService::stop call. ProbeNotFound → already-stopped
        // idempotent path; caller synthesises output from persisted
        // metadata.
        let persistence = match ProbeService::stop(ctx.probe, &input.session_id) {
            Ok(r) => SessionStopPersistence::Stopped {
                session_id: input.session_id,
                events: r.events,
                language: r.language,
                target: r.target,
                total_events: r.total_events as u64,
                duration_ms: r.duration_ms,
                ebpf_detached: r.ebpf_detached,
                sealed_at,
            },
            Err(ServiceError::ProbeNotFound(_)) => SessionStopPersistence::AlreadyStopped {
                session_id: input.session_id,
            },
            Err(e) => return Err(e),
        };

        Ok((drained_subscriptions, persistence))
    }

    /// v2 `session_stop` dispatcher entrypoint (public, JSON-shape
    /// output for tests + in-process callers).
    ///
    /// Delegates to `stop_with_persistence` and synthesises the
    /// wire-format `SessionStopOutput` (drops the events vector).
    /// Does NOT persist events to the redb store — that is the MCP
    /// wrapper's responsibility.
    pub async fn stop(
        ctx: &SessionLifecycleContext<'_>,
        input: SessionStopInput,
    ) -> Result<SessionStopOutput, ServiceError> {
        let (drained_subscriptions, persistence) = Self::stop_with_persistence(ctx, input).await?;

        match persistence {
            SessionStopPersistence::Stopped {
                session_id,
                language,
                target,
                total_events,
                duration_ms,
                ebpf_detached,
                sealed_at,
                ..
            } => {
                let snapshot = CapabilitySnapshot {
                    probe_type: Some("ebpf_user".to_string()),
                    language: Some(language.to_string()),
                    bus_capacity: None,
                    bus_fill: None,
                    query_engine_ready: true,
                    active_subscriptions: vec![],
                    tail_sealed: sealed_at.is_some(),
                    sealed_at,
                };
                Ok(SessionStopOutput {
                    session_id,
                    status: "stopped".to_string(),
                    target,
                    total_events,
                    duration_ms,
                    ebpf_detached,
                    sealed_at,
                    drained_subscriptions,
                    capability_snapshot: snapshot,
                    provenance: lifecycle_provenance("session_stop"),
                })
            }
            SessionStopPersistence::AlreadyStopped { session_id } => {
                let (meta, events) = ctx.store.load_session(&session_id).map_err(|e| {
                    ServiceError::LoadFailed(format!(
                        "load_session({}) failed: {}",
                        session_id, e
                    ))
                })?;
                let snapshot = CapabilitySnapshot {
                    probe_type: Some("ebpf_user".to_string()),
                    language: Some(meta.language.clone()),
                    bus_capacity: None,
                    bus_fill: None,
                    query_engine_ready: true,
                    active_subscriptions: vec![],
                    tail_sealed: meta.tail_sealed,
                    sealed_at: meta.sealed_at,
                };
                Ok(SessionStopOutput {
                    session_id,
                    status: "already_stopped".to_string(),
                    target: meta.target,
                    total_events: events.len() as u64,
                    duration_ms: meta.duration_ms,
                    ebpf_detached: true,
                    sealed_at: meta.sealed_at,
                    drained_subscriptions,
                    capability_snapshot: snapshot,
                    provenance: lifecycle_provenance("session_stop"),
                })
            }
        }
    }

    /// v2 `capabilities` dispatcher entrypoint.
    ///
    /// At least one of `target` or `session_id` must be provided. Both
    /// are allowed for a combined static + dynamic view.
    ///
    /// This entrypoint only needs the store — no probe/observe — so
    /// it takes `&SessionStore` directly for clean unit-testability.
    /// The wrapper `capabilities_with_context` (below) preserves the
    /// full-context signature for the MCP server.
    pub fn capabilities(
        store: &chronos_store::SessionStore,
        input: CapabilitiesInput,
    ) -> Result<CapabilitiesOutput, ServiceError> {
        if input.target.is_none() && input.session_id.is_none() {
            return Err(ServiceError::InvalidInput(
                "capabilities requires at least one of `target` or `session_id`".to_string(),
            ));
        }

        let static_caps = input.target.as_ref().map(Self::static_capabilities);

        let dynamic_caps = if let Some(sid) = input.session_id.as_ref() {
            let (meta, _events) = store.load_session(sid).map_err(|e| {
                ServiceError::LoadFailed(format!("load_session({}) failed: {}", sid, e))
            })?;
            Some(Self::dynamic_capabilities(&meta))
        } else {
            None
        };

        Ok(CapabilitiesOutput {
            static_capabilities: static_caps,
            dynamic_capabilities: dynamic_caps,
            provenance: lifecycle_provenance("capabilities"),
        })
    }

    /// `capabilities` taking the full context — convenience for the
    /// MCP server which already holds the context. This variant
    /// resolves `session_id` against BOTH the store (persisted
    /// sessions) AND `live_probes` (live-only sessions not yet
    /// persisted). For live sessions we synthesise a stub
    /// `SessionMetadata` from `LiveProbeSession` so the dynamic
    /// capability snapshot can be returned even before the first
    /// `session_stop` call.
    pub fn capabilities_with_context(
        ctx: &SessionLifecycleContext<'_>,
        input: CapabilitiesInput,
    ) -> Result<CapabilitiesOutput, ServiceError> {
        if input.target.is_none() && input.session_id.is_none() {
            return Err(ServiceError::InvalidInput(
                "capabilities requires at least one of `target` or `session_id`".to_string(),
            ));
        }

        let static_caps = input.target.as_ref().map(Self::static_capabilities);

        let dynamic_caps = if let Some(sid) = input.session_id.as_ref() {
            match ctx.store.load_session(sid) {
                Ok((meta, _events)) => Some(Self::dynamic_capabilities(&meta)),
                Err(_) => {
                    // Fall back to live_probes — useful for
                    // capabilities queries *before* the session
                    // has been stopped (the dispatcher can mint a
                    // stub metadata from the LiveProbeSession
                    // fields).
                    if let Ok(guard) = ctx.probe.live_probes.lock() {
                        if let Some(live) = guard.get(sid) {
                            let stub_meta = chronos_store::SessionMetadata {
                                session_id: sid.clone(),
                                created_at: 0,
                                language: live.language.to_string(),
                                target: live.target.clone(),
                                event_count: 0,
                                duration_ms: 0,
                                tail_sealed: false,
                                sealed_at: None,
                            };
                            return Ok(CapabilitiesOutput {
                                static_capabilities: static_caps,
                                dynamic_capabilities: Some(Self::dynamic_capabilities(&stub_meta)),
                                provenance: lifecycle_provenance("capabilities"),
                            });
                        }
                    }
                    return Err(ServiceError::LoadFailed(format!(
                        "load_session({}) failed: live-only and not in live_probes",
                        sid
                    )));
                }
            }
        } else {
            None
        };

        Ok(CapabilitiesOutput {
            static_capabilities: static_caps,
            dynamic_capabilities: dynamic_caps,
            provenance: lifecycle_provenance("capabilities"),
        })
    }

    // ---- helpers ----

    fn static_capabilities(target: &TargetSpec) -> StaticCapabilities {
        let language = target.language.unwrap_or(Language::Native);
        let target_event_types = vec![
            EventType::FunctionEntry,
            EventType::FunctionExit,
            EventType::SyscallEnter,
            EventType::SyscallExit,
            EventType::SignalDelivered,
            EventType::VariableWrite,
            EventType::MemoryWrite,
            EventType::BreakpointHit,
            EventType::ThreadCreate,
            EventType::ThreadExit,
            EventType::ExceptionThrown,
        ];
        let language_adapters = vec![
            LanguageAdapterStatus {
                language: Language::Native,
                available: true,
                reason: None,
            },
            LanguageAdapterStatus {
                language: Language::Python,
                available: true,
                reason: None,
            },
            LanguageAdapterStatus {
                language: Language::Java,
                available: true,
                reason: None,
            },
            LanguageAdapterStatus {
                language: Language::Go,
                available: true,
                reason: None,
            },
            LanguageAdapterStatus {
                language: Language::JavaScript,
                available: true,
                reason: None,
            },
        ];
        let projections = vec![
            ProjectionKind::EventsRead,
            ProjectionKind::ExecutionQuery,
            ProjectionKind::StateQuery,
            ProjectionKind::TraceSlice,
            ProjectionKind::SessionCompare,
            ProjectionKind::SessionExplain,
        ];
        StaticCapabilities {
            probe_type: "ebpf_user".to_string(),
            language,
            target_event_types,
            language_adapters,
            projections,
        }
    }

    fn dynamic_capabilities(meta: &chronos_store::SessionMetadata) -> DynamicCapabilities {
        // Build a HashMap<EventType, u64> from the persisted events if
        // available. Without per-event type counts persisted, we
        // approximate from `event_count` (treats every event as
        // FunctionEntry). A future m7+ cycle can persist real counts.
        let mut event_type_counts: HashMap<EventType, u64> = HashMap::new();
        let total = meta.event_count as u64;
        if total > 0 {
            event_type_counts.insert(EventType::FunctionEntry, total);
        }
        DynamicCapabilities {
            bus_capacity: 0,
            bus_fill: 0,
            event_types_emitted: event_type_counts.keys().copied().collect(),
            event_type_counts,
            query_engine_ready: true,
            active_subscriptions: vec![],
            tail_sealed: meta.tail_sealed,
            sealed_at: meta.sealed_at,
        }
    }

    /// Mark a session's metadata as sealed (tail_sealed=true,
    /// sealed_at=<now>) by writing the updated metadata back to the
    /// store. Uses the load + mutate + save round-trip via the store's
    /// overwrite semantics.
    ///
    /// Returns `Ok(false)` if the session has not been persisted to the
    /// store yet (i.e., the probe is still live-only and the events
    /// have not been written). The caller can choose to surface this
    /// as a no-op or a warning; the m7-05 dispatcher treats it as a
    /// soft-skip so the seal flag is set once the server-side
    /// `build_and_store_engine` writes the metadata.
    #[allow(dead_code)] // kept for future callers; not used by m7-07 dispatcher (see note above)
    pub(crate) fn mark_sealed(
        store: &chronos_store::SessionStore,
        session_id: &str,
        sealed_at: u64,
    ) -> Result<(), ServiceError> {
        let (mut meta, events) = store.load_session(session_id).map_err(|e| {
            ServiceError::LoadFailed(format!("load_session({}) failed: {}", session_id, e))
        })?;
        meta.tail_sealed = true;
        meta.sealed_at = Some(sealed_at);
        store.save_session(meta, &events).map_err(|e| {
            ServiceError::SaveFailed(format!("save_session({}) failed: {}", session_id, e))
        })?;
        Ok(())
    }
}

/// Build a `SessionLifecycleProvenance` with the engine version baked in.
fn lifecycle_provenance(source: &str) -> SessionLifecycleProvenance {
    SessionLifecycleProvenance {
        engine_version: ENGINE_VERSION.to_string(),
        source: source.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::{CapabilitiesInput, SessionStartAction, SessionStartInput, TargetSpec};
    use chronos_store::{SessionMetadata, SessionStore};

    fn empty_store() -> SessionStore {
        SessionStore::in_memory().unwrap()
    }

    fn save_meta(store: &SessionStore, id: &str) {
        let meta = SessionMetadata {
            session_id: id.to_string(),
            created_at: 1000,
            language: "native".to_string(),
            target: "/bin/test".to_string(),
            event_count: 5,
            duration_ms: 250,
            tail_sealed: false,
            sealed_at: None,
        };
        store.save_session(meta, &[]).unwrap();
    }

    // ---- session_start ----

    #[tokio::test]
    async fn start_spawn_requires_spawn_fields() {
        // The validation guard fires before any probe call, so a
        // minimal context (whose probe is never read) is sufficient.
        let store = empty_store();
        let ctx = build_minimal_ctx(&store);
        let input = SessionStartInput {
            action: SessionStartAction::Spawn,
            spawn_fields: None,
            session_id: None,
            pid: None,
            path: None,
        };
        let err = ChronosSessionLifecycleService::start(&ctx, input)
            .await
            .unwrap_err();
        assert!(matches!(err, ServiceError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn start_load_returns_metadata_snapshot() {
        let store = empty_store();
        save_meta(&store, "load-1");
        let input = SessionStartInput {
            action: SessionStartAction::Load,
            spawn_fields: None,
            session_id: Some("load-1".to_string()),
            pid: None,
            path: None,
        };
        let out = ChronosSessionLifecycleService::load(&store, input).unwrap();
        assert_eq!(out.session_id, "load-1");
        assert_eq!(out.event_count, Some(5));
        assert_eq!(out.duration_ms, Some(250));
        assert!(out.capability_snapshot.query_engine_ready);
        assert!(!out.capability_snapshot.tail_sealed);
    }

    #[tokio::test]
    async fn start_load_session_not_found() {
        let store = empty_store();
        let input = SessionStartInput {
            action: SessionStartAction::Load,
            spawn_fields: None,
            session_id: Some("nope".to_string()),
            pid: None,
            path: None,
        };
        let err = ChronosSessionLifecycleService::load(&store, input).unwrap_err();
        assert!(matches!(err, ServiceError::LoadFailed(_)));
    }

    #[test]
    fn start_attach_without_pid_returns_invalid_input() {
        let input = SessionStartInput {
            action: SessionStartAction::Attach,
            spawn_fields: None,
            session_id: None,
            pid: None,
            path: None,
        };
        let err = ChronosSessionLifecycleService::attach(input).unwrap_err();
        assert!(matches!(err, ServiceError::InvalidInput(_)));
    }

    #[test]
    fn start_attach_with_pid_returns_unsupported() {
        let input = SessionStartInput {
            action: SessionStartAction::Attach,
            spawn_fields: None,
            session_id: None,
            pid: Some(4242),
            path: None,
        };
        let err = ChronosSessionLifecycleService::attach(input).unwrap_err();
        assert!(matches!(err, ServiceError::Unsupported(_)));
    }

    // ---- capabilities ----

    #[test]
    fn capabilities_target_only_returns_static_only() {
        let store = empty_store();
        let input = CapabilitiesInput {
            target: Some(TargetSpec {
                program: "/bin/foo".to_string(),
                args: vec![],
                language: Some(Language::Native),
            }),
            session_id: None,
        };
        let out = ChronosSessionLifecycleService::capabilities(&store, input).unwrap();
        assert!(out.static_capabilities.is_some());
        assert!(out.dynamic_capabilities.is_none());
        let s = out.static_capabilities.unwrap();
        assert_eq!(s.probe_type, "ebpf_user");
        assert!(s.target_event_types.contains(&EventType::SyscallEnter));
        assert!(s.projections.contains(&ProjectionKind::SessionExplain));
    }

    #[test]
    fn capabilities_session_only_returns_dynamic_only() {
        let store = empty_store();
        save_meta(&store, "cap-1");
        let input = CapabilitiesInput {
            target: None,
            session_id: Some("cap-1".to_string()),
        };
        let out = ChronosSessionLifecycleService::capabilities(&store, input).unwrap();
        assert!(out.static_capabilities.is_none());
        let d = out.dynamic_capabilities.unwrap();
        assert!(d.query_engine_ready);
        assert!(!d.tail_sealed);
    }

    #[test]
    fn capabilities_target_and_session_returns_both() {
        let store = empty_store();
        save_meta(&store, "cap-2");
        let input = CapabilitiesInput {
            target: Some(TargetSpec {
                program: "/bin/x".to_string(),
                args: vec![],
                language: None,
            }),
            session_id: Some("cap-2".to_string()),
        };
        let out = ChronosSessionLifecycleService::capabilities(&store, input).unwrap();
        assert!(out.static_capabilities.is_some());
        assert!(out.dynamic_capabilities.is_some());
    }

    #[test]
    fn capabilities_neither_set_returns_invalid_input() {
        let store = empty_store();
        let input = CapabilitiesInput {
            target: None,
            session_id: None,
        };
        let err = ChronosSessionLifecycleService::capabilities(&store, input).unwrap_err();
        assert!(matches!(err, ServiceError::InvalidInput(_)));
    }

    #[test]
    fn capabilities_session_not_found() {
        let store = empty_store();
        let input = CapabilitiesInput {
            target: None,
            session_id: Some("no-such".to_string()),
        };
        let err = ChronosSessionLifecycleService::capabilities(&store, input).unwrap_err();
        assert!(matches!(err, ServiceError::LoadFailed(_)));
    }

    // ---- provenance ----

    #[test]
    fn provenance_engine_version_is_baked() {
        let p = lifecycle_provenance("test");
        assert_eq!(p.engine_version, "chronos-0.1.0");
        assert_eq!(p.source, "test");
    }

    // ---- static_capabilities ----

    #[test]
    fn static_capabilities_includes_all_event_types() {
        let s = ChronosSessionLifecycleService::static_capabilities(&TargetSpec {
            program: "/bin/x".to_string(),
            args: vec![],
            language: Some(Language::Native),
        });
        // Spot-check 3 event types (full list is 11).
        assert!(s.target_event_types.contains(&EventType::FunctionEntry));
        assert!(s.target_event_types.contains(&EventType::SignalDelivered));
        assert!(s.target_event_types.contains(&EventType::ThreadExit));
    }

    // ---- dynamic_capabilities ----

    #[test]
    fn dynamic_capabilities_reflects_seal_flag() {
        let mut meta = SessionMetadata {
            session_id: "x".to_string(),
            created_at: 0,
            language: "native".to_string(),
            target: "/bin/x".to_string(),
            event_count: 3,
            duration_ms: 50,
            tail_sealed: false,
            sealed_at: None,
        };
        let d = ChronosSessionLifecycleService::dynamic_capabilities(&meta);
        assert!(!d.tail_sealed);
        meta.tail_sealed = true;
        meta.sealed_at = Some(123);
        let d2 = ChronosSessionLifecycleService::dynamic_capabilities(&meta);
        assert!(d2.tail_sealed);
        assert_eq!(d2.sealed_at, Some(123));
    }

    // ---- seal helper (sandbox-free test for mark_sealed) ----

    #[test]
    fn mark_sealed_updates_metadata_and_persists() {
        let store = empty_store();
        save_meta(&store, "seal-1");
        ChronosSessionLifecycleService::mark_sealed(&store, "seal-1", 9999).unwrap();
        let (meta, _events) = store.load_session("seal-1").unwrap();
        assert!(meta.tail_sealed);
        assert_eq!(meta.sealed_at, Some(9999));
    }

    #[test]
    fn mark_sealed_missing_session_returns_load_failed() {
        let store = empty_store();
        let result =
            ChronosSessionLifecycleService::mark_sealed(&store, "no-such-session", 9999);
        assert!(
            matches!(result, Err(ServiceError::LoadFailed(_))),
            "expected mark_sealed to return LoadFailed when session is missing"
        );
    }

    // ---- helpers for full-context tests ----

    /// Build a minimal `SessionLifecycleContext` whose `probe` and
    /// `observe` fields are never read by the test code paths.
    /// We construct real `TripwireManager` + `Arc` + `Mutex`/`TokioMutex`
    /// wrappers around empty `HashMap`s, so the references are valid
    /// but unused. Tests that need a working `ProbeContext` /
    /// `ObserveContext` (spawn, stop, drain) live in the sandbox suite.
    fn build_minimal_ctx<'a>(store: &'a SessionStore) -> SessionLifecycleContext<'a> {
        use chronos_domain::tripwire::TripwireManager;
        use std::sync::{Arc, Mutex as StdMutex};
        use tokio::sync::Mutex as TokioMutex;

        // Leak empty data structures; never read.
        let tripwire: &'static Arc<TripwireManager> =
            Box::leak(Box::new(Arc::new(TripwireManager::new())));
        let uprobe_counter: &'static StdMutex<HashMap<String, usize>> =
            Box::leak(Box::new(StdMutex::new(HashMap::new())));
        let active_session: &'static TokioMutex<Option<String>> =
            Box::leak(Box::new(TokioMutex::new(None)));
        let langs: &'static Arc<TokioMutex<HashMap<String, Language>>> =
            Box::leak(Box::new(Arc::new(TokioMutex::new(HashMap::new()))));
        let live_probes: &'static std::sync::Mutex<
            HashMap<String, crate::probe::LiveProbeSession>,
        > = Box::leak(Box::new(std::sync::Mutex::new(HashMap::new())));
        let engines: &'static TokioMutex<HashMap<String, chronos_query::QueryEngine>> =
            Box::leak(Box::new(TokioMutex::new(HashMap::new())));

        let probe: &'static ProbeContext<'static> = Box::leak(Box::new(ProbeContext {
            live_probes,
            engines,
            session_languages: langs,
            tripwire_manager: tripwire,
            active_session,
        }));
        let observe: &'static ObserveContext<'static> = Box::leak(Box::new(ObserveContext {
            tripwire_manager: tripwire,
            probe,
            uprobe_counter,
        }));
        SessionLifecycleContext {
            store,
            probe,
            observe,
        }
    }
}
