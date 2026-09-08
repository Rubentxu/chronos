//! `LogConsumerId`, `ConsumerCursor`, `ReadResult`.
//!
//! Multi-consumer cursor isolation: each consumer is identified by a
//! `LogConsumerId`; cursors are independent across consumers.

use crate::gap::Gap;
use crate::record::ExecutionRecord;
use crate::seq::EventSeq;
use serde::{Deserialize, Serialize};

/// Stable identifier for one consumer of the log.
///
/// Each agent / index worker / persistence worker / UI client gets
/// its own `LogConsumerId`. The id is opaque to the backend — it
/// just needs to be unique within a session.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LogConsumerId(pub String);

impl LogConsumerId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl std::fmt::Display for LogConsumerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for LogConsumerId {
    fn from(s: &str) -> Self {
        LogConsumerId(s.to_string())
    }
}

impl From<String> for LogConsumerId {
    fn from(s: String) -> Self {
        LogConsumerId(s)
    }
}

/// Cursor for one consumer.
///
/// `last_seq` is the highest `EventSeq` the consumer has *processed*.
/// The next `read_after` returns records with seq > `last_seq`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConsumerCursor {
    pub consumer: LogConsumerId,
    pub last_seq: EventSeq,
}

impl ConsumerCursor {
    /// A fresh cursor that starts at the beginning of the log.
    pub fn fresh(consumer: LogConsumerId) -> Self {
        Self {
            consumer,
            last_seq: EventSeq::ZERO,
        }
    }

    /// A cursor that has processed every record with seq <= `last_seq`.
    pub fn at(consumer: LogConsumerId, last_seq: EventSeq) -> Self {
        Self { consumer, last_seq }
    }
}

/// Result of a `read_after` call. Either we have records + gaps to
/// deliver, or the cursor is stale and the caller must re-anchor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReadResult {
    Ok {
        records: Vec<ExecutionRecord>,
        /// Gaps observed in the returned slice. These are gaps the
        /// producer recorded into the seq space (or gaps the backend
        /// inferred because the cursor jumps over unallocated
        /// ranges); m1-01 only delivers producer-recorded gaps.
        gaps: Vec<Gap>,
        /// Cursor the caller should pass on the next `read_after` to
        /// continue from where this one left off.
        next_cursor: ConsumerCursor,
    },
    /// The cursor's `last_seq` is older than the oldest seq still in
    /// the log; the caller must re-anchor with
    /// `ConsumerCursor::fresh`.
    CursorStale {
        consumer: LogConsumerId,
        expected: EventSeq,
        current: EventSeq,
    },
    /// The session has no records at all in this backend. (Distinct
    /// from "fresh cursor returned empty" — that comes back as
    /// `Ok { records: vec![], gaps: vec![], next_cursor: fresh }`.)
    SessionNotFound,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_cursor_starts_at_zero() {
        let c = ConsumerCursor::fresh(LogConsumerId::new("agent-a"));
        assert_eq!(c.last_seq, EventSeq::ZERO);
        assert_eq!(c.consumer.0, "agent-a");
    }
}
