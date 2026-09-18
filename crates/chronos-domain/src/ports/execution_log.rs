//! `ExecutionLogProvider` — domain-side port for the execution log storage.
//!
//! ## REC-C3.3.1 — port definition
//!
//! The provider is **session-scoped**: it represents one specific
//! session. A `SessionExecutionLogRegistry` (future work, REC-C3.3.3+)
//! is what maps `SessionId -> Arc<dyn ExecutionLogProvider>` and is
//! the right place to surface "session not found"; the provider
//! itself never does.
//!
//! ### Object safety
//!
//! The trait is intentionally object-safe (no generic methods, no
//! `Self`-returning methods other than `&self` accessors). All
//! consumers must be able to hold the provider behind
//! `Arc<dyn ExecutionLogProvider>`.
//!
//! ### Why no `dir()`, `path()`, `compaction_metrics()`, `maybe_compact()`
//!
//! Those are storage-maintenance capabilities, not application-shape
//! evidence operations. They belong on a separate port (e.g.
//! `ExecutionLogMaintenance`) if and when a real use case appears —
//! adding them here would let filesystem details leak into the
//! domain-facing contract.
//!
//! ### Identity contract on `append`
//!
//! `NewExecutionRecord::session_id` MUST equal `self.session_id()`.
//! A mismatch returns `ExecutionLogError::IdentityMismatch` and
//! performs no append. This is the load-bearing invariant that lets
//! us later drop `session_id` from `NewExecutionRecord` without
//! silently corrupting cross-session evidence.
//!
//! ### `tail_seq` vs `tail_state`
//!
//! `tail_seq()` returns the **highest seq currently allocated** in
//! this session — including for an `Open` run. `tail_state()`
//! describes what is known about the end (semantic lifecycle).
//! `events_read` and friends need both: the seq range for
//! completeness guarantees, the lifecycle state for "is this
//! trustworthy enough to read".

use crate::evidence::{ExecutionRecord, Gap, NewExecutionRecord, SealedTail, TailState};
use crate::seq::EventSeq;
use crate::session_id::SessionId;

/// Closed discriminator for which concrete adapter implements
/// this provider (REC-C3.3.1).
///
/// The port surfaces only this closed enum — NOT a generic
/// `Any`/`downcast` — so callers can route storage-maintenance
/// capabilities without leaking `Any` into the domain contract.
///
/// Adding a new adapter means adding a variant here AND extending
/// the services-side `ProviderKind`. That load-bearing closure is
/// the whole point of using an enum instead of a runtime cast.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionLogKind {
    /// File-backed segment store (`SegmentedExecutionLog`).
    Segmented,
    /// In-memory store (`InMemoryExecutionLog`).
    InMemory,
}

/// Application-shape error for the execution log port.
///
/// **Session-scoped providers never return `SessionNotFound`** —
/// that error belongs to the factory/registry that hands out the
/// provider. Everything here describes failure modes of *an
/// already-resolved provider* for a *known session*.
///
/// `chronos_log::LogError` carries richer storage-mechanism detail
/// (filesystem paths, retention-metadata pointers, segment
/// cross-boundary detection). The adapter translates each of those
/// into exactly one of the variants below:
///
/// **Runtime-clean:** the domain crate has no `serde_json` runtime
/// dependency, so this type does not derive `Serialize` /
/// `Deserialize`. If a port ever needs wire serialization the
/// adapter does the translation at the boundary.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExecutionLogError {
    /// The caller asked for a position earlier than the retention
    /// boundary. The provider did not return anything past it.
    #[error(
        "execution log position {requested_next_seq} is before retention boundary {retained_from}"
    )]
    PositionBeforeRetention {
        requested_next_seq: EventSeq,
        retained_from: EventSeq,
    },

    /// `NewExecutionRecord::session_id` does not match the
    /// provider's session. The append is rejected, no evidence
    /// is written.
    #[error("execution log identity mismatch: expected {expected}, actual {actual}")]
    IdentityMismatch {
        expected: SessionId,
        actual: SessionId,
    },

    /// The session is sealed; further appends are refused.
    #[error("execution log for session {session_id} is sealed")]
    Sealed { session_id: SessionId },

    /// Integrity check failed (bad checksum, malformed segment,
    /// cross-boundary violation, etc.). `detail` is the human-readable
    /// description; machines should not parse it.
    #[error("execution evidence integrity failure: {detail}")]
    IntegrityFailure { detail: String },

    /// The provider is reachable but cannot serve the request right
    /// (transient I/O error, lock contention, compaction in progress,
    /// etc.). Distinct from `IntegrityFailure`: the evidence may be
    /// fine; the request could not be answered.
    #[error("execution log unavailable: {detail}")]
    Unavailable { detail: String },

    /// The supplied `Gap` is malformed for this session (negative
    /// span, span that re-uses already-allocated seqs, span that
    /// does not fit the next free seq, …). NOT an integrity failure
    /// of stored evidence — the caller asked for an invalid write,
    /// the provider refused, no gap was recorded.
    #[error("invalid gap for execution log: {detail}")]
    InvalidGap { detail: String },

    /// REC-C3.3.2 — the factory could not open or create the
    /// underlying storage at `path`. Distinct from `Unavailable`
    /// because the storage is not reachable at all (filesystem
    /// permission, missing parent dir, locked by another process).
    /// The factory signals this so the composition root can decide
    /// whether to fall back, surface a typed error, or fail closed.
    #[error("execution log open failed at {}: {}", path.display(), kind)]
    Open {
        path: std::path::PathBuf,
        kind: String,
    },
}

/// Domain-shaped read result. Lives here (not in `chronos_log`) so
/// the port signature never names a storage type.
///
/// ### `position_after`
///
/// `position_after` is the position **after** the last evidence
/// space the provider examined during this read — including past any
/// gaps it observed. Readers can always make progress past lost
/// evidence; the next read starts from `position_after`.
///
/// ### `exhausted`
///
/// `exhausted` means "the read reached the tail observed during this
/// read". It does **not** mean "the session is finished" — an
/// `Open` session may be `exhausted == true` because there is
/// nothing past the current tail yet. It also does not mean
/// "evidence is complete for the whole execution": between
/// `position_after` and the eventual end of the run there may be
/// more records that simply have not been written yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionLogPage {
    pub records: Vec<ExecutionRecord>,
    pub gaps: Vec<Gap>,
    pub position_after: EventSeq,
    pub exhausted: bool,
}

/// The application-shape port for an execution log bound to a single
/// session.
///
/// Object-safe: see module docs.
pub trait ExecutionLogProvider: Send + Sync {
    /// Which concrete adapter implements this provider. Closed enum,
    /// used by callers to route storage-maintenance capabilities to
    /// the right backend (REC-C3.3.1). NOT `Any`.
    fn kind(&self) -> ExecutionLogKind;

    /// The session this provider is bound to.
    fn session_id(&self) -> &SessionId;

    /// Append a record. The backend assigns the seq and returns it.
    ///
    /// On success the returned seq is strictly greater than the last
    /// successful seq on this session (or equal to `retained_from()`
    /// on the first append).
    ///
    /// On `IdentityMismatch` no evidence is written. The provider
    /// MUST NOT silently overwrite `record.session_id`.
    fn append(&self, record: NewExecutionRecord) -> Result<EventSeq, ExecutionLogError>;

    /// Record an explicit gap in this session's evidence.
    ///
    /// `record_gap` is **a write of canonical evidence**, not storage
    /// maintenance. The truth:
    ///
    /// ```text
    /// append(record)   = we know this evidence EXISTS
    /// record_gap(gap)  = we know this evidence WAS LOST
    /// ```
    ///
    /// Both mutate the authoritative truth of the `ExecutionLog`.
    /// Routing `record_gap` through a maintenance escape hatch would
    /// split evidence writes across two paths — that is the boundary
    /// the operator explicitly forbids.
    ///
    /// The provider is session-scoped, so the gap does NOT carry a
    /// `session_id`: the provider's own session is the only one to
    /// which the gap can belong. This eliminates a connascence the
    /// legacy backend had (`record_gap(session_id, gap)`).
    ///
    /// The provider assigns the gap's end seq and returns it. On
    /// success the returned seq is strictly greater than the last
    /// successful seq (record OR gap) on this session.
    ///
    /// On `InvalidGap` NO evidence is written — the caller asked for
    /// an impossible gap (negative span, span that re-uses allocated
    /// seqs, span that does not fit the next free seq, …).
    ///
    /// On `IdentityMismatch` (gap with a `session_id` field that
    /// differs — future-proofing for `Gap` carrying identity) no
    /// evidence is written.
    fn record_gap(&self, gap: Gap) -> Result<EventSeq, ExecutionLogError>;

    /// Read records and gaps from `from` (inclusive), up to `limit`
    /// records.
    fn read_from_seq(
        &self,
        from: EventSeq,
        limit: usize,
    ) -> Result<ExecutionLogPage, ExecutionLogError>;

    /// The earliest seq still queryable on this session. Reads
    /// starting before this boundary fail with
    /// `PositionBeforeRetention`.
    fn retained_from(&self) -> EventSeq;

    /// The highest seq currently allocated on this session, or `None`
    /// when the session is empty (no records, no gaps). Available
    /// for both `Open` and `Sealed` sessions.
    fn tail_seq(&self) -> Option<EventSeq>;

    /// Semantic lifecycle state of the tail (see [`TailState`]).
    fn tail_state(&self) -> TailState;

    /// Mark the session as sealed. Idempotent semantics are up to
    /// the implementation; the canonical contract is: the next
    /// `append` after a successful `seal` returns `Sealed`.
    fn seal(&self) -> Result<SealedTail, ExecutionLogError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// Compile-only check: `Arc<dyn ExecutionLogProvider>` is the
    /// canonical consumer shape. If a future change adds a
    /// non-object-safe method (generic / `Self`-returning) this
    /// stops compiling and the regression is caught at the boundary,
    /// not at the call site.
    #[allow(dead_code)]
    fn accepts_provider(_: Arc<dyn ExecutionLogProvider>) {}

    /// Compile-only check: `&dyn ExecutionLogProvider` works too.
    #[allow(dead_code)]
    fn accepts_provider_ref(_: &dyn ExecutionLogProvider) {}

    // -----------------------------------------------------------------
    // Identity-mismatch contract test.
    //
    // The port MUST refuse to append evidence whose `session_id`
    // differs from the provider's session. No silent overwrite, no
    // per-record session_id correction. This kills the future
    // "convenient" adapter that fixes the field for you.
    // -----------------------------------------------------------------

    /// Minimal fake provider that records every `append` and lets us
    /// observe whether a mismatched record slipped through.
    struct FakeProvider {
        session: SessionId,
        appended: std::sync::Mutex<Vec<NewExecutionRecord>>,
    }

    impl FakeProvider {
        fn new(session: SessionId) -> Arc<Self> {
            Arc::new(Self {
                session,
                appended: std::sync::Mutex::new(Vec::new()),
            })
        }
    }

    impl ExecutionLogProvider for FakeProvider {
        fn kind(&self) -> ExecutionLogKind {
            ExecutionLogKind::InMemory
        }

        fn session_id(&self) -> &SessionId {
            &self.session
        }

        fn append(&self, record: NewExecutionRecord) -> Result<EventSeq, ExecutionLogError> {
            if record.session_id != self.session {
                return Err(ExecutionLogError::IdentityMismatch {
                    expected: self.session.clone(),
                    actual: record.session_id.clone(),
                });
            }
            let seq = EventSeq::new(self.appended.lock().unwrap().len() as u64);
            self.appended.lock().unwrap().push(record);
            Ok(seq)
        }

        fn record_gap(&self, _gap: Gap) -> Result<EventSeq, ExecutionLogError> {
            // The fake does not enforce span shape — that contract
            // is per-adapter and tested there. The fake just needs to
            // exist so the trait stays implementable end-to-end.
            Err(ExecutionLogError::Unavailable {
                detail: "FakeProvider does not implement record_gap".to_string(),
            })
        }

        fn read_from_seq(
            &self,
            _from: EventSeq,
            _limit: usize,
        ) -> Result<ExecutionLogPage, ExecutionLogError> {
            Ok(ExecutionLogPage {
                records: Vec::new(),
                gaps: Vec::new(),
                position_after: EventSeq::ZERO,
                exhausted: true,
            })
        }

        fn retained_from(&self) -> EventSeq {
            EventSeq::ZERO
        }

        fn tail_seq(&self) -> Option<EventSeq> {
            let n = self.appended.lock().unwrap().len();
            if n == 0 {
                None
            } else {
                Some(EventSeq::new(n as u64 - 1))
            }
        }

        fn tail_state(&self) -> TailState {
            TailState::Open
        }

        fn seal(&self) -> Result<SealedTail, ExecutionLogError> {
            Ok(SealedTail { tail_seq: None })
        }
    }

    #[test]
    fn identity_mismatch_is_refused_no_silent_overwrite() {
        let provider = FakeProvider::new(SessionId::new("rec-c33-provider-A"));
        let other_session = SessionId::new("rec-c33-provider-B");

        let err = provider
            .as_ref()
            .append(NewExecutionRecord {
                session_id: other_session.clone(),
                ..NewExecutionRecord::default()
            })
            .expect_err("append must reject cross-session record");

        match err {
            ExecutionLogError::IdentityMismatch { expected, actual } => {
                assert_eq!(expected, SessionId::new("rec-c33-provider-A"));
                assert_eq!(actual, other_session);
            }
            other => panic!("expected IdentityMismatch, got {other:?}"),
        }
        assert!(
            provider.appended.lock().unwrap().is_empty(),
            "a rejected record must not be appended"
        );
    }

    #[test]
    fn matching_session_appends_normally() {
        let provider = FakeProvider::new(SessionId::new("rec-c33-ok"));
        let result = provider.as_ref().append(NewExecutionRecord {
            session_id: SessionId::new("rec-c33-ok"),
            ..NewExecutionRecord::default()
        });
        let seq = result.expect("matching session appends");
        assert_eq!(seq, EventSeq::new(0));
        assert_eq!(provider.appended.lock().unwrap().len(), 1);
    }
}
