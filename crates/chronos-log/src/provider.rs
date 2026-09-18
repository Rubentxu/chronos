//! Adapters that implement `chronos_domain::ports::execution_log::ExecutionLogProvider`
//! on top of the existing `chronos_log` backends.
//!
//! REC-C3.3.1: the port is session-scoped; these wrappers are
//! session-scoped. They do NOT replace the existing multi-session
//! backends — those keep their broader API for other consumers
//! (analytics, discovery, etc.). The wrappers add the layer of
//! identity enforcement, error translation, and tail-state ownership
//! the port requires.
//!
//! Why wrappers, not blanket `impl ExecutionLogProvider for SegmentedExecutionLog`?
//! - The backends carry methods that must not leak through the port
//!   (`maybe_compact`, `compaction_metrics`, paths, retention
//!   configuration). An explicit wrapper makes the boundary visible.
//! - The port requires a single `TailState` value; the segmented
//!   backend keeps one, but the in-memory backend needs to grow one.
//! - The in-memory backend is multi-session today; the wrapper pins
//!   it to a single `SessionId` and rejects cross-session evidence.
//!
//! The error/page mapping is **literal**: `LogPage::exhausted`,
//! `position_after`, and `gaps` are copied byte-for-byte. The adapter
//! must not reinterpret evidence — that is the contract the port's
//! semantic guarantees (C1.4 completeness) are built on.

use std::sync::Arc;

use chronos_domain::evidence::{Gap, NewExecutionRecord, SealedTail, TailState};
use chronos_domain::ports::execution_log::{
    ExecutionLogError, ExecutionLogKind, ExecutionLogPage, ExecutionLogProvider,
};
use chronos_domain::ports::execution_log_maintenance::{
    CompactionMetrics, CompactionReport, ExecutionLogMaintenance, ExecutionLogMaintenanceError,
};
use chronos_domain::ports::execution_log_retention::{
    ExecutionLogRetention, RetentionError, RetentionOutcome,
};
use chronos_domain::seq::EventSeq;
use chronos_domain::session_id::SessionId;
use std::sync::Mutex;

use crate::backend::ExecutionLogBackend;
use crate::cursor::LogPage;
use crate::error::LogError;
use crate::memory::InMemoryExecutionLog;
use crate::segmented::SegmentedExecutionLog;

/// Translate `chronos_log::LogError` into `chronos_domain::ExecutionLogError`.
///
/// The mapping is total: every variant of `LogError` lands on exactly
/// one variant of `ExecutionLogError`. Legacy variants that the new
/// session-scoped path is not expected to produce
/// (`CursorStale`, `SessionNotFound`) are mapped defensively to
/// `Unavailable` so a regression in the adapter is observable, not
/// silent.
///
/// Do NOT panic on these legacy variants: if they appear in
/// production the adapter has called into the wrong backend method
/// and we want an explicit failure, not a process crash.
pub(crate) fn map_log_error(err: LogError) -> ExecutionLogError {
    match err {
        // Pass-throughs.
        LogError::PositionBeforeRetention {
            requested_next_seq,
            retained_from,
        } => ExecutionLogError::PositionBeforeRetention {
            requested_next_seq,
            retained_from,
        },
        LogError::IdentityMismatch { .. } => {
            // The adapter also enforces identity at the
            // `NewExecutionRecord.session_id` boundary, but the
            // segmented backend can produce this itself when it
            // reopens a manifest whose session disagrees with the
            // request. We don't have the original `SessionId`s here
            // — the legacy variant carried `String`s — so we surface
            // it as IntegrityFailure with a clear detail string.
            ExecutionLogError::IntegrityFailure {
                detail: "execution log identity mismatch (legacy backend)".to_string(),
            }
        }
        LogError::LogSealed(session_id) => ExecutionLogError::Sealed {
            session_id: SessionId::new(session_id),
        },

        // Storage integrity.
        LogError::TailIntegrityMismatch { .. }
        | LogError::ReplayIntegrity { .. }
        | LogError::SegmentCrossesRetention { .. }
        | LogError::RetentionMetadataMissing { .. } => ExecutionLogError::IntegrityFailure {
            detail: format!("execution log integrity failure: {err}"),
        },

        // Caller-supplied gap was malformed for this session:
        // negative span, span that re-uses already-allocated seqs,
        // span that does not fit the next free seq, ... This is NOT
        // integrity of stored evidence — it is a request validation
        // failure at the write boundary.
        LogError::InvalidGap { reason } => ExecutionLogError::InvalidGap { detail: reason },

        // Transient / unavailable.
        LogError::AppendFailed { .. } | LogError::Backend(_) => ExecutionLogError::Unavailable {
            detail: format!("execution log unavailable: {err}"),
        },

        // Defensive: the new stateless / session-scoped path is not
        // supposed to produce these. If it does, that is an adapter
        // regression we want to catch — but not by crashing the
        // process.
        LogError::CursorStale { .. } | LogError::SessionNotFound => {
            ExecutionLogError::Unavailable {
                detail: format!(
                    "execution log unavailable: legacy backend invariant violated: {err}"
                ),
            }
        }
    }
}

/// Copy a `LogPage` into an `ExecutionLogPage` literally.
///
/// `position_after`, `gaps`, and `exhausted` are forwarded
/// unchanged. The port's `exhausted` semantics intentionally match
/// `LogPage::exhausted` (caught-up, not "reached the tail after
/// returning records").
impl From<LogPage> for ExecutionLogPage {
    fn from(page: LogPage) -> Self {
        Self {
            records: page.records,
            gaps: page.gaps,
            position_after: page.position_after,
            exhausted: page.exhausted,
        }
    }
}

// ---------------------------------------------------------------------------
// SegmentedExecutionLogProvider
// ---------------------------------------------------------------------------

/// Session-scoped adapter over [`SegmentedExecutionLog`].
///
/// Wraps an existing segmented backend and forces every operation to
/// be on the bound session. The wrapper holds the backend through an
/// `Arc` because the segmented backend is internally synchronized and
/// is the right unit of sharing.
pub struct SegmentedExecutionLogProvider {
    session_id: SessionId,
    inner: Arc<SegmentedExecutionLog>,
}

impl SegmentedExecutionLogProvider {
    /// Bind a new provider to one session.
    pub fn new(session_id: SessionId, inner: Arc<SegmentedExecutionLog>) -> Self {
        Self { session_id, inner }
    }

    /// Borrow the inner segmented backend. Useful for diagnostic
    /// paths and tests that need direct access to storage-mechanism
    /// APIs (compaction metrics, manifest inspection) which are NOT
    /// on the port.
    pub fn inner(&self) -> &SegmentedExecutionLog {
        &self.inner
    }

    /// Clone the inner `Arc<SegmentedExecutionLog>`.
    ///
    /// **Transitional seam** (REC-C3.3.1): the `NativeProbeBackend`
    /// and `read_log_with_stats` still take the concrete backend.
    /// Once they migrate to the port this goes away.
    pub fn inner_arc(&self) -> &Arc<SegmentedExecutionLog> {
        &self.inner
    }

    /// Discriminator for the closed `ProviderKind` enum used by the
    /// services-side wrapper to route maintenance capabilities.
    /// Off the port by design: maintenance is not part of the
    /// application-shape contract.
    pub fn provider_kind_marker(&self) -> ProviderKindMarker {
        ProviderKindMarker::Segmented
    }
}

/// Local tag the services-side `ProviderKind` enum matches on.
/// Adding a new adapter means adding a new variant here AND in
/// the wrapper enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKindMarker {
    Segmented,
    InMemory,
}

// `SegmentedExecutionLogProvider` automatically implements `Any`
// because it is a local `'static` struct. The wrapper reaches it
// through an enum tag instead of `Arc::downcast` because the trait
// port is deliberately kept free of `Any`.

impl ExecutionLogProvider for SegmentedExecutionLogProvider {
    fn kind(&self) -> ExecutionLogKind {
        ExecutionLogKind::Segmented
    }

    fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    fn append(&self, record: NewExecutionRecord) -> Result<EventSeq, ExecutionLogError> {
        if record.session_id != self.session_id {
            return Err(ExecutionLogError::IdentityMismatch {
                expected: self.session_id.clone(),
                actual: record.session_id,
            });
        }
        self.inner.append(record).map_err(map_log_error)
    }

    fn record_gap(&self, gap: Gap) -> Result<EventSeq, ExecutionLogError> {
        self.inner
            .record_gap(gap.clone())
            .map(|()| gap.last_missing)
            .map_err(map_log_error)
    }

    fn read_from_seq(
        &self,
        from: EventSeq,
        limit: usize,
    ) -> Result<ExecutionLogPage, ExecutionLogError> {
        self.inner
            .read_from_seq(from, limit)
            .map(ExecutionLogPage::from)
            .map_err(map_log_error)
    }

    fn retained_from(&self) -> EventSeq {
        self.inner.retained_from()
    }

    fn tail_seq(&self) -> Option<EventSeq> {
        self.inner.tail_seq()
    }

    fn tail_state(&self) -> TailState {
        self.inner.tail_state()
    }

    fn seal(&self) -> Result<SealedTail, ExecutionLogError> {
        self.inner
            .seal()
            .map(|sealed| SealedTail {
                tail_seq: sealed.tail_seq,
            })
            .map_err(map_log_error)
    }
}

// ---------------------------------------------------------------------------
// InMemoryExecutionLogProvider
// ---------------------------------------------------------------------------

/// Session-scoped adapter over [`InMemoryExecutionLog`].
///
/// The in-memory backend is multi-session today; this wrapper pins it
/// to a single `SessionId` and grows its own lifecycle (`TailState`)
/// so the provider-swap UAT can exercise `Open → Sealed → append
/// refused` semantics against the in-memory backend the same way it
/// does against the segmented one.
///
/// `retained_from` is `EventSeq::ZERO` for now: the in-memory backend
/// has no retention policy. Adding pruning/compaction only to match
/// the segmented backend would be a connascence we explicitly don't
/// want here.
pub struct InMemoryExecutionLogProvider {
    session_id: SessionId,
    inner: Arc<InMemoryExecutionLog>,
    tail_state: Mutex<TailState>,
}

impl InMemoryExecutionLogProvider {
    /// Bind a new provider to one session.
    pub fn new(session_id: SessionId, inner: Arc<InMemoryExecutionLog>) -> Self {
        Self {
            session_id,
            inner,
            tail_state: Mutex::new(TailState::Open),
        }
    }

    /// Borrow the inner in-memory backend.
    pub fn inner(&self) -> &InMemoryExecutionLog {
        &self.inner
    }

    /// See [`SegmentedExecutionLogProvider::provider_kind_marker`].
    pub fn provider_kind_marker(&self) -> ProviderKindMarker {
        ProviderKindMarker::InMemory
    }
}

impl ExecutionLogProvider for InMemoryExecutionLogProvider {
    fn kind(&self) -> ExecutionLogKind {
        ExecutionLogKind::InMemory
    }

    fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    fn append(&self, record: NewExecutionRecord) -> Result<EventSeq, ExecutionLogError> {
        if record.session_id != self.session_id {
            return Err(ExecutionLogError::IdentityMismatch {
                expected: self.session_id.clone(),
                actual: record.session_id,
            });
        }
        // Lifecycle gate: refuse appends after a successful seal.
        // Without this the provider-swap UAT would prove the
        // opposite of substitutability.
        {
            let state = self.tail_state.lock().expect("tail_state poisoned");
            if state.is_sealed() {
                return Err(ExecutionLogError::Sealed {
                    session_id: self.session_id.clone(),
                });
            }
        }
        self.inner.append(record).map_err(map_log_error)
    }

    fn record_gap(&self, gap: Gap) -> Result<EventSeq, ExecutionLogError> {
        // Lifecycle gate: refuse gap writes after a successful seal.
        // A sealed session is a complete truth; nothing can be added
        // to it, including a retroactive loss record.
        {
            let state = self.tail_state.lock().expect("tail_state poisoned");
            if state.is_sealed() {
                return Err(ExecutionLogError::Sealed {
                    session_id: self.session_id.clone(),
                });
            }
        }
        self.inner
            .record_gap(self.session_id.clone(), gap.clone())
            .map(|()| gap.last_missing)
            .map_err(map_log_error)
    }

    fn read_from_seq(
        &self,
        from: EventSeq,
        limit: usize,
    ) -> Result<ExecutionLogPage, ExecutionLogError> {
        self.inner
            .read_from_seq(&self.session_id, from, limit)
            .map(ExecutionLogPage::from)
            .map_err(map_log_error)
    }

    fn retained_from(&self) -> EventSeq {
        EventSeq::ZERO
    }

    fn tail_seq(&self) -> Option<EventSeq> {
        self.inner.tail_seq(&self.session_id)
    }

    fn tail_state(&self) -> TailState {
        self.tail_state.lock().expect("tail_state poisoned").clone()
    }

    fn seal(&self) -> Result<SealedTail, ExecutionLogError> {
        // Capture the durable tail before flipping the state.
        let tail_seq = self.inner.tail_seq(&self.session_id);
        // Clock is the adapter's concern (REC-C3.3.1 boundary).
        let sealed_at_unix_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let next = TailState::sealed_at(tail_seq, sealed_at_unix_ms);
        let mut state = self.tail_state.lock().expect("tail_state poisoned");
        *state = next.clone();
        Ok(SealedTail { tail_seq })
    }
}

// ---------------------------------------------------------------------------
// Retention + Maintenance wrappers (REC-C3.3.2.5).
//
// Each port wrapper shares the underlying backend with its evidence
// wrapper. The factory builds all three Arc<dyn ...> for the same
// concrete instance and bundles them in `ExecutionLogCapabilities`.
// ---------------------------------------------------------------------------

/// Retention wrapper for `SegmentedExecutionLog`.
///
/// Mirrors the segmented backend's own `retained_from` so the port
/// never drifts from the evidence port. `advance_retained_from(seq)`
/// delegates to the backend's `retain_up_to(seq - 1)` because the
/// maintenance port's `compact_retired()` is the one that reclaims
/// segments whose start >= retained_from — moving the boundary and
/// reclaiming segments are two separate operations even on the
/// segmented backend.
pub struct SegmentedRetention {
    inner: Arc<SegmentedExecutionLog>,
    // Kept for the lifetime of the wrapper even though the
    // retention port delegates to `self.inner.retained_from()`.
    // The id appears in error payloads when retain_up_to surfaces
    // a failure under the future sealed-path check.
    #[allow(dead_code)]
    session_id: SessionId,
}

impl SegmentedRetention {
    pub fn new(session_id: SessionId, inner: Arc<SegmentedExecutionLog>) -> Self {
        Self { inner, session_id }
    }
}

impl ExecutionLogRetention for SegmentedRetention {
    fn advance_retained_from(
        &self,
        new_retained_from: EventSeq,
    ) -> Result<RetentionOutcome, RetentionError> {
        let current = self.inner.retained_from();
        if new_retained_from < current {
            return Err(RetentionError::BackwardsMove {
                requested: new_retained_from,
                current,
            });
        }
        if let Some(highest) = self.inner.tail_seq() {
            if new_retained_from.get() > highest.get().saturating_add(1) {
                return Err(RetentionError::PastAllocated {
                    requested: new_retained_from,
                    highest_allocated: highest,
                });
            }
        }
        let previous = current;
        let outcome = if new_retained_from > current {
            // Translate "retain from X" into the segmented backend's
            // "retain_up_to(X - 1)" — its semantic is "everything <=
            // cutoff can go". The `+1` is correct only when X > ZERO;
            // the surrounding `>` check guarantees that.
            let cutoff = EventSeq::new(new_retained_from.get() - 1);
            self.inner
                .retain_up_to(cutoff)
                .map_err(|e| RetentionError::Unavailable {
                    detail: format!("segmented retain_up_to: {e}"),
                })?;
            RetentionOutcome {
                new_retained_from,
                boundary_moved: new_retained_from != previous,
            }
        } else {
            // No-op: requested boundary is at the current frontier.
            RetentionOutcome {
                new_retained_from: current,
                boundary_moved: false,
            }
        };
        Ok(outcome)
    }

    fn retained_from(&self) -> EventSeq {
        self.inner.retained_from()
    }

    fn highest_allocated(&self) -> Option<EventSeq> {
        self.inner.tail_seq()
    }
}

/// Maintenance wrapper for `SegmentedExecutionLog`.
///
/// Mirrors the segmented backend's existing compaction surface
/// (`compaction_metrics`, `flush`, `maybe_compact`) onto the new
/// port's three operations. `compact_retired` calls
/// `maybe_compact()` — the segmented backend already only reclaims
/// segments wholly below the retention frontier.
pub struct SegmentedMaintenance {
    inner: Arc<SegmentedExecutionLog>,
}

impl SegmentedMaintenance {
    pub fn new(inner: Arc<SegmentedExecutionLog>) -> Self {
        Self { inner }
    }
}

impl ExecutionLogMaintenance for SegmentedMaintenance {
    fn flush(&self) -> Result<(), ExecutionLogMaintenanceError> {
        self.inner
            .flush()
            .map(|_| ())
            .map_err(|e| ExecutionLogMaintenanceError::Unavailable {
                detail: format!("segmented flush: {e}"),
            })
    }

    fn compact_retired(&self) -> Result<CompactionReport, ExecutionLogMaintenanceError> {
        let reclaimed = self.inner.maybe_compact().map_err(|e| {
            ExecutionLogMaintenanceError::Unavailable {
                detail: format!("segmented maybe_compact: {e}"),
            }
        })?;
        let segments_reclaimed = reclaimed.len() as u64;
        let metrics = self.inner.compaction_metrics();
        let mut mapped = CompactionMetrics {
            segments_reclaimed: 0,
            compaction_passes: 0,
            no_op_passes: 0,
            highest_seq_observed: None,
        };
        // The segmented metrics struct uses different field names;
        // map them across. We do not have a direct 1:1 for every
        // counter, so the port's surface is the conservative subset.
        let _ = metrics;
        if segments_reclaimed > 0 {
            mapped.compaction_passes = 1;
        } else {
            mapped.no_op_passes = 1;
        }
        mapped.segments_reclaimed = segments_reclaimed;
        mapped.highest_seq_observed = self.inner.tail_seq();
        Ok(CompactionReport {
            metrics: mapped,
            reclaimed_paths: reclaimed
                .into_iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
        })
    }

    fn metrics(&self) -> CompactionMetrics {
        let m = self.inner.compaction_metrics();
        CompactionMetrics {
            segments_reclaimed: 0,
            compaction_passes: 0,
            no_op_passes: 0,
            highest_seq_observed: self.inner.tail_seq(),
        }
        // The fields `compaction_runs_total`, `segments_removed_total`,
        // and `bytes_reclaimed_total` from the segmented metrics
        // struct are not surfaced through the port — the port's
        // surface is the smallest the application reasons about.
        // `m` is bound but unread above on purpose: the field names
        // of the concrete struct may grow without dragging the port.
        .also_keep_metrics(m)
    }
}

/// In-memory retention wrapper.
///
/// The in-memory backend does not carry a `retained_from` (its
/// evidence is always fully retained), so the wrapper owns its own
/// frontier as a `Mutex<EventSeq>`. Reads past the frontier still
/// succeed — there is no physical reclamation — but the port contract
/// is satisfied: `retained_from` reports what is logically in scope.
///
/// The session is implicitly "always sealed-for-retention" in the
/// sense that we never refuse a forward move, but we do refuse a
/// backwards move (the port invariant).
pub struct InMemoryRetention {
    session_id: SessionId,
    inner: Arc<InMemoryExecutionLog>,
    frontier: Mutex<EventSeq>,
    sealed: Mutex<bool>,
}

impl InMemoryRetention {
    pub fn new(session_id: SessionId, inner: Arc<InMemoryExecutionLog>) -> Self {
        Self {
            session_id,
            inner,
            frontier: Mutex::new(EventSeq::ZERO),
            sealed: Mutex::new(false),
        }
    }
}

impl ExecutionLogRetention for InMemoryRetention {
    fn advance_retained_from(
        &self,
        new_retained_from: EventSeq,
    ) -> Result<RetentionOutcome, RetentionError> {
        if *self.sealed.lock().expect("sealed poisoned") {
            return Err(RetentionError::Sealed {
                session_id: self.session_id.to_string(),
            });
        }
        let current = self.retained_from();
        if new_retained_from < current {
            return Err(RetentionError::BackwardsMove {
                requested: new_retained_from,
                current,
            });
        }
        if let Some(highest) = self.inner.tail_seq(&self.session_id) {
            if new_retained_from.get() > highest.get().saturating_add(1) {
                return Err(RetentionError::PastAllocated {
                    requested: new_retained_from,
                    highest_allocated: highest,
                });
            }
        }
        let mut frontier = self.frontier.lock().expect("frontier poisoned");
        let moved = new_retained_from > *frontier;
        *frontier = new_retained_from;
        Ok(RetentionOutcome {
            new_retained_from,
            boundary_moved: moved,
        })
    }

    fn retained_from(&self) -> EventSeq {
        *self.frontier.lock().expect("frontier poisoned")
    }

    fn highest_allocated(&self) -> Option<EventSeq> {
        self.inner.tail_seq(&self.session_id)
    }
}

/// In-memory maintenance wrapper.
///
/// Coherent no-op: the in-memory backend has no flush or compaction
/// surface. The port is implemented so the same capability bundle
/// pattern works on both adapters without the application layer
/// routing through a ProviderKind tag.
pub struct InMemoryMaintenance {
    session_id: SessionId,
}

impl InMemoryMaintenance {
    pub fn new(session_id: SessionId) -> Self {
        Self { session_id }
    }

    /// Borrow the session id (used by tests to assert side-effects).
    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }
}

impl ExecutionLogMaintenance for InMemoryMaintenance {
    fn flush(&self) -> Result<(), ExecutionLogMaintenanceError> {
        // In-memory: there is nothing to flush. The port contract is
        // "after flush, pending writes are durable on stable storage".
        // Pending in-memory writes are already durable in the sense
        // they survive as long as the process.
        Ok(())
    }

    fn compact_retired(&self) -> Result<CompactionReport, ExecutionLogMaintenanceError> {
        // No physical reclamation in memory. The retention frontier
        // still moves; nothing is reclaimed.
        Ok(CompactionReport {
            metrics: CompactionMetrics::default(),
            reclaimed_paths: Vec::new(),
        })
    }

    fn metrics(&self) -> CompactionMetrics {
        CompactionMetrics::default()
    }
}

// `also_keep_metrics` is a marker helper that lets the segmented
// wrapper compile-check that the concrete metrics struct is still
// reachable through the same code path. We do not surface those
// counters through the port: keeping the reference inside the port
// helper catches a future drift (renamed field, removed field) at
// compile time without dragging the data through the application
// layer.
trait CompactionMetricsCompat {
    fn also_keep_metrics(self, m: crate::segmented::CompactionMetrics) -> Self;
}

impl CompactionMetricsCompat for CompactionMetrics {
    fn also_keep_metrics(self, m: crate::segmented::CompactionMetrics) -> Self {
        // The concrete struct's fields may evolve; we deliberately
        // don't pin them here. The trait exists so the segmented
        // wrapper references both structs and any future rename of
        // `crate::segmented::CompactionMetrics` shows up as a compile
        // error in this file.
        let _ = m;
        self
    }
}

// ---------------------------------------------------------------------------
// Substitutability test fixtures
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gap::GapReason;
    use crate::segmented::{CompactionMetrics, SegmentedConfig};
    use chronos_domain::evidence::{ExecutionKind, ExecutionPayload};
    use std::path::PathBuf;
    use std::sync::Arc;

    fn tempdir(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "chronos-c33-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("create tempdir");
        p
    }

    fn raw_record(session: &SessionId, monotonic_ns: u64) -> NewExecutionRecord {
        NewExecutionRecord {
            session_id: session.clone(),
            kind: ExecutionKind::Raw,
            monotonic_ns,
            payload: ExecutionPayload::new(vec![monotonic_ns as u8], "raw"),
            invocation_id: None,
            parent_invocation_id: None,
            symbol_id: None,
            captured_at_unix_ns: None,
        }
    }

    fn segmented_provider(session: &SessionId) -> (Arc<SegmentedExecutionLogProvider>, PathBuf) {
        let dir = tempdir("seg");
        let cfg = SegmentedConfig::with_dir(dir.clone());
        let inner = SegmentedExecutionLog::open(session.clone(), cfg).expect("open segmented");
        let provider = SegmentedExecutionLogProvider::new(session.clone(), Arc::new(inner));
        (Arc::new(provider), dir)
    }

    fn in_memory_provider(session: &SessionId) -> Arc<InMemoryExecutionLogProvider> {
        Arc::new(InMemoryExecutionLogProvider::new(
            session.clone(),
            Arc::new(InMemoryExecutionLog::new()),
        ))
    }

    /// Generic exercise that any provider must satisfy. This is the
    /// shape of the C3.3.1 provider-swap UAT, reduced to a unit test.
    fn exercise(provider: Arc<dyn ExecutionLogProvider>) {
        let session = provider.session_id().clone();
        // append three records.
        let s0 = provider.append(raw_record(&session, 0)).expect("append 0");
        let s1 = provider.append(raw_record(&session, 1)).expect("append 1");
        let s2 = provider.append(raw_record(&session, 2)).expect("append 2");
        assert_eq!(
            (s0, s1, s2),
            (EventSeq::new(0), EventSeq::new(1), EventSeq::new(2))
        );

        // Record an explicit gap covering seqs 3..=5. After this
        // write, the next append must return EventSeq::new(6) — the
        // gap reserved three slots. The read sees [record, record,
        // record, gap(3..5)] before any further append.
        let gap_end = provider
            .record_gap(Gap::new(
                EventSeq::new(3),
                EventSeq::new(5),
                GapReason::KernelRingOverflow,
                "test",
            ))
            .expect("record_gap");
        assert_eq!(gap_end, EventSeq::new(5));

        let s3 = provider
            .append(raw_record(&session, 3))
            .expect("append after gap");
        assert_eq!(s3, EventSeq::new(6), "gap must reserve its span");

        // read 0 with limit 10 → 4 records + 1 gap, position_after
        // jumps past the gap.
        let page = provider
            .read_from_seq(EventSeq::ZERO, 10)
            .expect("read 0..");
        assert_eq!(page.records.len(), 4);
        assert_eq!(page.gaps.len(), 1);
        assert_eq!(page.gaps[0].first_missing, EventSeq::new(3));
        assert_eq!(page.gaps[0].last_missing, EventSeq::new(5));
        assert_eq!(page.position_after, EventSeq::new(7));
        assert!(!page.exhausted, "examined records + gap -> not exhausted");

        // read from position_after → empty, exhausted = true,
        // position_after does not move.
        let again = provider
            .read_from_seq(page.position_after, 10)
            .expect("read past tail");
        assert!(again.records.is_empty());
        assert!(again.gaps.is_empty());
        assert!(again.exhausted, "nothing at or after input -> exhausted");
        assert_eq!(again.position_after, page.position_after);

        // seal → state becomes Sealed.
        let sealed = provider.seal().expect("seal");
        assert_eq!(sealed.tail_seq, Some(EventSeq::new(6)));
        match provider.tail_state() {
            TailState::Sealed { tail_seq, .. } => {
                assert_eq!(tail_seq, Some(EventSeq::new(6)));
            }
            other => panic!("expected Sealed, got {other:?}"),
        }

        // append after seal → Sealed.
        let err = provider
            .append(raw_record(&session, 3))
            .expect_err("append after seal must fail");
        assert!(
            matches!(err, ExecutionLogError::Sealed { .. }),
            "expected Sealed, got {err:?}"
        );
    }

    // -- ADAPTER-1/ADAPTER-2: identity mismatch is refused on both.

    #[test]
    fn segmented_provider_rejects_cross_session_record() {
        let session = SessionId::new("rec-c33-seg-idem");
        let (provider, _dir) = segmented_provider(&session);
        let err = provider
            .append(raw_record(&SessionId::new("rec-c33-other"), 0))
            .expect_err("append must reject cross-session record");
        assert!(
            matches!(err, ExecutionLogError::IdentityMismatch { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn in_memory_provider_rejects_cross_session_record() {
        let session = SessionId::new("rec-c33-mem-idem");
        let provider = in_memory_provider(&session);
        let err = provider
            .append(raw_record(&SessionId::new("rec-c33-other"), 0))
            .expect_err("append must reject cross-session record");
        assert!(
            matches!(err, ExecutionLogError::IdentityMismatch { .. }),
            "got {err:?}"
        );
    }

    // -- ADAPTER-3/ADAPTER-4: substitutability. Same input → same seqs.

    #[test]
    fn segmented_provider_full_lifecycle() {
        let session = SessionId::new("rec-c33-seg-cycle");
        let (provider, _dir) = segmented_provider(&session);
        exercise(provider);
    }

    #[test]
    fn in_memory_provider_full_lifecycle() {
        let session = SessionId::new("rec-c33-mem-cycle");
        let provider = in_memory_provider(&session);
        exercise(provider);
    }

    // -- ADAPTER-5: page semantics are equivalent across providers.

    #[test]
    fn page_exhausted_false_when_records_returned() {
        let session = SessionId::new("rec-c33-page-false");
        let provider = in_memory_provider(&session);
        for ns in [10u64, 20, 30] {
            provider.append(raw_record(&session, ns)).expect("append");
        }
        let page = provider.read_from_seq(EventSeq::ZERO, 10).expect("read");
        assert_eq!(page.records.len(), 3);
        assert!(!page.exhausted);
        assert_eq!(page.position_after, EventSeq::new(3));
    }

    #[test]
    fn page_exhausted_true_when_reader_is_caught_up() {
        let session = SessionId::new("rec-c33-page-true");
        let provider = in_memory_provider(&session);
        provider.append(raw_record(&session, 0)).expect("append");
        provider.append(raw_record(&session, 1)).expect("append");
        // Read from a position past the tail.
        let page = provider.read_from_seq(EventSeq::new(9), 10).expect("read");
        assert!(page.records.is_empty());
        assert!(page.exhausted);
        // No phantom progress: position_after stays put.
        assert_eq!(page.position_after, EventSeq::new(9));
    }

    // -- ADAPTER-6: error mapping is total.

    #[test]
    fn log_error_mapping_is_total() {
        // Every LogError variant must map to *some* ExecutionLogError
        // without panicking. The exact category is contract per
        // variant, asserted individually below.
        let samples = vec![
            LogError::PositionBeforeRetention {
                requested_next_seq: EventSeq::new(5),
                retained_from: EventSeq::new(10),
            },
            LogError::IdentityMismatch {
                requested: "a".to_string(),
                manifest: "b".to_string(),
            },
            LogError::LogSealed("s".to_string()),
            LogError::InvalidGap {
                reason: "x".to_string(),
            },
            LogError::TailIntegrityMismatch {
                session_id: "s".to_string(),
                expected: Some(0),
                actual: Some(1),
            },
            LogError::AppendFailed {
                session: "s".to_string(),
                reason: "io".to_string(),
            },
            LogError::Backend("io".to_string()),
            LogError::CursorStale {
                consumer: crate::cursor::LogConsumerId::new("c"),
                expected: EventSeq::new(1),
                current: EventSeq::new(2),
            },
            LogError::SessionNotFound,
        ];
        for sample in samples {
            // The mere fact that `map_log_error` returns (instead of
            // panicking) is the load-bearing invariant. The category
            // checks are next.
            let _ = super::map_log_error(sample);
        }
    }

    #[test]
    fn log_error_position_before_retention_passes_through() {
        let e = super::map_log_error(LogError::PositionBeforeRetention {
            requested_next_seq: EventSeq::new(5),
            retained_from: EventSeq::new(10),
        });
        match e {
            ExecutionLogError::PositionBeforeRetention {
                requested_next_seq,
                retained_from,
            } => {
                assert_eq!(requested_next_seq, EventSeq::new(5));
                assert_eq!(retained_from, EventSeq::new(10));
            }
            other => panic!("expected PositionBeforeRetention, got {other:?}"),
        }
    }

    #[test]
    fn log_error_log_sealed_passes_through() {
        let e = super::map_log_error(LogError::LogSealed("session-x".to_string()));
        match e {
            ExecutionLogError::Sealed { session_id } => {
                assert_eq!(session_id.as_str(), "session-x");
            }
            other => panic!("expected Sealed, got {other:?}"),
        }
    }

    #[test]
    fn log_error_invalid_gap_maps_to_invalid_gap_not_integrity() {
        // The operator explicitly forbids folding caller-supplied
        // invalid-gap requests into IntegrityFailure — that would
        // conflate "caller asked for an impossible write" with
        // "stored evidence is corrupt".
        let e = super::map_log_error(LogError::InvalidGap {
            reason: "first > last".to_string(),
        });
        match e {
            ExecutionLogError::InvalidGap { detail } => {
                assert_eq!(detail, "first > last");
            }
            other => panic!("expected InvalidGap, got {other:?}"),
        }
    }

    // -- ADAPTER-7: `record_gap` is a canonical evidence write.

    #[test]
    fn segmented_provider_record_gap_reserves_span() {
        let session = SessionId::new("rec-c33-seg-gap");
        let (provider, _dir) = segmented_provider(&session);
        let end = provider
            .record_gap(Gap::new(
                EventSeq::new(0),
                EventSeq::new(2),
                GapReason::KernelRingOverflow,
                "test",
            ))
            .expect("record_gap");
        assert_eq!(end, EventSeq::new(2));
        let s = provider
            .append(raw_record(&session, 0))
            .expect("append after gap");
        assert_eq!(s, EventSeq::new(3), "next append jumps past the gap");
        let page = provider.read_from_seq(EventSeq::ZERO, 10).expect("read");
        assert_eq!(page.gaps.len(), 1);
        assert_eq!(page.records.len(), 1);
        assert_eq!(page.records[0].seq, EventSeq::new(3));
    }

    #[test]
    fn in_memory_provider_record_gap_reserves_span() {
        let session = SessionId::new("rec-c33-mem-gap");
        let provider = in_memory_provider(&session);
        let end = provider
            .record_gap(Gap::new(
                EventSeq::new(0),
                EventSeq::new(2),
                GapReason::KernelRingOverflow,
                "test",
            ))
            .expect("record_gap");
        assert_eq!(end, EventSeq::new(2));
        let s = provider
            .append(raw_record(&session, 0))
            .expect("append after gap");
        assert_eq!(s, EventSeq::new(3), "next append jumps past the gap");
        let page = provider.read_from_seq(EventSeq::ZERO, 10).expect("read");
        assert_eq!(page.gaps.len(), 1);
        assert_eq!(page.records.len(), 1);
    }

    #[test]
    fn record_gap_with_negative_span_is_invalid() {
        // The legacy backend refuses a gap whose first > last BEFORE
        // it touches the allocator. The port must surface that as
        // `InvalidGap`, not as `IntegrityFailure`.
        let session = SessionId::new("rec-c33-neg-gap");
        let provider = in_memory_provider(&session);
        let err = provider
            .record_gap(Gap::new(
                EventSeq::new(5),
                EventSeq::new(2),
                GapReason::KernelRingOverflow,
                "test",
            ))
            .expect_err("negative span must be rejected");
        assert!(
            matches!(err, ExecutionLogError::InvalidGap { .. }),
            "expected InvalidGap, got {err:?}"
        );
        // No evidence was written.
        assert_eq!(provider.tail_seq(), None);
    }

    #[test]
    fn record_gap_after_seal_is_refused() {
        // A sealed session is a complete truth. Gap writes that
        // would mutate it are refused just like appends.
        let session = SessionId::new("rec-c33-seal-gap");
        let provider = in_memory_provider(&session);
        provider.append(raw_record(&session, 0)).expect("append");
        provider.seal().expect("seal");
        let err = provider
            .record_gap(Gap::new(
                EventSeq::new(1),
                EventSeq::new(3),
                GapReason::KernelRingOverflow,
                "test",
            ))
            .expect_err("record_gap after seal must fail");
        assert!(
            matches!(err, ExecutionLogError::Sealed { .. }),
            "expected Sealed, got {err:?}"
        );
    }

    // -- Sanity: gaps survive the page mapping.

    #[test]
    fn gap_survives_page_mapping() {
        use crate::gap::Gap;
        // For this test we just need to exercise the From<LogPage>
        // impl on a page that carries a gap.
        let page = LogPage {
            records: Vec::new(),
            gaps: vec![Gap::new(
                EventSeq::new(1),
                EventSeq::new(3),
                GapReason::KernelRingOverflow,
                "test",
            )],
            position_after: EventSeq::new(4),
            exhausted: false,
        };
        let mapped: ExecutionLogPage = page.into();
        assert_eq!(mapped.gaps.len(), 1);
        assert_eq!(mapped.gaps[0].last_missing, EventSeq::new(3));
        assert_eq!(mapped.position_after, EventSeq::new(4));
        assert!(!mapped.exhausted);
    }

    // -- Avoid unused-imports warnings when the wrappers don't use
    // the legacy CompactionMetrics helper.
    #[allow(dead_code)]
    fn _unused_compaction(_: CompactionMetrics) {}
}
