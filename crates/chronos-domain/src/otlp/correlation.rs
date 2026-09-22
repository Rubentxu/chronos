//! M6.3 lift: OTLP correlation bookkeeping — events ↔ RecordedInvocation.
//!
//! Lifted from `/home/rubentxu/m6-spikes/m6.3-otel-correlation/`.
//!
//! Provides:
//!   - [`ChronosEvent`] — OTLP-side event (probe name, timestamp, fields).
//!     Distinct from `chronos_domain::trace::event::TraceEvent` (the raw
//!     capture unit); this is the post-processed view used for outbound
//!     correlation.
//!   - [`EventField`] — tagged union for `Int | Str | Bool | DomainRef`.
//!   - [`CorrelationStore`] — bidirectional index:
//!     event_idx → RecordedInvocation, plus inverse indices by
//!     invocation_id and trace_id, plus a mutation-before/after map.
//!   - [`ingest_stream`] / [`ingest_mutations`] — bulk loaders.
//!   - [`mutation_is_correlated`] / [`unambiguous_correlation`] /
//!     [`count_unambiguous`] — read-side invariants.
//!   - [`CorrelationError`] — typed errors preserving raw values.

use std::collections::HashMap;
use uuid::Uuid;

use super::{OtlpInvocationId, RecordedInvocation, TraceId};

/// OTLP-side event representation. Distinct from `TraceEvent` (the raw
/// capture unit): this is the post-processed view consumed by exporters.
///
/// `ts_micros` is **session-monotonic microseconds**, NOT Unix time. This
/// matches the OTLP `start_time_unix_nano = "0"` disclosure convention in
/// `chronos-services::session_export::serialize_bundle_otlp_json` —
/// the canonical `chronos.timestamp_monotonic_ns` attribute carries the
/// real nanoseconds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChronosEvent {
    /// Monotonic microseconds since session start.
    pub ts_micros: u64,
    /// Probe name (e.g. `"function_entry"`, `"http.server.request"`).
    pub probe: String,
    /// Field key/value pairs attached to the event.
    pub fields: Vec<(String, EventField)>,
}

/// Tagged union for OTLP field values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventField {
    /// UTF-8 string.
    Str(String),
    /// Signed 64-bit integer.
    Int(i64),
    /// Boolean.
    Bool(bool),
    /// Reference to a domain object (rendered as `"ref:{name}"` in OTLP).
    DomainRef(String),
}

/// Bidirectional correlation index. See module docs for invariants.
#[derive(Debug, Default)]
pub struct CorrelationStore {
    /// Forward map: event_idx → RecordedInvocation.
    bindings: HashMap<u64, RecordedInvocation>,
    /// Reverse map: trace_id bytes → event_idxs.
    by_trace: HashMap<Vec<u8>, Vec<u64>>,
    /// Reverse map: invocation UUID → event_idxs.
    by_invocation: HashMap<Uuid, Vec<u64>>,
    /// Mutation map: event_idx → (before_idx, after_idx).
    mutations: HashMap<u64, (u64, u64)>,
}

impl CorrelationStore {
    /// Build a fresh empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Bind a single `event_idx` to a `RecordedInvocation`. Updates the
    /// inverse indices for fast lookup.
    pub fn bind(&mut self, event_idx: u64, rec: &RecordedInvocation) {
        self.bindings.insert(event_idx, rec.clone());
        self.by_invocation
            .entry(rec.invocation_id.as_uuid())
            .or_default()
            .push(event_idx);
        if let Some(etc) = &rec.external {
            let trace_bytes: Vec<u8> = etc.trace_id().bytes().to_vec();
            self.by_trace
                .entry(trace_bytes)
                .or_default()
                .push(event_idx);
        }
    }

    /// Record a mutation pair. Both endpoints index the same pair, so
    /// lookup from either side returns the full pair.
    pub fn record_mutation(&mut self, before_idx: u64, after_idx: u64) {
        self.mutations.insert(before_idx, (before_idx, after_idx));
        self.mutations.insert(after_idx, (before_idx, after_idx));
    }

    /// Forward lookup: event_idx → RecordedInvocation.
    pub fn lookup(&self, event_idx: u64) -> Option<&RecordedInvocation> {
        self.bindings.get(&event_idx)
    }

    /// Reverse lookup: invocation → event_idxs (in insertion order).
    pub fn events_for_invocation(&self, inv: OtlpInvocationId) -> Vec<u64> {
        self.by_invocation
            .get(&inv.as_uuid())
            .cloned()
            .unwrap_or_default()
    }

    /// Reverse lookup: trace_id → event_idxs (in insertion order).
    pub fn events_for_trace(&self, trace_id: &TraceId) -> Vec<u64> {
        self.by_trace
            .get(trace_id.bytes().as_slice())
            .cloned()
            .unwrap_or_default()
    }

    /// Lookup the mutation pair an event belongs to.
    pub fn mutation_of(&self, event_idx: u64) -> Option<(u64, u64)> {
        self.mutations.get(&event_idx).copied()
    }

    /// Total bindings count.
    pub fn bound_count(&self) -> usize {
        self.bindings.len()
    }

    /// Number of distinct traces currently in the store.
    pub fn trace_count(&self) -> usize {
        self.by_trace.len()
    }

    /// Number of distinct invocations currently in the store.
    pub fn invocation_count(&self) -> usize {
        self.by_invocation.len()
    }

    /// Iterate over `(event_idx, RecordedInvocation)` pairs.
    pub fn bindings_iter(&self) -> impl Iterator<Item = (u64, &RecordedInvocation)> + '_ {
        self.bindings.iter().map(|(k, v)| (*k, v))
    }
}

/// Bind a contiguous range of events to a single RecordedInvocation. The
/// first event gets `event_idx = base`, subsequent events increment by 1.
/// Returns the number of events bound.
pub fn ingest_stream(
    store: &mut CorrelationStore,
    rec: &RecordedInvocation,
    events: &[ChronosEvent],
) -> usize {
    let base = store.bound_count() as u64;
    for i in 0..events.len() {
        store.bind(base + i as u64, rec);
    }
    events.len()
}

/// Record a set of mutation pairs relative to a stream's start index.
/// `mutations` are `(relative_before, relative_after)` pairs; absolute
/// indexes are computed as `stream_start + relative_*`. Returns the
/// number of pairs recorded.
pub fn ingest_mutations(
    store: &mut CorrelationStore,
    stream_start: u64,
    mutations: &[(u64, u64)],
) -> usize {
    let mut recorded = 0;
    for (rel_before, rel_after) in mutations {
        let abs_before = stream_start + rel_before;
        let abs_after = stream_start + rel_after;
        store.record_mutation(abs_before, abs_after);
        recorded += 1;
    }
    recorded
}

/// Check whether a mutation pair is correlated — i.e. the two events
/// share the same RecordedInvocation. Returns Err if either side is
/// unbound.
pub fn mutation_is_correlated(
    store: &CorrelationStore,
    before_idx: u64,
    after_idx: u64,
) -> Result<bool, CorrelationError> {
    let before = store
        .lookup(before_idx)
        .ok_or(CorrelationError::EventNotBound(before_idx))?;
    let after = store
        .lookup(after_idx)
        .ok_or(CorrelationError::EventNotBound(after_idx))?;
    Ok(before == after)
}

/// Typed errors from correlation queries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorrelationError {
    /// Requested event idx has no binding.
    EventNotBound(u64),
    /// Mutation spans two distinct invocation ids.
    InvocationMismatch {
        /// Invocation before.
        before: OtlpInvocationId,
        /// Invocation after.
        after: OtlpInvocationId,
    },
    /// Mutation spans two distinct trace ids (carried as raw bytes).
    TraceMismatch {
        /// Trace bytes before.
        before: Vec<u8>,
        /// Trace bytes after.
        after: Vec<u8>,
    },
}

impl std::fmt::Display for CorrelationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EventNotBound(idx) => {
                write!(f, "event {} is not bound to any RecordedInvocation", idx)
            }
            Self::InvocationMismatch { before, after } => write!(
                f,
                "mutation spans InvocationIds: before={}, after={}",
                before, after
            ),
            Self::TraceMismatch { before, after } => write!(
                f,
                "mutation spans trace_ids: before={}, after={}",
                hex_lower(before),
                hex_lower(after)
            ),
        }
    }
}

impl std::error::Error for CorrelationError {}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// Look up the `RecordedInvocation` for `event_idx`. Errors on unbound.
pub fn unambiguous_correlation(
    store: &CorrelationStore,
    event_idx: u64,
) -> Result<&RecordedInvocation, CorrelationError> {
    store
        .lookup(event_idx)
        .ok_or(CorrelationError::EventNotBound(event_idx))
}

/// Count (unambiguous, ambiguous, unbound) events across the first
/// `event_count` indexes. Currently "ambiguous" is always 0 because the
/// model has 1:1 bindings; reserved for future use.
pub fn count_unambiguous(store: &CorrelationStore, event_count: u64) -> (u64, u64, u64) {
    let mut unambig = 0u64;
    for i in 0..event_count {
        if store.lookup(i).is_some() {
            unambig += 1;
        }
    }
    let unbound = event_count - unambig;
    (unambig, 0, unbound)
}
