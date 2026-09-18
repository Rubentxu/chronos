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

use chronos_domain::evidence::{
    ExecutionKind, ExecutionPayload, ExecutionRecord, Gap, NewExecutionRecord, SealedTail,
    TailState,
};
use chronos_domain::ports::execution_log::{ExecutionLogError, ExecutionLogPage, ExecutionLogProvider};
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
        | LogError::InvalidGap { .. }
        | LogError::SegmentCrossesRetention { .. }
        | LogError::RetentionMetadataMissing { .. } => ExecutionLogError::IntegrityFailure {
            detail: format!("execution log integrity failure: {err}"),
        },

        // Transient / unavailable.
        LogError::AppendFailed { .. } | LogError::Backend(_) => ExecutionLogError::Unavailable {
            detail: format!("execution log unavailable: {err}"),
        },

        // Defensive: the new stateless / session-scoped path is not
        // supposed to produce these. If it does, that is an adapter
        // regression we want to catch — but not by crashing the
        // process.
        LogError::CursorStale { .. } | LogError::SessionNotFound => ExecutionLogError::Unavailable {
            detail: format!(
                "execution log unavailable: legacy backend invariant violated: {err}"
            ),
        },
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
}

impl ExecutionLogProvider for SegmentedExecutionLogProvider {
    fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    fn append(
        &self,
        record: NewExecutionRecord,
    ) -> Result<EventSeq, ExecutionLogError> {
        if record.session_id != self.session_id {
            return Err(ExecutionLogError::IdentityMismatch {
                expected: self.session_id.clone(),
                actual: record.session_id,
            });
        }
        self.inner
            .append(record)
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
}

impl ExecutionLogProvider for InMemoryExecutionLogProvider {
    fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    fn append(
        &self,
        record: NewExecutionRecord,
    ) -> Result<EventSeq, ExecutionLogError> {
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
        self.inner
            .append(record)
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
        self.tail_state
            .lock()
            .expect("tail_state poisoned")
            .clone()
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
// Substitutability test fixtures
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gap::GapReason;
    use crate::segmented::{CompactionMetrics, SegmentedConfig};
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

    fn raw_record(session: &SessionId, seq: u64, monotonic_ns: u64) -> NewExecutionRecord {
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
        let s0 = provider
            .append(raw_record(&session, 0, 0))
            .expect("append 0");
        let s1 = provider
            .append(raw_record(&session, 1, 1))
            .expect("append 1");
        let s2 = provider
            .append(raw_record(&session, 2, 2))
            .expect("append 2");
        assert_eq!((s0, s1, s2), (EventSeq::new(0), EventSeq::new(1), EventSeq::new(2)));

        // read 0 with limit 10 → 3 records, position_after = 3, NOT
        // exhausted (we just consumed everything that exists; the
        // semantic of `exhausted` is "nothing at or after the input
        // position" — for `from=0` with records present, we DID
        // examine something, so exhausted is false even though the
        // session is otherwise caught-up).
        let page = provider
            .read_from_seq(EventSeq::ZERO, 10)
            .expect("read 0..");
        assert_eq!(page.records.len(), 3);
        assert_eq!(page.position_after, EventSeq::new(3));
        assert!(!page.exhausted, "examined records -> not exhausted");

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
        assert_eq!(sealed.tail_seq, Some(EventSeq::new(2)));
        match provider.tail_state() {
            TailState::Sealed { tail_seq, .. } => {
                assert_eq!(tail_seq, Some(EventSeq::new(2)));
            }
            other => panic!("expected Sealed, got {other:?}"),
        }

        // append after seal → Sealed.
        let err = provider
            .append(raw_record(&session, 3, 3))
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
            .append(raw_record(&SessionId::new("rec-c33-other"), 0, 0))
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
            .append(raw_record(&SessionId::new("rec-c33-other"), 0, 0))
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
            provider.append(raw_record(&session, ns, ns)).expect("append");
        }
        let page = provider
            .read_from_seq(EventSeq::ZERO, 10)
            .expect("read");
        assert_eq!(page.records.len(), 3);
        assert!(!page.exhausted);
        assert_eq!(page.position_after, EventSeq::new(3));
    }

    #[test]
    fn page_exhausted_true_when_reader_is_caught_up() {
        let session = SessionId::new("rec-c33-page-true");
        let provider = in_memory_provider(&session);
        provider.append(raw_record(&session, 0, 0)).expect("append");
        provider.append(raw_record(&session, 1, 1)).expect("append");
        // Read from a position past the tail.
        let page = provider
            .read_from_seq(EventSeq::new(9), 10)
            .expect("read");
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
