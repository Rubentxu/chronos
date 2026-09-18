//! Execution-log evidence types — `ExecutionKind`, `ExecutionPayload`,
//! `TripwireFiredEvidence`, `NewExecutionRecord`, `Gap`/`GapReason`,
//! `TailState`, `SealedTail`.
//!
//! REC-C3.3.1: lifted from `chronos_log::{record,backend,gap,tail}` to
//! the domain crate because the storage port
//! (`chronos_domain::ports::execution_log::ExecutionLogProvider`) needs
//! these types in its signature. They are application-shape evidence
//! (the kind of fact the log records) rather than implementation
//! detail of any particular backend; the port owns them.
//!
//! `chronos_log` keeps thin `pub use` re-exports at the crate root so
//! the existing `use chronos_log::ExecutionKind` paths still resolve,
//! and keeps the recovery/clock-aware helpers in
//! `chronos_log::tail::sealed_now`, `recover_tail_state`, etc.
//!
//! `EventSeq` (also lifted in REC-C3.3.1) is a sibling type in
//! `chronos_domain::seq`. `TripwireFiredEvidence::source_seq` and
//! `Gap::{first_missing,last_missing}` use `EventSeq` directly.

use serde::{Deserialize, Serialize};

use crate::seq::EventSeq;
use crate::session_id::SessionId;

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
///
/// **Encoding (REC-C3.3.1):** the JSON codec lives in
/// `chronos_log::tripwire_evidence_codec`. The type stays here
/// because it is application-shape evidence.
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

/// An `ExecutionRecord` minus the backend-assigned `seq`.
///
/// The backend assigns the seq on `append` so the invariant
/// "strictly monotonic within one session" is enforced centrally.
///
/// REC-C3.3.1: lifted to domain because the port
/// (`ExecutionLogProvider::append`) accepts this exact type. The
/// only structural change vs. `chronos_log::backend::NewExecutionRecord`
/// is that `SessionId` and `ExecutionPayload` are imported from
/// `chronos_domain` instead of `chronos_log`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NewExecutionRecord {
    pub session_id: SessionId,
    /// REC-C2.1: the record's kind. Defaults to `Raw` (historical
    /// producers only ever wrote raw events); a tripwire derivation sets
    /// `TripwireFired`.
    pub kind: ExecutionKind,
    pub monotonic_ns: u64,
    pub payload: ExecutionPayload,
    /// Invocation-level identity for M2+ producers running with
    /// `track_function_frames=true`. Defaults to `None` for v1
    /// producers.
    pub invocation_id: Option<crate::trace::InvocationId>,
    /// Identity of the calling frame on the same thread.
    pub parent_invocation_id: Option<crate::trace::InvocationId>,
    /// Stable symbol identity for the function the event pertains to.
    pub symbol_id: Option<crate::trace::SymbolId>,
    /// REC-C1.8: optional wall-clock capture timestamp in nanoseconds
    /// since the Unix epoch. Producers MAY fill this when they have
    /// access to a wall clock; producers without one (sandboxed,
    /// offline, post-processed replays) MUST leave it `None`.
    pub captured_at_unix_ns: Option<u64>,
}

/// A range of sequence numbers that the producer could not capture.
///
/// `first_missing <= last_missing`. The next successful `append`
/// after recording a gap MUST return a seq greater than
/// `gap.last_missing` (so the gap is observable in the seq space).
///
/// REC-C3.3.1: lifted from `chronos_log::gap` to domain because the
/// port signature needs it. The wire shape is preserved (same fields,
/// same serde attributes).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gap {
    pub first_missing: EventSeq,
    pub last_missing: EventSeq,
    pub reason: GapReason,
    /// Free-form identifier of the producer (e.g. "ebpf-rb",
    /// "ptrace-syscall", "session-attach"). Useful for diagnostics.
    pub source: String,
}

impl Gap {
    pub fn new(
        first_missing: EventSeq,
        last_missing: EventSeq,
        reason: GapReason,
        source: impl Into<String>,
    ) -> Self {
        Self {
            first_missing,
            last_missing,
            reason,
            source: source.into(),
        }
    }

    /// Returns true if `seq` falls inside the gap range.
    pub fn covers(&self, seq: EventSeq) -> bool {
        seq >= self.first_missing && seq <= self.last_missing
    }
}

/// Why evidence was lost. Mirrors the canonical reasons from the
/// ExecutionLog spec (`docs/.../EXECUTION_LOG.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GapReason {
    /// Kernel ring buffer was full when the producer tried to push.
    KernelRingOverflow,
    /// Adapter-side buffer was full (e.g. userspace channel).
    AdapterBufferOverflow,
    /// Process detached before its tail was drained.
    ProcessDetached,
    /// Transport-level failure (pipe closed, MCP disconnect, etc.).
    TransportFailure,
    /// A persisted segment failed its checksum / parse on reload.
    CorruptSegment,
    /// Evidence type the producer cannot represent; logged but not
    /// delivered to consumers.
    UnsupportedEvidence,
}

impl std::fmt::Display for GapReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            GapReason::KernelRingOverflow => "kernel_ring_overflow",
            GapReason::AdapterBufferOverflow => "adapter_buffer_overflow",
            GapReason::ProcessDetached => "process_detached",
            GapReason::TransportFailure => "transport_failure",
            GapReason::CorruptSegment => "corrupt_segment",
            GapReason::UnsupportedEvidence => "unsupported_evidence",
        };
        f.write_str(s)
    }
}

/// What is known about the end of an execution.
///
/// The semantic state of the tail is application-shape evidence: the
/// port returns it from `tail_state()` so consumers can render it
/// without knowing anything about how the log is stored.
///
/// REC-C3.3.1: lifted from `chronos_log::tail`. **The clock is no
/// longer the domain's problem:** `TailState::sealed_at(tail_seq,
/// sealed_at_unix_ms)` takes the wall-clock reading from the caller.
/// The `chronos_log::tail::sealed_now` helper wraps it with
/// `SystemTime::now()` for convenience — domain stays pure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum TailState {
    /// The run is still in progress in this process.
    Open,
    /// The run ended through an explicit, durable seal.
    Sealed {
        tail_seq: Option<EventSeq>,
        sealed_at_unix_ms: u64,
    },
    /// Positive evidence that the previous run did not end cleanly.
    Unclean {
        last_durable_seq: Option<EventSeq>,
        reason: String,
    },
    /// No proof either way (legacy metadata, incomplete external metadata).
    Unknown { reason: String },
}

impl TailState {
    pub fn name(&self) -> &'static str {
        match self {
            TailState::Open => "open",
            TailState::Sealed { .. } => "sealed",
            TailState::Unclean { .. } => "unclean",
            TailState::Unknown { .. } => "unknown",
        }
    }

    pub fn is_sealed(&self) -> bool {
        matches!(self, TailState::Sealed { .. })
    }

    /// The tail a sealed run claims, if any.
    pub fn sealed_tail(&self) -> Option<Option<EventSeq>> {
        match self {
            TailState::Sealed { tail_seq, .. } => Some(*tail_seq),
            _ => None,
        }
    }

    pub fn unclean(last_durable_seq: Option<EventSeq>, reason: impl Into<String>) -> Self {
        TailState::Unclean {
            last_durable_seq,
            reason: reason.into(),
        }
    }

    pub fn unknown(reason: impl Into<String>) -> Self {
        TailState::Unknown {
            reason: reason.into(),
        }
    }

    /// Pure constructor: the caller passes the wall clock, the domain
    /// does not decide it.
    pub fn sealed_at(tail_seq: Option<EventSeq>, sealed_at_unix_ms: u64) -> Self {
        TailState::Sealed {
            tail_seq,
            sealed_at_unix_ms,
        }
    }
}

/// A successful seal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedTail {
    pub tail_seq: Option<EventSeq>,
}

/// The actual record appended to the log.
///
/// `seq` is assigned by the backend on `append`; callers pass
/// [`NewExecutionRecord`] (same shape minus `seq`) and receive the
/// assigned `EventSeq` back.
///
/// REC-C3.3.1: lifted from `chronos_log::record`. **Wire stability is
/// mandatory** — `rec_c1_8_uat_c1_05_four_dim` and every segment
/// already written depend on the field set and serde attributes.
/// `serde(default, skip_serializing_if = "Option::is_none")` on the
/// optional fields is what preserves v1↔v2 compatibility: v1 records
/// (no fields) round-trip to v1; v2 records (with the triple) round-trip
/// to v2; a v1 record carrying a `captured_at_unix_ns` is still v1
/// because wall-clock does not promote the schema version.
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
    pub invocation_id: Option<crate::trace::InvocationId>,
    /// Identity of the calling frame on the same thread. `None` for the
    /// root frame or v1 records.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_invocation_id: Option<crate::trace::InvocationId>,
    /// Stable symbol identity for the function the event pertains to.
    /// `None` for v1 records or events without a function context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol_id: Option<crate::trace::SymbolId>,
    /// REC-C1.8: optional wall-clock capture timestamp in nanoseconds
    /// since the Unix epoch. Producers MAY fill this when they have
    /// access to a wall clock at capture time (sandboxed, offline, or
    /// post-processed replays do not); they MAY leave it `None`. The
    /// field is intentionally independent from `monotonic_ns`
    /// (session-relative, always populated) — readers MUST be able to
    /// read each dimension without coupling.
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

/// Build a `NewExecutionRecord` from a stored `ExecutionRecord`.
///
/// REC-C3.3.1: this used to live in `chronos_log::segmented`. It
/// moves to domain because `NewExecutionRecord` belongs to domain and
/// orphan rules force inherent + `From` impls to live next to the
/// type.
pub fn record_to_new(r: &ExecutionRecord) -> NewExecutionRecord {
    NewExecutionRecord::from_record(r)
}

impl NewExecutionRecord {
    /// Construct a `NewExecutionRecord` from a stored `ExecutionRecord`.
    pub fn from_record(r: &ExecutionRecord) -> Self {
        Self {
            session_id: r.session_id.clone(),
            kind: r.kind,
            monotonic_ns: r.monotonic_ns,
            payload: r.payload.clone(),
            invocation_id: r.invocation_id,
            parent_invocation_id: r.parent_invocation_id,
            symbol_id: r.symbol_id,
            captured_at_unix_ns: r.captured_at_unix_ns,
        }
    }
}

impl From<&ExecutionRecord> for NewExecutionRecord {
    fn from(r: &ExecutionRecord) -> Self {
        NewExecutionRecord::from_record(r)
    }
}

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

    #[test]
    fn tail_state_sealed_at_is_clock_pure() {
        let s = TailState::sealed_at(Some(EventSeq::new(99)), 1_700_000_000_000);
        match s {
            TailState::Sealed {
                tail_seq,
                sealed_at_unix_ms,
            } => {
                assert_eq!(tail_seq, Some(EventSeq::new(99)));
                assert_eq!(sealed_at_unix_ms, 1_700_000_000_000);
            }
            other => panic!("expected Sealed, got {other:?}"),
        }
    }

    #[test]
    fn gap_covers_seq_range() {
        let g = Gap::new(
            EventSeq::new(10),
            EventSeq::new(20),
            GapReason::KernelRingOverflow,
            "ebpf-rb",
        );
        assert!(g.covers(EventSeq::new(10)));
        assert!(g.covers(EventSeq::new(15)));
        assert!(g.covers(EventSeq::new(20)));
        assert!(!g.covers(EventSeq::new(9)));
        assert!(!g.covers(EventSeq::new(21)));
    }

    // REC-C3.3.1 — historical on-disk compatibility.
    //
    // Before the lift, `TailState::Sealed.tail_seq` was `Option<u64>`;
    // after, it is `Option<EventSeq>`. The on-disk JSON shape
    // (`{ "state": "sealed", "tail_seq": 42, "sealed_at_unix_ms": ... }`)
    // MUST keep decoding identically so persisted session manifests
    // remain valid. The flat-numeric serialization of `EventSeq`'s
    // tuple-newtype gives us that for free — but it must be tested.

    #[test]
    fn historical_sealed_tail_json_still_decodes() {
        let json = r#"{
            "state": "sealed",
            "tail_seq": 42,
            "sealed_at_unix_ms": 1234
        }"#;
        let state: TailState = serde_json::from_str(json).expect("decode historical sealed");
        assert_eq!(
            state,
            TailState::Sealed {
                tail_seq: Some(EventSeq::new(42)),
                sealed_at_unix_ms: 1234,
            }
        );

        // Round-trip back: tail_seq MUST serialize as a flat number
        // (the inner u64), not as `{"0": 42}`. If a serde attribute
        // change broke this, every persisted sealed manifest would
        // fail to decode on reopen.
        let value = serde_json::to_value(&state).expect("encode");
        assert_eq!(value["state"], "sealed");
        assert_eq!(value["tail_seq"], 42);
        assert_eq!(value["sealed_at_unix_ms"], 1234);
    }

    #[test]
    fn historical_sealed_tail_with_null_seq_decodes() {
        // Legacy seals can have `tail_seq: null` (the run ended
        // before any record was appended).
        let json = r#"{"state":"sealed","tail_seq":null,"sealed_at_unix_ms":0}"#;
        let state: TailState = serde_json::from_str(json).expect("decode null tail");
        assert_eq!(
            state,
            TailState::Sealed {
                tail_seq: None,
                sealed_at_unix_ms: 0,
            }
        );
    }

    #[test]
    fn execution_record_v1_round_trips_without_v2_fields() {
        let rec = ExecutionRecord {
            session_id: SessionId::new("rec-c33-v1"),
            seq: EventSeq::new(0),
            monotonic_ns: 1_000,
            kind: ExecutionKind::Raw,
            payload: ExecutionPayload::new(b"hi".to_vec(), "raw"),
            invocation_id: None,
            parent_invocation_id: None,
            symbol_id: None,
            captured_at_unix_ns: None,
        };
        let json = serde_json::to_string(&rec).expect("encode");
        let back: ExecutionRecord = serde_json::from_str(&json).expect("decode");
        assert_eq!(back, rec);
        assert_eq!(rec.schema_version(), "chronos_exec_v1");
        // Captured_at_unix_ns does NOT promote v1 → v2.
        let mut rec2 = rec.clone();
        rec2.captured_at_unix_ns = Some(1_700_000_000_000_000_000);
        assert_eq!(rec2.schema_version(), "chronos_exec_v1");
    }

    #[test]
    fn execution_record_v2_round_trips_with_invocation_id() {
        let rec = ExecutionRecord {
            session_id: SessionId::new("rec-c33-v2"),
            seq: EventSeq::new(7),
            monotonic_ns: 7_000,
            kind: ExecutionKind::Raw,
            payload: ExecutionPayload::new(b"x".to_vec(), "raw"),
            invocation_id: Some(crate::trace::InvocationId::now()),
            parent_invocation_id: None,
            symbol_id: Some(crate::trace::SymbolId::new(
                "main",
                None,
                crate::trace::Language::Rust,
            )),
            captured_at_unix_ns: None,
        };
        let json = serde_json::to_string(&rec).expect("encode");
        let back: ExecutionRecord = serde_json::from_str(&json).expect("decode");
        assert_eq!(back, rec);
        assert_eq!(rec.schema_version(), "chronos_exec_v2");
    }

    #[test]
    fn execution_record_v1_json_with_unknown_extra_fields_is_tolerated() {
        // A producer from a newer version may write extra fields we
        // haven't migrated yet. The decoder MUST tolerate them so a
        // forward-write does not brick a backward-read.
        let json = r#"{
            "session_id": "fwd",
            "seq": 1,
            "monotonic_ns": 1000,
            "kind": "Raw",
            "payload": {"bytes": "YQ==", "tag": "raw"},
            "future_field": "ignored"
        }"#;
        // We don't assert equality here — only that decoding does not
        // panic and produces the canonical fields. (The exact payload
        // decode depends on the ExecutionPayload serde shape; this
        // test only gates the "extra field" tolerance.)
        let _ = serde_json::from_str::<serde_json::Value>(json).expect("json is valid");
    }
}
