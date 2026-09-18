//! Execution-log evidence types — `ExecutionKind`, `ExecutionPayload`,
//! `TripwireFiredEvidence`.
//!
//! REC-C3.3.1: lifted from `chronos_log::record` to the domain
//! crate because the storage port
//! (`chronos_domain::ports::execution_log::ExecutionLogProvider`)
//! needs these types in its signature. They are application-shape
//! evidence (the kind of fact the log records) rather than
//! implementation detail of any particular backend; the port owns
//! them. `chronos_log` re-exports each one for backward
//! compatibility.
//!
//! `EventSeq` (also lifted in REC-C3.3.1) is a sibling type in
//! `chronos_domain::seq`. `TripwireFiredEvidence::source_seq` uses
//! `EventSeq` directly.

use serde::{Deserialize, Serialize};

use crate::seq::EventSeq;

/// Type tag for an execution record. The full `ExecutionKind` shape
/// (with `FunctionEntry`, `VariableWrite`, etc.) lands across
/// m1-01..m1-03; this module ships only the variants needed for the
/// current tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ExecutionKind {
    /// Raw trace event (FunctionEntry, VariableWrite, etc.). The full
    /// enum lives in `chronos_domain::EventType`; we re-export the
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
    /// historical on-disk discriminants (see `chronos_log::segment::kind_tag`).
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
///
/// **Wire stability (REC-C3.3.1):** the field set and serde attributes
/// are preserved byte-for-byte from the previous
/// `chronos_log::record::TripwireFiredEvidence`. The on-disk segment
/// format depends on these field names; changing them would break
/// `rec_c2_1_tripwire_evidence` and every segment already written.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TripwireFiredEvidence {
    /// Which subscription produced this firing (runtime allocation).
    pub tripwire_id: crate::tripwire::TripwireId,
    /// Authoritative cause: the `ExecutionRecord.seq` of the accepted source
    /// `Raw` evidence.
    pub source_seq: EventSeq,
    /// Producer-reported event id of the source, when it had one. Metadata
    /// only; never the correlation key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_event_id: Option<u64>,
    /// Snapshot of the condition that matched, so the firing is
    /// self-describing after replay.
    pub condition: crate::tripwire::TripwireCondition,
    /// Subscription label at fire time, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Source event's timestamp (session-relative ns) at fire time.
    pub source_timestamp_ns: u64,
    /// Source event's thread id at fire time.
    pub source_thread_id: u64,
}

// Note (REC-C3.3.1): the previous `to_payload` / `from_payload` methods
// used `serde_json` to round-trip the struct through an
// `ExecutionPayload`. `serde_json` is a dev-only dep of
// `chronos_domain` (test fixtures), so the implementation is now a
// free function in `chronos_log::tripwire_evidence_codec`. The codec
// is a backend implementation detail; the type stays in domain as
// application-shape evidence.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_constructs() {
        let p = ExecutionPayload::new(vec![1, 2, 3], "raw");
        assert_eq!(p.bytes, vec![1, 2, 3]);
        assert_eq!(p.tag, "raw");
    }

    #[test]
    fn execution_kind_default_is_raw() {
        assert_eq!(ExecutionKind::default(), ExecutionKind::Raw);
    }
}
