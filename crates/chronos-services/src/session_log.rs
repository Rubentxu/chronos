//! REC-C1.2/C3.3.2 — the session OWNS its `ExecutionLog`.
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
//! ## What this type is (REC-C3.3.2.5)
//!
//! [`SessionExecutionLog`] is the single authoritative handle to a session's
//! durable event log. It carries:
//!
//! * the [`SessionId`] the log belongs to — derived from the provider, never
//!   duplicated from the caller,
//! * the directory the segments live in, when the wrapper knows (pure
//!   provenance metadata, NOT on the evidence port),
//! * three trait objects sharing one concrete instance:
//!   `Arc<dyn ExecutionLogProvider>` (evidence),
//!   `Arc<dyn ExecutionLogRetention>` (logical frontier),
//!   `Arc<dyn ExecutionLogMaintenance>` (physical reclamation).
//!
//! ## Boundary discipline (REC-C3.3.2.5)
//!
//! ```text
//! CANONICAL EVIDENCE (port: ExecutionLogProvider)
//!     append, record_gap, read_from_seq, retained_from, tail_seq,
//!     tail_state, seal
//!
//! LOGICAL RETENTION (port: ExecutionLogRetention)
//!     advance_retained_from, retained_from, highest_allocated
//!
//! PHYSICAL MAINTENANCE (port: ExecutionLogMaintenance)
//!     flush, compact_retired, metrics
//! ```
//!
//! There is **no** `compact_up_to(seq)` on the wrapper. Maintenance has no
//! opinion on which evidence is logically reachable; that is the retention
//! port's job. The application layer first moves the boundary via
//! `advance_retained_from`, then asks maintenance to reclaim what is
//! already logically gone.
//!
//! There is **no** `ProviderKind` enum and **no** downcast. The capability
//! split turned maintenance into a first-class port; the application layer
//! calls `maintenance.flush()` instead of routing through a closed-tag
//! enum on the wrapper.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use chronos_domain::ports::execution_log::ExecutionLogProvider;
use chronos_domain::ports::execution_log_factory::{ExecutionLogCapabilities, ExecutionLogFactory};
use chronos_domain::ports::execution_log_maintenance::{
    CompactionMetrics, CompactionReport, ExecutionLogMaintenance, ExecutionLogMaintenanceError,
};
use chronos_domain::ports::execution_log_retention::{
    ExecutionLogRetention, RetentionError, RetentionOutcome,
};
use chronos_log::SessionId;

use crate::error::ServiceError;
use crate::events_cursor::EventsCursorV1;

/// Authoritative handle to one session's durable execution log.
pub struct SessionExecutionLog {
    session_id: SessionId,
    /// Where the segments live, when this handle knows. `None` when
    /// the session adopted a log that was opened elsewhere.
    ///
    /// **NOT on the provider port.** This is wrapper-level provenance.
    dir: Option<PathBuf>,
    /// Canonical-evidence port. The single Arc shared with retention
    /// and maintenance (see module docs).
    log: Arc<dyn ExecutionLogProvider>,
    /// Logical-retention port. Same concrete instance as `log`.
    retention: Arc<dyn ExecutionLogRetention>,
    /// Physical-maintenance port. Same concrete instance as `log`.
    maintenance: Arc<dyn ExecutionLogMaintenance>,
}

impl Clone for SessionExecutionLog {
    fn clone(&self) -> Self {
        Self {
            session_id: self.session_id.clone(),
            dir: self.dir.clone(),
            log: Arc::clone(&self.log),
            retention: Arc::clone(&self.retention),
            maintenance: Arc::clone(&self.maintenance),
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
    /// Build a capability bundle from a freshly-opened
    /// `SegmentedExecutionLog` and wrap it in a `SessionExecutionLog`.
    ///
    /// Production callers should NOT need this: the composition root
    /// drives `factory.create(...)`. This helper exists for bridge
    /// call sites (today: tests, observation adapters that open the
    /// concrete log directly) that already have an
    /// `Arc<SegmentedExecutionLog>` and need a bundle around it.
    ///
    /// The function lives on `SessionExecutionLog` (not in
    /// `test_support`) because the native probe bridge in
    /// `chronos_native` will produce the same shape once it consumes
    /// the port directly.
    pub fn from_segmented_log(
        session_id: SessionId,
        inner: Arc<chronos_log::SegmentedExecutionLog>,
        dir: Option<PathBuf>,
    ) -> Self {
        let provider: Arc<dyn ExecutionLogProvider> = Arc::new(
            chronos_log::SegmentedExecutionLogProvider::new(session_id.clone(), inner.clone()),
        );
        let retention: Arc<dyn ExecutionLogRetention> = Arc::new(
            chronos_log::SegmentedRetention::new(session_id, inner.clone()),
        );
        let maintenance: Arc<dyn ExecutionLogMaintenance> =
            Arc::new(chronos_log::SegmentedMaintenance::new(inner));
        let bundle = ExecutionLogCapabilities {
            evidence: provider,
            retention,
            maintenance,
        };
        Self::from_capabilities(bundle, dir)
    }

    // -----------------------------------------------------------------
    // Capability-bundle constructor (REC-C3.3.2.5).
    //
    // The session identity is taken from the evidence port — never
    // from the caller — so the wrapper cannot accidentally carry a
    // stale or mismatched `SessionId`.
    // -----------------------------------------------------------------

    /// Canonical constructor (REC-C3.3.2.5).
    ///
    /// Bundles the three trait objects sharing one concrete instance.
    pub fn from_capabilities(capabilities: ExecutionLogCapabilities, dir: Option<PathBuf>) -> Self {
        let session_id = capabilities.evidence.session_id().clone();
        Self {
            session_id,
            dir,
            log: capabilities.evidence,
            retention: capabilities.retention,
            maintenance: capabilities.maintenance,
        }
    }

    /// Adopt a capability bundle when the caller already holds a
    /// `SessionId` and needs identity confirmed before adopting it.
    pub fn from_capabilities_for_session(
        expected: &SessionId,
        capabilities: ExecutionLogCapabilities,
        dir: Option<PathBuf>,
    ) -> Result<Self, ServiceError> {
        let actual = capabilities.evidence.session_id();
        if actual != expected {
            return Err(ServiceError::ExecutionLogIdentityMismatch {
                expected: expected.as_str().to_string(),
                actual: actual.as_str().to_string(),
            });
        }
        Ok(Self::from_capabilities(capabilities, dir))
    }

    // -----------------------------------------------------------------
    // Transitional factories — REC-C3.3.2 construction is delegated
    // to the injected `Arc<dyn ExecutionLogFactory>`. The factory
    // is not stored on the struct — bootstrap supplies it for the
    // call and may build the same factory at the composition root.
    // -----------------------------------------------------------------

    /// Reopen an EXISTING durable log (REC-C1.5.4).
    ///
    /// Never creates, never infers. Bootstrap uses this exclusively.
    pub fn reopen_existing(
        dir: impl AsRef<Path>,
        session_id: SessionId,
        factory: &Arc<dyn ExecutionLogFactory>,
    ) -> Result<Self, ServiceError> {
        let dir = dir.as_ref().to_path_buf();
        let capabilities = factory
            .reopen_existing(dir.clone(), session_id)
            .map_err(|e| {
                ServiceError::ProbeStartFailed(format!("reopen {}: {e}", dir.display()))
            })?;
        Ok(Self::from_capabilities(capabilities, Some(dir)))
    }

    /// Create (or open) the log for a NEW session under `dir`.
    pub fn create(
        dir: impl AsRef<Path>,
        session_id: SessionId,
        factory: &Arc<dyn ExecutionLogFactory>,
    ) -> Result<Self, ServiceError> {
        let dir = dir.as_ref().to_path_buf();
        let capabilities = factory.create(dir.clone(), session_id).map_err(|e| {
            ServiceError::ProbeStartFailed(format!("create ExecutionLog at {}: {e}", dir.display()))
        })?;
        Ok(Self::from_capabilities(capabilities, Some(dir)))
    }

    /// Test-only convenience: create with the segmented backend
    /// directly. Production code MUST use `create(..., factory)` or
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

    /// Take ownership of a capability bundle handed in by an external
    /// component (the native probe backend in C3.3.2.6 / Tren B once
    /// it consumes the port directly).
    ///
    /// Production code outside this module uses
    /// [`from_capabilities`](Self::from_capabilities) when the bundle
    /// is already built; this constructor exists for bridge calls
    /// where the bridge caller has the bundle and needs identity
    /// confirmed.
    pub fn adopt_capabilities(
        expected: &SessionId,
        capabilities: ExecutionLogCapabilities,
        dir: Option<PathBuf>,
    ) -> Result<Self, ServiceError> {
        Self::from_capabilities_for_session(expected, capabilities, dir)
    }

    // -----------------------------------------------------------------
    // Canonical evidence operations — go through the port, NEVER
    // through any concrete reference.
    // -----------------------------------------------------------------

    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    /// Earliest queryable seq for this log (REC-C1.5.1).
    pub fn retained_from(&self) -> chronos_log::EventSeq {
        self.log.retained_from()
    }

    /// The current tail seq.
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

    /// The shared evidence provider. The ONLY way for callers outside
    /// this module to reach the canonical-evidence port.
    pub fn provider(&self) -> Arc<dyn ExecutionLogProvider> {
        Arc::clone(&self.log)
    }

    /// Backwards-compatible alias for [`provider`](Self::provider).
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
    pub fn cursor_start(&self) -> EventsCursorV1 {
        EventsCursorV1::start(self.session_id.clone())
    }

    // -----------------------------------------------------------------
    // Logical retention port — moves the boundary forward only.
    // -----------------------------------------------------------------

    /// Move the logical retention frontier to `new_retained_from`.
    ///
    /// Maintenance port's `compact_retired()` then reclaims what is
    /// already logically out of scope. The split is deliberate —
    /// `compact_up_to(seq)` was deleted because maintenance has no
    /// opinion on which evidence is logically reachable.
    pub fn advance_retained_from(
        &self,
        new_retained_from: chronos_domain::seq::EventSeq,
    ) -> Result<RetentionOutcome, ServiceError> {
        self.retention
            .advance_retained_from(new_retained_from)
            .map_err(map_retention_error)
    }

    /// Counter snapshot of the maintenance port.
    pub fn compaction_metrics(&self) -> Result<CompactionMetrics, ServiceError> {
        Ok(self.maintenance.metrics())
    }

    /// Flush pending writes through the maintenance port.
    pub fn flush(&self) -> Result<(), ServiceError> {
        self.maintenance.flush().map_err(map_maintenance_error)
    }

    /// Reclaim physical storage for evidence strictly before the
    /// current retention frontier.
    pub fn compact_retired(&self) -> Result<CompactionReport, ServiceError> {
        self.maintenance
            .compact_retired()
            .map_err(map_maintenance_error)
    }
}

/// Translate `RetentionError` into `ServiceError`. Distinct from the
/// evidence error translator because the retention port fails for
/// reasons the evidence port never sees (backwards move, sealed, …).
fn map_retention_error(err: RetentionError) -> ServiceError {
    use RetentionError::*;
    match err {
        BackwardsMove { requested, current } => ServiceError::RetentionBackwardsMove {
            requested: requested.get(),
            current: current.get(),
        },
        PastAllocated {
            requested,
            highest_allocated,
        } => ServiceError::RetentionPastAllocated {
            requested: requested.get(),
            highest_allocated: highest_allocated.get(),
        },
        Sealed { session_id } => ServiceError::RetentionSealed { session_id },
        Unavailable { detail } => {
            ServiceError::DrainFailed(format!("retention unavailable: {detail}"))
        }
    }
}

/// Translate `ExecutionLogMaintenanceError` into `ServiceError`.
fn map_maintenance_error(err: ExecutionLogMaintenanceError) -> ServiceError {
    use ExecutionLogMaintenanceError::*;
    match err {
        Unavailable { detail } => {
            ServiceError::DrainFailed(format!("maintenance unavailable: {detail}"))
        }
        Inconsistent { detail } => {
            ServiceError::DrainFailed(format!("maintenance inconsistent: {detail}"))
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
/// The registry lives for the whole logical life of the session:
///
/// ```text
/// session_start -> registry.register(clone of the SAME handle)
/// probe_stop    -> live_probes.remove() ; registry untouched
/// drop/delete   -> cleanup_session_memory removes the entry
/// ```
///
/// Reads never change source when the session changes state.
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
/// The registry owns the `Arc<dyn ExecutionLogFactory>` so every
/// `register_*` call that builds a log (create / reopen) goes through
/// the injected factory. The composition root
/// (`chronos_mcp::composition`) builds the factory once and hands it
/// to the registry at boot.
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

    /// Adopt a capability bundle built outside the registry (today:
    /// the native probe bridge; tomorrow: any composition-root that
    /// already holds the concrete).
    pub fn register_capabilities(
        &self,
        capabilities: ExecutionLogCapabilities,
        dir: Option<PathBuf>,
    ) -> Result<SessionExecutionLog, ServiceError> {
        let session_id = capabilities.evidence.session_id().clone();
        let log = SessionExecutionLog::from_capabilities(capabilities, dir);
        self.register(log.clone())?;
        Ok(log.with_session_marker(session_id))
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

// Helper trait used by `register_capabilities` to recover the
// original SessionId for the registry key without changing the
// wrapper's public surface. The wrapper already carries the id
// internally, so this is a no-op accessor the helper needs.
trait SessionMarker {
    fn with_session_marker(self, session_id: SessionId) -> Self;
}

impl SessionMarker for SessionExecutionLog {
    fn with_session_marker(self, session_id: SessionId) -> Self {
        // Sanity check: the wrapper already carries the id from the
        // evidence port. We re-assert it here so a regression that
        // drops the identity from the bundle becomes a hard failure
        // at the registry boundary.
        debug_assert_eq!(self.session_id, session_id);
        self
    }
}

#[cfg(test)]
mod tests {
    //! REC-C3.3.2 — substitutability at the services-consumer level.
    use super::*;
    use chronos_domain::seq::EventSeq;
    use chronos_log::{
        InMemoryExecutionLog, InMemoryExecutionLogProvider, SegmentedExecutionLog,
        SegmentedExecutionLogProvider,
    };

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

    fn segmented_log(session: &SessionId) -> SessionExecutionLog {
        let dir = tmpdir("swap-seg");
        let log = SegmentedExecutionLog::open(
            session.clone(),
            chronos_log::SegmentedConfig::with_dir(dir.clone()),
        )
        .expect("open segmented");
        let inner = Arc::new(log);
        let provider: Arc<dyn ExecutionLogProvider> = Arc::new(SegmentedExecutionLogProvider::new(
            session.clone(),
            inner.clone(),
        ));
        let retention: Arc<dyn ExecutionLogRetention> = Arc::new(
            chronos_log::SegmentedRetention::new(session.clone(), inner.clone()),
        );
        let maintenance: Arc<dyn ExecutionLogMaintenance> =
            Arc::new(chronos_log::SegmentedMaintenance::new(inner));
        SessionExecutionLog::from_capabilities(
            ExecutionLogCapabilities {
                evidence: provider,
                retention,
                maintenance,
            },
            Some(dir),
        )
    }

    fn in_memory_log(session: &SessionId) -> SessionExecutionLog {
        let inner = Arc::new(InMemoryExecutionLog::new());
        let provider: Arc<dyn ExecutionLogProvider> = Arc::new(InMemoryExecutionLogProvider::new(
            session.clone(),
            inner.clone(),
        ));
        let retention: Arc<dyn ExecutionLogRetention> = Arc::new(
            chronos_log::InMemoryRetention::new(session.clone(), inner.clone()),
        );
        let maintenance: Arc<dyn ExecutionLogMaintenance> =
            Arc::new(chronos_log::InMemoryMaintenance::new(session.clone()));
        SessionExecutionLog::from_capabilities(
            ExecutionLogCapabilities {
                evidence: provider,
                retention,
                maintenance,
            },
            None,
        )
    }

    /// End-to-end canonical-evidence exercise that closes C31-DEBT-01.
    fn exercise_session_log(label: &str, log: SessionExecutionLog) {
        let session = log.session_id().clone();

        let s0 = log.append(raw(&session, 0)).expect("append 0");
        let reserved_through = log
            .record_gap(chronos_log::Gap::new(
                EventSeq::new(1),
                EventSeq::new(3),
                chronos_log::GapReason::KernelRingOverflow,
                "test",
            ))
            .expect("record_gap");
        assert_eq!(
            reserved_through,
            EventSeq::new(3),
            "[{label}] reserved_through must equal gap.last_missing"
        );
        let s1 = log.append(raw(&session, 1)).expect("append after gap");
        assert_eq!(
            (s0, s1),
            (EventSeq::new(0), EventSeq::new(4)),
            "[{label}] gap must reserve its span"
        );

        let page = log.read_from_seq(EventSeq::ZERO, 10).expect("read");
        assert_eq!(page.records.len(), 2, "[{label}]");
        assert_eq!(page.gaps.len(), 1, "[{label}]");

        assert_eq!(log.retained_from(), log.provider().retained_from());
        assert_eq!(log.tail_seq(), log.provider().tail_seq());
        assert_eq!(log.session_id(), log.provider().session_id());

        let _ = log.seal().expect("seal");
        assert!(log.provider().tail_state().is_sealed());

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
    // Construction discipline: from_capabilities is canonical.
    // -----------------------------------------------------------------

    #[test]
    fn from_capabilities_takes_identity_from_evidence_port_not_caller() {
        let session = SessionId::new("provider-wins");
        let inner = Arc::new(InMemoryExecutionLog::new());
        let provider: Arc<dyn ExecutionLogProvider> = Arc::new(InMemoryExecutionLogProvider::new(
            session.clone(),
            inner.clone(),
        ));
        let retention: Arc<dyn ExecutionLogRetention> = Arc::new(
            chronos_log::InMemoryRetention::new(session.clone(), inner.clone()),
        );
        let maintenance: Arc<dyn ExecutionLogMaintenance> =
            Arc::new(chronos_log::InMemoryMaintenance::new(session.clone()));
        let wrapper = SessionExecutionLog::from_capabilities(
            ExecutionLogCapabilities {
                evidence: provider,
                retention,
                maintenance,
            },
            None,
        );
        assert_eq!(wrapper.session_id(), &session);
    }

    #[test]
    fn from_capabilities_for_session_rejects_mismatch() {
        let real = SessionId::new("native-1234");
        let inner = Arc::new(InMemoryExecutionLog::new());
        let provider: Arc<dyn ExecutionLogProvider> = Arc::new(InMemoryExecutionLogProvider::new(
            real.clone(),
            inner.clone(),
        ));
        let retention: Arc<dyn ExecutionLogRetention> = Arc::new(
            chronos_log::InMemoryRetention::new(real.clone(), inner.clone()),
        );
        let maintenance: Arc<dyn ExecutionLogMaintenance> =
            Arc::new(chronos_log::InMemoryMaintenance::new(real));
        let err = SessionExecutionLog::from_capabilities_for_session(
            &SessionId::new("service-uuid"),
            ExecutionLogCapabilities {
                evidence: provider,
                retention,
                maintenance,
            },
            None,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ServiceError::ExecutionLogIdentityMismatch { .. }
        ));
    }

    // -----------------------------------------------------------------
    // Maintenance: every adapter now has a coherent port implementation.
    // -----------------------------------------------------------------

    #[test]
    fn compaction_metrics_succeeds_on_both_adapters() {
        let seg = segmented_log(&SessionId::new("seg-cm"));
        let mem = in_memory_log(&SessionId::new("mem-cm"));
        let _ = seg
            .compaction_metrics()
            .expect("segmented compaction_metrics");
        let _ = mem
            .compaction_metrics()
            .expect("in-memory compaction_metrics");
    }

    #[test]
    fn flush_succeeds_on_both_adapters() {
        let seg = segmented_log(&SessionId::new("seg-flush"));
        let mem = in_memory_log(&SessionId::new("mem-flush"));
        seg.flush().expect("segmented flush");
        mem.flush().expect("in-memory flush");
    }

    #[test]
    fn compact_retired_succeeds_on_both_adapters() {
        let seg = segmented_log(&SessionId::new("seg-compact"));
        let mem = in_memory_log(&SessionId::new("mem-compact"));
        seg.compact_retired().expect("segmented compact_retired");
        mem.compact_retired().expect("in-memory compact_retired");
    }

    #[test]
    fn advance_retained_from_refuses_backwards_on_both_adapters() {
        let seg = segmented_log(&SessionId::new("seg-bw"));
        let mem = in_memory_log(&SessionId::new("mem-bw"));
        // First advance to a non-zero frontier so a subsequent request
        // for 99 must be rejected as backwards.
        seg.append(raw(seg.session_id(), 0)).expect("append");
        seg.append(raw(seg.session_id(), 1)).expect("append");
        seg.advance_retained_from(EventSeq::new(2))
            .expect("advance to 2");
        mem.append(raw(mem.session_id(), 0)).expect("append");
        mem.append(raw(mem.session_id(), 1)).expect("append");
        mem.advance_retained_from(EventSeq::new(2))
            .expect("advance to 2");

        let err = seg
            .advance_retained_from(EventSeq::new(1))
            .expect_err("backwards move must fail");
        assert!(
            matches!(err, ServiceError::RetentionBackwardsMove { .. }),
            "got {err:?}"
        );
        let err = mem
            .advance_retained_from(EventSeq::new(1))
            .expect_err("backwards move must fail");
        assert!(
            matches!(err, ServiceError::RetentionBackwardsMove { .. }),
            "got {err:?}"
        );
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
        assert!(Arc::ptr_eq(&a, &log.handle()));
    }

    #[test]
    fn cursor_is_minted_from_the_owned_session() {
        let log = in_memory_handle(&SessionId::new("sess-cursor"));
        let cursor = log.cursor_start();
        assert_eq!(cursor.session_id(), log.session_id());
        assert_eq!(cursor.next_seq(), EventSeq::ZERO);
    }

    #[test]
    fn debug_does_not_leak_the_handle() {
        let log = in_memory_handle(&SessionId::new("sess-d"));
        let s = format!("{log:?}");
        assert!(s.contains("sess-d"));
        assert!(s.contains("SessionExecutionLog"));
    }
}
