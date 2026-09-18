//! `ExecutionRecord`, `ExecutionKind`, `ExecutionPayload`.
//!
//! `ExecutionRecord` carries the producer-reported `invocation_id`,
//! `parent_invocation_id`, and `symbol_id` identity fields (v2) plus the
//! opaque payload. v1 records (m0/m1 producers without frame tracking)
//! leave the identity fields `None`; readers tolerate both shapes via
//! serde defaults. (m2-09 corrected the module docs to describe the
//! shipped v2 shape rather than the original m1-01 subset.)
//!
//! `SessionId` used to live here as a duplicate of
//! `chronos_domain::session_id::SessionId`. REC-C3.3.1 deletes the
//! duplicate and re-exports the canonical type from
//! `crates/chronos-log/src/lib.rs`. All call sites that used
//! `chronos_log::record::SessionId` now resolve to the same type via
//! `pub use chronos_domain::session_id::SessionId`.

use crate::seq::EventSeq;
use serde::{Deserialize, Serialize};

// REC-C3.3.1: `SessionId` lives in `chronos_domain::session_id`; the
// duplicate definition in this module was deleted. The field
// `pub session_id: SessionId` (used by `ExecutionRecord`) refers to
// the canonical domain type. Re-exporting through `crate::SessionId`
// is a public-API move; the *internal* path uses the canonical one
// so the lib compiles without depending on `crate::lib`'s re-export
// resolution order.
pub use chronos_domain::session_id::SessionId;

/// The actual record appended to the log.
///
/// `seq` is assigned by the backend on `append`; callers pass
/// `NewExecutionRecord` (same shape minus `seq`) and receive the
/// assigned `EventSeq` back.
///
/// The `invocation_id`, `parent_invocation_id`, and `symbol_id` fields
/// are populated by producers that run with `track_function_frames=true`
/// (M2+); m0/m1 producers leave them as `None`. Readers MUST tolerate
/// both v1 (no fields) and v2 (fields populated) records — serde's
/// default behaviour treats missing fields as `None`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub session_id: SessionId,
    pub seq: EventSeq,
    pub monotonic_ns: u64,
    pub kind: ExecutionKind,
    pub payload: ExecutionPayload,
    /// Invocation-level identity when the producer ran with
    /// `track_function_frames=true`. `None` for v1 records.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invocation_id: Option<chronos_domain::InvocationId>,
    /// Identity of the calling frame on the same thread. `None` for the
    /// root frame or v1 records.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_invocation_id: Option<chronos_domain::InvocationId>,
    /// Stable symbol identity for the function the event pertains to.
    /// `None` for v1 records or events without a function context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol_id: Option<chronos_domain::SymbolId>,
    /// REC-C1.8: optional wall-clock capture timestamp in nanoseconds
    /// since the Unix epoch. Producers MAY fill this when they have
    /// access to a wall clock at capture time (sandboxed, offline, or
    /// post-processed replays do not); they MAY leave it `None`. The
    /// field is intentionally independent from `monotonic_ns`
    /// (session-relative, always populated) — readers MUST be able to
    /// read each dimension without coupling. UAT-REC-C1-05 closes on
    /// the four-dimension independence invariant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captured_at_unix_ns: Option<u64>,
}

impl ExecutionRecord {
    /// Logical schema version of the record. `"chronos_exec_v1"` when
    /// the invocation/symbol fields are all `None`; `"chronos_exec_v2"`
    /// when any of them is populated.
    ///
    /// REC-C1.8: `captured_at_unix_ns` does NOT promote v1 → v2; a
    /// v1-shape record that happens to carry a wall-clock capture
    /// timestamp is still `"chronos_exec_v1"`. Promotion is reserved
    /// for the invocation/symbol triple (the M2+ fields), which
    /// distinguish record types — wall-clock is a producer-optional
    /// dimension that any record can carry without changing its type.
    pub fn schema_version(&self) -> &'static str {
        if self.invocation_id.is_some()
            || self.parent_invocation_id.is_some()
            || self.symbol_id.is_some()
        {
            "chronos_exec_v2"
        } else {
            "chronos_exec_v1"
        }
    }
}

/// REC-C3.3.1: re-export `ExecutionKind`, `ExecutionPayload`, and
/// `TripwireFiredEvidence` (plus the tripwire tag) from
/// `chronos_domain::evidence`. The duplicate definitions that used
/// to live here were deleted; the canonical types now live in the
/// domain crate because the storage port
/// (`chronos_domain::ports::execution_log::ExecutionLogProvider`)
/// owns them. Wire shape is preserved byte-for-byte (same field set,
/// same serde attributes, same on-disk discriminants).
pub use chronos_domain::evidence::{
    ExecutionKind, ExecutionPayload, TripwireFiredEvidence, TRIPWIRE_FIRED_EVIDENCE_TAG,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_id_display() {
        let s = SessionId::new("abc-123");
        assert_eq!(s.to_string(), "abc-123");
        assert_eq!(s.as_str(), "abc-123");
    }

    #[test]
    fn event_seq_next_is_pure_arithmetic() {
        let s = EventSeq::new(7);
        assert_eq!(s.next(), EventSeq::new(8));
        // next() does not allocate seq#8 — it is just arithmetic.
        let s2 = s.next();
        assert_eq!(s2.get(), 8);
    }

    #[test]
    fn payload_constructs() {
        let p = ExecutionPayload::new(vec![1, 2, 3], "raw");
        assert_eq!(p.bytes, vec![1, 2, 3]);
        assert_eq!(p.tag, "raw");
    }
}
