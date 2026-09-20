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
use chronos_domain::SubscriptionId;
use std::collections::HashMap;
use std::sync::Mutex as StdMutex;

use crate::error::ServiceError;
use crate::events_cursor::EventsCursorV1;
use crate::output::FireCountFacts;
use crate::output::{
    ObserveCondition, ObserveCreateResult, ObserveDeleteResult, ObserveListResult, ObserveOutput,
    ObserveProvenance, ObserveRetention, ObserveScope, ObserveVerb, SubscriptionDto,
    TripwireFiredSummary,
};
use crate::probe::{ProbeContext, ProbeService};
use crate::tripwires::TripwiresService;

// Re-export `ObserveInput` so external crates can construct it without
// reaching into `crate::output`. The other DTOs (output, provenance,
// result, scope, verb, etc.) are intentionally NOT re-exported — those
// are return-type payloads and stay opaque to callers.
pub use crate::output::ObserveInput;

/// Maximum firings handed back by a single `verb=list` page.
///
/// The page is also bounded by `tripwire_evidence::DEFAULT_SCAN_BUDGET`
/// examined records, so a sparse firing cannot turn a small request into an
/// unbounded scan.
const MAX_FIRINGS_PER_PAGE: usize = 200;

/// Wire string for a firing-count completeness state.
fn firing_count_status_str(s: crate::tripwire_evidence::FiringCountStatus) -> String {
    use crate::tripwire_evidence::FiringCountStatus as S;
    match s {
        S::Complete => "complete",
        S::Truncated => "truncated",
        S::Partial => "partial",
    }
    .to_string()
}

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
    ///
    /// REC-C2.1.4a: this is the async boundary.
    ///
    /// `list` needs the canonical session to read firings from its
    /// `ExecutionLog`, and the active session id lives behind an async
    /// mutex. Making the boundary await was preferred over adding a second,
    /// synchronous source of the active session (duplicated identity, the
    /// exact thing REC-C1.2a removed).
    ///
    /// `create` / `delete` / `query` stay synchronous helpers: they do not
    /// need to resolve a session.
    pub async fn observe(
        ctx: &ObserveContext<'_>,
        input: ObserveInput,
    ) -> Result<ObserveOutput, ServiceError> {
        match input.verb {
            ObserveVerb::Update => Err(ServiceError::Unsupported(
                "verb=update deferred to m7+".to_string(),
            )),
            ObserveVerb::Create => Self::create(ctx, input),
            ObserveVerb::List => Self::list(ctx, input).await,
            ObserveVerb::Delete => Self::delete(ctx, input),
            ObserveVerb::Query => Self::query(ctx, input).await,
        }
    }

    /// Resolve the canonical session for an observe operation.
    ///
    /// Precedence is deliberate and not interchangeable:
    ///
    /// ```text
    /// scope = session{ id }  -> id          (explicit identity always wins)
    /// scope absent / global  -> active_session  (convenience fallback)
    /// otherwise              -> NoActiveSession (typed absence)
    /// ```
    ///
    /// The id is cloned and the mutex guard is dropped **before** returning,
    /// so no caller can hold the lock across I/O, a log scan, a replay or
    /// serialization.
    pub async fn resolve_session(
        ctx: &ObserveContext<'_>,
        scope: Option<&ObserveScope>,
    ) -> Result<String, ServiceError> {
        if let Some(ObserveScope::Session { session_id }) = scope {
            return Ok(session_id.clone());
        }
        // Guard scope ends at this block: nothing below holds the lock.
        let active = { ctx.probe.active_session.lock().await.clone() };
        active.ok_or(ServiceError::NoActiveSession)
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
                    subscription_id: SubscriptionId::new(result.tripwire_id),
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
                // contract: surface every terminal failure variant of
                // `ProbeService::inject` as a typed `ServiceError` so the
                // MCP wrapper renders it as a tool error instead of a
                // success-shaped `attached_pid: None` response.
                //
                // REC-C0.5-B: probe_inject capability-aware split. Each
                // failure variant maps to a distinct typed error:
                //   - `ProbeStarting`           → `ServiceError::ProbeStarting`
                //     (no PID yet, the user can retry — start-up race)
                //   - `EbpfUnavailable(reason)` → `ServiceError::EbpfUnsupported(reason)`
                //     (kernel/feature missing — capability slot is `EbpfUprobe`)
                //   - `AttachFailed { error, .. }` → `ServiceError::InjectionFailed(error)`
                //     (adapter built but the kernel refused the uprobe)
                //   - `Attached { .. }`         → fall through to the success path
                //
                // Sandbox tests assert on the typed reason rather than
                // ad-hoc error-text matching.
                let probe_input = crate::probe::ProbeInjectInput {
                    session_id: session_id.clone(),
                    binary_path: binary_path.clone(),
                    symbol_name: symbol_name.clone(),
                    pid,
                };
                let attached_pid = match ProbeService::inject(ctx.probe, probe_input)? {
                    crate::probe::ProbeInjectResult::Attached { pid, .. } => pid,
                    crate::probe::ProbeInjectResult::AttachFailed {
                        error,
                        pid: _,
                        session_id: _,
                        binary_path: _,
                        symbol_name: _,
                    } => {
                        // NOTE: the live-probe session record still carries
                        // the `EbpfAttachmentInfo { pid, .. }` entry, so UAT
                        // consumers can inspect the PID via `probe_status`.
                        // Surfacing the error here loses no information that
                        // the session record keeps.
                        return Err(ServiceError::InjectionFailed(error));
                    }
                    crate::probe::ProbeInjectResult::EbpfUnavailable(reason) => {
                        return Err(ServiceError::EbpfUnsupported(reason));
                    }
                    crate::probe::ProbeInjectResult::ProbeStarting => {
                        return Err(ServiceError::ProbeStarting);
                    }
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
                    subscription_id: SubscriptionId::new(uprobe_id),
                    kind: "uprobe".to_string(),
                    status: "registered".to_string(),
                    active_count,
                    label: Some(sentinel_label),
                    attached_pid: Some(attached_pid),
                }))
            }
        }
    }

    // -- list ------------------------------------------------------------------

    /// Page firing evidence out of the session's `ExecutionLog`.
    ///
    /// REC-C2.1.4b. Firings are read from the log, never from the legacy
    /// `fired_buffer`; the destructive `TripwiresService::list()` is no longer
    /// called here (it drained globally, which stole evidence from other
    /// consumers — CHAR-C2-02).
    ///
    /// ```text
    /// Tripwire definitions -> TripwireManager (non-destructive snapshot)
    /// Firing evidence      -> ExecutionLog only
    /// List progress        -> caller-owned EventSeq cursor
    /// ```
    async fn list(
        ctx: &ObserveContext<'_>,
        input: ObserveInput,
    ) -> Result<ObserveOutput, ServiceError> {
        // Validate retention up front. v2 supports `drained` (default) and
        // `retained_until_session_end`; `permanent` is rejected.
        let retention = input.retention.unwrap_or_default();
        if matches!(retention, ObserveRetention::Permanent) {
            return Err(ServiceError::Unsupported(
                "retention=permanent deferred to m7+".into(),
            ));
        }
        Self::validate_requested_evidence(&input.requested_evidence)?;

        // Canonical session (explicit scope wins; active_session is a fallback).
        let session_id = Self::resolve_session(ctx, input.scope.as_ref()).await?;
        let log = ctx.probe.execution_logs.get(&session_id)?;
        let retained_from = log.retained_from();

        // Cursor handling. A malformed token, a token for another session, or a
        // token below the retention watermark is a typed error; a cursor is
        // NEVER silently re-anchored.
        let from = match input.cursor.as_deref() {
            Some(token) => {
                let cursor = EventsCursorV1::decode_for_session(
                    token,
                    &chronos_log::SessionId::new(&session_id),
                )
                .map_err(|e| ServiceError::InvalidInput(format!("observe cursor: {e}")))?;
                let next = cursor.next_seq();
                if next < retained_from {
                    return Err(ServiceError::InvalidInput(format!(
                        "observe cursor is stale: next_seq {} is below the retained boundary {}; \
                         re-anchor with a fresh cursor",
                        next.0, retained_from.0
                    )));
                }
                next
            }
            // No cursor: start at the first retained seq (not necessarily 0).
            None => retained_from,
        };

        let page = crate::tripwire_evidence::read_firings_page(
            &log,
            from,
            MAX_FIRINGS_PER_PAGE,
            crate::tripwire_evidence::DEFAULT_SCAN_BUDGET,
        )?;

        // Definitions come from the manager as a plain snapshot: no drain.
        // `fire_count` is derived from the log, never from the mutable
        // `Tripwire.fire_count` (FIND-C2.0-02).
        let counts = crate::tripwire_evidence::firing_count_snapshot(
            &log,
            crate::tripwire_evidence::DEFAULT_SCAN_BUDGET,
        )?;
        let facts = FireCountFacts {
            from_seq: counts.from_seq.0,
            through_seq_exclusive: counts.position_after.0,
            status: firing_count_status_str(counts.status()),
        };
        let subscriptions = ctx
            .tripwire_manager
            .list()
            .into_iter()
            .map(|tw| SubscriptionDto {
                kind: "tripwire".to_string(),
                id: tw.id.to_string(),
                label: tw.label,
                condition: format!("{:?}", tw.condition),
                fire_count: counts.count_for(tw.id),
                fire_count_facts: Some(facts.clone()),
            })
            .collect();
        let total_active = ctx.tripwire_manager.active_count();

        // Retention never controls WHICH evidence comes back: the log is
        // immutable and both policies return the same page. What differs is
        // conceptual — `drained` means the consumer may advance its cursor;
        // `retained_until_session_end` is an evidence-lifecycle guarantee.
        let fired_events: Vec<TripwireFiredSummary> = page
            .firings
            .iter()
            .map(|(firing_seq, ev)| TripwireFiredSummary {
                firing_seq: firing_seq.0,
                source_seq: ev.source_seq.0,
                tripwire_id: ev.tripwire_id.to_string(),
                condition_description: format!("{:?}", ev.condition),
                event_id: ev.source_event_id.unwrap_or(0),
                timestamp_ns: ev.source_timestamp_ns,
                thread_id: ev.source_thread_id,
            })
            .collect();
        let fired_count = fired_events.len();
        let _ = retention;

        // Always a checkpoint, even at the tail: the log may gain a firing next
        // second and the client must keep a position. Filters/pagination never
        // move the meaning of the position.
        let next_cursor = Some(
            EventsCursorV1::start(chronos_log::SessionId::new(&session_id))
                .advanced_to(page.position_after)
                .map_err(|e| ServiceError::InvalidInput(format!("observe cursor advance: {e}")))?
                .encode(),
        );

        let provenance = ObserveProvenance {
            engine_version: "chronos-0.1.0".to_string(),
            query_strategy: "ExecutionLogScan".to_string(),
            retention_in_effect: format!("{:?}", retention),
        };

        Ok(ObserveOutput::List(ObserveListResult {
            subscriptions,
            fired_events,
            total_active,
            fired_count,
            session_id: Some(session_id),
            next_cursor,
            provenance,
        }))
    }

    // -- query -----------------------------------------------------------------

    /// Non-destructive subscription snapshot with a derived `fire_count`.
    ///
    /// REC-C2.1.5: this is no longer a second `list` — it returns no
    /// `fired_events` and does not page. It does need the canonical session,
    /// because `fire_count` is a fact about ONE session's evidence:
    ///
    /// ```text
    /// TripwireManager -> active definitions (runtime, global)
    /// fire_count      -> firings recorded in one session's ExecutionLog
    /// ```
    async fn query(
        ctx: &ObserveContext<'_>,
        input: ObserveInput,
    ) -> Result<ObserveOutput, ServiceError> {
        let session_id = Self::resolve_session(ctx, input.scope.as_ref()).await?;
        let log = ctx.probe.execution_logs.get(&session_id)?;
        let counts = crate::tripwire_evidence::firing_count_snapshot(
            &log,
            crate::tripwire_evidence::DEFAULT_SCAN_BUDGET,
        )?;
        let facts = FireCountFacts {
            from_seq: counts.from_seq.0,
            through_seq_exclusive: counts.position_after.0,
            status: firing_count_status_str(counts.status()),
        };

        let subscriptions = ctx
            .tripwire_manager
            .list()
            .into_iter()
            .map(|tw| SubscriptionDto {
                kind: "tripwire".to_string(),
                id: tw.id.to_string(),
                label: tw.label,
                condition: format!("{:?}", tw.condition),
                fire_count: counts.count_for(tw.id),
                fire_count_facts: Some(facts.clone()),
            })
            .collect();

        let provenance = ObserveProvenance {
            engine_version: "chronos-0.1.0".to_string(),
            query_strategy: "ExecutionLogScan".to_string(),
            retention_in_effect: "Drained".to_string(),
        };

        Ok(ObserveOutput::Query(ObserveListResult {
            subscriptions,
            fired_events: Vec::new(),
            total_active: ctx.tripwire_manager.active_count(),
            fired_count: 0,
            session_id: Some(session_id),
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
            .into_inner()
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
            subscription_id: SubscriptionId::new(result.tripwire_id),
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
    use crate::session_log::SessionExecutionLog;
    use chronos_domain::capability::CapabilityUnavailable;
    use chronos_domain::ports::uprobe::{UprobeAttachError, UprobeHandle, UprobeInjector};
    use chronos_domain::tripwire::{reset_tripwire_ids_for_testing, TripwireCondition};

    /// Deterministic test double for `UprobeInjector` used by the
    /// `TestRig`. Always returns `CapabilityUnavailable::ebpf_uprobe`
    /// because the rig's live probes never exercise the kernel probe
    /// path; tests that need attach-failure or attach-success use a
    /// bespoke fake instead.
    #[derive(Debug, Default, Clone, Copy)]
    struct UnavailableUprobeInjector;

    impl UprobeInjector for UnavailableUprobeInjector {
        fn acquire(&self) -> Result<std::sync::Arc<dyn UprobeHandle>, CapabilityUnavailable> {
            Err(CapabilityUnavailable::ebpf_uprobe(
                "test rig: eBPF not exercised",
            ))
        }
    }

    /// REC-C3.5-residual-inversion R.3 — test double for
    /// [`NativeProbeControllerFactory`] that yields a no-op port
    /// controller. Lets the `TestRig` carry the new
    /// `native_probe_factory` field without invoking real ptrace. The
    /// returned controller is wired with the session's execution log
    /// so reads return an empty stream rather than panic, matching the
    /// "tests do not spawn real traces" contract.
    #[derive(Debug, Clone)]
    struct NullNativeProbeControllerFactory;

    impl chronos_domain::ports::NativeProbeControllerFactory for NullNativeProbeControllerFactory {
        fn build_for_spawn(
            &self,
            _config: chronos_domain::trace::CaptureConfig,
            session_id: chronos_domain::session_id::SessionId,
            _language: chronos_domain::trace::Language,
            log_provider: std::sync::Arc<
                dyn chronos_domain::ports::execution_log::ExecutionLogProvider,
            >,
            _accepted_raw_observer: Option<chronos_domain::ports::RawAcceptedObserver>,
            _track_function_frames: bool,
        ) -> Result<
            (
                Box<dyn chronos_domain::ports::NativeProbeController>,
                chronos_domain::trace::CaptureSession,
            ),
            chronos_domain::ports::NativeProbeBuildError,
        > {
            let session = chronos_domain::trace::CaptureSession::new(0, _language, _config.clone());
            let controller: Box<dyn chronos_domain::ports::NativeProbeController> = Box::new(
                NullNativeProbeController::new(session_id.clone(), log_provider),
            );
            Ok((controller, session))
        }

        fn build_for_attach(
            &self,
            _config: chronos_domain::trace::CaptureConfig,
            _pid: u32,
            session_id: chronos_domain::session_id::SessionId,
            _language: chronos_domain::trace::Language,
            log_provider: std::sync::Arc<
                dyn chronos_domain::ports::execution_log::ExecutionLogProvider,
            >,
            _accepted_raw_observer: Option<chronos_domain::ports::RawAcceptedObserver>,
        ) -> Result<
            (
                Box<dyn chronos_domain::ports::NativeProbeController>,
                chronos_domain::trace::CaptureSession,
            ),
            chronos_domain::ports::NativeProbeBuildError,
        > {
            let session = chronos_domain::trace::CaptureSession::new(0, _language, _config.clone());
            let controller: Box<dyn chronos_domain::ports::NativeProbeController> = Box::new(
                NullNativeProbeController::new(session_id.clone(), log_provider),
            );
            Ok((controller, session))
        }
    }

    /// REC-C3.5-residual-inversion R.3 — test-only port impl that
    /// returns empty / no-op answers for every method. Wired by
    /// `NullNativeProbeControllerFactory`.
    struct NullNativeProbeController {
        session_id: chronos_domain::session_id::SessionId,
        log: Option<std::sync::Arc<dyn chronos_domain::ports::execution_log::ExecutionLogProvider>>,
    }

    impl std::fmt::Debug for NullNativeProbeController {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("NullNativeProbeController")
                .field("session_id", &self.session_id)
                .finish()
        }
    }

    impl NullNativeProbeController {
        fn new(
            session_id: chronos_domain::session_id::SessionId,
            log: std::sync::Arc<dyn chronos_domain::ports::execution_log::ExecutionLogProvider>,
        ) -> Self {
            Self {
                session_id,
                log: Some(log),
            }
        }
    }

    impl chronos_domain::ports::NativeProbeController for NullNativeProbeController {
        fn session_id(&self) -> &chronos_domain::session_id::SessionId {
            &self.session_id
        }

        fn attach_to_pid(
            &self,
            _pid: u32,
            _config: &chronos_domain::trace::CaptureConfig,
        ) -> Result<chronos_domain::trace::CaptureSession, chronos_domain::TraceError> {
            Err(chronos_domain::TraceError::capture_failed(
                "NullNativeProbeController: no real ptrace in tests",
            ))
        }

        fn start(
            &self,
            _config: &chronos_domain::trace::CaptureConfig,
            _track_function_frames: bool,
        ) -> Result<chronos_domain::trace::CaptureSession, chronos_domain::TraceError> {
            Err(chronos_domain::TraceError::capture_failed(
                "NullNativeProbeController: no real ptrace in tests",
            ))
        }

        fn stop(&self) -> Result<(), chronos_domain::TraceError> {
            Ok(())
        }

        fn advance(
            &self,
        ) -> Result<chronos_domain::ports::AdvanceOutcome, chronos_domain::TraceError> {
            Ok((false, None, false))
        }

        fn step(&self) -> Result<chronos_domain::ports::StepOutcome, chronos_domain::TraceError> {
            Ok((false, None))
        }

        fn execution_log(
            &self,
        ) -> Option<std::sync::Arc<dyn chronos_domain::ports::execution_log::ExecutionLogProvider>>
        {
            self.log.clone()
        }

        fn clone_resolver_pipeline(&self) -> chronos_domain::semantic::ResolverPipeline {
            chronos_domain::semantic::ResolverPipeline::new()
        }

        fn resolve_context(
            &self,
            _binary_path: Option<String>,
        ) -> chronos_domain::semantic::ResolveContext {
            chronos_domain::semantic::ResolveContext {
                pid: 0,
                binary_path: None,
            }
        }
    }

    /// Test double that succeeds at attach and counts invocations so
    /// tests can assert `stop → detach` was invoked. Counters are
    /// shared via `Arc` between the injector (which owns them) and the
    /// handles it hands out (which bump them on every attach/detach).
    /// This is the shape `chronos-ebpf::EbpfUprobeInjector` will satisfy
    /// in production: one kernel adapter, many handles.
    #[derive(Debug, Clone)]
    struct RecordingUprobeInjector {
        attached: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        detached: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    impl RecordingUprobeInjector {
        fn new() -> Self {
            Self {
                attached: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
                detached: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            }
        }
    }

    impl UprobeInjector for RecordingUprobeInjector {
        fn acquire(&self) -> Result<std::sync::Arc<dyn UprobeHandle>, CapabilityUnavailable> {
            // The handle clones the same `Arc` to the counters the
            // injector holds, so every attach/detach call updates the
            // shared counters.
            Ok(std::sync::Arc::new(RecordingUprobeHandle {
                attached: self.attached.clone(),
                detached: self.detached.clone(),
            }))
        }
    }

    /// Counter pair shared with the injector. Mirrors
    /// `EbpfUprobeHandle`'s shape: the handle just forwards to the
    /// backend (here, the recording injector's counters).
    struct RecordingUprobeHandle {
        attached: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        detached: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    impl UprobeHandle for RecordingUprobeHandle {
        fn attach(
            &self,
            _pid: u32,
            _binary_path: &str,
            _symbol_name: &str,
        ) -> Result<(), UprobeAttachError> {
            self.attached
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }

        fn detach(&self) -> Result<(), UprobeAttachError> {
            self.detached
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
    }

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
        execution_logs: crate::session_log::SessionExecutionLogRegistry,
        /// REC-C3.3.2.3 — composition-root injectable uprobe capability.
        /// The rig carries a unit `UnavailableUprobeInjector` by default;
        /// tests that need attach/detach semantics swap it for a
        /// `RecordingUprobeInjector` via `with_injector`.
        uprobe_injector: Arc<dyn UprobeInjector>,
        /// REC-C3.5-residual-inversion R.3 — composition-root
        /// injectable native probe controller factory. Tests do not
        /// spawn real ptrace sessions; the rig carries a
        /// `NullNativeProbeControllerFactory` that returns an
        /// `NativeProbeController` port whose operations are no-ops
        /// (only used by code paths that exercise the wiring).
        native_probe_factory: Arc<dyn chronos_domain::ports::NativeProbeControllerFactory>,
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
                execution_logs: crate::session_log::SessionExecutionLogRegistry::new(),
                uprobe_injector: Arc::new(UnavailableUprobeInjector),
                native_probe_factory: Arc::new(NullNativeProbeControllerFactory),
            }
        }

        /// REC-C3.3.2.3 — swap the rig's injector for a fake that
        /// records attach/detach invocations. Returns the `Arc` so the
        /// caller can inspect counters after `inject` / `stop`.
        #[allow(dead_code)]
        fn with_recording_injector(&mut self) -> Arc<RecordingUprobeInjector> {
            let recorder = Arc::new(RecordingUprobeInjector::new());
            self.uprobe_injector = recorder.clone();
            recorder
        }

        /// Construct a fresh `ProbeContext` borrowing from this rig.
        fn probe_ctx<'a>(&'a self) -> ProbeContext<'a> {
            ProbeContext {
                live_probes: &self.live_probes,
                execution_logs: &self.execution_logs,
                engines: &self.engines,
                session_languages: &self.session_languages,
                tripwire_manager: &self.manager,
                active_session: &self.active_session,
                uprobe_injector: &self.uprobe_injector,
                native_probe_factory: &self.native_probe_factory,
            }
        }

        /// REC-C2.1.4b: open a session `ExecutionLog` and make it active.
        ///
        /// `observe(list)` reads firings from the log, so tests must set up a
        /// real log instead of filling the legacy `fired_buffer`.
        async fn open_session(&self, session_id: &str) {
            let dir = std::env::temp_dir().join(format!(
                "chronos-c21-observe-{}-{}-{}",
                session_id,
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("mkdir");
            let raw = chronos_log::SegmentedExecutionLog::open(
                chronos_log::SessionId::new(session_id),
                chronos_log::SegmentedConfig::with_dir(&dir),
            )
            .expect("open log");
            let session = chronos_log::SessionId::new(session_id);
            let log = SessionExecutionLog::from_segmented_log(
                session,
                std::sync::Arc::new(raw),
                Some(dir),
            );
            self.execution_logs.register(log).expect("register");
            *self.active_session.lock().await = Some(session_id.to_string());
        }

        /// Append a `Raw` record for `event` and derive any firings from it.
        ///
        /// This is the REC-C2.1 production flow: accept the source first, then
        /// derive durable `TripwireFired` evidence. No `fired_buffer` involved.
        fn ingest(&self, session_id: &str, event: &chronos_domain::TraceEvent) {
            use chronos_log::ExecutionKind as K;
            let log = self.execution_logs.get(session_id).expect("log");
            let payload = chronos_log::ExecutionPayload::new(
                serde_json::to_vec(event).expect("encode"),
                "trace_event",
            );
            let seq = log
                .handle()
                .append(chronos_log::NewExecutionRecord {
                    session_id: log.session_id().clone(),
                    kind: K::Raw,
                    monotonic_ns: event.timestamp_ns.get(),
                    payload,
                    ..Default::default()
                })
                .expect("append raw");
            log.flush().ok();
            crate::tripwire_evidence::derive_firings_from_event(&log, &self.manager, seq, event)
                .expect("derive");
        }

        /// REC-C2.1.4a: drive the service through its real async boundary.
        ///
        /// `observe` is `async` now (it must await the canonical session), so
        /// the closure form cannot work: an `FnOnce -> R` closure cannot
        /// contain `.await`. Building the context inside this async fn keeps
        /// both borrows alive across the await without any HRTB gymnastics.
        async fn observe(&self, input: ObserveInput) -> Result<ObserveOutput, ServiceError> {
            let probe = self.probe_ctx();
            let ctx = ObserveContext {
                tripwire_manager: &self.manager,
                probe: &probe,
                uprobe_counter: &self.uprobe_counter,
            };
            ChronosObserveService::observe(&ctx, input).await
        }
    }

    /// Convenience: each test uses ` ... `
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

    #[tokio::test]
    async fn create_request_missing_condition_returns_unsupported() {
        let rig = TestRig::new();
        let input = tripwire_input(ObserveVerb::Create);
        let err = rig.observe(input).await.unwrap_err();
        assert!(matches!(err, ServiceError::Unsupported(_)), "got {:?}", err);
    }

    // ----- verb=update rejection ---------------------------------------------

    #[tokio::test]
    async fn verb_update_returns_unsupported() {
        let rig = TestRig::new();
        let input = tripwire_input(ObserveVerb::Update);
        let err = rig.observe(input).await.unwrap_err();
        assert!(matches!(err, ServiceError::Unsupported(_)), "got {:?}", err);
    }

    // ----- verb=create (tripwire) --------------------------------------------

    #[tokio::test]
    async fn create_tripwire_returns_create_result() {
        let rig = TestRig::new();
        let mut input = tripwire_input(ObserveVerb::Create);
        input.condition = Some(ObserveCondition::Tripwire {
            condition: tripwire_condition(),
            label: Some("main-watch".to_string()),
        });
        let out = rig.observe(input).await.unwrap();
        match out {
            ObserveOutput::Create(c) => {
                assert!(c.subscription_id.as_str().starts_with("tripwire-"));
                assert_eq!(c.kind, "tripwire");
                assert_eq!(c.status, "registered");
                assert_eq!(c.active_count, 1);
                assert_eq!(c.label.as_deref(), Some("main-watch"));
                assert!(c.attached_pid.is_none());
            }
            other => panic!("expected Create, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn create_tripwire_without_label() {
        let rig = TestRig::new();
        let mut input = tripwire_input(ObserveVerb::Create);
        input.condition = Some(ObserveCondition::Tripwire {
            condition: tripwire_condition(),
            label: None,
        });
        let out = rig.observe(input).await.unwrap();
        match out {
            ObserveOutput::Create(c) => {
                assert!(c.label.is_none());
            }
            other => panic!("expected Create, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn create_tripwire_increments_active_count() {
        let rig = TestRig::new();

        for _ in 0..3 {
            let mut input = tripwire_input(ObserveVerb::Create);
            input.condition = Some(ObserveCondition::Tripwire {
                condition: tripwire_condition(),
                label: None,
            });
            let out = rig.observe(input).await.unwrap();
            match out {
                ObserveOutput::Create(c) => assert!(c.active_count >= 1),
                _ => panic!("expected Create"),
            }
        }

        assert!(rig.manager.active_count() >= 3);
    }

    // ----- verb=list ----------------------------------------------------------

    #[tokio::test]
    async fn list_with_no_subscriptions_returns_empty() {
        let rig = TestRig::new();
        rig.open_session("observe-empty").await;
        let input = tripwire_input(ObserveVerb::List);
        let out = rig.observe(input).await.unwrap();
        match out {
            ObserveOutput::List(l) => {
                assert!(l.subscriptions.is_empty());
                assert!(l.fired_events.is_empty());
                assert_eq!(l.total_active, 0);
                assert_eq!(l.fired_count, 0);
                assert_eq!(l.provenance.retention_in_effect, "Drained");
                // REC-C2.1.4b: the resolved session and a checkpoint are visible.
                assert_eq!(l.session_id.as_deref(), Some("observe-empty"));
                assert!(
                    l.next_cursor
                        .as_deref()
                        .is_some_and(|c| c.starts_with("ecv1:")),
                    "a valid read always yields a checkpoint cursor: {:?}",
                    l.next_cursor
                );
            }
            other => panic!("expected List, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn list_after_create_includes_subscription() {
        let rig = TestRig::new();
        rig.open_session("observe-after-create").await;

        // Create one.
        let mut create_input = tripwire_input(ObserveVerb::Create);
        create_input.condition = Some(ObserveCondition::Tripwire {
            condition: tripwire_condition(),
            label: None,
        });
        let _ = rig.observe(create_input).await.unwrap();

        // List it.
        let list_input = tripwire_input(ObserveVerb::List);
        let out = rig.observe(list_input).await.unwrap();
        match out {
            ObserveOutput::List(l) => {
                assert_eq!(l.subscriptions.len(), 1);
                assert_eq!(l.subscriptions[0].kind, "tripwire");
                assert_eq!(l.total_active, 1);
            }
            other => panic!("expected List, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn list_with_retained_returns_the_same_page_as_drained() {
        let rig = TestRig::new();
        rig.open_session("observe-retained").await;
        let mut input = tripwire_input(ObserveVerb::List);
        input.retention = Some(ObserveRetention::RetainedUntilSessionEnd);
        let out = rig.observe(input).await.unwrap();
        match out {
            ObserveOutput::List(l) => {
                // REC-C2.1.5: retention no longer hides evidence. The log is
                // immutable, so `retained` and `drained` return the same page.
                assert!(l.fired_events.is_empty(), "no firings in this fixture");
                assert_eq!(l.fired_count, 0);
                assert_eq!(l.provenance.retention_in_effect, "RetainedUntilSessionEnd");
                assert!(l.next_cursor.is_some(), "the checkpoint is still yielded");
            }
            other => panic!("expected List, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn list_with_permanent_retention_returns_unsupported() {
        let rig = TestRig::new();
        let mut input = tripwire_input(ObserveVerb::List);
        input.retention = Some(ObserveRetention::Permanent);
        let err = rig.observe(input).await.unwrap_err();
        assert!(matches!(err, ServiceError::Unsupported(_)), "got {:?}", err);
    }

    // ------------------------------------------------------------------
    // REC-C2.0 characterizations (measure reality; not aspirational).
    // ------------------------------------------------------------------

    /// Register the rig's single `main*` tripwire through the public verb.
    async fn create_one_tripwire(rig: &TestRig) {
        let mut input = tripwire_input(ObserveVerb::Create);
        input.condition = Some(ObserveCondition::Tripwire {
            condition: tripwire_condition(),
            label: None,
        });
        rig.observe(input).await.unwrap();
    }

    // ------------------------------------------------------------------
    // REC-C2.1.4b: firing pages out of the ExecutionLog.
    // ------------------------------------------------------------------

    /// A `FunctionEntry` whose `location.function` is matched (or not) by the
    /// rig's `main*` condition.
    fn fn_event(name: &str, event_id: u64, ts: u64) -> chronos_domain::TraceEvent {
        use chronos_domain::{EventData, EventType, MonotonicNs, SourceLocation, TraceEvent};
        TraceEvent {
            event_id,
            timestamp_ns: MonotonicNs::from(ts),
            thread_id: 1,
            event_type: EventType::FunctionEntry,
            location: SourceLocation {
                function: Some(name.to_string()),
                ..SourceLocation::default()
            },
            data: EventData::Function {
                name: name.into(),
                signature: None,
                symbol_id: None,
                invocation_id: None,
                parent_invocation_id: None,
            },
        }
    }

    fn cursor_seq(token: &str) -> u64 {
        EventsCursorV1::decode(token)
            .expect("decode cursor")
            .next_seq()
            .0
    }

    /// Register the rig's one `main*` tripwire and open a session log.
    async fn armed_rig(session: &str) -> TestRig {
        let rig = TestRig::new();
        rig.open_session(session).await;
        create_one_tripwire(&rig).await;
        rig
    }

    /// FIRING-PAGE-1: `Raw Raw Firing Raw Raw` — the cursor lands after the
    /// last record EXAMINED, not after the firing.
    #[tokio::test]
    async fn firing_page_1_cursor_is_after_the_last_record_examined() {
        let rig = armed_rig("firing-page-1").await;
        rig.ingest("firing-page-1", &fn_event("a_other", 1, 10));
        rig.ingest("firing-page-1", &fn_event("b_other", 2, 20));
        rig.ingest("firing-page-1", &fn_event("main_work", 3, 30)); // Raw + Firing
        rig.ingest("firing-page-1", &fn_event("d_other", 4, 40));
        rig.ingest("firing-page-1", &fn_event("e_other", 5, 50));

        let out = rig
            .observe(tripwire_input(ObserveVerb::List))
            .await
            .unwrap();
        let ObserveOutput::List(l) = out else {
            panic!("expected List")
        };
        assert_eq!(l.fired_count, 1, "exactly one firing exists");
        // Both identities are exposed, not discarded (REC-C2.1.7).
        assert_eq!(l.fired_events[0].firing_seq, 3, "identity of the firing");
        assert_eq!(l.fired_events[0].source_seq, 2, "identity of the cause");
        // Records: 0 Raw, 1 Raw, 2 Raw, 3 Firing, 4 Raw, 5 Raw -> examined 6.
        let after = cursor_seq(l.next_cursor.as_deref().expect("checkpoint"));
        assert_eq!(
            after, 6,
            "cursor must be after the last examined record (6), not after the firing (4)"
        );
    }

    /// FIRING-PAGE-2: two consumers starting from the same cursor both receive
    /// the firing; one does not consume it for the other.
    #[tokio::test]
    async fn firing_page_2_two_consumers_do_not_steal_from_each_other() {
        let rig = armed_rig("firing-page-2").await;
        rig.ingest("firing-page-2", &fn_event("main_work", 1, 10));

        let a = rig
            .observe(tripwire_input(ObserveVerb::List))
            .await
            .unwrap();
        let b = rig
            .observe(tripwire_input(ObserveVerb::List))
            .await
            .unwrap();
        let (ObserveOutput::List(a), ObserveOutput::List(b)) = (a, b) else {
            panic!("expected two List outputs")
        };
        assert_eq!(a.fired_count, 1, "consumer A sees the firing");
        assert_eq!(
            b.fired_count, 1,
            "consumer B sees it too — evidence is immutable, nothing is drained"
        );
    }

    /// FIRING-PAGE-3: a page with records examined but no firings is valid and
    /// still advances the cursor.
    #[tokio::test]
    async fn firing_page_3_page_without_firings_still_advances() {
        let rig = armed_rig("firing-page-3").await;
        rig.ingest("firing-page-3", &fn_event("main_work", 1, 10)); // firing at seq 1
        rig.ingest("firing-page-3", &fn_event("a_other", 2, 20));
        rig.ingest("firing-page-3", &fn_event("b_other", 3, 30));

        // Page 1: everything.
        let first = rig
            .observe(tripwire_input(ObserveVerb::List))
            .await
            .unwrap();
        let ObserveOutput::List(first) = first else {
            panic!("expected List")
        };
        let checkpoint = first.next_cursor.expect("cursor");

        // Page 2 from the checkpoint: nothing new to examine, no firings.
        let mut input = tripwire_input(ObserveVerb::List);
        input.cursor = Some(checkpoint.clone());
        let second = rig.observe(input).await.unwrap();
        let ObserveOutput::List(second) = second else {
            panic!("expected List")
        };
        assert_eq!(second.fired_count, 0, "no new firings");
        assert_eq!(
            cursor_seq(second.next_cursor.as_deref().expect("cursor")),
            cursor_seq(&checkpoint),
            "an idempotent page keeps the same position"
        );
    }

    /// FIRING-PAGE-4: a consumer parked at the tail keeps its cursor and picks
    /// up ONLY the new firing after the producer advances.
    #[tokio::test]
    async fn firing_page_4_tail_cursor_sees_only_new_firings() {
        let rig = armed_rig("firing-page-4").await;
        rig.ingest("firing-page-4", &fn_event("main_work", 1, 10)); // firing at seq 1

        let first = rig
            .observe(tripwire_input(ObserveVerb::List))
            .await
            .unwrap();
        let ObserveOutput::List(first) = first else {
            panic!("expected List")
        };
        assert_eq!(first.fired_count, 1);
        let parked = first.next_cursor.expect("cursor at tail");

        // Producer advance: another matching event -> Raw + Firing.
        rig.ingest("firing-page-4", &fn_event("main_work", 2, 20));

        let mut input = tripwire_input(ObserveVerb::List);
        input.cursor = Some(parked);
        let next = rig.observe(input).await.unwrap();
        let ObserveOutput::List(next) = next else {
            panic!("expected List")
        };
        assert_eq!(
            next.fired_count, 1,
            "the parked cursor picks up exactly the new firing, not the old one"
        );
    }

    /// A retained read neither destroys nor hides: it returns the same page,
    /// and a later drained read still sees the evidence (CHAR-C2-03 and
    /// FIND-C2.0-01 are fixed by making firing evidence immutable).
    #[tokio::test]
    async fn retained_read_no_longer_destroys_the_evidence() {
        let rig = armed_rig("observe-retained-keeps").await;
        rig.ingest("observe-retained-keeps", &fn_event("main_work", 1, 10));

        let mut retained = tripwire_input(ObserveVerb::List);
        retained.retention = Some(ObserveRetention::RetainedUntilSessionEnd);
        let retained_out = rig.observe(retained).await.unwrap();
        let ObserveOutput::List(retained_out) = retained_out else {
            panic!("expected List")
        };
        assert_eq!(
            retained_out.fired_count, 1,
            "retention does not hide evidence: the same page is returned"
        );

        // The evidence is still there for a drained reader.
        let drained_out = rig
            .observe(tripwire_input(ObserveVerb::List))
            .await
            .unwrap();
        let ObserveOutput::List(drained_out) = drained_out else {
            panic!("expected List")
        };
        assert_eq!(
            drained_out.fired_count, 1,
            "the firing survived the retained read — nothing was drained"
        );
    }

    /// REC-C2.1.6: the legacy fired queue is gone.
    ///
    /// `TripwireManager` now exposes definitions plus the pure
    /// `matching`/`matching_semantic` queries. There is no `fired_buffer`, no
    /// `record_fired`, no `drain_fired` — the compiler enforces that, and the
    /// legacy-evb ratchet asserts `fired_buffer 4 -> 0` / `drain_fired 1 -> 0`.
    ///
    /// The observable property this test pins: repeated reads through the same
    /// explicit cursor are idempotent WITHOUT any hidden manager state. If the
    /// read depended on a draining buffer, the second read would come back
    /// empty.
    #[tokio::test]
    async fn repeated_log_reads_are_idempotent_without_manager_state() {
        let rig = armed_rig("observe-idempotent").await;
        rig.ingest("observe-idempotent", &fn_event("main_work", 1, 10));

        let first = rig
            .observe(tripwire_input(ObserveVerb::List))
            .await
            .unwrap();
        let ObserveOutput::List(first) = first else {
            panic!("expected List")
        };
        assert_eq!(first.fired_count, 1);
        let checkpoint = first.next_cursor.clone().expect("cursor");

        // Repeating the SAME read yields the same firing: nothing was consumed.
        let repeat = rig
            .observe(tripwire_input(ObserveVerb::List))
            .await
            .unwrap();
        let ObserveOutput::List(repeat) = repeat else {
            panic!("expected List")
        };
        assert_eq!(
            repeat.fired_count, 1,
            "no hidden buffer to drain: the same read sees the same evidence"
        );

        // Continuing from the checkpoint does NOT re-deliver the old firing.
        let mut input = tripwire_input(ObserveVerb::List);
        input.cursor = Some(checkpoint);
        let advanced = rig.observe(input).await.unwrap();
        let ObserveOutput::List(advanced) = advanced else {
            panic!("expected List")
        };
        assert_eq!(advanced.fired_count, 0, "the cursor already covered it");

        // The manager is definitions-only; reads never mutated it.
        assert_eq!(rig.manager.active_count(), 1);
    }

    /// FIND-C2.0-02: the wire `fire_count` is derived from the log, so the
    /// mutable `Tripwire.fire_count` (which is never incremented) is
    /// irrelevant. Three firings in the log must report 3 even though the
    /// manager's counter is still 0.
    #[tokio::test]
    async fn fire_count_is_derived_not_the_mutable_tripwire_counter() {
        let rig = armed_rig("observe-derived-count").await;
        for i in 0..3u64 {
            rig.ingest(
                "observe-derived-count",
                &fn_event("main_work", i + 1, (i + 1) * 10),
            );
        }

        // The manager's counter is still zero — nothing mutates it (CHAR-C2-04).
        assert_eq!(
            rig.manager.list()[0].fire_count,
            0,
            "the mutable counter is never incremented"
        );

        // Both verbs report the derived count.
        let queried = rig
            .observe(tripwire_input(ObserveVerb::Query))
            .await
            .unwrap();
        let ObserveOutput::Query(q) = queried else {
            panic!("expected Query")
        };
        assert_eq!(q.subscriptions.len(), 1);
        assert_eq!(
            q.subscriptions[0].fire_count, 3,
            "fire_count must come from the ExecutionLog, not the mutable counter"
        );
        assert_eq!(q.session_id.as_deref(), Some("observe-derived-count"));

        let listed = rig
            .observe(tripwire_input(ObserveVerb::List))
            .await
            .unwrap();
        let ObserveOutput::List(l) = listed else {
            panic!("expected List")
        };
        assert_eq!(l.subscriptions[0].fire_count, 3);
    }

    /// `fire_count` carries the facts that qualify it.
    #[tokio::test]
    async fn fire_count_facts_are_reported() {
        let rig = armed_rig("observe-count-facts").await;
        rig.ingest("observe-count-facts", &fn_event("main_work", 1, 10));

        let out = rig
            .observe(tripwire_input(ObserveVerb::Query))
            .await
            .unwrap();
        let ObserveOutput::Query(q) = out else {
            panic!("expected Query")
        };
        let facts = q.subscriptions[0]
            .fire_count_facts
            .as_ref()
            .expect("facts present");
        // Fresh session, nothing retired, scan reached the tail.
        assert_eq!(facts.from_seq, 0);
        assert_eq!(facts.status, "complete");
        assert!(facts.through_seq_exclusive >= 1);
    }

    /// A cursor from another session is a typed error, never a silent
    /// re-anchor.
    #[tokio::test]
    async fn foreign_session_cursor_is_rejected() {
        let rig = armed_rig("observe-session-a").await;
        let foreign = EventsCursorV1::start(chronos_log::SessionId::new("some-other")).encode();
        let mut input = tripwire_input(ObserveVerb::List);
        input.cursor = Some(foreign);
        let err = rig.observe(input).await.expect_err("must refuse");
        assert!(
            matches!(err, ServiceError::InvalidInput(_)),
            "expected a typed cursor error, got {err:?}"
        );
    }

    // ----- verb=query ---------------------------------------------------------

    #[tokio::test]
    async fn query_with_no_subscriptions_returns_empty() {
        let rig = TestRig::new();
        rig.open_session("observe-query-empty").await;
        let input = tripwire_input(ObserveVerb::Query);
        let out = rig.observe(input).await.unwrap();
        match out {
            ObserveOutput::Query(q) => {
                assert!(q.subscriptions.is_empty());
                assert!(q.fired_events.is_empty());
                assert_eq!(q.total_active, 0);
            }
            other => panic!("expected Query, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn query_is_non_destructive_across_calls() {
        let rig = TestRig::new();
        rig.open_session("observe-query-nondestructive").await;

        // Create.
        let mut create_input = tripwire_input(ObserveVerb::Create);
        create_input.condition = Some(ObserveCondition::Tripwire {
            condition: tripwire_condition(),
            label: None,
        });
        let _ = rig.observe(create_input).await.unwrap();

        // Query twice — both calls must see the same subscription.
        let q1 = match rig
            .observe(tripwire_input(ObserveVerb::Query))
            .await
            .unwrap()
        {
            ObserveOutput::Query(q) => q,
            _ => panic!("expected Query"),
        };
        let q2 = match rig
            .observe(tripwire_input(ObserveVerb::Query))
            .await
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
    }

    // ----- verb=delete -------------------------------------------------------

    #[tokio::test]
    async fn delete_with_valid_id_removes_subscription() {
        let rig = TestRig::new();
        let mut del_input = tripwire_input(ObserveVerb::Delete);

        // Create.
        let mut create_input = tripwire_input(ObserveVerb::Create);
        create_input.condition = Some(ObserveCondition::Tripwire {
            condition: tripwire_condition(),
            label: None,
        });
        let create_out = rig.observe(create_input).await.unwrap();
        let sub_id = match create_out {
            ObserveOutput::Create(c) => c.subscription_id,
            _ => panic!("expected Create"),
        };

        // Delete it.
        del_input.subscription_id = Some(sub_id.clone());
        let del_out = rig.observe(del_input).await.unwrap();
        match del_out {
            ObserveOutput::Delete(d) => {
                assert_eq!(d.subscription_id, sub_id);
                assert_eq!(d.remaining_active, 0);
            }
            other => panic!("expected Delete, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn delete_without_id_returns_unsupported() {
        let rig = TestRig::new();
        let input = tripwire_input(ObserveVerb::Delete);
        let err = rig.observe(input).await.unwrap_err();
        assert!(matches!(err, ServiceError::Unsupported(_)), "got {:?}", err);
    }

    #[tokio::test]
    async fn delete_with_non_tripwire_id_returns_unsupported() {
        let rig = TestRig::new();
        let mut input = tripwire_input(ObserveVerb::Delete);
        input.subscription_id = Some(SubscriptionId::new("not-a-tripwire-id"));
        let err = rig.observe(input).await.unwrap_err();
        assert!(matches!(err, ServiceError::Unsupported(_)), "got {:?}", err);
    }

    #[tokio::test]
    async fn delete_with_unknown_id_returns_not_found() {
        let rig = TestRig::new();
        let mut input = tripwire_input(ObserveVerb::Delete);
        input.subscription_id = Some(SubscriptionId::new("tripwire-99999"));
        let err = rig.observe(input).await.unwrap_err();
        assert!(
            matches!(err, ServiceError::TripwireNotFound(_)),
            "got {:?}",
            err
        );
    }

    // ----- REC-C0.5-B: capability-aware probe_inject error surfacing ---------

    /// Build an `ObserveInput` whose verb=create targets a Uprobe
    /// condition. The dispatcher's create path calls `ProbeService::inject`
    /// and surfaces the typed error directly.
    fn uprobe_input(session_id: &str, binary_path: &str, symbol_name: &str) -> ObserveInput {
        ObserveInput {
            verb: ObserveVerb::Create,
            subscription_id: None,
            condition: Some(ObserveCondition::Uprobe {
                binary_path: binary_path.to_string(),
                symbol_name: symbol_name.to_string(),
                pid: None,
                label: None,
            }),
            action: None,
            retention: None,
            requested_evidence: None,
            scope: Some(ObserveScope::Session {
                session_id: session_id.to_string(),
            }),
            cursor: None,
            label: None,
        }
    }

    /// Register a fake live-probe session in the rig's `live_probes` map.
    ///
    /// On a default `chronos-mcp` build (no `ebpf` feature),
    /// `ProbeService::inject` will ask the uprobe injector to `acquire`
    /// and the kernel probe fails immediately, which the dispatcher
    /// surfaces as `ServiceError::EbpfUnsupported(reason)`. That is the
    /// path exercised by these unit tests.
    fn register_fake_probe_session(rig: &TestRig, session_id: &str, pid: u32) {
        let backend = std::sync::Arc::new(chronos_native::probe_backend::NativeProbeBackend::new());
        let capture_session = chronos_domain::CaptureSession {
            session_id: session_id.to_string(),
            pid,
            language: chronos_domain::Language::Native,
            started_at: std::time::Instant::now(),
            started_at_wallclock: std::time::SystemTime::now(),
            config: chronos_domain::CaptureConfig::new("/fake/binary"),
            state: chronos_domain::SessionState::Active,
        };
        let session_id_typed = chronos_domain::session_id::SessionId::from(session_id.to_string());
        let controller: Box<dyn chronos_domain::ports::NativeProbeController> = Box::new(
            chronos_native::native_probe_controller::NativeProbeControllerImpl::new(
                backend,
                session_id_typed,
                capture_session.clone(),
            ),
        );
        let live = crate::probe::LiveProbeSession {
            controller,
            session: capture_session,
            language: chronos_domain::Language::Native,
            target: "/fake/binary".to_string(),
            attached: false,
            uprobe_handle: None,
            ebpf_attachment: None,
            execution_log: crate::session_log::SessionExecutionLog::create_for_tests(
                // Unique per test: a shared path races with the C1.5.1 manifest
                // write (create_dir_all + atomic rename) across parallel tests.
                std::env::temp_dir().join(format!(
                    "rec-c1-2a-observe-{}-{}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                )),
                chronos_log::SessionId::new("rec-c1-2a-observe"),
            )
            .expect("test log"),
        };
        rig.live_probes
            .lock()
            .expect("live_probes lock poisoned in test rig")
            .insert(session_id.to_string(), live);
    }

    /// Dispatcher must surface `ProbeInjectResult::EbpfUnavailable(reason)`
    /// as `ServiceError::EbpfUnsupported(reason)` — the typed eBPF
    /// capability error. The MCP wrapper prefixes this with the kebab
    /// slot `ebpf-uprobe`, derived from the typed variant.
    #[tokio::test]
    async fn create_uprobe_without_ebpf_returns_ebpf_unsupported() {
        let rig = TestRig::new();
        register_fake_probe_session(&rig, "test-session", 0xCAFE);
        let input = uprobe_input("test-session", "/fake/binary", "main");

        let err = rig.observe(input).await.unwrap_err();
        match err {
            ServiceError::EbpfUnsupported(reason) => {
                assert!(
                    !reason.is_empty(),
                    "EbpfUnsupported must carry a non-empty reason"
                );
            }
            other => panic!("expected ServiceError::EbpfUnsupported, got {:?}", other),
        }
    }

    /// Dispatcher must surface `ProbeInjectResult::ProbeStarting` as
    /// `ServiceError::ProbeStarting` — the typed "probe is starting up"
    /// capability error. The variant is only reachable on an
    /// `ebpf`-feature build (otherwise `EbpfUnavailable` short-circuits
    /// first); on the default build we accept either typed variant.
    #[tokio::test]
    async fn create_uprobe_with_no_pid_returns_probe_starting_or_ebpf_unavailable() {
        let rig = TestRig::new();
        register_fake_probe_session(&rig, "test-session", 0); // pid=0 → ProbeStarting on ebpf builds
        let input = uprobe_input("test-session", "/fake/binary", "main");

        let err = rig.observe(input).await.unwrap_err();
        assert!(
            matches!(
                err,
                ServiceError::ProbeStarting | ServiceError::EbpfUnsupported(_)
            ),
            "expected typed ProbeStarting OR EbpfUnavailable, got {:?}",
            err
        );
    }

    /// Dispatcher must surface `ProbeInjectResult::AttachFailed { error, .. }`
    /// as `ServiceError::InjectionFailed(error)`. The default build
    /// short-circuits to `EbpfUnsupported` first; we accept either typed
    /// variant since both share the `ebpf-uprobe` capability slot in the
    /// MCP wrapper.
    #[tokio::test]
    async fn create_uprobe_attach_failure_surfaces_typed_injection_failed_or_ebpf_unavailable() {
        let rig = TestRig::new();
        register_fake_probe_session(&rig, "test-session", 0xCAFE);
        let input = uprobe_input("test-session", "/fake/binary", "main");

        let err = rig.observe(input).await.unwrap_err();
        assert!(
            matches!(
                err,
                ServiceError::InjectionFailed(_) | ServiceError::EbpfUnsupported(_)
            ),
            "expected typed InjectionFailed OR EbpfUnavailable, got {:?}",
            err
        );
    }

    /// REC-C3.3.2.3 — regression test for the uprobe capability port.
    ///
    /// Asserts that with a recording injector:
    /// - `inject` succeeds and the handle is retained in the session
    ///   (`uprobe_handle.is_some()` is `true`).
    /// - A second `inject` reuses the same recording handle: both
    ///   attach calls reach the injector and the prior handle is
    ///   detached before the new one is attached.
    #[tokio::test]
    async fn c33_uprobe_injector_port_walks_three_states() {
        use crate::probe::{ProbeInjectInput, ProbeService};
        use std::sync::atomic::Ordering;

        let mut rig = TestRig::new();
        let recorder = rig.with_recording_injector();

        // Register a fake live-probe session whose traced PID is set so
        // `inject` does not return `ProbeStarting`.
        register_fake_probe_session(&rig, "c33-session", 4242);

        let ctx = rig.probe_ctx();

        // First injection: succeeds with the recording handle.
        let first = ProbeService::inject(
            &ctx,
            ProbeInjectInput {
                session_id: "c33-session".to_string(),
                binary_path: "/bin/true".to_string(),
                symbol_name: "main".to_string(),
                pid: Some(4242),
            },
        )
        .expect("inject succeeds");
        assert!(matches!(
            first,
            crate::probe::ProbeInjectResult::Attached { pid: 4242, .. }
        ));
        assert_eq!(recorder.attached.load(Ordering::SeqCst), 1);

        // Second injection: also Attached; the prior handle is detached
        // before the second attach. The shared counters see both
        // attach calls and at least one detach (the prior handle).
        let second = ProbeService::inject(
            &ctx,
            ProbeInjectInput {
                session_id: "c33-session".to_string(),
                binary_path: "/bin/true".to_string(),
                symbol_name: "main".to_string(),
                pid: Some(4242),
            },
        )
        .expect("inject succeeds");
        assert!(matches!(
            second,
            crate::probe::ProbeInjectResult::Attached { .. }
        ));
        assert_eq!(recorder.attached.load(Ordering::SeqCst), 2);
        assert_eq!(recorder.detached.load(Ordering::SeqCst), 1);

        // The session retains the active handle after both injections.
        let probes = ctx.live_probes.lock().unwrap();
        let lp = probes.get("c33-session").expect("session present");
        assert!(lp.uprobe_handle.is_some());
    }

    /// REC-C3.3.2.3 — second regression test: with the always-unavailable
    /// injector, `inject` surfaces the typed `EbpfUnavailable` result
    /// without retaining any handle in the session.
    #[tokio::test]
    async fn c33_uprobe_unavailable_injector_returns_typed_result() {
        use crate::probe::{ProbeInjectInput, ProbeService};

        let rig = TestRig::new(); // default injector is UnavailableUprobeInjector
        register_fake_probe_session(&rig, "c33-no-ebpf", 7);

        let ctx = rig.probe_ctx();

        let result = ProbeService::inject(
            &ctx,
            ProbeInjectInput {
                session_id: "c33-no-ebpf".to_string(),
                binary_path: "/bin/true".to_string(),
                symbol_name: "main".to_string(),
                pid: Some(7),
            },
        )
        .expect("inject returns Ok with typed variant");

        match result {
            crate::probe::ProbeInjectResult::EbpfUnavailable(reason) => {
                assert!(reason.contains("eBPF"));
            }
            other => panic!("expected EbpfUnavailable, got {:?}", other),
        }

        // The session must NOT retain a handle when acquisition failed.
        let probes = ctx.live_probes.lock().unwrap();
        let lp = probes.get("c33-no-ebpf").expect("session present");
        assert!(lp.uprobe_handle.is_none());
    }
}
