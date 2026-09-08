//! `ExecutionRecord`, `ExecutionKind`, `ExecutionPayload`, `SessionId`.
//!
//! The full `ExecutionRecord` shape from the spec is large; this
//! module ships the subset needed for m1-01 (cases 1–4 from the
//! spec). Fields like `invocation_id`, `links`, `provenance`, etc.
//! are added in subsequent m1-NN cycles when the producer/query
//! paths that need them are migrated.

use crate::seq::EventSeq;
use serde::{Deserialize, Serialize};

/// Identifier of a capture session. Distinct from the in-memory
/// `McpSessionId` used in `chronos-mcp`; one Chronos session may
/// produce multiple `SessionId`s if it covers multiple targets,
/// and one capture may span multiple processes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub String);

impl SessionId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for SessionId {
    fn from(s: &str) -> Self {
        SessionId(s.to_string())
    }
}

impl From<String> for SessionId {
    fn from(s: String) -> Self {
        SessionId(s)
    }
}

/// The actual record appended to the log.
///
/// `seq` is assigned by the backend on `append`; callers pass
/// `NewExecutionRecord` (same shape minus `seq`) and receive the
/// assigned `EventSeq` back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub session_id: SessionId,
    pub seq: EventSeq,
    pub monotonic_ns: u64,
    pub kind: ExecutionKind,
    pub payload: ExecutionPayload,
}

/// Type tag for an execution record. The full `ExecutionKind` shape
/// from the spec (with `SymbolId`, `InvocationId`, etc.) lands
/// across m1-01..m1-03; m1-01 ships only the two variants needed
/// for the four required tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionKind {
    /// Raw trace event (FunctionEntry, VariableWrite, etc.). The full
    /// enum lives in `chronos-domain::EventType`; we re-export the
    /// variants we need for m1-01 tests.
    Raw,
    /// Producer-reported gap marker (the producer created a Gap
    /// record so consumers see the discontinuity).
    GapMarker,
}

/// Opaque record payload for m1-01. The full payload shape grows
/// across m1-01..m1-03 as more producers are migrated; for now we
/// just carry bytes plus a string tag for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
