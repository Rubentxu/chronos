//! `ExecutionLogBackend` trait + high-level `ExecutionLog<B>` wrapper.

use crate::cursor::{ConsumerCursor, LogConsumerId, ReadResult};
use crate::error::LogError;
use crate::gap::Gap;
use crate::record::{ExecutionRecord, SessionId};
use crate::seq::EventSeq;

/// An `ExecutionRecord` minus the backend-assigned `seq`.
///
/// The backend assigns the seq on `append` so the invariant
/// "strictly monotonic within one session" is enforced centrally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewExecutionRecord {
    pub session_id: SessionId,
    pub monotonic_ns: u64,
    pub payload: crate::record::ExecutionPayload,
}

/// Backend trait for the append-only execution log.
///
/// Implementations must satisfy the four invariants documented in
/// `docs/chronos-agentic-reconstruction/docs/specs/EXECUTION_LOG.md`:
/// 1. Strict monotonicity per session.
/// 2. Multi-consumer cursor isolation.
/// 3. Non-destructive reads.
/// 4. No silent gaps.
pub trait ExecutionLogBackend: Send + Sync {
    /// Append a record to the log. The backend assigns the seq and
    /// returns it.
    ///
    /// On success, the returned seq is strictly greater than the
    /// last successful seq on the same session (or equal to 0 on
    /// the first append).
    fn append(&self, record: NewExecutionRecord) -> Result<EventSeq, LogError>;

    /// Record an explicit gap into the log. After a gap, the next
    /// successful `append` returns a seq greater than
    /// `gap.last_missing`.
    fn record_gap(&self, session_id: SessionId, gap: Gap) -> Result<(), LogError>;

    /// Read records and any observed gaps for `session_id` after
    /// `cursor.last_seq`. If `cursor` is `None`, the backend uses a
    /// fresh cursor for `consumer`.
    ///
    /// Returns `ReadResult::SessionNotFound` if the session has no
    /// records AND no gap has been recorded for it (a "fresh empty
    /// log" returns `Ok { records: vec![], gaps: vec![], ... }`
    /// instead — see the `ReadResult` docs).
    fn read_after(
        &self,
        session_id: SessionId,
        consumer: LogConsumerId,
        cursor: Option<ConsumerCursor>,
    ) -> Result<ReadResult, LogError>;

    /// Returns the highest seq currently allocated on `session_id`,
    /// or `None` if the session is empty (no records, no gaps).
    fn tail_seq(&self, session_id: &SessionId) -> Option<EventSeq>;

    /// Optional efficiency hook: append many records at once. The
    /// default implementation loops over `append`; backends may
    /// override for batching.
    fn append_many(
        &self,
        records: impl IntoIterator<Item = NewExecutionRecord>,
    ) -> Result<Vec<EventSeq>, LogError> {
        let mut out = Vec::new();
        for r in records {
            out.push(self.append(r)?);
        }
        Ok(out)
    }
}

/// High-level ergonomic wrapper around a backend.
#[derive(Debug, Clone)]
pub struct ExecutionLog<B: ExecutionLogBackend + ?Sized> {
    backend: std::sync::Arc<B>,
}

impl<B: ExecutionLogBackend + ?Sized> ExecutionLog<B> {
    pub fn new(backend: std::sync::Arc<B>) -> Self {
        Self { backend }
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    pub fn append(&self, record: NewExecutionRecord) -> Result<EventSeq, LogError> {
        self.backend.append(record)
    }

    pub fn append_record(
        &self,
        session_id: SessionId,
        monotonic_ns: u64,
        payload: crate::record::ExecutionPayload,
    ) -> Result<EventSeq, LogError> {
        self.backend.append(NewExecutionRecord {
            session_id,
            monotonic_ns,
            payload,
        })
    }

    pub fn record_gap(&self, session_id: SessionId, gap: Gap) -> Result<(), LogError> {
        self.backend.record_gap(session_id, gap)
    }

    pub fn read_after(
        &self,
        session_id: SessionId,
        consumer: LogConsumerId,
        cursor: Option<ConsumerCursor>,
    ) -> Result<ReadResult, LogError> {
        self.backend.read_after(session_id, consumer, cursor)
    }

    pub fn tail_seq(&self, session_id: &SessionId) -> Option<EventSeq> {
        self.backend.tail_seq(session_id)
    }
}

// `ExecutionRecord` is re-exported at the crate root, but we want
// `ExecutionLogBackend::append` to accept a `NewExecutionRecord`
// (sans seq). The `ExecutionRecord` type itself is constructed
// internally by the backend.
#[allow(dead_code)]
fn _execution_record_marker(_r: ExecutionRecord) {}
