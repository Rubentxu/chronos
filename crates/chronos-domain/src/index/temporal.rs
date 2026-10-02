//! Temporal index: maps timestamps to event IDs.

use crate::trace::{EventId, TimestampNs};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Size of each time chunk in nanoseconds (10 milliseconds).
const CHUNK_SIZE_NS: u64 = 10_000_000;

/// Index that maps timestamps to event IDs for fast temporal queries.
///
/// Used for queries like "what happened between T1 and T2?" or
/// "what was the state at timestamp T?"
///
/// The value is a **list**, not a single id: timestamps are not unique. Two
/// threads can be observed within the same nanosecond, and the clock is not
/// the event's identity. Keying by timestamp and holding one id made the
/// second insert overwrite the first, so a queried event silently vanished
/// and the indexed path disagreed with the unindexed scan.
#[derive(Debug, Clone, Default)]
pub struct TemporalIndex {
    /// Timestamp → every event ID observed at that timestamp, in push order.
    entries: BTreeMap<TimestampNs, Vec<EventId>>,
    /// Precomputed chunk boundaries for fast range seeks.
    chunks: Vec<TimeChunk>,
    /// Whether chunks need to be rebuilt.
    dirty: bool,
}

/// A time chunk representing a window of events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeChunk {
    /// Start of the time window (inclusive).
    pub start_ts: TimestampNs,
    /// End of the time window (exclusive).
    pub end_ts: TimestampNs,
    /// First event ID in this chunk.
    pub first_event_id: EventId,
    /// Number of events in this chunk.
    pub event_count: u64,
}

impl TemporalIndex {
    /// Create a new empty temporal index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a timestamp → event ID mapping.
    ///
    /// A timestamp already present gains a second id rather than losing the
    /// first: nanosecond resolution does not make timestamps unique, and
    /// dropping an event here made it undiscoverable for every later query.
    pub fn insert(&mut self, timestamp: TimestampNs, event_id: EventId) {
        self.entries.entry(timestamp).or_default().push(event_id);
        self.dirty = true;
    }

    /// Build chunks for fast range queries. Must be called after all inserts.
    pub fn build_chunks(&mut self) {
        if !self.dirty || self.entries.is_empty() {
            return;
        }

        self.chunks.clear();

        let mut current_start = 0u64;
        let mut current_end = CHUNK_SIZE_NS;
        let mut first_event_id = None;
        let mut event_count = 0u64;

        for (&ts, ids) in &self.entries {
            if ts.get() >= current_end {
                // Flush current chunk
                if let Some(first_eid) = first_event_id {
                    self.chunks.push(TimeChunk {
                        start_ts: TimestampNs::from(current_start),
                        end_ts: TimestampNs::from(current_end),
                        first_event_id: first_eid,
                        event_count,
                    });
                }

                // Advance to the chunk containing this timestamp
                current_start = (ts.get() / CHUNK_SIZE_NS) * CHUNK_SIZE_NS;
                current_end = current_start + CHUNK_SIZE_NS;
                first_event_id = ids.first().copied();
                event_count = 0;
            }

            if first_event_id.is_none() {
                first_event_id = ids.first().copied();
            }
            // Count every event at this timestamp, not every timestamp, so
            // the chunk census matches the number of events actually indexed.
            event_count += ids.len() as u64;
        }

        // Flush last chunk
        if let Some(first_eid) = first_event_id {
            self.chunks.push(TimeChunk {
                start_ts: TimestampNs::from(current_start),
                end_ts: TimestampNs::from(current_end),
                first_event_id: first_eid,
                event_count,
            });
        }

        self.dirty = false;
    }

    /// Get all event IDs within a time range [start, end).
    pub fn range(&self, start: TimestampNs, end: TimestampNs) -> Vec<EventId> {
        // `[start, end)` is empty when `start >= end`, but `BTreeMap::range`
        // panics on an inverted range. The bounds are built from two
        // independent tool parameters and arrive unsorted, so this input is
        // reachable from tool input. Answer it as the empty set, which is
        // also what the unindexed scan returns for the same query — the two
        // paths must not disagree about a query with no answer.
        if start >= end {
            return Vec::new();
        }
        self.entries
            .range(start..end)
            .flat_map(|(_, ids)| ids.iter().copied())
            .collect()
    }

    /// Find the event ID closest to a given timestamp.
    /// Returns (timestamp, event_id) of the nearest event.
    ///
    /// When several events share the nearest timestamp, the **last** one in
    /// push order wins. That is the id the old single-valued map used to
    /// hold, so the observable answer is unchanged, and for "what was the
    /// state at T" the latest event at T is the informative one.
    pub fn nearest(&self, target: TimestampNs) -> Option<(TimestampNs, EventId)> {
        if self.entries.is_empty() {
            return None;
        }

        // Get the entry at or after target
        let after = self.entries.range(target..).next();
        // Get the entry before target
        let before = self.entries.range(..=target).next_back();

        // The last id of a timestamp's list is the one this method reports.
        let pick = |v: &Vec<EventId>| v.last().copied();

        match (before, after) {
            (Some((ts_before, ids_before)), Some((ts_after, ids_after))) => {
                let (Some(eid_before), Some(eid_after)) = (pick(ids_before), pick(ids_after))
                else {
                    return None;
                };
                let dist_before = target.get() - ts_before.get();
                let dist_after = ts_after.get() - target.get();
                if dist_before <= dist_after {
                    Some((*ts_before, eid_before))
                } else {
                    Some((*ts_after, eid_after))
                }
            }
            (Some((ts, ids)), None) => pick(ids).map(|eid| (*ts, eid)),
            (None, Some((ts, ids))) => pick(ids).map(|eid| (*ts, eid)),
            (None, None) => None,
        }
    }

    /// Get the earliest timestamp in the index.
    pub fn min_timestamp(&self) -> Option<TimestampNs> {
        self.entries.first_key_value().map(|(&ts, _)| ts)
    }

    /// Get the latest timestamp in the index.
    pub fn max_timestamp(&self) -> Option<TimestampNs> {
        self.entries.last_key_value().map(|(&ts, _)| ts)
    }

    /// Returns the total number of indexed events.
    ///
    /// Counts events, not distinct timestamps. The builder counts pushes, and
    /// the two silently disagreed whenever two events shared a nanosecond.
    pub fn len(&self) -> usize {
        self.entries.values().map(Vec::len).sum()
    }

    /// Returns true if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the number of time chunks.
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An inverted range is reachable from tool input: `timestamp_start` and
    /// `timestamp_end` are independent parameters and arrive unsorted. The
    /// old code handed them straight to `BTreeMap::range`, which panics when
    /// `start > end`, so this query crashed the server. `[start, end)` is
    /// empty, and that is also what the unindexed scan returns for it.
    #[test]
    fn inverted_range_is_empty_not_a_panic() {
        let mut index = TemporalIndex::new();
        index.insert(TimestampNs::from(100), 1);
        index.insert(TimestampNs::from(200), 2);
        index.insert(TimestampNs::from(300), 3);

        assert!(
            index
                .range(TimestampNs::from(500), TimestampNs::from(100))
                .is_empty(),
            "an inverted range must answer empty, not panic"
        );
        // A degenerate empty range must stay empty too.
        assert!(index
            .range(TimestampNs::from(200), TimestampNs::from(200))
            .is_empty());
        // The ordered cases must be unaffected by the guard.
        assert_eq!(
            index.range(TimestampNs::from(100), TimestampNs::from(300)),
            vec![1, 2]
        );
    }

    /// The discriminating case. Nanosecond resolution does not make a
    /// timestamp unique — two threads can be observed in the same nanosecond.
    /// With a single id per timestamp the second insert overwrote the first,
    /// so `range` reported one event where the unindexed scan reports two, and
    /// `len` under-counted. Both must now account for every event.
    #[test]
    fn colliding_timestamps_keep_every_event() {
        let mut index = TemporalIndex::new();
        index.insert(TimestampNs::from(100), 1);
        index.insert(TimestampNs::from(100), 2);
        index.insert(TimestampNs::from(200), 3);

        let mut found = index.range(TimestampNs::from(0), TimestampNs::from(300));
        found.sort_unstable();
        assert_eq!(
            found,
            vec![1, 2, 3],
            "a shared timestamp must contribute every event, not just the last insert"
        );
        assert_eq!(
            index.len(),
            3,
            "len counts events, so it must agree with the builder's push count"
        );
    }

    /// The chunk census must count events too, or `TimeChunk::event_count`
    /// under-reports exactly where collisions happen.
    #[test]
    fn colliding_timestamps_are_counted_in_chunks() {
        let mut index = TemporalIndex::new();
        index.insert(TimestampNs::from(100), 1);
        index.insert(TimestampNs::from(100), 2);
        index.insert(TimestampNs::from(200), 3);
        index.build_chunks();

        let counted: u64 = index.chunks.iter().map(|c| c.event_count).sum();
        assert_eq!(counted, 3, "chunk census must count events, not timestamps");
    }

    /// `nearest` keeps reporting the last event at the nearest timestamp,
    /// which is what the old single-valued map held.
    #[test]
    fn nearest_keeps_reporting_the_last_event_at_a_timestamp() {
        let mut index = TemporalIndex::new();
        index.insert(TimestampNs::from(100), 1);
        index.insert(TimestampNs::from(100), 7);
        index.insert(TimestampNs::from(200), 3);

        assert_eq!(
            index.nearest(TimestampNs::from(100)),
            Some((TimestampNs::from(100), 7))
        );
    }

    /// The ordered, collision-free path must be untouched.
    #[test]
    fn distinct_timestamps_still_behave() {
        let mut index = TemporalIndex::new();
        index.insert(TimestampNs::from(100), 1);
        index.insert(TimestampNs::from(200), 2);
        index.insert(TimestampNs::from(300), 3);

        assert_eq!(index.len(), 3);
        assert_eq!(
            index.range(TimestampNs::from(150), TimestampNs::from(350)),
            vec![2, 3]
        );
    }

    #[test]
    fn test_insert_and_range() {
        let mut index = TemporalIndex::new();
        index.insert(TimestampNs::from(100), 1);
        index.insert(TimestampNs::from(200), 2);
        index.insert(TimestampNs::from(300), 3);
        index.insert(TimestampNs::from(400), 4);

        let range = index.range(TimestampNs::from(150), TimestampNs::from(350));
        assert_eq!(range, vec![2, 3]);
    }

    #[test]
    fn test_range_empty_result() {
        let mut index = TemporalIndex::new();
        index.insert(TimestampNs::from(100), 1);
        index.insert(TimestampNs::from(200), 2);

        let range = index.range(TimestampNs::from(500), TimestampNs::from(600));
        assert!(range.is_empty());
    }

    #[test]
    fn test_nearest() {
        let mut index = TemporalIndex::new();
        index.insert(TimestampNs::from(100), 1);
        index.insert(TimestampNs::from(200), 2);
        index.insert(TimestampNs::from(400), 3);

        // Exact match
        assert_eq!(
            index.nearest(TimestampNs::from(200)),
            Some((TimestampNs::from(200), 2))
        );
        // Closer to 200 than to 400
        assert_eq!(
            index.nearest(TimestampNs::from(250)),
            Some((TimestampNs::from(200), 2))
        );
        // Closer to 400
        assert_eq!(
            index.nearest(TimestampNs::from(350)),
            Some((TimestampNs::from(400), 3))
        );
        // Before first
        assert_eq!(
            index.nearest(TimestampNs::from(50)),
            Some((TimestampNs::from(100), 1))
        );
        // After last
        assert_eq!(
            index.nearest(TimestampNs::from(500)),
            Some((TimestampNs::from(400), 3))
        );
    }

    #[test]
    fn test_nearest_empty() {
        let index = TemporalIndex::new();
        assert!(index.nearest(TimestampNs::from(100)).is_none());
    }

    #[test]
    fn test_min_max_timestamp() {
        let mut index = TemporalIndex::new();
        assert!(index.min_timestamp().is_none());
        assert!(index.max_timestamp().is_none());

        index.insert(TimestampNs::from(500), 1);
        index.insert(TimestampNs::from(100), 2);
        index.insert(TimestampNs::from(1000), 3);

        assert_eq!(index.min_timestamp(), Some(TimestampNs::from(100)));
        assert_eq!(index.max_timestamp(), Some(TimestampNs::from(1000)));
    }

    #[test]
    fn test_build_chunks() {
        let mut index = TemporalIndex::new();
        // Insert events across multiple 10ms chunks
        for i in 0..25 {
            index.insert(TimestampNs::from(i * 5_000_000), i); // every 5ms
        }
        index.build_chunks();

        // Should have chunks covering 0-10ms, 10-20ms, 20-30ms
        assert!(index.chunk_count() >= 3);
        assert!(!index.is_empty());
    }

    #[test]
    fn test_build_chunks_noop_when_clean() {
        let mut index = TemporalIndex::new();
        index.insert(TimestampNs::from(100), 1);
        index.build_chunks();
        let count = index.chunk_count();

        // Calling again should be a no-op
        index.build_chunks();
        assert_eq!(index.chunk_count(), count);
    }

    #[test]
    fn test_len() {
        let mut index = TemporalIndex::new();
        assert_eq!(index.len(), 0);

        for i in 0..100 {
            index.insert(TimestampNs::from(i * 1000), i);
        }
        assert_eq!(index.len(), 100);
    }
}
