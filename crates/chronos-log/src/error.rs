//! Errors produced by an `ExecutionLogBackend`.

use crate::cursor::LogConsumerId;
use crate::seq::EventSeq;
use std::fmt;

/// All errors that can be returned from a backend.
///
/// Backends MAY add their own error variants behind a
/// `LogError::Backend(String)` payload when they need to surface
/// backend-specific failure modes (I/O errors, etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogError {
    /// The requested session has no records in this backend.
    SessionNotFound,
    /// The supplied cursor's `last_seq` is older than the oldest
    /// seq still in the log. The caller must re-anchor with
    /// `ConsumerCursor::fresh`.
    CursorStale {
        consumer: LogConsumerId,
        expected: EventSeq,
        current: EventSeq,
    },
    /// The backend refused to append (overflow policy = fail).
    AppendFailed { session: String, reason: String },
    /// The supplied `Gap` is malformed (e.g. `first_missing >
    /// last_missing`, or it overlaps an existing allocated seq
    /// range).
    InvalidGap { reason: String },
    /// Backend-specific failure. The wrapped string is human-readable
    /// and intended for logs / error payloads — not for programmatic
    /// matching.
    Backend(String),
}

impl fmt::Display for LogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogError::SessionNotFound => write!(f, "session not found"),
            LogError::CursorStale {
                consumer,
                expected,
                current,
            } => write!(
                f,
                "cursor for consumer `{}` is stale: last_seq={}, oldest available={}",
                consumer.0, expected, current
            ),
            LogError::AppendFailed { session, reason } => {
                write!(f, "append failed for session `{}`: {}", session, reason)
            }
            LogError::InvalidGap { reason } => write!(f, "invalid gap: {}", reason),
            LogError::Backend(msg) => write!(f, "backend error: {}", msg),
        }
    }
}

impl std::error::Error for LogError {}
