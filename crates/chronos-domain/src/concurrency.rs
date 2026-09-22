//! M9.2 — Typed Concurrency Model: primitive + provenance + event types.
//!
//! ## Why this exists
//!
//! ADR-0028 §2.1 introduces a typed model for concurrency primitives
//! (lock/atomic/task/goroutine/message) with provenance. ROADMAP §M9 §91
//! first deliverable: "M9.1 modelo typed de lock/atomic/task/goroutine/message
//! con procedencia". This module is the source of truth for that model.
//!
//! ## Foundation reuse (vs reinventing)
//!
//! `CausalityIndex` (`index/causality.rs`) already tracks write mutations
//! with provenance. We extend it via `Provenance` (richer context) +
//! `ConcurrencyPrimitive` (typed discriminator for lock/atomic/etc.) +
//! `TypedConcurrencyEvent` (the unit of analysis).
//!
//! The integration with `CausalityIndex` is a structural fit: each
//! `CausalityEntry` can be promoted to a `TypedConcurrencyEvent` when the
//! runtime context identifies the sync primitive used. M9.3 (happens-before
//! graph) builds on this foundation.
//!
//! ## Design constraints (per ADR-0028 §3)
//!
//! - **No wall-clock Unix timestamps** (per M6.* / M7.* / M8.* rule).
//! - **Pure types**: no I/O, no global state, no async. Just data.
//! - **`Provenance` carries the "where/when/who"**, `ConcurrencyPrimitive`
//!   carries the "what type of sync", `TypedConcurrencyEvent` is the union.
//!
//! ## Out-of-scope (per ADR-0028 §8)
//!
//! - Causal-clock protocols (Lamport/Vector clocks for multi-host) — M9 NO
//!   decides clock model; single-trace analysis.
//! - Lock-set inference (requires inter-procedural alias analysis).
//! - Per-instruction memory model (x86 TSO, ARM weak, etc.).
//! - Cross-session / cross-trace correlation (M10+ scope).

use crate::index::{CausalityEntry, CausalityIndex};
use crate::trace::{EventId, ThreadId, TimestampNs};
use serde::{Deserialize, Serialize};

/// Type of synchronization primitive observed at a write/read event.
///
/// Per ADR-0028 §2.2 D1. ROADMAP §M9 §91 mentions "lock/atomic/task/
/// goroutine/message". This enum is the canonical discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConcurrencyPrimitive {
    /// Mutex / RwLock / spinlock / critical section.
    Lock,
    /// Atomic operation (atomic load/store/CAS).
    Atomic,
    /// Task / thread spawn (Rust `thread::spawn`, C `pthread_create`, etc.).
    Task,
    /// Goroutine spawn (Go `go func()`).
    Goroutine,
    /// Message-passing primitive (channel send/recv, queue enqueue/dequeue).
    Message,
    /// Unknown / unclassified primitive.
    Unknown,
}

impl ConcurrencyPrimitive {
    /// Human-readable label for diagnostics.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Lock => "lock",
            Self::Atomic => "atomic",
            Self::Task => "task",
            Self::Goroutine => "goroutine",
            Self::Message => "message",
            Self::Unknown => "unknown",
        }
    }

    /// Conservative default for events with no classified primitive.
    /// Per ADR-0004: unknown = `Unknown`, never invent a specific primitive.
    pub fn default_for_unclassified() -> Self {
        Self::Unknown
    }
}

/// Provenance: who/where/when produced a concurrency event.
///
/// Distinct from `CausalityEntry.value_before / value_after` (those are
/// observed values). `Provenance` is the *runtime context* (function +
/// file:line + thread + monotonic timestamp).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    /// Function name where the event occurred (best-effort).
    #[serde(default)]
    pub function: Option<String>,
    /// Source file (best-effort).
    #[serde(default)]
    pub file: Option<String>,
    /// Source line (best-effort).
    #[serde(default)]
    pub line: Option<u32>,
    /// Thread / goroutine / task identifier (runtime-specific).
    #[serde(default)]
    pub thread_id: Option<ThreadId>,
    /// Monotonic timestamp in nanoseconds (NOT wall-clock Unix).
    /// Per project rule: no wall-clock Unix timestamps in M9.*.
    #[serde(default)]
    pub timestamp_ns: Option<TimestampNs>,
}

impl Provenance {
    /// Build provenance from a `CausalityEntry`'s observable fields.
    /// The `thread_id` from `CausalityEntry` is also propagated.
    pub fn from_causality_entry(entry: &CausalityEntry) -> Self {
        Self {
            function: if entry.function.is_empty() {
                None
            } else {
                Some(entry.function.clone())
            },
            file: entry.file.clone(),
            line: entry.line,
            thread_id: Some(entry.thread_id),
            timestamp_ns: Some(entry.timestamp),
        }
    }

    /// Conservative empty provenance (unknown source).
    pub fn empty() -> Self {
        Self {
            function: None,
            file: None,
            line: None,
            thread_id: None,
            timestamp_ns: None,
        }
    }
}

/// Typed concurrency event: the unit of analysis for M9 (D1).
///
/// Combines a discriminator (`primitive`) with provenance (`provenance`) and
/// a stable event ID. The actual observation details (address, value
/// before/after) come from the associated `CausalityEntry` if available.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedConcurrencyEvent {
    /// Stable identifier (typically the `event_id` of the underlying
    /// `CausalityEntry`).
    pub event_id: EventId,
    /// Type of synchronization primitive.
    pub primitive: ConcurrencyPrimitive,
    /// Runtime context (function, file, line, thread, timestamp).
    pub provenance: Provenance,
}

impl TypedConcurrencyEvent {
    /// Build a typed event from a `CausalityEntry` and a primitive kind.
    ///
    /// If `primitive` is `Unknown`, this is a "triage signal" only — not
    /// a verdict. Per ADR-0004 + ADR-0028 §3.2, `detect_concurrent_access`
    /// (engine.rs:766) returns Unknown for unclassified events; M9.3 +
    /// M9.4 downgrades these or escalates them with more context.
    pub fn from_causality_entry(entry: &CausalityEntry, primitive: ConcurrencyPrimitive) -> Self {
        Self {
            event_id: entry.event_id,
            primitive,
            provenance: Provenance::from_causality_entry(entry),
        }
    }

    /// Build an event with explicit provenance (no `CausalityEntry`).
    pub fn new(
        event_id: EventId,
        primitive: ConcurrencyPrimitive,
        provenance: Provenance,
    ) -> Self {
        Self {
            event_id,
            primitive,
            provenance,
        }
    }
}

/// Iterate over `TypedConcurrencyEvent`s derived from a `CausalityIndex`
/// (helper for M9.3 / M9.4 integration).
///
/// This is a thin view built on the public `CausalityIndex` API
/// (`writes_at` + `trace_lineage` + `address_count`). For a given
/// index, it visits each tracked address and emits the per-write entries
/// found there. We keep this simple for M9.2; streaming/incremental is
/// M9.3 concern.
pub fn typed_events_from_index(
    index: &CausalityIndex,
    primitive: ConcurrencyPrimitive,
    addresses: impl IntoIterator<Item = u64>,
) -> Vec<TypedConcurrencyEvent> {
    let mut out = Vec::new();
    for addr in addresses {
        for entry in index.writes_at(addr) {
            out.push(TypedConcurrencyEvent::from_causality_entry(entry, primitive));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::MonotonicNs;

    #[test]
    fn primitive_labels_are_distinct() {
        let labels = [
            ConcurrencyPrimitive::Lock.label(),
            ConcurrencyPrimitive::Atomic.label(),
            ConcurrencyPrimitive::Task.label(),
            ConcurrencyPrimitive::Goroutine.label(),
            ConcurrencyPrimitive::Message.label(),
            ConcurrencyPrimitive::Unknown.label(),
        ];
        let unique: std::collections::HashSet<_> = labels.iter().collect();
        assert_eq!(unique.len(), 6, "primitive labels must be distinct");
    }

    #[test]
    fn default_for_unclassified_is_unknown() {
        assert_eq!(
            ConcurrencyPrimitive::default_for_unclassified(),
            ConcurrencyPrimitive::Unknown,
            "unclassified events default to Unknown per ADR-0004"
        );
    }

    #[test]
    fn provenance_empty_has_no_fields() {
        let p = Provenance::empty();
        assert!(p.function.is_none());
        assert!(p.file.is_none());
        assert!(p.line.is_none());
        assert!(p.thread_id.is_none());
        assert!(p.timestamp_ns.is_none());
    }

    #[test]
    fn typed_event_new_round_trip() {
        let ev = TypedConcurrencyEvent::new(
            42,
            ConcurrencyPrimitive::Lock,
            Provenance {
                function: Some("do_work".to_string()),
                file: Some("src/lib.rs".to_string()),
                line: Some(100),
                thread_id: Some(7),
                timestamp_ns: Some(MonotonicNs(1_000_000)),
            },
        );
        assert_eq!(ev.event_id, 42);
        assert_eq!(ev.primitive, ConcurrencyPrimitive::Lock);
        assert_eq!(ev.provenance.function.as_deref(), Some("do_work"));
        assert_eq!(ev.provenance.thread_id, Some(7));
    }

    #[test]
    fn typed_event_serde_round_trip() {
        let ev = TypedConcurrencyEvent::new(
            99,
            ConcurrencyPrimitive::Atomic,
            Provenance::empty(),
        );
        let json = serde_json::to_string(&ev).expect("serialize");
        let parsed: TypedConcurrencyEvent =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(ev, parsed);
    }

    #[test]
    fn primitive_serde_uses_snake_case() {
        let p = ConcurrencyPrimitive::Goroutine;
        let json = serde_json::to_string(&p).expect("serialize");
        assert_eq!(json, "\"goroutine\"", "serde rename_all = snake_case");
    }

    #[test]
    fn typed_events_from_empty_index_is_empty() {
        let index = CausalityIndex::new();
        // Empty iterator of addresses → empty events list.
        let events = typed_events_from_index(&index, ConcurrencyPrimitive::Lock, std::iter::empty());
        assert!(
            events.is_empty(),
            "fresh CausalityIndex + empty address iterator yields no typed events"
        );
    }

    #[test]
    fn typed_events_from_index_with_recorded_writes() {
        let mut index = CausalityIndex::new();
        let entry = CausalityEntry {
            event_id: 1,
            timestamp: MonotonicNs(100),
            thread_id: 5,
            value_before: None,
            value_after: "v".to_string(),
            function: "go".to_string(),
            file: None,
            line: None,
        };
        index.record_write(0xCAFE, entry, Some("counter"));
        let events = typed_events_from_index(&index, ConcurrencyPrimitive::Atomic, [0xCAFE]);
        assert_eq!(events.len(), 1, "one write should yield one event");
        assert_eq!(events[0].event_id, 1);
        assert_eq!(events[0].primitive, ConcurrencyPrimitive::Atomic);
        assert_eq!(events[0].provenance.function.as_deref(), Some("go"));
        assert_eq!(events[0].provenance.thread_id, Some(5));
    }

    #[test]
    fn typed_event_from_causality_entry_preserves_fields() {
        let entry = CausalityEntry {
            event_id: 7,
            timestamp: MonotonicNs(2000),
            thread_id: 11,
            value_before: None,
            value_after: "42".to_string(),
            function: "main".to_string(),
            file: Some("lib.rs".to_string()),
            line: Some(10),
        };
        let ev = TypedConcurrencyEvent::from_causality_entry(&entry, ConcurrencyPrimitive::Lock);
        assert_eq!(ev.event_id, 7);
        assert_eq!(ev.primitive, ConcurrencyPrimitive::Lock);
        assert_eq!(ev.provenance.function.as_deref(), Some("main"));
        assert_eq!(ev.provenance.file.as_deref(), Some("lib.rs"));
        assert_eq!(ev.provenance.line, Some(10));
        assert_eq!(ev.provenance.thread_id, Some(11));
        assert_eq!(ev.provenance.timestamp_ns, Some(MonotonicNs(2000)));
    }
}
