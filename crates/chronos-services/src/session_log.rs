//! REC-C1.2 — the session OWNS its `ExecutionLog`.
//!
//! ## Why this exists
//!
//! ```text
//! AgenticSession
//!      │ owns
//!      ▼
//! ExecutionLog        <-- introduced here (C1.2)
//!      ▲ reads
//!      │
//! events_read         <-- cut over in C1.3
//! ```
//!
//! Ownership must come before the reader. If `events_read` were changed first,
//! the read path would inevitably grow another fallback/transitional branch to
//! cope with "the log might not be attached", and that branch would then have
//! to be deleted again in REC-C2.
//!
//! ## What this type is (after REC-C3.3.1)
//!
//! [`SessionExecutionLog`] is the single authoritative handle to a session's
//! durable event log. It carries:
//!
//! * the [`SessionId`] the log belongs to — derived from the provider, never
//!   duplicated from the caller,
//! * the directory the segments live in, when the wrapper knows (pure
//!   provenance metadata, NOT on the evidence port),
//! * the open provider behind `Arc<dyn ExecutionLogProvider>`.
//!
//! The canonical constructor is [`SessionExecutionLog::from_provider`].
//!
//! ## Boundary discipline (REC-C3.3.1)
//!
//! ```text
//! CANONICAL — port only, never escape hatch
//!     append
//!     record_gap
//!     read_from_seq
//!     retained_from
//!     tail_seq
//!     tail_state
//!     seal
//!
//! ESCAPE HATCH — concrete adapter, temporary
//!     flush
//!     compaction_metrics
//!     maybe_compact
//!     compact_up_to
//!     retain_up_to (see C33-DEBT-RETENTION-01)
//! ```
//!
//! The wrapper keeps a `ProviderKind` enum tag built at construction
//! time so maintenance calls route to the right concrete backend
//! without `Any`. The tag is ONLY inspected by maintenance
//! capabilities; canonical-evidence operations always go through the
//! trait object directly. The operator forbids `match provider.kind()`
//! anywhere else in the workspace.
//!
//! ## Composition inversion (REC-C3.3.2, not yet)
//!
//! Commit 7 inverts the FIELD (the wrapper carries `Arc<dyn>`).
//! Composition root inversion — building the provider outside
//! services — is C3.3.2 and is deliberately NOT in this commit.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use chronos_domain::ports::execution_log::ExecutionLogProvider;
use chronos_domain::ports::execution_log_factory::ExecutionLogFactory;
use chronos_log::{SegmentedExecutionLog, SegmentedExecutionLogProvider, SessionId};

use crate::error::ServiceError;
use crate::events_cursor::EventsCursorV1;

/// Local downcast seam (REC-C3.3.1).
///
/// Built at construction time. Maintenance capabilities
/// (`compaction_metrics`, `maybe_compact`, `flush`,
/// `compact_up_to`, `as_segmented_backend`) match on this enum.
///
/// NOT inspected by canonical-evidence operations. NOT inspected
/// anywhere outside `session_log.rs`.
pub(crate) enum ProviderKind {
    Segmented(Arc<SegmentedExecutionLog>),
    InMemory(Arc<chronos_log::InMemoryExecutionLog>),
    /// The wrapper holds an adapter we do not know how to maintain
    /// (a future composition root could inject one). Maintenance
    /// calls return `ExecutionLogMaintenanceUnsupported`.
    Other,
}

fn tag_segmented(inner: Arc<SegmentedExecutionLog>) -> ProviderKind {
    ProviderKind::Segmented(inner)
}

#[allow(dead_code)]
fn tag_inmemory(inner: Arc<chronos_log::InMemoryExecutionLog>) -> ProviderKind {
    ProviderKind::InMemory(inner)
}

fn tag_other() -> ProviderKind {
    ProviderKind::Other
}

/// Authoritative handle to one session's durable execution log.
pub struct SessionExecutionLog {
    session_id: SessionId,
    /// Where the segments live, when this handle knows. `None` when
    /// the session adopted a log that was opened elsewhere.
    ///
    /// **NOT on the provider port.** This is wrapper-level provenance.
    dir: Option<PathBuf>,
    /// The provider. Canonical-evidence operations go through this
    /// directly; never through any concrete `SegmentedExecutionLog`
    /// reference.
    log: Arc<dyn ExecutionLogProvider>,
    /// Closed enum tag for the maintenance escape hatch (see
    /// module docs).
    kind: ProviderKind,
}

impl Clone for SessionExecutionLog {
    fn clone(&self) -> Self {
        Self {
            session_id: self.session_id.clone(),
            dir: self.dir.clone(),
            log: Arc::clone(&self.log),
            kind: match &self.kind {
                ProviderKind::Segmented(s) => ProviderKind::Segmented(Arc::clone(s)),
                ProviderKind::InMemory(m) => ProviderKind::InMemory(Arc::clone(m)),
                ProviderKind::Other => ProviderKind::Other,
            },
        }
    }
}

impl std::fmt::Debug for SessionExecutionLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionExecutionLog")
            .field("session_id", &self.session_id)
            .field("dir", &self.dir)
            .finish_non_exhaustive()
    }
}

impl SessionExecutionLog {
    // -----------------------------------------------------------------
    // Canonical constructors (REC-C3.3.1).
    //
    // The canonical path takes the provider from outside. The
    // wrapper never builds a concrete adapter; that is C3.3.2.
    // -----------------------------------------------------------------

    /// Canonical constructor (REC-C3.3.1).
    ///
    /// The session identity is taken from the provider — never from
    /// the caller — so the wrapper cannot accidentally carry a stale
    /// or mismatched `SessionId`.
    pub fn from_provider(provider: Arc<dyn ExecutionLogProvider>, dir: Option<PathBuf>) -> Self {
        let session_id = provider.session_id().clone();
        // The canonical path cannot know the concrete adapter
        // behind `Arc<dyn>` without `Any`. Maintenance capabilities
        // therefore come back typed-unavailable until a future
        // composition root supplies a tagged variant.
        let kind = tag_other();
        Self {
            session_id,
            dir,
            log: provider,
            kind,
        }
    }

    /// Adopt a provider when the caller already holds a `SessionId`
    /// and needs identity confirmed before adopting it.
    pub fn from_provider_for_session(
        expected: &SessionId,
        provider: Arc<dyn ExecutionLogProvider>,
        dir: Option<PathBuf>,
    ) -> Result<Self, ServiceError> {
        let actual = provider.session_id();
        if actual != expected {
            return Err(ServiceError::ExecutionLogIdentityMismatch {
                expected: expected.as_str().to_string(),
                actual: actual.as_str().to_string(),
            });
        }
        Ok(Self::from_provider(provider, dir))
    }

    /// Internal constructor used by the transitional factories
    /// (`reopen_existing`, `create`, `try_adopt`) AND by tests.
    /// Production code outside this module uses
    /// [`from_provider`](Self::from_provider).
    pub(crate) fn from_provider_with_concrete(
        provider: Arc<dyn ExecutionLogProvider>,
        dir: Option<PathBuf>,
        kind: ProviderKind,
    ) -> Self {
        let session_id = provider.session_id().clone();
        Self {
            session_id,
            dir,
            log: provider,
            kind,
        }
    }

    // -----------------------------------------------------------------
    // Transitional factories — REC-C3.3.1 keeps the field
    // inverted but the construction inside services for now.
    // C3.3.2 moves these to `chronos-mcp::composition`.
    //
    // The field is `Arc<dyn ExecutionLogProvider>` (canonical
    // shape) but the adapter still happens to be the segmented one
    // because that is the only storage backend with a working
    // open-on-disk lifecycle in services today.
    //
    // **Composition inversion (C3.3.2):** when these factories
    // move, `from_provider` is the only constructor left.
    // -----------------------------------------------------------------

    /// Reopen an EXISTING durable log (REC-C1.5.4).
    ///
    /// Never creates, never infers. Bootstrap uses this exclusively.
    ///
    /// **C3.3.2 composition inversion:** construction is delegated
    /// to the injected `Arc<dyn ExecutionLogFactory>`. The factory
    /// is not stored on the struct — bootstrap supplies it for the
    /// call and may build the same factory at the composition root.
    ///
    /// Maintenance capabilities (`flush`, `compaction_metrics`,
    /// `compact_up_to`) return `ExecutionLogMaintenanceUnsupported`
    /// because the composition root owns the concrete adapter now.
    /// C3.3.2.2 closes this gap when the native probe bridge retires.
    pub fn reopen_existing(
        dir: impl AsRef<Path>,
        session_id: SessionId,
        factory: &Arc<dyn ExecutionLogFactory>,
    ) -> Result<Self, ServiceError> {
        let dir = dir.as_ref().to_path_buf();
        let provider = factory
            .reopen_existing(dir.clone(), session_id)
            .map_err(|e| {
                ServiceError::ProbeStartFailed(format!("reopen {}: {e}", dir.display()))
            })?;
        Ok(Self::from_provider(provider, Some(dir)))
    }

    /// Create (or open) the log for a NEW session under `dir`.
    ///
    /// **C3.3.2 composition inversion:** construction is delegated to
    /// the injected `Arc<dyn ExecutionLogFactory>`. Maintenance
    /// capabilities return `ExecutionLogMaintenanceUnsupported` until
    /// C3.3.2.2 retires the native-log bridge (see `reopen_existing`
    /// for the long-form note).
    pub fn create(
        dir: impl AsRef<Path>,
        session_id: SessionId,
        factory: &Arc<dyn ExecutionLogFactory>,
    ) -> Result<Self, ServiceError> {
        let dir = dir.as_ref().to_path_buf();
        let provider = factory.create(dir.clone(), session_id).map_err(|e| {
            ServiceError::ProbeStartFailed(format!("create ExecutionLog at {}: {e}", dir.display()))
        })?;
        Ok(Self::from_provider(provider, Some(dir)))
    }

    /// Test-only convenience: create with the segmented backend
    /// directly so maintenance capabilities (`flush`, `compact_up_to`,
    /// `retain_up_to`) keep working until the native-log bridge
    /// retires in C3.3.2.2.
    ///
    /// Production code MUST use `create(..., factory)` or
    /// `SessionExecutionLogRegistry::register_create`; this helper
    /// exists so unit tests in this crate do not need a factory
    /// argument for every fixture.
    ///
    /// R6 (no concrete adapter construction in production services)
    /// is upheld: this helper is gated to non-production builds.
    /// Production binaries cannot call it because the symbol only
    /// exists when `cfg(test)` or `feature = "test-utils"` is set.
    #[cfg(any(test, feature = "test-utils"))]
    #[doc(hidden)]
    pub fn create_for_tests(
        dir: impl AsRef<Path>,
        session_id: SessionId,
    ) -> Result<Self, ServiceError> {
        crate::test_support::create_with_segmented_backend(dir, session_id)
    }

    /// Test-only convenience: reopen with the segmented backend
    /// directly. Same contract as `create_for_tests`.
    #[cfg(any(test, feature = "test-utils"))]
    #[doc(hidden)]
    pub fn reopen_existing_for_tests(
        dir: impl AsRef<Path>,
        session_id: SessionId,
    ) -> Result<Self, ServiceError> {
        crate::test_support::reopen_existing_with_segmented_backend(dir, session_id)
    }

    /// Take ownership of a `SegmentedExecutionLog` opened elsewhere.
    ///
    /// **C3.3.2 transition:** this is the LAST site in `chronos_services`
    /// that names the concrete `SegmentedExecutionLog`. It exists only
    /// because the native probe backend (`chronos_native`) opens the
    /// concrete during the C3.3.1 bridge. C3.3.2.2 retires the bridge;
    /// after that the backend consumes the `ExecutionLogFactory` port
    /// directly and this method loses its sole remaining call site.
    pub fn try_adopt(
        dir: Option<PathBuf>,
        session_id: SessionId,
        handle: Arc<SegmentedExecutionLog>,
    ) -> Result<Self, ServiceError> {
        let expected = session_id;
        let actual_sid = handle.session_id().clone();
        if actual_sid != expected {
            return Err(ServiceError::ExecutionLogIdentityMismatch {
                expected: expected.as_str().to_string(),
                actual: actual_sid.as_str().to_string(),
            });
        }
        let provider: Arc<dyn ExecutionLogProvider> = Arc::new(SegmentedExecutionLogProvider::new(
            actual_sid,
            Arc::clone(&handle),
        ));
        Ok(Self::from_provider_with_concrete(
            provider,
            dir,
            tag_segmented(handle),
        ))
    }

    // -----------------------------------------------------------------
    // Canonical evidence operations — go through the port, NEVER
    // through any concrete reference. These are the operations the
    // operator marks as "CANONICAL — only port".
    // -----------------------------------------------------------------

    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    /// Earliest queryable seq for this log (REC-C1.5.1).
    pub fn retained_from(&self) -> chronos_log::EventSeq {
        self.log.retained_from()
    }

    /// The current tail seq (REC-C3.3.1: separate from `tail_state`
    /// because an `Open` session may carry a non-None tail).
    pub fn tail_seq(&self) -> Option<chronos_log::EventSeq> {
        self.log.tail_seq()
    }

    /// What is known about the end of the execution (REC-C1.5.3).
    pub fn tail_state(&self) -> chronos_log::TailState {
        self.log.tail_state()
    }

    pub fn dir(&self) -> Option<&Path> {
        self.dir.as_deref()
    }

    /// The shared provider. The ONLY way for callers outside this
    /// module to reach the underlying backend.
    pub fn provider(&self) -> Arc<dyn ExecutionLogProvider> {
        Arc::clone(&self.log)
    }

    /// Backwards-compatible alias for [`provider`](Self::provider).
    ///
    /// **The name is a misnomer left over from when the wrapper
    /// carried a concrete `SegmentedExecutionLog`.** Since C3.3.1
    /// this returns `Arc<dyn ExecutionLogProvider>`, which exposes
    /// only the evidence port — NOT storage-mechanism methods
    /// (`flush`, `compaction_metrics`, `maybe_compact`). Use
    /// [`provider`](Self::provider) in new code; this alias will be
    /// deleted in C3.3.2.
    pub fn handle(&self) -> Arc<dyn ExecutionLogProvider> {
        Arc::clone(&self.log)
    }

    /// Append a record (canonical evidence).
    pub fn append(
        &self,
        record: chronos_log::NewExecutionRecord,
    ) -> Result<chronos_log::EventSeq, ServiceError> {
        self.log.append(record).map_err(map_execution_log_error)
    }

    /// Record an explicit gap (canonical evidence).
    ///
    /// Routes through the provider port, never through a maintenance
    /// escape hatch. See module docs.
    pub fn record_gap(&self, gap: chronos_log::Gap) -> Result<chronos_log::EventSeq, ServiceError> {
        self.log.record_gap(gap).map_err(map_execution_log_error)
    }

    /// Read records and gaps from `from` (inclusive).
    pub fn read_from_seq(
        &self,
        from: chronos_log::EventSeq,
        limit: usize,
    ) -> Result<chronos_domain::ports::execution_log::ExecutionLogPage, ServiceError> {
        self.log
            .read_from_seq(from, limit)
            .map_err(map_execution_log_error)
    }

    /// Durably seal this session's execution log after a clean
    /// lifecycle stop.
    pub fn seal(&self) -> Result<chronos_log::SealedTail, ServiceError> {
        self.log
            .seal()
            .map_err(|e| ServiceError::ProbeStartFailed(format!("seal execution log: {e}")))
    }

    /// Fresh read cursor for this log's session.
    ///
    /// Bridge between ownership (C1.2) and the authoritative cursor
    /// (C1.1): the cursor can only be minted from a session that
    /// owns a log, so it can never point at a session with no log
    /// behind it.
    pub fn cursor_start(&self) -> EventsCursorV1 {
        EventsCursorV1::start(self.session_id.clone())
    }

    // -----------------------------------------------------------------
    // Maintenance escape hatch — concrete adapter only, temporary.
    //
    // Every method in this block routes through the `ProviderKind`
    // enum tag. Adding a new adapter means extending the enum, NOT
    // adding a new downcast path.
    // -----------------------------------------------------------------

    /// Compaction counters (m1-07 surface).
    ///
    /// **Storage-maintenance capability, not part of the evidence
    /// port.** Returns
    /// `ExecutionLogMaintenanceUnsupported` for non-segmented
    /// adapters; callers must decide whether to react or ignore
    /// explicitly.
    pub fn compaction_metrics(&self) -> Result<chronos_log::CompactionMetrics, ServiceError> {
        match &self.kind {
            ProviderKind::Segmented(s) => Ok(s.compaction_metrics()),
            _ => Err(ServiceError::ExecutionLogMaintenanceUnsupported {
                capability: "compaction_metrics".to_string(),
            }),
        }
    }

    /// Run one compaction pass; returns the removed segment paths.
    pub fn maybe_compact(&self) -> Result<Vec<PathBuf>, ServiceError> {
        match &self.kind {
            ProviderKind::Segmented(s) => s
                .maybe_compact()
                .map_err(|e| ServiceError::DrainFailed(format!("{e:?}"))),
            _ => Err(ServiceError::ExecutionLogMaintenanceUnsupported {
                capability: "maybe_compact".to_string(),
            }),
        }
    }

    /// Compact segments strictly before `cutoff`. Returns the
    /// removed segment paths. Maintenance capability — only valid
    /// for the segmented adapter (C33-DEBT-RETENTION-01).
    pub fn compact_up_to(
        &self,
        cutoff: chronos_domain::seq::EventSeq,
    ) -> Result<Vec<PathBuf>, ServiceError> {
        match &self.kind {
            ProviderKind::Segmented(s) => s
                .compact_up_to(cutoff)
                .map_err(|e| ServiceError::DrainFailed(format!("{e:?}"))),
            _ => Err(ServiceError::ExecutionLogMaintenanceUnsupported {
                capability: "compact_up_to".to_string(),
            }),
        }
    }

    /// Retention: trim evidence strictly before `cutoff`. Returns
    /// the outcome (new `retained_from`, removed segments, and any
    /// filesystem reclaim failures). Maintenance capability — only
    /// valid for the segmented adapter (C33-DEBT-RETENTION-01).
    pub fn retain_up_to(
        &self,
        cutoff: chronos_domain::seq::EventSeq,
    ) -> Result<chronos_log::CompactionOutcome, ServiceError> {
        match &self.kind {
            ProviderKind::Segmented(s) => s
                .retain_up_to(cutoff)
                .map_err(|e| ServiceError::DrainFailed(format!("{e:?}"))),
            _ => Err(ServiceError::ExecutionLogMaintenanceUnsupported {
                capability: "retain_up_to".to_string(),
            }),
        }
    }

    /// Flush pending writes to disk. Maintenance capability.
    /// Returns `Ok(())` once pending writes are durable; the
    /// segment path returned by `SegmentedExecutionLog::flush`
    /// is discarded (compaction decision lives in `maybe_compact`).
    pub fn flush(&self) -> Result<(), ServiceError> {
        match &self.kind {
            ProviderKind::Segmented(s) => s
                .flush()
                .map(|_| ())
                .map_err(|e| ServiceError::DrainFailed(format!("{e:?}"))),
            _ => Err(ServiceError::ExecutionLogMaintenanceUnsupported {
                capability: "flush".to_string(),
            }),
        }
    }

    /// Borrow the underlying `SegmentedExecutionLog` when the
    /// provider is the segmented adapter — **narrow native-bridge
    /// escape hatch (C33-DEBT-NATIVE-LOG-BRIDGE-01)**.
    ///
    /// Allowed call sites (and ONLY these):
    /// - `NativeProbeBackend::attach_execution_log`
    /// - `chronos_native::read_log_with_stats`
    ///
    /// Every other caller MUST go through the canonical-evidence
    /// path. The next cycle (C3.3.2/C3.3.3) moves these native
    /// callers to the port and deletes this bridge.
    pub fn legacy_segmented_backend_for_native_bridge(&self) -> Option<Arc<SegmentedExecutionLog>> {
        match &self.kind {
            ProviderKind::Segmented(s) => Some(Arc::clone(s)),
            _ => None,
        }
    }

    /// Storage-mechanism helper: a `record_gap` invocation already
    /// reserves the span; this is the bridge between the port's
    /// canonical-evidence path and the legacy `record_gap(session_id,
    /// gap)` callers that take `Arc<SegmentedExecutionLog>` directly.
    ///
    /// **Maintenance-adjacent escape hatch.** Returns
    /// `ExecutionLogMaintenanceUnsupported` for non-segmented adapters.
    pub fn record_gap_on_segmented(&self, gap: chronos_log::Gap) -> Result<(), ServiceError> {
        match &self.kind {
            ProviderKind::Segmented(s) => s
                .record_gap(gap)
                .map_err(|e| ServiceError::DrainFailed(format!("{e:?}"))),
            _ => Err(ServiceError::ExecutionLogMaintenanceUnsupported {
                capability: "record_gap_on_segmented".to_string(),
            }),
        }
    }
}

/// Translate `ExecutionLogError` (port) into `ServiceError`
/// (services). The canonical-evidence ratchet: as the codebase moves
/// off legacy `LogError`, the count of `map_log_error` callers in
/// the canonical-evidence path must only decrease.
pub(crate) fn map_execution_log_error(
    err: chronos_domain::ports::execution_log::ExecutionLogError,
) -> ServiceError {
    use chronos_domain::ports::execution_log::ExecutionLogError;
    match err {
        ExecutionLogError::IdentityMismatch { expected, actual } => {
            ServiceError::ExecutionLogIdentityMismatch {
                expected: expected.as_str().to_string(),
                actual: actual.as_str().to_string(),
            }
        }
        // All other variants currently map to a plain drain failure
        // at the services boundary; finer-grained translation is a
        // follow-up. `Open` (C3.3.2) is a factory-level failure and
        // is routed to the same catch-all; composition-root callers
        // catch it at the boot boundary, not here.
        ExecutionLogError::Sealed { .. }
        | ExecutionLogError::PositionBeforeRetention { .. }
        | ExecutionLogError::IntegrityFailure { .. }
        | ExecutionLogError::InvalidGap { .. }
        | ExecutionLogError::Unavailable { .. }
        | ExecutionLogError::Open { .. } => ServiceError::DrainFailed(format!("{err}")),
    }
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// Session-scoped registry of `ExecutionLog` handles (REC-C1.3).
///
/// ## Why a registry and not a second map of finalized logs
///
/// The log used to be reachable only through `live_probes`, so
/// `probe_stop` destroyed the only route to it and a read of a
/// stopped session could only answer `SessionNotFound`. Keeping a
/// *second* `finalized_logs` map would mean two places that can hold
/// the log and a lifecycle transition to move it, including a window
/// ("removed from live, not yet inserted into finalized") where a
/// read fails.
///
/// The registry instead lives for the whole logical life of the
/// session:
///
/// ```text
/// session_start -> registry.register(clone of the SAME handle)
/// probe_stop    -> live_probes.remove() ; registry untouched
/// drop/delete   -> cleanup_session_memory removes the entry
/// ```
///
/// Reads never change source when the session changes state, and
/// there is no alternative-source chain to maintain.
#[derive(Clone)]
pub enum ExecutionLogRegistration {
    Available(SessionExecutionLog),
    Unavailable { reason: String },
}

impl std::fmt::Debug for ExecutionLogRegistration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Available(l) => f.debug_tuple("Available").field(l).finish(),
            Self::Unavailable { reason } => f
                .debug_struct("Unavailable")
                .field("reason", reason)
                .finish(),
        }
    }
}

/// In-process registry of `SessionExecutionLog` instances keyed by
/// `session_id`.
///
/// **C3.3.2 composition inversion:** the registry owns the
/// `Arc<dyn ExecutionLogFactory>` so every `register_*` call that
/// builds a log (create / reopen) goes through the injected factory.
/// The composition root (`chronos_mcp::composition`) builds the
/// factory once and hands it to the registry at boot.
///
/// `register_pre_adopted` is the **last bridge call site** that names
/// `SegmentedExecutionLog` concretely; C3.3.2.2 retires it when
/// `chronos_native` consumes the port directly.
pub struct SessionExecutionLogRegistry {
    factory: Arc<dyn ExecutionLogFactory>,
    logs: std::sync::Mutex<std::collections::HashMap<String, ExecutionLogRegistration>>,
}

impl Default for SessionExecutionLogRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for SessionExecutionLogRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ids: Vec<String> = self
            .logs
            .lock()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        f.debug_struct("SessionExecutionLogRegistry")
            .field("sessions", &ids)
            .finish()
    }
}

impl SessionExecutionLogRegistry {
    /// Build a registry backed by the given factory. This is the
    /// composition-root injection point: services do not name the
    /// concrete factory type.
    pub fn with_factory(factory: Arc<dyn ExecutionLogFactory>) -> Self {
        Self {
            factory,
            logs: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// Backwards-compatible constructor used by tests and by sites
    /// that have not yet been migrated to composition-root wiring.
    /// Wires a `SegmentedExecutionLogFactory` so call sites keep
    /// compiling during the C3.3.2 migration.
    pub fn new() -> Self {
        Self::with_factory(std::sync::Arc::new(
            chronos_log::factory::SegmentedExecutionLogFactory::new(),
        ))
    }

    /// Open (or create) the log for `session_id` under `dir` via the
    /// injected factory, register it, and return a handle.
    pub fn register_create(
        &self,
        dir: impl AsRef<Path>,
        session_id: SessionId,
    ) -> Result<SessionExecutionLog, ServiceError> {
        let log = SessionExecutionLog::create(dir, session_id, &self.factory)?;
        self.register(log.clone())?;
        Ok(log)
    }

    /// Reopen an existing durable log under `dir` via the injected
    /// factory, register it, and return a handle.
    pub fn register_reopen(
        &self,
        dir: impl AsRef<Path>,
        session_id: SessionId,
    ) -> Result<SessionExecutionLog, ServiceError> {
        let log = SessionExecutionLog::reopen_existing(dir, session_id, &self.factory)?;
        self.register(log.clone())?;
        Ok(log)
    }

    /// Adopt a concrete `SegmentedExecutionLog` opened elsewhere and
    /// register it. **Bridge-only** — C3.3.2.2 retires the native
    /// bridge and this method loses its sole remaining caller.
    pub fn register_pre_adopted(
        &self,
        dir: Option<PathBuf>,
        session_id: SessionId,
        handle: Arc<SegmentedExecutionLog>,
    ) -> Result<SessionExecutionLog, ServiceError> {
        let log = SessionExecutionLog::try_adopt(dir, session_id, handle)?;
        self.register(log.clone())?;
        Ok(log)
    }

    pub fn register(&self, log: SessionExecutionLog) -> Result<(), ServiceError> {
        let key = log.session_id().as_str().to_string();
        let mut map = self.logs.lock().map_err(|_| ServiceError::LockPoisoned)?;
        if let Some(existing) = map.get(&key) {
            if let ExecutionLogRegistration::Available(existing) = existing {
                if std::sync::Arc::ptr_eq(
                    &existing.provider() as &Arc<dyn ExecutionLogProvider>,
                    &log.provider(),
                ) {
                    return Ok(());
                }
            }
            return Err(ServiceError::ExecutionLogIdentityMismatch {
                expected: key.clone(),
                actual: format!("a different ExecutionLog handle for {key}"),
            });
        }
        map.insert(key, ExecutionLogRegistration::Available(log));
        Ok(())
    }

    pub fn get(&self, session_id: &str) -> Result<SessionExecutionLog, ServiceError> {
        let map = self.logs.lock().map_err(|_| ServiceError::LockPoisoned)?;
        match map.get(session_id) {
            Some(ExecutionLogRegistration::Available(log)) => Ok(log.clone()),
            Some(ExecutionLogRegistration::Unavailable { reason }) => {
                Err(ServiceError::ExecutionLogUnavailable {
                    session_id: session_id.to_string(),
                    reason: reason.clone(),
                })
            }
            None => Err(ServiceError::ExecutionLogUnavailable {
                session_id: session_id.to_string(),
                reason: "no ExecutionLog registered for this session (a session loaded \
                         from the session store has none yet; reopen belongs to REC-C1.5)"
                    .to_string(),
            }),
        }
    }

    pub fn register_unavailable(
        &self,
        session_id: &str,
        reason: impl Into<String>,
    ) -> Result<(), ServiceError> {
        let mut map = self.logs.lock().map_err(|_| ServiceError::LockPoisoned)?;
        map.insert(
            session_id.to_string(),
            ExecutionLogRegistration::Unavailable {
                reason: reason.into(),
            },
        );
        Ok(())
    }

    pub fn remove(&self, session_id: &str) -> Option<ExecutionLogRegistration> {
        self.logs
            .lock()
            .ok()
            .and_then(|mut map| map.remove(session_id))
    }

    pub fn contains(&self, session_id: &str) -> bool {
        self.logs
            .lock()
            .map(|map| map.contains_key(session_id))
            .unwrap_or(false)
    }

    pub fn len(&self) -> usize {
        self.logs.lock().map(|map| map.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    //! REC-C3.3.1 — substitutability at the services-consumer level.
    //!
    //! `exercise_session_log(log)` runs against the same logical
    //! sequence on both adapters. Identical observable behaviour
    //! closes C31-DEBT-01.
    use super::*;
    use chronos_log::{InMemoryExecutionLog, InMemoryExecutionLogProvider};

    fn tmpdir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "rec-c1-2-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    // Helper: build a record with the given monotonic_ns.
    fn raw(session: &SessionId, ns: u64) -> chronos_log::NewExecutionRecord {
        chronos_log::NewExecutionRecord {
            session_id: session.clone(),
            kind: chronos_log::ExecutionKind::Raw,
            monotonic_ns: ns,
            payload: chronos_log::ExecutionPayload::new(vec![ns as u8], "raw"),
            invocation_id: None,
            parent_invocation_id: None,
            symbol_id: None,
            captured_at_unix_ns: None,
        }
    }

    // -----------------------------------------------------------------
    // DoD of commit 7: provider-swap end-to-end through the
    // canonical-evidence path.
    // -----------------------------------------------------------------

    fn segmented_log(session: &SessionId) -> SessionExecutionLog {
        let dir = tmpdir("swap-seg");
        let log = SegmentedExecutionLog::open(
            session.clone(),
            chronos_log::SegmentedConfig::with_dir(dir.clone()),
        )
        .expect("open segmented");
        let provider: Arc<dyn ExecutionLogProvider> = Arc::new(SegmentedExecutionLogProvider::new(
            session.clone(),
            Arc::new(log.clone()),
        ));
        SessionExecutionLog::from_provider_with_concrete(
            provider,
            Some(dir),
            tag_segmented(Arc::new(log)),
        )
    }

    fn in_memory_log(session: &SessionId) -> SessionExecutionLog {
        let inner = Arc::new(InMemoryExecutionLog::new());
        let provider: Arc<dyn ExecutionLogProvider> = Arc::new(InMemoryExecutionLogProvider::new(
            session.clone(),
            inner.clone(),
        ));
        SessionExecutionLog::from_provider_with_concrete(provider, None, tag_inmemory(inner))
    }

    /// End-to-end canonical-evidence exercise. This is the test
    /// that closes C31-DEBT-01: the same logical sequence on both
    /// adapters produces the same observable behaviour.
    fn exercise_session_log(label: &str, log: SessionExecutionLog) {
        let session = log.session_id().clone();

        // append seq 0
        let s0 = log.append(raw(&session, 0)).expect("append 0");
        // record_gap 1..3 (canonical-evidence write)
        let reserved_through = log
            .record_gap(chronos_log::Gap::new(
                chronos_log::EventSeq::new(1),
                chronos_log::EventSeq::new(3),
                chronos_log::GapReason::KernelRingOverflow,
                "test",
            ))
            .expect("record_gap");
        assert_eq!(
            reserved_through,
            chronos_log::EventSeq::new(3),
            "[{label}] reserved_through must equal gap.last_missing"
        );
        // next append must jump past the gap
        let s1 = log.append(raw(&session, 1)).expect("append after gap");
        assert_eq!(
            (s0, s1),
            (chronos_log::EventSeq::new(0), chronos_log::EventSeq::new(4)),
            "[{label}] gap must reserve its span"
        );

        // read shows [record, gap, record]
        let page = log
            .read_from_seq(chronos_log::EventSeq::ZERO, 10)
            .expect("read");
        assert_eq!(page.records.len(), 2, "[{label}]");
        assert_eq!(page.gaps.len(), 1, "[{label}]");
        assert_eq!(page.gaps[0].first_missing, chronos_log::EventSeq::new(1));
        assert_eq!(page.gaps[0].last_missing, chronos_log::EventSeq::new(3));

        // wrapper-level accessors mirror the provider
        assert_eq!(log.retained_from(), log.provider().retained_from());
        assert_eq!(log.tail_seq(), log.provider().tail_seq());
        assert_eq!(log.session_id(), log.provider().session_id());

        // seal
        let _ = log.seal().expect("seal");
        assert!(log.provider().tail_state().is_sealed());

        // append after seal must fail at the provider boundary
        let err = log.append(raw(&session, 2)).expect_err("append after seal");
        assert!(
            matches!(err, ServiceError::DrainFailed(_)),
            "[{label}] expected drain-failed translation of Sealed, got {err:?}"
        );
    }

    #[test]
    fn session_log_drives_segmented_end_to_end() {
        exercise_session_log(
            "segmented",
            segmented_log(&SessionId::new("rec-c33-session-seg")),
        );
    }

    #[test]
    fn session_log_drives_in_memory_end_to_end() {
        exercise_session_log(
            "in-memory",
            in_memory_log(&SessionId::new("rec-c33-session-mem")),
        );
    }

    // -----------------------------------------------------------------
    // Construction discipline: from_provider is canonical; the
    // composition root will live there in C3.3.2.
    // -----------------------------------------------------------------

    #[test]
    fn from_provider_takes_identity_from_provider_not_caller() {
        let session = SessionId::new("provider-wins");
        let provider: Arc<dyn ExecutionLogProvider> = Arc::new(InMemoryExecutionLogProvider::new(
            session.clone(),
            Arc::new(InMemoryExecutionLog::new()),
        ));
        let wrapper = SessionExecutionLog::from_provider(provider, None);
        assert_eq!(wrapper.session_id(), &session);
    }

    #[test]
    fn from_provider_for_session_rejects_mismatch() {
        let real = SessionId::new("native-1234");
        let provider: Arc<dyn ExecutionLogProvider> = Arc::new(InMemoryExecutionLogProvider::new(
            real,
            Arc::new(InMemoryExecutionLog::new()),
        ));
        let err = SessionExecutionLog::from_provider_for_session(
            &SessionId::new("service-uuid"),
            provider,
            None,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ServiceError::ExecutionLogIdentityMismatch { .. }
        ));
    }

    // -----------------------------------------------------------------
    // Maintenance escape hatch: loud failures on non-segmented
    // adapters (no silent no-op).
    // -----------------------------------------------------------------

    #[test]
    fn compaction_metrics_fail_loudly_for_non_segmented_provider() {
        let session = SessionId::new("mem-maint");
        let log = in_memory_log(&session);
        let err = log.compaction_metrics().unwrap_err();
        assert!(matches!(
            err,
            ServiceError::ExecutionLogMaintenanceUnsupported { .. }
        ));
    }

    #[test]
    fn flush_fails_loudly_for_non_segmented_provider() {
        let session = SessionId::new("mem-flush");
        let log = in_memory_log(&session);
        let err = log.flush().unwrap_err();
        assert!(matches!(
            err,
            ServiceError::ExecutionLogMaintenanceUnsupported { .. }
        ));
    }

    #[test]
    fn maybe_compact_fails_loudly_for_non_segmented_provider() {
        let session = SessionId::new("mem-compact");
        let log = in_memory_log(&session);
        let err = log.maybe_compact().unwrap_err();
        assert!(matches!(
            err,
            ServiceError::ExecutionLogMaintenanceUnsupported { .. }
        ));
    }

    #[test]
    fn legacy_segmented_backend_for_native_bridge_returns_none_for_non_segmented() {
        let session = SessionId::new("mem-seg-bridge");
        let log = in_memory_log(&session);
        assert!(log.legacy_segmented_backend_for_native_bridge().is_none());
    }

    // -----------------------------------------------------------------
    // Registry invariants.
    // -----------------------------------------------------------------

    fn in_memory_handle(session: &SessionId) -> SessionExecutionLog {
        in_memory_log(session)
    }

    #[test]
    fn registry_keys_by_the_logs_own_identity_and_is_idempotent() {
        let log = in_memory_handle(&SessionId::new("sess-reg"));
        let registry = SessionExecutionLogRegistry::new();
        registry.register(log.clone()).expect("register");
        assert!(registry.contains("sess-reg"));
        registry
            .register(log.clone())
            .expect("idempotent re-register");
        let got = registry.get("sess-reg").expect("get");
        assert!(Arc::ptr_eq(&got.provider(), &log.provider()), "same Arc");
    }

    #[test]
    fn registry_refuses_to_swap_the_handle_for_an_existing_session() {
        let first = in_memory_handle(&SessionId::new("dup"));
        let second = in_memory_handle(&SessionId::new("dup"));
        let registry = SessionExecutionLogRegistry::new();
        registry.register(first.clone()).expect("first");
        let err = registry.register(second).unwrap_err();
        assert!(
            matches!(err, ServiceError::ExecutionLogIdentityMismatch { .. }),
            "{err:?}"
        );
        assert!(Arc::ptr_eq(
            &registry.get("dup").unwrap().provider(),
            &first.provider()
        ));
    }

    #[test]
    fn registry_reports_unavailable_distinctly_from_unknown() {
        let registry = SessionExecutionLogRegistry::new();
        let err = registry.get("never-existed").unwrap_err();
        match err {
            ServiceError::ExecutionLogUnavailable { session_id, reason } => {
                assert_eq!(session_id, "never-existed");
                assert!(reason.contains("REC-C1.5"), "{reason}");
            }
            other => panic!("expected ExecutionLogUnavailable, got {other:?}"),
        }
    }

    #[test]
    fn registry_remove_drops_the_entry() {
        let log = in_memory_handle(&SessionId::new("gone"));
        let registry = SessionExecutionLogRegistry::new();
        registry.register(log).expect("register");
        assert!(registry.remove("gone").is_some());
        assert!(!registry.contains("gone"));
        assert!(registry.get("gone").is_err());
    }

    #[test]
    fn closed_handle_is_shared_with_writers_not_transferred() {
        let log = in_memory_handle(&SessionId::new("sess-shared"));
        let a = log.provider();
        let b = log.provider();
        assert!(
            Arc::ptr_eq(&a, &b),
            "clones must share one provider, not fork it"
        );
        // `handle()` is now the same alias returning Arc<dyn>.
        assert!(Arc::ptr_eq(&a, &log.handle()));
    }

    #[test]
    fn cursor_is_minted_from_the_owned_session() {
        let log = in_memory_handle(&SessionId::new("sess-cursor"));
        let cursor = log.cursor_start();
        assert_eq!(cursor.session_id(), log.session_id());
        assert_eq!(cursor.next_seq(), chronos_log::EventSeq::ZERO);
    }

    #[test]
    fn debug_does_not_leak_the_handle() {
        let log = in_memory_handle(&SessionId::new("sess-d"));
        let s = format!("{log:?}");
        assert!(s.contains("sess-d"));
        assert!(s.contains("SessionExecutionLog"));
    }
}
