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
#[derive(Debug, Clone, Default)]
pub struct TemporalIndex {
    /// Timestamp → event ID.
    entries: BTreeMap<TimestampNs, EventId>,
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
    pub fn insert(&mut self, timestamp: TimestampNs, event_id: EventId) {
        self.entries.insert(timestamp, event_id);
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

        for (&ts, &eid) in &self.entries {
            if ts >= current_end {
                // Flush current chunk
                if let Some(first_eid) = first_event_id {
                    self.chunks.push(TimeChunk {
                        start_ts: current_start,
                        end_ts: current_end,
                        first_event_id: first_eid,
                        event_count,
                    });
                }

                // Advance to the chunk containing this timestamp
                current_start = (ts / CHUNK_SIZE_NS) * CHUNK_SIZE_NS;
                current_end = current_start + CHUNK_SIZE_NS;
                first_event_id = Some(eid);
                event_count = 0;
            }

            if first_event_id.is_none() {
                first_event_id = Some(eid);
            }
            event_count += 1;
        }

        // Flush last chunk
        if let Some(first_eid) = first_event_id {
            self.chunks.push(TimeChunk {
                start_ts: current_start,
                end_ts: current_end,
                first_event_id: first_eid,
                event_count,
            });
        }

        self.dirty = false;
    }

    /// Get all event IDs within a time range [start, end).
    pub fn range(&self, start: TimestampNs, end: TimestampNs) -> Vec<EventId> {
        self.entries
            .range(start..end)
            .map(|(_, &eid)| eid)
            .collect()
    }

    /// Find the event ID closest to a given timestamp.
    /// Returns (timestamp, event_id) of the nearest event.
    pub fn nearest(&self, target: TimestampNs) -> Option<(TimestampNs, EventId)> {
        if self.entries.is_empty() {
            return None;
        }

        // Get the entry at or after target
        let after = self.entries.range(target..).next();
        // Get the entry before target
        let before = self.entries.range(..=target).next_back();

        match (before, after) {
            (Some((ts_before, eid_before)), Some((ts_after, eid_after))) => {
                let dist_before = target - ts_before;
                let dist_after = ts_after - target;
                if dist_before <= dist_after {
                    Some((*ts_before, *eid_before))
                } else {
                    Some((*ts_after, *eid_after))
                }
            }
            (Some((ts, eid)), None) => Some((*ts, *eid)),
            (None, Some((ts, eid))) => Some((*ts, *eid)),
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
    pub fn len(&self) -> usize {
        self.entries.len()
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

    #[test]
    fn test_insert_and_range() {
        let mut index = TemporalIndex::new();
        index.insert(100, 1);
        index.insert(200, 2);
        index.insert(300, 3);
        index.insert(400, 4);

        let range = index.range(150, 350);
        assert_eq!(range, vec![2, 3]);
    }

    #[test]
    fn test_range_empty_result() {
        let mut index = TemporalIndex::new();
        index.insert(100, 1);
        index.insert(200, 2);

        let range = index.range(500, 600);
        assert!(range.is_empty());
    }

    #[test]
    fn test_nearest() {
        let mut index = TemporalIndex::new();
        index.insert(100, 1);
        index.insert(200, 2);
        index.insert(400, 3);

        // Exact match
        assert_eq!(index.nearest(200), Some((200, 2)));
        // Closer to 200 than to 400
        assert_eq!(index.nearest(250), Some((200, 2)));
        // Closer to 400
        assert_eq!(index.nearest(350), Some((400, 3)));
        // Before first
        assert_eq!(index.nearest(50), Some((100, 1)));
        // After last
        assert_eq!(index.nearest(500), Some((400, 3)));
    }

    #[test]
    fn test_nearest_empty() {
        let index = TemporalIndex::new();
        assert!(index.nearest(100).is_none());
    }

    #[test]
    fn test_min_max_timestamp() {
        let mut index = TemporalIndex::new();
        assert!(index.min_timestamp().is_none());
        assert!(index.max_timestamp().is_none());

        index.insert(500, 1);
        index.insert(100, 2);
        index.insert(1000, 3);

        assert_eq!(index.min_timestamp(), Some(100));
        assert_eq!(index.max_timestamp(), Some(1000));
    }

    #[test]
    fn test_build_chunks() {
        let mut index = TemporalIndex::new();
        // Insert events across multiple 10ms chunks
        for i in 0..25 {
            index.insert((i as u64) * 5_000_000, i); // every 5ms
        }
        index.build_chunks();

        // Should have chunks covering 0-10ms, 10-20ms, 20-30ms
        assert!(index.chunk_count() >= 3);
        assert!(!index.is_empty());
    }

    #[test]
    fn test_build_chunks_noop_when_clean() {
        let mut index = TemporalIndex::new();
        index.insert(100, 1);
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
            index.insert(i as u64 * 1000, i);
        }
        assert_eq!(index.len(), 100);
    }
}
