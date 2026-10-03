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
//! plus one seq assignment under the same lock). m1-01 scope is API
//! plus invariants; lock-free redesign is m1-02. (REC-C2.3 retired the
//! historical `EventBus` comparator; this backend now stands alone.)

use crate::backend::{ExecutionLogBackend, NewExecutionRecord};
use crate::cursor::{ConsumerCursor, LogConsumerId, LogPage, ReadResult};
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

impl RecordEntry {
    /// The earliest `from_seq` at which `read_from_seq` must examine this
    /// entry — the value its reader-side skip test compares against.
    ///
    /// It is `r.seq` for a record (skipped while `r.seq < from_seq`) and
    /// `g.last_missing` for a gap (skipped while `g.last_missing < from_seq`),
    /// i.e. exactly the condition the reader already used. It exists as a
    /// named function so the seek in `read_from_seq` and the scan it replaces
    /// cannot drift apart: they read the same key.
    fn reach(&self) -> EventSeq {
        match self {
            RecordEntry::Record(r) => r.seq,
            RecordEntry::Gap(g) => g.last_missing,
        }
    }

    /// The earliest seq this entry occupies. A record occupies exactly one;
    /// a gap occupies its whole declared range.
    fn span_first(&self) -> EventSeq {
        match self {
            RecordEntry::Record(r) => r.seq,
            RecordEntry::Gap(g) => g.first_missing,
        }
    }

    /// The latest seq this entry occupies.
    fn span_last(&self) -> EventSeq {
        self.reach()
    }
}

/// The per-session entry list, plus the one fact `read_from_seq` needs in
/// order to seek instead of scan: whether the list is still in non-decreasing
/// `RecordEntry::reach` order.
///
/// **This is not a re-ordering, and it does not impose one.** The list stays
/// append-only and authoritative in append order; nothing here sorts, and the
/// reader still walks the list in that order. The flag only records whether
/// the seek's precondition happens to hold, and it is *derived* at every push
/// rather than assumed — the reason is that the precondition is false on a
/// reachable state, see `reach_disordered`'s doc comment.
#[derive(Debug, Default, Clone)]
struct SessionEntries {
    entries: Vec<RecordEntry>,
    /// Sticky: set the first time an entry lands whose `reach` is below the
    /// previous entry's, and never cleared. The list is append-only and
    /// nothing ever reorders or removes from it, so once it is out of order
    /// no later append can repair it — a flag that could flip back would be
    /// a flag a reader could be misled by.
    ///
    /// It IS reachable. `record_gap` only rejects a gap starting *beyond* the
    /// allocator; a gap whose whole range lies *below* it is accepted and
    /// appended after higher seqs, so `reach` drops at that point. That state
    /// is also self-contradictory evidence (a gap declaring seqs that are
    /// present in the log), which strict replay later rejects, so the log
    /// cannot be reopened — but it can exist in memory, and `read_from_seq`
    /// must answer it correctly rather than binary-search a list that is not
    /// sorted. When this is set, the reader scans, exactly as it always did.
    reach_disordered: bool,
}

impl std::ops::Deref for SessionEntries {
    type Target = [RecordEntry];

    fn deref(&self) -> &[RecordEntry] {
        &self.entries
    }
}

impl SessionEntries {
    /// The only way an entry enters the list, so the ordering fact is
    /// maintained in exactly one place.
    fn push(&mut self, entry: RecordEntry) {
        if let Some(prev) = self.entries.last() {
            if entry.reach() < prev.reach() {
                self.reach_disordered = true;
            }
        }
        self.entries.push(entry);
    }

    /// The first index whose entry can reach `from_seq`, or `len()` when none
    /// can. Only meaningful while `!reach_disordered`.
    fn first_reachable(&self, from_seq: EventSeq) -> usize {
        debug_assert!(
            !self.reach_disordered,
            "seeked a list already known to be out of reach order"
        );
        self.entries.partition_point(|e| e.reach() < from_seq)
    }

    /// The lowest seq any entry occupies, or `ZERO` for an empty list.
    ///
    /// This is `span_first`, not `reach()`: a `Record` occupies the single seq
    /// `r.seq` and a `Gap` occupies its whole `[first_missing, last_missing]`,
    /// so the oldest thing present can be the START of a gap that was recorded
    /// after later records. That is also why the `first_reachable` seek does
    /// not apply here — it partitions on `reach()`, and the minimum of
    /// `span_first` over a reach-ordered list is not the first entry's.
    ///
    /// `read_after` calls this only when the caller's cursor is behind the
    /// tail, because it is O(N) in time; see the call site.
    fn oldest_seq(&self) -> EventSeq {
        self.entries
            .iter()
            .map(|e| e.span_first())
            .min()
            .unwrap_or(EventSeq::ZERO)
    }

    /// Whether any entry already in the list occupies any seq in
    /// `[first, last]`. Used by `record_gap` to refuse a gap that would
    /// contradict evidence the session already holds.
    ///
    /// A `Record` occupies the single seq `r.seq`; a `Gap` occupies its whole
    /// `[first_missing, last_missing]`. Two closed ranges intersect when
    /// `a.first <= b.last && b.first <= a.last`, so a gap that touches an
    /// existing entry at either end counts as an overlap — a gap and a record
    /// sharing one seq is exactly as contradictory as sharing many.
    ///
    /// Cost is `O(log n + k)` while the list is in reach order: the seek
    /// lands on the first entry that can reach `first`, and the scan stops at
    /// the first entry that starts after `last`. `k` is the number of entries
    /// the proposed gap would touch, which is **zero** for a legitimate gap —
    /// so the common case is a binary search and nothing else. A disordered
    /// list falls back to a full scan, because there is nothing to seek into.
    fn intersects(&self, first: EventSeq, last: EventSeq) -> bool {
        let start = if self.reach_disordered {
            0
        } else {
            self.first_reachable(first)
        };
        self.entries[start..]
            .iter()
            .take_while(|e| e.span_first() <= last)
            .any(|e| e.span_last() >= first)
    }
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
    records: Mutex<HashMap<SessionId, SessionEntries>>,
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
            kind: ExecutionKind::Raw,
            monotonic_ns,
            payload,
            invocation_id: None,
            parent_invocation_id: None,
            symbol_id: None,
            captured_at_unix_ns: None,
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
            for entry in entries.iter() {
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
            kind: record.kind,
            payload: record.payload,
            invocation_id: record.invocation_id,
            parent_invocation_id: record.parent_invocation_id,
            symbol_id: record.symbol_id,
            captured_at_unix_ns: record.captured_at_unix_ns,
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
        // A gap declares evidence that WAS LOST in `[first_missing,
        // last_missing]`. If it starts beyond the next seq to be allocated,
        // the range in between was never handed out, so it cannot have been
        // lost — accepting it would leave an undeclared hole between the last
        // record and the gap, and replay treats such a hole as corruption, so
        // the whole log becomes unreadable on the next `open`.
        //
        // The overflow path in `SegmentedExecutionLog::append` never trips
        // this: it calls `allocate_seq_for_gap` first, so its gaps always
        // start exactly at the allocator. This guard makes the public API
        // hold the same discipline instead of trusting every caller.
        if gap.first_missing > *allocator {
            return Err(LogError::InvalidGap {
                reason: format!(
                    "first_missing ({}) is beyond the next seq to allocate ({}): \
                     the range in between was never written, so it cannot be a gap",
                    gap.first_missing, allocator.0
                ),
            });
        }
        if gap.last_missing >= *allocator {
            *allocator = gap.last_missing.next();
        }

        // A gap declares evidence that WAS LOST. If the range it declares lost
        // is already occupied — by a record, or by an earlier gap — the log is
        // now asserting two contradictory things about the same seq, and that
        // state is not recoverable: `build_replay_plan` walks the entries in
        // order and requires each to start where the previous ended, so a
        // revisiting entry breaks the chaining and the segment can never be
        // reopened. A log you can write to and then cannot open is worse than
        // one that refuses the write, so the refusal happens here, at the
        // boundary, while the caller still gets a useful error.
        //
        // This is checked against the real entries, NOT against the allocator.
        // "Below the allocator" is not a legitimate category: every production
        // path keeps the entries tiling the seq space (`append` takes the next
        // seq, the overflow path reserves its own with `allocate_seq_for_gap`),
        // so there is no real hole underneath — a gap down there can only step
        // on a record or on another gap, and both make the segment unreadable.
        // Asking the entries is therefore both the correct question and the one
        // that stays right if tiling is ever violated.
        {
            let session = records.get(&session_id);
            let conflicts =
                session.is_some_and(|s| s.intersects(gap.first_missing, gap.last_missing));
            if conflicts {
                return Err(LogError::InvalidGap {
                    reason: format!(
                        "range [{}, {}] overlaps evidence the session already holds: \
                         a gap declaring seqs that are present is contradictory, and \
                         the resulting segment cannot be reopened",
                        gap.first_missing.0, gap.last_missing.0
                    ),
                });
            }
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

        // C4 of SCALE_BUDGETS §7, and the third arm of this same defect
        // family. This used to take `records.get(&session_id).cloned()` — a
        // full clone of the session `Vec` — and then walk the owned copy,
        // paying O(N) of the session in transient allocations before the cursor
        // filter got a say. The comment above it said so in as many words
        // ("this arm keeps its own full-clone cost, which is a separate open
        // item"), and it had production callers the whole time: `analytics`,
        // `call_graph`, the segmented backend, and through it
        // `chronos_capture::session_feed`.
        //
        // What that costs is not the first read, it is the IDLE one. A
        // consumer that has caught up asks "what is new", gets nothing, and
        // used to allocate the entire log to learn it: measured at 48.8 MB per
        // call on a 200k-event session, 9,939x the cost of the same call on a
        // 20-event session — a linear clone signature, and a poll loop paying
        // it every tick.
        //
        // So: read under the lock and clone only what is RETURNED, the same
        // shape `read_from_seq` already uses. What is returned does not change;
        // what it costs to return it does.
        //
        // The two early exits below have to happen BEFORE the cursor is
        // persisted, which is what returning from inside the old `match` on the
        // owned snapshot achieved. `Early` carries them out of the lock scope
        // so the ordering is preserved rather than implied.
        // The stale-cursor check below needs `oldest_seq`, the minimum over
        // every entry — an O(N) walk of the session, on EVERY call. It cannot
        // fire unless the cursor is behind the tail, and `oldest_seq` is always
        // at or below the tail, so when the cursor has already caught up the
        // answer is known without walking anything.
        //
        // The invariant: a record's `seq` is allocated below the allocator, and
        // `record_gap` refuses a gap that starts beyond it, so every entry's
        // `span_first` is below the allocator — and `tail_seq` IS the allocator
        // minus one. Hence `oldest_seq <= tail_seq`, and a cursor at or past
        // the tail cannot be stale.
        //
        // This is the SECOND cost C1/C2 warned about: the clone is layer 1 and
        // is visible to an allocation counter, while this scan is layer 2 and
        // is invisible to one. Removing the clone without removing this leaves
        // a call that allocates nothing and still walks a million entries —
        // measured at 51 ms per idle poll over a 1M-event session, linear in N
        // exactly as the clone was.
        let may_be_stale = !fresh
            && stored.is_some()
            && effective_cursor.last_seq < self.tail_seq(&session_id).unwrap_or(EventSeq::ZERO);

        enum Early {
            /// The session is not in the map at all.
            Missing,
            /// The caller's cursor is older than the oldest retained seq.
            Stale(LogError),
        }

        let mut out_records: Vec<ExecutionRecord> = Vec::new();
        let mut out_gaps: Vec<Gap> = Vec::new();
        let mut max_seq = effective_cursor.last_seq;

        let early = {
            let records = self.records.lock().expect("records lock poisoned");
            match records.get(&session_id) {
                None => Some(Early::Missing),
                Some(entries) => {
                    // The oldest seq currently in the log. Only computed when
                    // the cursor could actually be behind it — see `may_be_stale`
                    // above. When it is computed it still walks everything, by
                    // reference: it has to see every entry because a gap can be
                    // the oldest thing present, and a gap's `first_missing` is
                    // not ordered against a record's `seq` the way `reach()` is,
                    // so the seek `read_from_seq` uses does not apply to it.
                    let stale = may_be_stale && effective_cursor.last_seq < entries.oldest_seq();

                    if stale {
                        Some(Early::Stale(LogError::CursorStale {
                            consumer: consumer.clone(),
                            expected: effective_cursor.last_seq,
                            current: entries.oldest_seq(),
                        }))
                    } else {
                        //   - fresh cursor: include every record (even seq#0).
                        //   - cursor with last_seq = n: include records with seq > n.
                        //
                        // Both branches of the filter below reduce to the SAME
                        // predicate — a `Record` is included when
                        // `r.seq > last_seq`, a `Gap` when
                        // `g.last_missing > last_seq`, and `reach()` is exactly
                        // `r.seq` for a record and `last_missing` for a gap. So
                        // the two tests are one test, `reach() >= last_seq + 1`,
                        // and the first entry that can satisfy it is
                        // `first_reachable(last_seq + 1)`. Seeking there skips
                        // the whole prefix the loop would have rejected one
                        // entry at a time, which is the difference between
                        // O(session) and O(new) for a caught-up consumer.
                        //
                        // Gated on `!reach_disordered`, exactly as
                        // `read_from_seq` gates its own seek: a list whose
                        // `reach` dropped is not partitioned by `reach`, and a
                        // binary search into it would return an arbitrary
                        // match. A fresh cursor takes everything, so it seeks
                        // nothing.
                        let window: &[RecordEntry] = if fresh || entries.reach_disordered {
                            &entries.entries
                        } else {
                            &entries.entries[entries.first_reachable(EventSeq::new(
                                effective_cursor.last_seq.0.saturating_add(1),
                            ))..]
                        };

                        for entry in window {
                            match entry {
                                RecordEntry::Record(r) => {
                                    let include = fresh || r.seq > effective_cursor.last_seq;
                                    if include {
                                        if r.seq > max_seq {
                                            max_seq = r.seq;
                                        }
                                        out_records.push(r.clone());
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
                                        out_gaps.push(g.clone());
                                    }
                                }
                            }
                        }
                        None
                    }
                }
            }
        };

        match early {
            Some(Early::Missing) => {
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
            Some(Early::Stale(err)) => return Err(err),
            None => {}
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

    fn read_from_seq(
        &self,
        session_id: &SessionId,
        from_seq: EventSeq,
        limit: usize,
    ) -> Result<LogPage, LogError> {
        // Stateless: no cursor is read or written. Two callers with different
        // `from_seq` cannot interfere.
        //
        // C1/C2 of docs/roadmap/SCALE_BUDGETS.md §7. This used to clone the
        // WHOLE session Vec before looking at `limit`, then walk it from index
        // 0 discarding `seq < from_seq` — so one call cost O(N) of the session
        // and ~1x the session in transient allocations, and `limit=1` cost the
        // same as `limit=100`. Two separate costs had to go:
        //
        //   1. the clone, removed by reading under the lock and cloning only
        //      what is returned (at most `limit` records);
        //   2. the O(position) walk, removed by seeking to the first entry that
        //      can reach `from_seq`.
        //
        // (2) is only legal while the list is in non-decreasing `reach` order,
        // so the seek is gated on `SessionEntries::reach_disordered` — a fact
        // derived at every push, not assumed. See that field's doc comment for
        // the reachable state that breaks the order.
        let records = self.records.lock().expect("records lock poisoned");
        let Some(session) = records.get(session_id) else {
            return Ok(LogPage::empty_at(from_seq));
        };
        let window: &[RecordEntry] = if session.reach_disordered {
            // Out of order: walk it, exactly as this always has.
            &session.entries
        } else {
            // Every entry before the first reachable one is skipped by the loop
            // below, so starting there is the same walk minus that prefix —
            // provably the same page, without paying for the prefix.
            &session.entries[session.first_reachable(from_seq)..]
        };

        let mut out_records: Vec<ExecutionRecord> = Vec::new();
        let mut out_gaps: Vec<Gap> = Vec::new();
        let mut examined_any = false;
        let mut max_examined = from_seq;

        for entry in window {
            match entry {
                RecordEntry::Record(r) => {
                    if r.seq < from_seq {
                        continue;
                    }
                    if out_records.len() >= limit {
                        break;
                    }
                    examined_any = true;
                    if r.seq > max_examined {
                        max_examined = r.seq;
                    }
                    out_records.push(r.clone());
                }
                RecordEntry::Gap(g) => {
                    // A gap is examined when its range reaches the position.
                    if g.last_missing < from_seq {
                        continue;
                    }
                    if out_records.len() >= limit {
                        break;
                    }
                    examined_any = true;
                    out_gaps.push(g.clone());
                    // Advance over the gap: a reader must not stall before
                    // lost evidence.
                    if g.last_missing > max_examined {
                        max_examined = g.last_missing;
                    }
                }
            }
        }
        drop(records);

        Ok(LogPage {
            records: out_records,
            gaps: out_gaps,
            position_after: if examined_any {
                EventSeq::new(max_examined.0 + 1)
            } else {
                from_seq
            },
            exhausted: !examined_any,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gap::GapReason;

    #[test]
    fn fresh_log_returns_empty_session() {
        let log = InMemoryExecutionLog::new();
        let result = log
            .read_after(SessionId::new("s1"), LogConsumerId::new("agent-a"), None)
            .unwrap();
        match result {
            ReadResult::Ok {
                records,
                gaps,
                next_cursor,
            } => {
                assert!(records.is_empty());
                assert!(gaps.is_empty());
                assert_eq!(next_cursor.last_seq, EventSeq::ZERO);
            }
            other => panic!("expected Ok, got {:?}", other),
        }
    }

    #[test]
    fn append_assigns_consecutive_seqs() {
        let log = InMemoryExecutionLog::new();
        let s1 = SessionId::new("s1");
        let s0 = log.append_raw(s1.clone(), 100, "raw").unwrap();
        let s1_seq = log.append_raw(s1.clone(), 200, "raw").unwrap();
        let s2_seq = log.append_raw(s1.clone(), 300, "raw").unwrap();
        assert_eq!(s0, EventSeq::new(0));
        assert_eq!(s1_seq, EventSeq::new(1));
        assert_eq!(s2_seq, EventSeq::new(2));
        assert_eq!(log.entry_count(&s1), 3);
    }

    #[test]
    fn sessions_have_independent_seqs() {
        let log = InMemoryExecutionLog::new();
        let a = SessionId::new("a");
        let b = SessionId::new("b");
        assert_eq!(log.append_raw(a.clone(), 0, "x").unwrap(), EventSeq::new(0));
        assert_eq!(log.append_raw(b.clone(), 0, "x").unwrap(), EventSeq::new(0));
        assert_eq!(log.append_raw(a.clone(), 0, "x").unwrap(), EventSeq::new(1));
        assert_eq!(log.append_raw(b.clone(), 0, "x").unwrap(), EventSeq::new(1));
    }

    /// A gap that starts beyond the next seq to be allocated would leave an
    /// undeclared hole between the last record and the gap. Replay treats
    /// such a hole as corruption, so accepting it makes the whole log
    /// unreadable on the next `open`. Evidence cannot be lost for seqs that
    /// were never handed out.
    #[test]
    fn gap_beyond_the_allocator_is_rejected() {
        let log = InMemoryExecutionLog::new();
        let s = SessionId::new("s");
        log.append_raw(s.clone(), 0, "x").unwrap();
        // Seq 0 is written, so the allocator is at 1. A gap starting at 100
        // leaves 1..=99 neither written nor declared.
        let err = log
            .record_gap(
                s.clone(),
                Gap::new(
                    EventSeq::new(100),
                    EventSeq::new(200),
                    GapReason::KernelRingOverflow,
                    "hole",
                ),
            )
            .expect_err("a gap beyond the allocator must be rejected");
        assert!(matches!(err, LogError::InvalidGap { .. }));

        // The rejected gap must not have moved the allocator, so the very
        // next append still continues contiguously.
        let next = log.append_raw(s.clone(), 0, "x").unwrap();
        assert_eq!(
            next,
            EventSeq::new(1),
            "a rejected gap must leave the allocator untouched"
        );
    }

    /// The contiguous case is what the production overflow path produces and
    /// must keep working.
    #[test]
    fn contiguous_gap_at_the_allocator_is_accepted() {
        let log = InMemoryExecutionLog::new();
        let s = SessionId::new("s");
        log.append_raw(s.clone(), 0, "x").unwrap();
        log.record_gap(
            s.clone(),
            Gap::new(
                EventSeq::new(1),
                EventSeq::new(3),
                GapReason::KernelRingOverflow,
                "contiguous",
            ),
        )
        .unwrap();
        let next = log.append_raw(s.clone(), 0, "x").unwrap();
        assert!(next > EventSeq::new(3));
    }

    #[test]
    fn record_gap_bumps_seq_allocator() {
        let log = InMemoryExecutionLog::new();
        let s = SessionId::new("s");
        // First a normal append.
        log.append_raw(s.clone(), 0, "x").unwrap();
        // Now record a gap spanning seqs 1..=5.
        log.record_gap(
            s.clone(),
            Gap::new(
                EventSeq::new(1),
                EventSeq::new(5),
                GapReason::KernelRingOverflow,
                "test",
            ),
        )
        .unwrap();
        // Next append must be > gap.last_missing.
        let next = log.append_raw(s.clone(), 0, "x").unwrap();
        assert!(
            next > EventSeq::new(5),
            "expected seq > 5 after gap, got {}",
            next
        );
    }

    #[test]
    fn invalid_gap_rejected() {
        let log = InMemoryExecutionLog::new();
        let err = log
            .record_gap(
                SessionId::new("s"),
                Gap::new(
                    EventSeq::new(5),
                    EventSeq::new(1),
                    GapReason::KernelRingOverflow,
                    "test",
                ),
            )
            .unwrap_err();
        assert!(matches!(err, LogError::InvalidGap { .. }));
    }

    // ----------------------------------------------------------------
    // Identity-index tests (m2-02)
    // ----------------------------------------------------------------

    fn make_record_with_ids(
        session_id: SessionId,
        seq: u64,
        monotonic_ns: u64,
        invocation: Option<chronos_domain::InvocationId>,
        parent: Option<chronos_domain::InvocationId>,
        symbol: Option<chronos_domain::SymbolId>,
    ) -> ExecutionRecord {
        ExecutionRecord {
            session_id,
            seq: EventSeq::new(seq),
            monotonic_ns,
            kind: ExecutionKind::Raw,
            payload: ExecutionPayload::new(Vec::new(), "v2"),
            invocation_id: invocation,
            parent_invocation_id: parent,
            symbol_id: symbol,
            captured_at_unix_ns: None,
        }
    }

    #[test]
    fn identity_get_by_invocation_returns_only_matching_records() {
        let log = InMemoryExecutionLog::new();
        let s = SessionId::new("idem");
        let inv = chronos_domain::InvocationId::now();
        let other = chronos_domain::InvocationId::now();
        let sym = chronos_domain::SymbolId::new("foo", None, chronos_domain::Language::Rust);

        log.append(NewExecutionRecord {
            kind: ExecutionKind::Raw,

            session_id: s.clone(),
            monotonic_ns: 10,
            payload: ExecutionPayload::new(Vec::new(), "a"),
            invocation_id: Some(inv),
            parent_invocation_id: None,
            symbol_id: Some(sym),
            captured_at_unix_ns: None,
        })
        .unwrap();
        log.append(NewExecutionRecord {
            kind: ExecutionKind::Raw,

            session_id: s.clone(),
            monotonic_ns: 20,
            payload: ExecutionPayload::new(Vec::new(), "b"),
            invocation_id: Some(other),
            parent_invocation_id: Some(inv),
            symbol_id: Some(sym),
            captured_at_unix_ns: None,
        })
        .unwrap();
        // v1 record — must be invisible to the identity index.
        log.append_raw(s.clone(), 30, "v1").unwrap();

        let by_inv = log.get_by_invocation(&s, inv);
        assert_eq!(by_inv.len(), 1);
        assert_eq!(by_inv[0].monotonic_ns, 10);
    }

    #[test]
    fn identity_get_by_invocation_unknown_returns_empty() {
        let log = InMemoryExecutionLog::new();
        let s = SessionId::new("idem-unknown");
        // No appends. Lookup for any invocation returns empty.
        assert!(log
            .get_by_invocation(&s, chronos_domain::InvocationId::now())
            .is_empty());
    }

    #[test]
    fn identity_children_of_returns_descendants_only() {
        let log = InMemoryExecutionLog::new();
        let s = SessionId::new("tree");
        let root = chronos_domain::InvocationId::now();
        let child1 = chronos_domain::InvocationId::now();
        let child2 = chronos_domain::InvocationId::now();
        let unrelated = chronos_domain::InvocationId::now();
        let sym = chronos_domain::SymbolId::new("f", None, chronos_domain::Language::Rust);

        log.append(NewExecutionRecord {
            kind: ExecutionKind::Raw,

            session_id: s.clone(),
            monotonic_ns: 1,
            payload: ExecutionPayload::new(Vec::new(), "root"),
            invocation_id: Some(root),
            parent_invocation_id: None,
            symbol_id: Some(sym),
            captured_at_unix_ns: None,
        })
        .unwrap();
        log.append(NewExecutionRecord {
            kind: ExecutionKind::Raw,

            session_id: s.clone(),
            monotonic_ns: 2,
            payload: ExecutionPayload::new(Vec::new(), "c1"),
            invocation_id: Some(child1),
            parent_invocation_id: Some(root),
            symbol_id: Some(sym),
            captured_at_unix_ns: None,
        })
        .unwrap();
        log.append(NewExecutionRecord {
            kind: ExecutionKind::Raw,

            session_id: s.clone(),
            monotonic_ns: 3,
            payload: ExecutionPayload::new(Vec::new(), "c2"),
            invocation_id: Some(child2),
            parent_invocation_id: Some(root),
            symbol_id: Some(sym),
            captured_at_unix_ns: None,
        })
        .unwrap();
        log.append(NewExecutionRecord {
            kind: ExecutionKind::Raw,

            session_id: s.clone(),
            monotonic_ns: 4,
            payload: ExecutionPayload::new(Vec::new(), "unrelated"),
            invocation_id: Some(unrelated),
            parent_invocation_id: None,
            symbol_id: Some(sym),
            captured_at_unix_ns: None,
        })
        .unwrap();

        let children = log.children_of(&s, root);
        assert_eq!(children.len(), 2);
        // BTreeSet gives sorted seq order.
        assert_eq!(children[0].monotonic_ns, 2);
        assert_eq!(children[1].monotonic_ns, 3);
        // No record with parent_invocation_id == None appears.
        assert!(log.children_of(&s, unrelated).is_empty());
    }

    #[test]
    fn identity_in_range_by_symbol_filters_by_time() {
        let log = InMemoryExecutionLog::new();
        let s = SessionId::new("sym-range");
        let sym = chronos_domain::SymbolId::new("foo", None, chronos_domain::Language::Rust);
        let other = chronos_domain::SymbolId::new("bar", None, chronos_domain::Language::Rust);
        let inv = chronos_domain::InvocationId::now();
        for ns in [10u64, 20, 30, 40, 50] {
            log.append(NewExecutionRecord {
                kind: ExecutionKind::Raw,

                session_id: s.clone(),
                monotonic_ns: ns,
                payload: ExecutionPayload::new(Vec::new(), "f"),
                invocation_id: Some(inv),
                parent_invocation_id: None,
                symbol_id: if ns == 30 { Some(other) } else { Some(sym) },
                captured_at_unix_ns: None,
            })
            .unwrap();
        }
        // Range [20, 40) over symbol foo: 20 (seq=1) qualifies, 40 is
        // excluded because the upper bound is half-open. seq 2 has
        // symbol=bar so it isn't in this index at all.
        let in_range = log.in_range_by_symbol(&s, sym, 20, 40);
        let mut ns_values: Vec<u64> = in_range.iter().map(|r| r.monotonic_ns).collect();
        ns_values.sort();
        assert_eq!(ns_values, vec![20]);
        // Empty range returns nothing.
        assert!(log.in_range_by_symbol(&s, sym, 100, 200).is_empty());
        // Unknown symbol returns nothing.
        let unknown = chronos_domain::SymbolId::new("nope", None, chronos_domain::Language::Rust);
        assert!(log.in_range_by_symbol(&s, unknown, 0, 1000).is_empty());
    }

    #[test]
    fn identity_replay_rebuilds_indexes() {
        // Build a record list, replay it into a fresh backend, and
        // verify the identity reads return identical results.
        let s = SessionId::new("replay");
        let inv1 = chronos_domain::InvocationId::now();
        let inv2 = chronos_domain::InvocationId::now();
        let sym = chronos_domain::SymbolId::new("f", None, chronos_domain::Language::Rust);

        let live_log = InMemoryExecutionLog::new();
        let records: Vec<ExecutionRecord> = vec![
            make_record_with_ids(s.clone(), 0, 1, Some(inv1), None, Some(sym)),
            make_record_with_ids(s.clone(), 1, 2, Some(inv2), Some(inv1), Some(sym)),
            make_record_with_ids(s.clone(), 2, 3, None, None, None),
        ];
        for r in &records {
            live_log.replay_record(r).unwrap();
        }

        let by_inv1 = live_log.get_by_invocation(&s, inv1);
        assert_eq!(by_inv1.len(), 1);
        assert_eq!(by_inv1[0].seq, EventSeq::new(0));
        let children = live_log.children_of(&s, inv1);
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].seq, EventSeq::new(1));
        // v1 record (seq=2) carries None on every identity field, so
        // a lookup for an unknown invocation returns empty even
        // though the session has 3 records.
        let unknown = chronos_domain::InvocationId::now();
        assert!(live_log.get_by_invocation(&s, unknown).is_empty());
    }

    #[test]
    fn identity_prune_drops_evicted_seqs() {
        let log = InMemoryExecutionLog::new();
        let s = SessionId::new("prune");
        let inv = chronos_domain::InvocationId::now();
        let sym = chronos_domain::SymbolId::new("f", None, chronos_domain::Language::Rust);
        for ns in 0..5u64 {
            log.append(NewExecutionRecord {
                kind: ExecutionKind::Raw,

                session_id: s.clone(),
                monotonic_ns: ns,
                payload: ExecutionPayload::new(Vec::new(), "x"),
                invocation_id: Some(inv),
                parent_invocation_id: None,
                symbol_id: Some(sym),
                captured_at_unix_ns: None,
            })
            .unwrap();
        }
        // Pre-prune: 5 records reachable.
        assert_eq!(log.get_by_invocation(&s, inv).len(), 5);
        // Prune everything ≤ seq=2 → 3 records (seqs 3,4) survive.
        log.prune_secondary_indexes_up_to(EventSeq::new(2));
        let after = log.get_by_invocation(&s, inv);
        assert_eq!(after.len(), 2);
        let mut seqs: Vec<u64> = after.iter().map(|r| r.seq.0).collect();
        seqs.sort();
        assert_eq!(seqs, vec![3, 4]);
    }
}

/// REC-C1.3 — stateless page reader tests.
///
/// These pin the contract the agent-visible cursor depends on:
/// `from_seq == 0` includes seq#0; a page advances the position exactly; two
/// readers with different positions are independent; and a position advances
/// OVER an observed gap instead of stalling before it.
#[cfg(test)]
mod rec_c1_3_read_from_seq {
    use super::*;
    use crate::gap::GapReason;

    fn seeded(n: usize) -> (InMemoryExecutionLog, SessionId) {
        let log = InMemoryExecutionLog::new();
        let s = SessionId::new("rec-c1-3");
        for i in 0..n {
            log.append_raw(s.clone(), i as u64, "ev").unwrap();
        }
        (log, s)
    }

    #[test]
    fn from_zero_includes_seq_zero() {
        let (log, s) = seeded(3);
        let page = log.read_from_seq(&s, EventSeq::ZERO, 10).unwrap();
        let seqs: Vec<u64> = page.records.iter().map(|r| r.seq.0).collect();
        assert_eq!(seqs, vec![0, 1, 2], "seq#0 must be delivered, not skipped");
        assert_eq!(page.position_after, EventSeq::new(3));
        assert!(!page.exhausted);
    }

    #[test]
    fn page_boundary_is_exact_and_resume_starts_there() {
        let (log, s) = seeded(25);
        let p1 = log.read_from_seq(&s, EventSeq::ZERO, 10).unwrap();
        assert_eq!(p1.records.first().unwrap().seq, EventSeq::new(0));
        assert_eq!(p1.records.last().unwrap().seq, EventSeq::new(9));
        assert_eq!(p1.position_after, EventSeq::new(10));

        let p2 = log.read_from_seq(&s, p1.position_after, 10).unwrap();
        assert_eq!(p2.records.first().unwrap().seq, EventSeq::new(10));
        assert_eq!(p2.records.last().unwrap().seq, EventSeq::new(19));
        assert_eq!(p2.position_after, EventSeq::new(20));

        // Exact resume at 10 reproduces page 2, with no duplicate or skip.
        let resumed = log.read_from_seq(&s, EventSeq::new(10), 10).unwrap();
        assert_eq!(
            resumed.records.iter().map(|r| r.seq.0).collect::<Vec<_>>(),
            p2.records.iter().map(|r| r.seq.0).collect::<Vec<_>>()
        );
    }

    #[test]
    fn two_readers_with_different_positions_are_independent() {
        let (log, s) = seeded(100);
        let a = log.read_from_seq(&s, EventSeq::new(10), 5).unwrap();
        let b = log.read_from_seq(&s, EventSeq::new(40), 5).unwrap();
        assert_eq!(a.records.first().unwrap().seq, EventSeq::new(10));
        assert_eq!(b.records.first().unwrap().seq, EventSeq::new(40));
        // Reading A again is unaffected by B having read.
        let a2 = log.read_from_seq(&s, EventSeq::new(10), 5).unwrap();
        assert_eq!(a.records, a2.records);
    }

    #[test]
    fn caught_up_reader_does_not_move_its_position() {
        let (log, s) = seeded(4);
        let page = log.read_from_seq(&s, EventSeq::new(9), 10).unwrap();
        assert!(page.records.is_empty());
        assert!(page.exhausted);
        assert_eq!(page.position_after, EventSeq::new(9), "no phantom progress");
    }

    #[test]
    fn position_advances_over_an_observed_gap() {
        // seq space: 0, then a gap 1..=3, then record 4.
        let (log, s) = gappy_log();
        let page = log.read_from_seq(&s, EventSeq::ZERO, 10).unwrap();
        assert_eq!(
            page.records.iter().map(|r| r.seq.0).collect::<Vec<_>>(),
            vec![0, 4],
            "evidence after the gap stays reachable in the same page"
        );
        assert_eq!(page.gaps.len(), 1);
        assert_eq!(page.gaps[0].last_missing, EventSeq::new(3));
        // position_after = (highest seq examined) + 1, gaps included.
        assert_eq!(page.position_after, EventSeq::new(5));
    }

    #[test]
    fn a_record_limited_page_still_reaches_the_gap_on_the_next_read() {
        // `limit` limits RECORDS. A page can therefore stop before examining a
        // gap; the next read starts exactly at `position_after`, meets the gap
        // and clears it. Progress is monotonic across consecutive reads.
        let (log, s) = gappy_log();
        let p1 = log.read_from_seq(&s, EventSeq::ZERO, 1).unwrap();
        assert_eq!(
            p1.records.iter().map(|r| r.seq.0).collect::<Vec<_>>(),
            vec![0]
        );
        assert_eq!(
            p1.gaps.len(),
            0,
            "limit reached before the gap was examined"
        );
        assert!(p1.position_after > EventSeq::ZERO, "position must move");

        let p2 = log.read_from_seq(&s, p1.position_after, 1).unwrap();
        assert!(
            p2.position_after > p1.position_after,
            "no stall between pages"
        );
        assert_eq!(p2.gaps.len(), 1, "the gap is examined on the next page");
        assert_eq!(
            p2.records.iter().map(|r| r.seq.0).collect::<Vec<_>>(),
            vec![4],
            "and the record after the gap is delivered"
        );
        assert_eq!(p2.position_after, EventSeq::new(5));
    }

    fn gappy_log() -> (InMemoryExecutionLog, SessionId) {
        let log = InMemoryExecutionLog::new();
        let s = SessionId::new("gappy");
        log.append_raw(s.clone(), 0, "before").unwrap();
        log.record_gap(
            s.clone(),
            Gap::new(
                EventSeq::new(1),
                EventSeq::new(3),
                GapReason::AdapterBufferOverflow,
                "test",
            ),
        )
        .unwrap();
        log.append_raw(s.clone(), 4, "after").unwrap();
        (log, s)
    }
}

/// C1/C2 of `docs/roadmap/SCALE_BUDGETS.md` §7, and the ordering fact the
/// seek in `read_from_seq` is allowed to rely on.
///
/// `read_from_seq` walks a `Vec<RecordEntry>` in append order and skips every
/// entry whose `reach` is below `from_seq`. Skipping the *prefix* by
/// `partition_point` instead of walking it is only equivalent while the list is
/// in non-decreasing `reach` order, so these tests pin three separate things:
///
///   1. which write paths preserve that order (appends, the overflow gap,
///      strict replay);
///   2. which one does NOT (`record_gap` with a range below the allocator),
///      because that is what `SessionEntries::reach_disordered` exists for;
///   3. that a seek and a full walk return the SAME page either way, which is
///      the property that actually keeps the optimization safe.
#[cfg(test)]
mod c1_c2_read_from_seq {
    use super::*;
    use crate::gap::GapReason;

    /// The pre-optimisation walk, verbatim, kept here as the oracle. Every
    /// equivalence assertion below compares the production path against this,
    /// not against a hand-written expectation of what the new code "should"
    /// return — otherwise the oracle and the bug would be written by the same
    /// assumption.
    fn reference_page(entries: &[RecordEntry], from_seq: EventSeq, limit: usize) -> LogPage {
        let mut out_records: Vec<ExecutionRecord> = Vec::new();
        let mut out_gaps: Vec<Gap> = Vec::new();
        let mut examined_any = false;
        let mut max_examined = from_seq;
        for entry in entries {
            match entry {
                RecordEntry::Record(r) => {
                    if r.seq < from_seq {
                        continue;
                    }
                    if out_records.len() >= limit {
                        break;
                    }
                    examined_any = true;
                    if r.seq > max_examined {
                        max_examined = r.seq;
                    }
                    out_records.push(r.clone());
                }
                RecordEntry::Gap(g) => {
                    if g.last_missing < from_seq {
                        continue;
                    }
                    if out_records.len() >= limit {
                        break;
                    }
                    examined_any = true;
                    out_gaps.push(g.clone());
                    if g.last_missing > max_examined {
                        max_examined = g.last_missing;
                    }
                }
            }
        }
        LogPage {
            records: out_records,
            gaps: out_gaps,
            position_after: if examined_any {
                EventSeq::new(max_examined.0 + 1)
            } else {
                from_seq
            },
            exhausted: !examined_any,
        }
    }

    fn entries_of(log: &InMemoryExecutionLog, s: &SessionId) -> Vec<RecordEntry> {
        let records = log.records.lock().expect("records lock poisoned");
        records
            .get(s)
            .map(|e| e.entries.clone())
            .unwrap_or_default()
    }

    fn is_reach_ordered(log: &InMemoryExecutionLog, s: &SessionId) -> bool {
        let records = log.records.lock().expect("records lock poisoned");
        !records.get(s).expect("session present").reach_disordered
    }

    fn reaches(log: &InMemoryExecutionLog, s: &SessionId) -> Vec<u64> {
        entries_of(log, s).iter().map(|e| e.reach().0).collect()
    }

    fn seeded(n: u64) -> (InMemoryExecutionLog, SessionId) {
        let log = InMemoryExecutionLog::new();
        let s = SessionId::new("c1-c2");
        for i in 0..n {
            log.append_raw(s.clone(), i, "ev").unwrap();
        }
        (log, s)
    }

    /// Build a reach-disordered entry list **without** going through
    /// `record_gap`.
    ///
    /// `record_gap` now refuses any gap that overlaps evidence the session
    /// already holds, so the reach-disordered state is no longer constructible
    /// through the public API: `append` takes the next seq and stays ordered,
    /// `replay_record` `assert!`s before it can push out of order, and
    /// `record_gap` refuses. That is the correct outcome — the state was
    /// evidence that could not be reopened.
    ///
    /// So `SessionEntries::reach_disordered` and the reader's scan fallback are
    /// now defence in depth with no current trigger. They stay, and stay
    /// tested, because the difference between them is a correct page and a
    /// silently wrong one if any future path ever produces the state, and
    /// because "no current trigger" is a statement about today, not a proof
    /// about tomorrow. This seam is how the tests reach it, and it bypasses the
    /// guard on purpose: what is under test here is the *reader*, not the API.
    fn force_entry(log: &InMemoryExecutionLog, s: &SessionId, entry: RecordEntry) {
        log.records
            .lock()
            .expect("records lock poisoned")
            .entry(s.clone())
            .or_default()
            .push(entry);
    }

    /// The overflow path's shape: `SegmentedExecutionLog::append` reserves the
    /// seq with `allocate_seq_for_gap` and records a single-seq gap at it, so
    /// the gap is written at the allocator and the next append continues after
    /// it. Order preserved.
    #[test]
    fn the_overflow_gap_shape_preserves_the_reach_order() {
        let log = InMemoryExecutionLog::new();
        let s = SessionId::new("c1-c2-overflow");
        for _ in 0..5 {
            log.append_raw(s.clone(), 0, "ev").unwrap();
        }
        let reserved = log.allocate_seq_for_gap(&s).unwrap();
        assert_eq!(reserved, EventSeq::new(5));
        log.record_gap(
            s.clone(),
            Gap::new(
                reserved,
                reserved,
                GapReason::AdapterBufferOverflow,
                "overflow",
            ),
        )
        .unwrap();
        log.append_raw(s.clone(), 0, "ev").unwrap();
        assert_eq!(reaches(&log, &s), vec![0, 1, 2, 3, 4, 5, 6]);
        assert!(
            is_reach_ordered(&log, &s),
            "the production overflow path must keep the list seekable"
        );
    }

    /// Strict replay is release-active validation, not an assert: the plan
    /// builder requires each entry to start exactly where the previous one
    /// ended (`replay.rs:251-260`) and each segment to continue the previous
    /// one, so a replayed list is densely tiled and therefore ordered.
    #[test]
    fn a_strictly_replayed_run_preserves_the_reach_order() {
        let log = InMemoryExecutionLog::new();
        let s = SessionId::new("c1-c2-replay");
        for seq in 0..8u64 {
            let record = ExecutionRecord {
                session_id: s.clone(),
                seq: EventSeq::new(seq),
                monotonic_ns: seq,
                kind: ExecutionKind::Raw,
                payload: ExecutionPayload::new(Vec::new(), "v2"),
                invocation_id: None,
                parent_invocation_id: None,
                symbol_id: None,
                captured_at_unix_ns: None,
            };
            log.replay_record(&record).unwrap();
        }
        assert_eq!(reaches(&log, &s), vec![0, 1, 2, 3, 4, 5, 6, 7]);
        assert!(is_reach_ordered(&log, &s));
    }

    /// THE REFUTATION, as a test. `record_gap` rejects a gap that starts
    /// *beyond* the allocator, but nothing rejects one that ends *below* it, so
    /// a `reach` (the gap's `last_missing`) smaller than the preceding records'
    /// seqs lands at the end of the list and the order breaks. This is why
    /// `read_from_seq` cannot seek unconditionally.
    ///
    /// The state is also self-contradictory evidence — the gap declares seqs
    /// 3..=5 lost while records 3, 4 and 5 are present — and strict replay
    /// rejects such a segment, so the log cannot be reopened afterwards. That
    /// is a separate defect, reported, not fixed here.
    #[test]
    fn a_gap_below_the_allocator_breaks_the_reach_order() {
        let log = InMemoryExecutionLog::new();
        let s = SessionId::new("c1-c2-retroactive");
        for _ in 0..10 {
            log.append_raw(s.clone(), 0, "ev").unwrap();
        }
        force_entry(
            &log,
            &s,
            RecordEntry::Gap(Gap::new(
                EventSeq::new(3),
                EventSeq::new(5),
                GapReason::KernelRingOverflow,
                "late",
            )),
        );

        let reaches = reaches(&log, &s);
        assert_eq!(
            reaches,
            vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 5],
            "the gap's reach (5) lands after higher seqs: the list is NOT ordered"
        );
        assert!(
            !is_reach_ordered(&log, &s),
            "the break must be recorded, or read_from_seq would seek an unsorted list"
        );
    }

    /// The flag is sticky. A later, perfectly ordinary append cannot repair a
    /// list that is already out of order, and a flag that could flip back would
    /// be a flag a reader could be misled by.
    #[test]
    fn the_disorder_flag_does_not_clear_itself_on_a_later_append() {
        let log = InMemoryExecutionLog::new();
        let s = SessionId::new("c1-c2-sticky");
        for _ in 0..10 {
            log.append_raw(s.clone(), 0, "ev").unwrap();
        }
        force_entry(
            &log,
            &s,
            RecordEntry::Gap(Gap::new(
                EventSeq::new(3),
                EventSeq::new(5),
                GapReason::KernelRingOverflow,
                "late",
            )),
        );
        log.append_raw(s.clone(), 0, "ev").unwrap();
        assert_eq!(reaches(&log, &s).last(), Some(&10));
        assert!(
            !is_reach_ordered(&log, &s),
            "appending cannot heal an append-only list that is already out of order"
        );
    }

    /// The safety property: on an ORDERED list, seeking must return exactly what
    /// the full walk returned, for every position and limit. Checked
    /// differentially against `reference_page` rather than against a hand-written
    /// expectation, so a wrong seek predicate cannot pass.
    #[test]
    fn seeking_matches_the_full_walk_on_an_ordered_list() {
        let log = InMemoryExecutionLog::new();
        let s = SessionId::new("c1-c2-equiv-ordered");
        for _ in 0..6 {
            log.append_raw(s.clone(), 0, "before").unwrap();
        }
        log.record_gap(
            s.clone(),
            Gap::new(
                EventSeq::new(6),
                EventSeq::new(9),
                GapReason::AdapterBufferOverflow,
                "gap",
            ),
        )
        .unwrap();
        for _ in 0..5 {
            log.append_raw(s.clone(), 0, "after").unwrap();
        }
        assert!(is_reach_ordered(&log, &s));

        let entries = entries_of(&log, &s);
        let mut positions = 0..=entries.len() as u64 + 2;
        let mut compared = 0usize;
        for from in positions.by_ref() {
            for limit in 0..=entries.len() + 2 {
                let got = log.read_from_seq(&s, EventSeq::new(from), limit).unwrap();
                let want = reference_page(&entries, EventSeq::new(from), limit);
                assert_eq!(
                    got, want,
                    "seek diverged from the walk at from_seq={from} limit={limit}"
                );
                compared += 1;
            }
        }
        assert!(compared > 100, "the comparison must actually cover a grid");
    }

    /// And the same property on a DISORDERED list, where the reader must fall
    /// back to the walk. Without the fallback the seek would binary-search an
    /// unsorted list, land inside the record run, and report the trailing gap
    /// as if it reached `from_seq` — a page that never existed.
    #[test]
    fn a_disordered_list_answers_exactly_as_the_full_walk_does() {
        let log = InMemoryExecutionLog::new();
        let s = SessionId::new("c1-c2-equiv-disordered");
        for _ in 0..10 {
            log.append_raw(s.clone(), 0, "ev").unwrap();
        }
        force_entry(
            &log,
            &s,
            RecordEntry::Gap(Gap::new(
                EventSeq::new(3),
                EventSeq::new(5),
                GapReason::KernelRingOverflow,
                "late",
            )),
        );
        assert!(!is_reach_ordered(&log, &s));

        let entries = entries_of(&log, &s);
        for from in 0..=12u64 {
            for limit in 0..=12usize {
                let got = log.read_from_seq(&s, EventSeq::new(from), limit).unwrap();
                let want = reference_page(&entries, EventSeq::new(from), limit);
                assert_eq!(
                    got, want,
                    "fallback diverged from the walk at from_seq={from} limit={limit}"
                );
            }
        }

        // The concrete leak an unconditional seek would produce: at
        // from_seq=6 the gap's last_missing is 5, so it must not appear.
        let page = log.read_from_seq(&s, EventSeq::new(6), 10).unwrap();
        assert_eq!(page.records.len(), 4, "seqs 6..=9");
        assert!(
            page.gaps.is_empty(),
            "a gap ending below from_seq leaked into the page: {:?}",
            page.gaps
        );
    }

    /// The minimal case that shows WHY the flag is load-bearing, found by
    /// exhaustive search over the shapes the public API can produce (5 records,
    /// then two retroactive gaps over 0..=2, so `reach` is
    /// `[0,1,2,3,4,2,2]`).
    ///
    /// Seeking that list without the flag binary-searches an unsorted slice and
    /// returns index 7 — the trailing gaps' `reach` of 2 is below `from_seq=4`,
    /// so the search concludes the list ends before the record at seq 4 and
    /// skips it. The page comes back EMPTY and `exhausted`, which tells a
    /// reader the log is caught up when it is not: the record is lost, not
    /// merely misreported. This is the whole reason `read_from_seq` consults
    /// `reach_disordered` instead of seeking unconditionally.
    #[test]
    fn a_seek_would_lose_a_record_on_a_retroactively_gapped_list() {
        let log = InMemoryExecutionLog::new();
        let s = SessionId::new("c1-c2-lost-record");
        for _ in 0..5 {
            log.append_raw(s.clone(), 0, "ev").unwrap();
        }
        for _ in 0..2 {
            force_entry(
                &log,
                &s,
                RecordEntry::Gap(Gap::new(
                    EventSeq::ZERO,
                    EventSeq::new(2),
                    GapReason::KernelRingOverflow,
                    "late",
                )),
            );
        }
        assert_eq!(reaches(&log, &s), vec![0, 1, 2, 3, 4, 2, 2]);
        assert!(!is_reach_ordered(&log, &s));

        let page = log.read_from_seq(&s, EventSeq::new(4), 1).unwrap();
        assert_eq!(
            page.records.iter().map(|r| r.seq.0).collect::<Vec<_>>(),
            vec![4],
            "the record at seq 4 must be delivered, not skipped"
        );
        assert!(!page.exhausted, "the log is not caught up at seq 4 of 5");
        assert_eq!(page.position_after, EventSeq::new(5));
    }

    /// The grid the minimal case came from, kept small enough for the hot gate.
    /// Any shape whose seek diverges from the full walk must fail here, so a
    /// future change to the seek predicate or to the flag cannot silently pass.
    #[test]
    fn no_reachable_shape_diverges_from_the_full_walk() {
        let mut shapes = 0usize;
        for n_appends in [3u64, 5, 8, 13] {
            for g1 in 0..=n_appends {
                for g2 in [g1, n_appends, 0] {
                    let log = InMemoryExecutionLog::new();
                    let s = SessionId::new("c1-c2-grid");
                    for _ in 0..n_appends {
                        log.append_raw(s.clone(), 0, "ev").unwrap();
                    }
                    let mut ok = true;
                    for g in [g1, g2] {
                        let hi = (g + 2).min(n_appends);
                        // Bypasses `record_gap` on purpose: this grid exists to
                        // compare a seek against a full walk on lists that are
                        // out of reach order, and those are exactly the lists the
                        // public `record_gap` now refuses to build.
                        force_entry(
                            &log,
                            &s,
                            RecordEntry::Gap(Gap::new(
                                EventSeq::new(g),
                                EventSeq::new(hi),
                                GapReason::KernelRingOverflow,
                                "grid",
                            )),
                        );
                        if false {
                            ok = false;
                            break;
                        }
                    }
                    if !ok {
                        continue;
                    }
                    shapes += 1;
                    let entries = entries_of(&log, &s);
                    for from in 0..=(n_appends + 4) {
                        for limit in 0..=(n_appends as usize + 4) {
                            let got = log.read_from_seq(&s, EventSeq::new(from), limit).unwrap();
                            let want = reference_page(&entries, EventSeq::new(from), limit);
                            assert_eq!(
                                got, want,
                                "diverged from the full walk at n={n_appends} \
                                 gaps=({g1},{g2}) from={from} limit={limit}"
                            );
                        }
                    }
                }
            }
        }
        assert!(
            shapes > 20,
            "the grid must actually cover shapes, got {shapes}"
        );
    }

    /// A long run of appends, then a read at a deep position, must be answered
    /// without walking the prefix. This is the cheap in-tree stand-in for C1:
    /// it does not assert a duration, it asserts that a page from the tail is
    /// exactly the tail. The duration claim is C1 itself, which lives in the
    /// ignored scale target (see `chronos-sandbox/tests/`), because asserting a
    /// wall clock in the hot gate would be a flaky gate rather than a contract.
    #[test]
    fn a_deep_position_answers_with_the_deep_window() {
        let n = 200_000u64;
        let (log, s) = seeded(n);
        let page = log.read_from_seq(&s, EventSeq::new(n - 3), 100).unwrap();
        let seqs: Vec<u64> = page.records.iter().map(|r| r.seq.0).collect();
        assert_eq!(seqs, vec![n - 3, n - 2, n - 1]);
        assert_eq!(page.position_after, EventSeq::new(n));
        assert!(!page.exhausted);
    }

    // ---------------------------------------------------------------------
    // `read_after`'s seek, in the two states the public API cannot build.
    //
    // `read_after` now seeks to `first_reachable(last_seq + 1)` exactly as
    // `read_from_seq` does, and `crates/chronos-log/tests/
    // read_after_seek_equivalence.rs` covers every state that IS reachable
    // from outside. These two are the ones that are not, and they live here
    // because `force_entry` bypasses the guard on purpose — what is under test
    // is the reader, not the API.
    // ---------------------------------------------------------------------

    /// The pre-seek loop of `read_after`, verbatim, as the oracle.
    fn reference_read_after(
        entries: &[RecordEntry],
        fresh: bool,
        last_seq: EventSeq,
    ) -> (Vec<u64>, Vec<String>) {
        let mut out_records: Vec<u64> = Vec::new();
        let mut out_gaps: Vec<String> = Vec::new();
        for entry in entries {
            match entry {
                RecordEntry::Record(r) => {
                    if fresh || r.seq > last_seq {
                        out_records.push(r.seq.0);
                    }
                }
                RecordEntry::Gap(g) => {
                    let include = if fresh {
                        g.last_missing > EventSeq::ZERO
                    } else {
                        g.last_missing > last_seq
                    };
                    if include {
                        out_gaps.push(format!("{}..{}", g.first_missing.0, g.last_missing.0));
                    }
                }
            }
        }
        (out_records, out_gaps)
    }

    fn actual_read_after(
        log: &InMemoryExecutionLog,
        s: &SessionId,
        last_seq: EventSeq,
    ) -> (Vec<u64>, Vec<String>) {
        match log
            .read_after(
                s.clone(),
                LogConsumerId::new("oracle"),
                Some(ConsumerCursor::at(LogConsumerId::new("oracle"), last_seq)),
            )
            .expect("read_after")
        {
            ReadResult::Ok { records, gaps, .. } => (
                records.iter().map(|r| r.seq.0).collect(),
                gaps.iter()
                    .map(|g| format!("{}..{}", g.first_missing.0, g.last_missing.0))
                    .collect(),
            ),
            other => panic!("expected Ok, got {other:?}"),
        }
    }

    /// A reach-`disordered` list must still be read correctly, by scanning.
    ///
    /// The list is built with `force_entry` because `record_gap` refuses gaps
    /// that overlap held evidence, which is what made the state unreachable
    /// from the API. The assertion is that the reader's fallback is still
    /// right if any future path ever produces the state: the difference is a
    /// correct read and a silently wrong one.
    #[test]
    fn read_after_on_a_disordered_list_matches_the_full_walk() {
        let (log, s) = seeded(10);
        // A gap whose reach (3) drops below the previous record's (9), and
        // which overlaps nothing because it was injected directly.
        force_entry(
            &log,
            &s,
            RecordEntry::Gap(Gap::new(
                EventSeq::new(1),
                EventSeq::new(3),
                GapReason::KernelRingOverflow,
                "forced",
            )),
        );
        assert!(
            !is_reach_ordered(&log, &s),
            "the fixture must actually be disordered, or this test proves nothing"
        );

        for last in [0u64, 3, 5, 9, 40] {
            let want = reference_read_after(&entries_of(&log, &s), false, EventSeq::new(last));
            let got = actual_read_after(&log, &s, EventSeq::new(last));
            assert_eq!(
                got, want,
                "disordered read at cursor {last} differs from the walk"
            );
        }
    }

    /// A cursor genuinely behind the oldest retained seq must still raise
    /// `CursorStale`, not be answered with a short page.
    ///
    /// This is the case the `may_be_stale` short-circuit could plausibly have
    /// broken: it skips the `oldest_seq` walk when the cursor is at or past
    /// the tail. The walk is what detects this error, so "the seek made it
    /// fast" and "the seek made it wrong" are the same change — which is why
    /// the check is asserted here rather than assumed from the other tests.
    #[test]
    fn read_after_still_raises_cursor_stale_when_the_walk_would_have() {
        let (log, s) = seeded(10);
        let consumer = LogConsumerId::new("stale");

        // Establish a stored high-water at the tail, as a caught-up consumer
        // would have.
        log.read_after(s.clone(), consumer.clone(), None)
            .expect("seed cursor");

        // Now prune the head so the stored cursor is behind what remains. The
        // in-memory backend exposes no retention API of its own, so the state
        // is forced the same way: drop the leading entries, which is exactly
        // what advancing a retained watermark does to the list.
        let keep_from = 6u64;
        {
            let mut records = log.records.lock().expect("records lock poisoned");
            let entries = records.get_mut(&s).expect("session present");
            entries
                .entries
                .retain(|e| e.reach() >= EventSeq::new(keep_from));
        }
        let entries_of_log = entries_of(&log, &s);
        assert_eq!(
            entries_of_log.first().map(|e| e.reach().0),
            Some(keep_from),
            "the fixture must have dropped a prefix, or the stale path is untested"
        );

        let want_stale = reference_oldest(&entries_of_log) > EventSeq::new(keep_from - 1);
        assert!(
            want_stale,
            "the fixture must make a tail cursor stale, or this test proves nothing"
        );
        match log.read_after(
            s.clone(),
            consumer,
            Some(ConsumerCursor::at(
                LogConsumerId::new("stale"),
                EventSeq::new(0),
            )),
        ) {
            Err(LogError::CursorStale {
                expected, current, ..
            }) => {
                assert_eq!(expected, EventSeq::new(0));
                assert_eq!(current, reference_oldest(&entries_of_log));
            }
            other => {
                panic!("a cursor behind the retained head must raise CursorStale, got {other:?}")
            }
        }
    }

    fn reference_oldest(entries: &[RecordEntry]) -> EventSeq {
        entries
            .iter()
            .map(|e| e.span_first())
            .min()
            .unwrap_or(EventSeq::ZERO)
    }
}
