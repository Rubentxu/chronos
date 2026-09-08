//! `EventSeq` — strictly monotonic sequence number within one session.
//!
//! Strict monotonicity is a *backend* invariant; the newtype itself is
//! `Ord` so the backend can use it directly.

use serde::{Deserialize, Serialize};

/// A sequence number assigned to each record by the backend on
/// `append`. Strictly monotonic within one session: two successful
/// `append` calls on the same session receive seqs `n` and `n + 1`
/// (or higher if a gap was recorded in between).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EventSeq(pub u64);

impl EventSeq {
    pub const ZERO: EventSeq = EventSeq(0);

    #[inline]
    pub const fn new(value: u64) -> Self {
        EventSeq(value)
    }

    #[inline]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Returns the next sequence number. Pure arithmetic — does NOT
    /// imply the next seq is unallocated.
    #[inline]
    pub const fn next(self) -> Self {
        EventSeq(self.0 + 1)
    }
}

impl std::fmt::Display for EventSeq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "seq#{}", self.0)
    }
}
