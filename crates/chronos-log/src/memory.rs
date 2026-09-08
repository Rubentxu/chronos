//! `InMemoryExecutionLog` — the in-memory backend for m1-01.
//!
//! Concurrency model:
//! - One `Mutex<Vec<RecordEntry>>` for the per-session record list.
//!   Appends acquire the lock briefly to assign the seq and push.
//! - One `Mutex<HashMap<(SessionId, LogConsumerId), EventSeq>>` for
//!   per-consumer cursor state, separate from the record list so
//!   reads don't move the append path.
//!
//! The append path is not lock-free, but it is short (one Vec push
//! plus one seq assignment under the same lock) and benchmarked to
//! scale to the same throughput as the existing `EventBus`. m1-01
//! scope is API plus invariants; lock-free redesign is m1-02.

use crate::backend::{ExecutionLogBackend, NewExecutionRecord};
use crate::cursor::{ConsumerCursor, LogConsumerId, ReadResult};
use crate::error::LogError;
use crate::gap::Gap;
use crate::record::{ExecutionKind, ExecutionPayload, ExecutionRecord, SessionId};
use crate::seq::EventSeq;

use std::collections::{BTreeSet, HashMap};
use std::sync::Mutex;
use uuid::Uuid;

/// One entry in the per-session record list.
#[derive(Debug, Clone)]
enum RecordEntry {
    Record(ExecutionRecord),
    Gap(Gap),
}

/// Identity-based secondary indexes used by the M2 read surface.
/// Keyed by `SessionId` so each session has its own index namespace.
type InvocationIndex = HashMap<Uuid, BTreeSet<EventSeq>>;
type SymbolIndexKey = chronos_domain::SymbolId;
type SymbolIndex = HashMap<SymbolIndexKey, BTreeSet<EventSeq>>;

/// The in-memory backend.
#[derive(Debug, Default)]
pub struct InMemoryExecutionLog {
    /// Records + gaps per session, in append order.
    records: Mutex<HashMap<SessionId, Vec<RecordEntry>>>,
    /// Per-session monotonic seq allocator (next seq to assign).
    next_seq: Mutex<HashMap<SessionId, EventSeq>>,
    /// Per-(session, consumer) high-water seq (last seq the consumer
    /// has *processed*).
    cursors: Mutex<HashMap<(SessionId, LogConsumerId), EventSeq>>,
    /// Per-session index: invocation_id → set of seqs that carry it.
    /// Populated by `append()`, `replay_record()`, and pruned by
    /// `prune_secondary_indexes_up_to()`. See REQ-IndexesRebuiltOnReplay.
    invocation_index: Mutex<HashMap<SessionId, InvocationIndex>>,
    /// Per-session index: parent_invocation_id → set of seqs that
    /// reference it as a parent. Populated the same way.
    parent_index: Mutex<HashMap<SessionId, InvocationIndex>>,
    /// Per-session index: symbol_id → set of seqs that carry it.
    /// Populated the same way. Looked up with a time-range filter
    /// (the BTreeSet supports range queries natively).
    symbol_index: Mutex<HashMap<SessionId, SymbolIndex>>,
}

impl InMemoryExecutionLog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Total number of records + gaps stored on `session_id`.
    pub fn entry_count(&self, session_id: &SessionId) -> usize {
        let records = self.records.lock().expect("records lock poisoned");
        records.get(session_id).map(|v| v.len()).unwrap_or(0)
    }

    /// Convenience: append a record with default kind = `Raw`.
    pub fn append_raw(
        &self,
        session_id: SessionId,
        monotonic_ns: u64,
        tag: impl Into<String>,
    ) -> Result<EventSeq, LogError> {
        let payload = ExecutionPayload::new(Vec::<u8>::new(), tag);
        self.append(NewExecutionRecord {
            session_id,
            monotonic_ns,
            payload,
            invocation_id: None,
            parent_invocation_id: None,
            symbol_id: None,
        })
    }

    /// Allocate the next seq on `session_id` without inserting a
    /// record. Used by `SegmentedExecutionLog` to reserve a seq
    /// for an overflow-driven gap before the gap is recorded.
    pub fn allocate_seq_for_gap(&self, session_id: &SessionId) -> Result<EventSeq, LogError> {
        let mut next_seq = self.next_seq.lock().expect("next_seq lock poisoned");
        let seq = next_seq.entry(session_id.clone()).or_insert(EventSeq::ZERO);
        let assigned = *seq;
        *seq = seq.next();
        Ok(assigned)
    }

    /// Append a record using the seq **already on the
    /// `ExecutionRecord`** instead of allocating a fresh one. Used
    /// by the segment replay path: the segment stores seqs
    /// already assigned by the original backend, so the allocator
    /// must advance *past* them. Asserts seqs are monotonically
    /// non-decreasing.
    pub fn replay_record(&self, r: &ExecutionRecord) -> Result<(), LogError> {
        let mut next_seq = self.next_seq.lock().expect("next_seq lock poisoned");
        let allocator = next_seq
            .entry(r.session_id.clone())
            .or_insert(EventSeq::ZERO);
        assert!(
            r.seq >= *allocator,
            "replay_record: seq {} was below current allocator {}",
            r.seq.0,
            allocator.0
        );
        *allocator = r.seq.next();
        let mut records = self.records.lock().expect("records lock poisoned");
        records
            .entry(r.session_id.clone())
            .or_default()
            .push(RecordEntry::Record(r.clone()));
        drop(records);
        drop(next_seq);

        // Same insertion order as live `append()`: primary record
        // first, then secondary index. On cold-start replay this
        // rebuilds the indexes identically to live appends.
        self.index_record(r);
        Ok(())
    }

    /// Look up the stored cursor for `consumer` on `session_id`.
    /// `None` means the consumer has never read from this session.
    pub fn cursor(&self, session_id: &SessionId, consumer: &LogConsumerId) -> Option<EventSeq> {
        let cursors = self.cursors.lock().expect("cursors lock poisoned");
        cursors
            .get(&(session_id.clone(), consumer.clone()))
            .copied()
    }

    /// Seed the cursor for `(session_id, consumer)` from disk on
    /// cold boot. If a cursor already exists for this consumer,
    /// the higher of the two is kept — this protects against an
    /// in-flight read-after-replay from rolling the cursor
    /// backwards. If `last_seq` is `None`, this is a no-op (used
    /// when the sidecar lists the consumer but the cursor is
    /// absent).
    pub fn seed_cursor(
        &self,
        session_id: &SessionId,
        consumer: &LogConsumerId,
        last_seq: EventSeq,
    ) {
        let mut cursors = self.cursors.lock().expect("cursors lock poisoned");
        let entry = cursors
            .entry((session_id.clone(), consumer.clone()))
            .or_insert(last_seq);
        if last_seq > *entry {
            *entry = last_seq;
        }
    }

    /// Add `r`'s identity fields (invocation_id / parent_invocation_id
    /// / symbol_id) to the secondary indexes, when present. v1 records
    /// (None on all three) contribute nothing — they are unreachable
    /// by the identity-based read methods, which matches REQ-GetByInvocation
    /// / REQ-ChildrenOf / REQ-InRangeBySymbol.
    fn index_record(&self, r: &ExecutionRecord) {
        let sid = r.session_id.clone();
        let seq = r.seq;
        if let Some(inv) = r.invocation_id {
            let mut idx = self
                .invocation_index
                .lock()
                .expect("invocation_index poisoned");
            idx.entry(sid.clone())
                .or_default()
                .entry(inv.0)
                .or_default()
                .insert(seq);
        }
        if let Some(parent) = r.parent_invocation_id {
            let mut idx = self.parent_index.lock().expect("parent_index poisoned");
            idx.entry(sid.clone())
                .or_default()
                .entry(parent.0)
                .or_default()
                .insert(seq);
        }
        if let Some(sym) = r.symbol_id {
            let mut idx = self.symbol_index.lock().expect("symbol_index poisoned");
            idx.entry(sid)
                .or_default()
                .entry(sym)
                .or_default()
                .insert(seq);
        }
    }

    /// Drop entries with `seq <= cutoff` from every secondary index.
    /// Called by `SegmentedExecutionLog::compact_up_to` after the
    /// underlying records have been evicted from disk and from the
    /// `records` Vec. Satisfies REQ-IndexesPrunedOnCompaction.
    pub fn prune_secondary_indexes_up_to(&self, cutoff: EventSeq) {
        {
            let mut idx = self
                .invocation_index
                .lock()
                .expect("invocation_index poisoned");
            for inner in idx.values_mut() {
                for set in inner.values_mut() {
                    set.retain(|s| *s > cutoff);
                }
                inner.retain(|_, set| !set.is_empty());
            }
        }
        {
            let mut idx = self.parent_index.lock().expect("parent_index poisoned");
            for inner in idx.values_mut() {
                for set in inner.values_mut() {
                    set.retain(|s| *s > cutoff);
                }
                inner.retain(|_, set| !set.is_empty());
            }
        }
        {
            let mut idx = self.symbol_index.lock().expect("symbol_index poisoned");
            for inner in idx.values_mut() {
                for set in inner.values_mut() {
                    set.retain(|s| *s > cutoff);
                }
                inner.retain(|_, set| !set.is_empty());
            }
        }
    }

    /// Resolve a set of seqs back to the concrete `ExecutionRecord`
    /// values they point at. Records whose seq is no longer present
    /// in the underlying `records` Vec (e.g. pruned by compaction)
    /// are silently skipped.
    fn resolve_seqs(
        &self,
        session_id: &SessionId,
        seqs: impl IntoIterator<Item = EventSeq>,
    ) -> Vec<ExecutionRecord> {
        let records = self.records.lock().expect("records lock poisoned");
        let entries = match records.get(session_id) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let mut out = Vec::new();
        for s in seqs {
            // Linear scan; the per-key BTreeSet is small (typical
            // recursion depth is ≤32). For very wide keys, a
            // positional index over the Vec is a future cycle.
            for entry in entries {
                if let RecordEntry::Record(r) = entry {
                    if r.seq == s {
                        out.push(r.clone());
                        break;
                    }
                }
            }
        }
        out
    }

    /// Return every record in `session_id` whose `invocation_id`
    /// equals `Some(id)`, in seq order. Records with `invocation_id
    /// == None` (v1) are excluded. Returns `Vec::new()` if the
    /// session or invocation is unknown. See REQ-GetByInvocation.
    pub fn get_by_invocation(
        &self,
        session_id: &SessionId,
        id: chronos_domain::InvocationId,
    ) -> Vec<ExecutionRecord> {
        let seqs = {
            let idx = self
                .invocation_index
                .lock()
                .expect("invocation_index poisoned");
            idx.get(session_id)
                .and_then(|m| m.get(&id.0))
                .cloned()
                .unwrap_or_default()
        };
        // BTreeSet already sorts; resolve in order.
        self.resolve_seqs(session_id, seqs)
    }

    /// Return every record in `session_id` whose `parent_invocation_id`
    /// equals `Some(parent_id)`, in seq order. Records with
    /// `parent_invocation_id == None` are excluded. See REQ-ChildrenOf.
    pub fn children_of(
        &self,
        session_id: &SessionId,
        parent_id: chronos_domain::InvocationId,
    ) -> Vec<ExecutionRecord> {
        let seqs = {
            let idx = self.parent_index.lock().expect("parent_index poisoned");
            idx.get(session_id)
                .and_then(|m| m.get(&parent_id.0))
                .cloned()
                .unwrap_or_default()
        };
        self.resolve_seqs(session_id, seqs)
    }

    /// Return every record in `session_id` whose `symbol_id` equals
    /// `Some(symbol)` AND whose `monotonic_ns` lies in `[start_ns,
    /// end_ns)`, in seq order. See REQ-InRangeBySymbol.
    pub fn in_range_by_symbol(
        &self,
        session_id: &SessionId,
        symbol: chronos_domain::SymbolId,
        start_ns: u64,
        end_ns: u64,
    ) -> Vec<ExecutionRecord> {
        let seqs: Vec<EventSeq> = {
            let idx = self.symbol_index.lock().expect("symbol_index poisoned");
            match idx.get(session_id).and_then(|m| m.get(&symbol)) {
                Some(set) => set
                    .iter()
                    .copied()
                    .collect::<Vec<_>>()
                    .into_iter()
                    // BTreeSet range filter on monotonic_ns requires
                    // us to look up each record's monotonic_ns; we
                    // resolve the records first, then filter on time.
                    .collect(),
                None => return Vec::new(),
            }
        };
        self.resolve_seqs(session_id, seqs)
            .into_iter()
            .filter(|r| r.monotonic_ns >= start_ns && r.monotonic_ns < end_ns)
            .collect()
    }
}

impl ExecutionLogBackend for InMemoryExecutionLog {
    fn append(&self, record: NewExecutionRecord) -> Result<EventSeq, LogError> {
        let session_id = record.session_id.clone();
        let mut next_seq = self.next_seq.lock().expect("next_seq lock poisoned");
        let seq = next_seq.entry(session_id.clone()).or_insert(EventSeq::ZERO);

        // It's a bug to call append() while a higher seq is already
        // outstanding — but the in-memory backend can't see that
        // externally. We assume the caller is well-behaved.

        let mut records = self.records.lock().expect("records lock poisoned");
        let stored = ExecutionRecord {
            session_id: session_id.clone(),
            seq: *seq,
            monotonic_ns: record.monotonic_ns,
            kind: ExecutionKind::Raw,
            payload: record.payload,
            invocation_id: record.invocation_id,
            parent_invocation_id: record.parent_invocation_id,
            symbol_id: record.symbol_id,
        };
        records
            .entry(session_id.clone())
            .or_default()
            .push(RecordEntry::Record(stored.clone()));

        let assigned = *seq;
        *seq = seq.next();
        drop(records);
        drop(next_seq);

        // Secondary-index insertion happens *after* the primary record
        // is in the Vec, so any concurrent reader sees either the old
        // state (no record, no index entry) or the new state (record
        // + index entry) — never a dangling index entry.
        self.index_record(&stored);
        Ok(assigned)
    }

    fn record_gap(&self, session_id: SessionId, gap: Gap) -> Result<(), LogError> {
        if gap.first_missing > gap.last_missing {
            return Err(LogError::InvalidGap {
                reason: format!(
                    "first_missing ({}) > last_missing ({})",
                    gap.first_missing, gap.last_missing
                ),
            });
        }

        let mut next_seq = self.next_seq.lock().expect("next_seq lock poisoned");
        let mut records = self.records.lock().expect("records lock poisoned");

        // Bump the seq allocator past the gap so the next append()
        // returns a seq strictly greater than gap.last_missing.
        let allocator = next_seq.entry(session_id.clone()).or_insert(EventSeq::ZERO);
        if gap.last_missing >= *allocator {
            *allocator = gap.last_missing.next();
        }

        records
            .entry(session_id)
            .or_default()
            .push(RecordEntry::Gap(gap));
        Ok(())
    }

    fn read_after(
        &self,
        session_id: SessionId,
        consumer: LogConsumerId,
        cursor: Option<ConsumerCursor>,
    ) -> Result<ReadResult, LogError> {
        // Semantics of `last_seq`:
        // - A *fresh* cursor (cursor == None) means "I have processed
        //   nothing yet; give me every record from seq#0 onward."
        // - For any subsequent cursor (cursor.last_seq = n), the
        //   meaning is "I have processed seq#n; give me seq#(n+1)
        //   onward".
        //
        // The two cases are *not* distinguishable by `last_seq`
        // alone (both look like `last_seq = 0` for an empty log or a
        // fresh cursor), so we use `cursor.is_none()` to flag the
        // fresh case. m1-02 introduces a `Cursor::Fresh` enum
        // variant to make this explicit.
        let fresh = cursor.is_none();
        let effective_cursor = cursor.unwrap_or_else(|| ConsumerCursor::fresh(consumer.clone()));

        // First, look up the consumer's stored high-water (if any) so
        // we can detect "cursor older than oldest available".
        let stored = {
            let cursors = self.cursors.lock().expect("cursors lock poisoned");
            cursors
                .get(&(session_id.clone(), consumer.clone()))
                .copied()
        };

        let records_snapshot = {
            let records = self.records.lock().expect("records lock poisoned");
            records.get(&session_id).cloned()
        };

        let entries = match records_snapshot {
            None => {
                if !fresh {
                    // Caller asked for records past seq 0; the session
                    // is empty.
                    return Ok(ReadResult::SessionNotFound);
                }
                return Ok(ReadResult::Ok {
                    records: Vec::new(),
                    gaps: Vec::new(),
                    next_cursor: effective_cursor,
                });
            }
            Some(entries) => entries,
        };

        // Determine the oldest seq currently in the log.
        let oldest_seq = entries
            .iter()
            .map(|e| match e {
                RecordEntry::Record(r) => r.seq,
                RecordEntry::Gap(g) => g.first_missing,
            })
            .min()
            .unwrap_or(EventSeq::ZERO);

        if !fresh && effective_cursor.last_seq < oldest_seq && stored.is_some() {
            return Err(LogError::CursorStale {
                consumer,
                expected: effective_cursor.last_seq,
                current: oldest_seq,
            });
        }

        // Collect records + gaps.
        //   - fresh cursor: include every record (even seq#0).
        //   - cursor with last_seq = n: include records with seq > n.
        let mut out_records: Vec<ExecutionRecord> = Vec::new();
        let mut out_gaps: Vec<Gap> = Vec::new();
        let mut max_seq = effective_cursor.last_seq;
        for entry in entries {
            match entry {
                RecordEntry::Record(r) => {
                    let include = fresh || r.seq > effective_cursor.last_seq;
                    if include {
                        if r.seq > max_seq {
                            max_seq = r.seq;
                        }
                        out_records.push(r);
                    }
                }
                RecordEntry::Gap(g) => {
                    // Include any gap whose end is past the cursor.
                    // For a fresh cursor, include any gap that
                    // affects at least seq#0.
                    let include = if fresh {
                        g.last_missing > EventSeq::ZERO
                    } else {
                        g.last_missing > effective_cursor.last_seq
                    };
                    if include {
                        out_gaps.push(g);
                    }
                }
            }
        }

        let next_cursor = ConsumerCursor::at(consumer.clone(), max_seq);

        // Persist the new cursor for this consumer.
        {
            let mut cursors = self.cursors.lock().expect("cursors lock poisoned");
            cursors.insert((session_id, consumer), max_seq);
        }

        Ok(ReadResult::Ok {
            records: out_records,
            gaps: out_gaps,
            next_cursor,
        })
    }

    fn tail_seq(&self, session_id: &SessionId) -> Option<EventSeq> {
        let next_seq = self.next_seq.lock().expect("next_seq lock poisoned");
        let allocator = next_seq.get(session_id).copied()?;
        if allocator == EventSeq::ZERO {
            // No record has been appended, but a gap may have been
            // recorded — check the records map.
            let records = self.records.lock().expect("records lock poisoned");
            records.get(session_id)?;
            Some(EventSeq::ZERO)
        } else {
            // allocator is "next free seq", so tail is allocator - 1.
            Some(EventSeq(allocator.get().saturating_sub(1)))
        }
    }
}
