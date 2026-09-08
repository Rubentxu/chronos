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

use std::collections::HashMap;
use std::sync::Mutex;

/// One entry in the per-session record list.
#[derive(Debug, Clone)]
enum RecordEntry {
    Record(ExecutionRecord),
    Gap(Gap),
}

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
        Ok(())
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
        let entry = RecordEntry::Record(ExecutionRecord {
            session_id: session_id.clone(),
            seq: *seq,
            monotonic_ns: record.monotonic_ns,
            kind: ExecutionKind::Raw,
            payload: record.payload,
        });
        records.entry(session_id.clone()).or_default().push(entry);

        let assigned = *seq;
        *seq = seq.next();
        drop(records);
        drop(next_seq);

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
}
