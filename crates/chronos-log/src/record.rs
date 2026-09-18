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

/// Type tag for an execution record. The full `ExecutionKind` shape
/// from the spec (with `SymbolId`, `InvocationId`, etc.) lands
/// across m1-01..m1-03; m1-01 ships only the two variants needed
/// for the four required tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ExecutionKind {
    /// Raw trace event (FunctionEntry, VariableWrite, etc.). The full
    /// enum lives in `chronos-domain::EventType`; we re-export the
    /// variants we need for m1-01 tests.
    #[default]
    Raw,
    /// Producer-reported gap marker (the producer created a Gap
    /// record so consumers see the discontinuity).
    GapMarker,
    /// REC-C2.1: a tripwire fired, recorded as durable derived evidence.
    ///
    /// Appended **after** the source `Raw` record it derives from, so the
    /// firing has its own authoritative `ExecutionRecord.seq` while
    /// pointing at the cause through `TripwireFiredEvidence::source_seq`.
    ///
    /// Appended last on purpose: `Raw = 0` and `GapMarker = 1` keep their
    /// historical on-disk discriminants (see `segment::kind_tag`).
    ///
    /// The evaluator must never evaluate *this* kind — that is the
    /// recursion barrier, enforced by matching on `ExecutionKind`, not by
    /// a payload-tag string.
    TripwireFired,
}

/// Opaque record payload for m1-01. The full payload shape grows
/// across m1-01..m1-03 as more producers are migrated; for now we
/// just carry bytes plus a string tag for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ExecutionPayload {
    /// Opaque bytes for m1-01. The full payload shape (with
    /// `EventData`, `SourceLocation`, etc.) grows across later
    /// cycles.
    pub bytes: Vec<u8>,
    /// Free-form diagnostic tag, e.g. `"raw_trace_event"`,
    /// `"ebpf_uprobe"`. Intended for human inspection; not a
    /// contract.
    pub tag: String,
}

impl ExecutionPayload {
    pub fn new(bytes: impl Into<Vec<u8>>, tag: impl Into<String>) -> Self {
        Self {
            bytes: bytes.into(),
            tag: tag.into(),
        }
    }
}

/// Payload tag carried by a `ExecutionKind::TripwireFired` record.
pub const TRIPWIRE_FIRED_EVIDENCE_TAG: &str = "tripwire_fired_evidence";

/// REC-C2.1 — the durable representation of a tripwire firing.
///
/// This is **not** `chronos_domain::TripwireFired`. That type is a runtime
/// notification (it carries a description string and the source `event_id`,
/// which is `0` on the semantic path and is not the authoritative identity).
/// This type is evidence: it must let a replay prove *why* the firing
/// happened, without duplicating the source event (which is already at
/// `source_seq`).
///
/// Identity split (REC-C2.1 contract):
/// - `ExecutionRecord.seq`   — identity of the firing as a durable fact;
/// - `TripwireFiredEvidence::source_seq` — authoritative identity of the
///   event that caused it (`Ref 421 CAUSES_FROM 419`);
/// - `tripwire_id` — a *snapshot of which subscription produced it*, not an
///   identity of the firing (ids are runtime counters, not durable across
///   restarts).
///
/// `source_event_id` is kept as useful metadata for UI/compatibility but is
/// **not** used to correlate durable evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TripwireFiredEvidence {
    /// Which subscription produced this firing (runtime allocation).
    pub tripwire_id: chronos_domain::TripwireId,
    /// Authoritative cause: the `ExecutionRecord.seq` of the accepted source
    /// `Raw` evidence.
    pub source_seq: EventSeq,
    /// Producer-reported event id of the source, when it had one. Metadata
    /// only; never the correlation key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_event_id: Option<u64>,
    /// Snapshot of the condition that matched, so the firing is
    /// self-describing after replay.
    pub condition: chronos_domain::TripwireCondition,
    /// Subscription label at fire time, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Source event's timestamp (session-relative ns) at fire time.
    pub source_timestamp_ns: u64,
    /// Source event's thread id at fire time.
    pub source_thread_id: u64,
}

impl TripwireFiredEvidence {
    /// Encode into an `ExecutionPayload` for a `TripwireFired` record.
    pub fn to_payload(&self) -> Result<ExecutionPayload, serde_json::Error> {
        Ok(ExecutionPayload::new(
            serde_json::to_vec(self)?,
            TRIPWIRE_FIRED_EVIDENCE_TAG,
        ))
    }

    /// Decode from a record payload. Returns `None` when the payload is not
    /// a tripwire-firing payload.
    pub fn from_payload(payload: &ExecutionPayload) -> Result<Option<Self>, serde_json::Error> {
        if payload.tag != TRIPWIRE_FIRED_EVIDENCE_TAG {
            return Ok(None);
        }
        Ok(Some(serde_json::from_slice(&payload.bytes)?))
    }
}

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
