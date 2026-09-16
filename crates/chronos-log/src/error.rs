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
    /// The requested position is older than the retention boundary
    /// (REC-C1.5.1).
    ///
    /// Retention is a LOGICAL boundary: from the instant a range is declared
    /// retained the answer is the same before and after a restart, even while
    /// physical reclamation is still in flight. This is deliberately NOT a gap:
    /// the evidence was retired by policy, not lost, and no silent re-anchor is
    /// performed.
    PositionBeforeRetention {
        requested_next_seq: EventSeq,
        retained_from: EventSeq,
    },
    /// Retention metadata is absent and the layout does not allow it to be
    /// inferred (REC-C1.5.1).
    ///
    /// A directory without a manifest whose first segment does not start at
    /// seq#0 could mean "history legitimately retained" or "files are missing".
    /// Rather than fabricate truth from absence, the reopen refuses.
    RetentionMetadataMissing { dir: String, first_segment_seq: u64 },
    /// A manifest's session identity disagrees with the requested one.
    IdentityMismatch { requested: String, manifest: String },
    /// A surviving segment crosses the retention boundary, so the manifest and
    /// the on-disk layout are incompatible (REC-C1.5.1).
    SegmentCrossesRetention {
        segment_start: u64,
        segment_end: u64,
        retained_from: u64,
    },
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
            LogError::PositionBeforeRetention {
                requested_next_seq,
                retained_from,
            } => write!(
                f,
                "position {} is before the retention boundary {}",
                requested_next_seq, retained_from
            ),
            LogError::RetentionMetadataMissing {
                dir,
                first_segment_seq,
            } => write!(
                f,
                "retention metadata missing in {:?}: first segment starts at {} \
                 (cannot distinguish retained history from missing files)",
                dir, first_segment_seq
            ),
            LogError::IdentityMismatch {
                requested,
                manifest,
            } => write!(
                f,
                "execution log identity mismatch: requested {:?}, manifest {:?}",
                requested, manifest
            ),
            LogError::SegmentCrossesRetention {
                segment_start,
                segment_end,
                retained_from,
            } => write!(
                f,
                "segment {}..={} crosses the retention boundary {}",
                segment_start, segment_end, retained_from
            ),
            LogError::Backend(msg) => write!(f, "backend error: {}", msg),
        }
    }
}

impl std::error::Error for LogError {}
