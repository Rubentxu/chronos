//! Shadow index: maps memory addresses to event IDs.

use crate::trace::EventId;
use std::collections::BTreeMap;

/// Index that maps memory addresses to the events that accessed them.
///
/// Used for queries like "who wrote to this address?" or
/// "what memory was accessed in this range?"
#[derive(Debug, Clone, Default)]
pub struct ShadowIndex {
    /// Address → list of event IDs that accessed this address.
    entries: BTreeMap<u64, Vec<EventId>>,
}

impl ShadowIndex {
    /// Create a new empty shadow index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that an event accessed a memory address.
    pub fn insert(&mut self, address: u64, event_id: EventId) {
        self.entries.entry(address).or_default().push(event_id);
    }

    /// Get all event IDs that accessed a specific address.
    pub fn get(&self, address: u64) -> &[EventId] {
        self.entries
            .get(&address)
            .map(|v: &Vec<EventId>| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get all event IDs that accessed any address in a range [start, end).
    pub fn get_range(&self, start: u64, end: u64) -> Vec<EventId> {
        let mut result = Vec::new();
        for (_addr, event_ids) in self.entries.range(start..end) {
            result.extend_from_slice(event_ids);
        }
        result
    }

    /// Returns the total number of indexed entries.
    pub fn len(&self) -> usize {
        self.entries.values().map(|v: &Vec<EventId>| v.len()).sum()
    }

    /// Returns true if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the number of unique addresses indexed.
    pub fn unique_addresses(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_get() {
        let mut index = ShadowIndex::new();
        index.insert(0x1000, 1);
        index.insert(0x1000, 5);
        index.insert(0x2000, 3);

        assert_eq!(index.get(0x1000), &[1, 5]);
        assert_eq!(index.get(0x2000), &[3]);
        let empty: &[EventId] = &[];
        assert_eq!(index.get(0x3000), empty);
    }

    #[test]
    fn test_get_range() {
        let mut index = ShadowIndex::new();
        index.insert(0x1000, 1);
        index.insert(0x1500, 2);
        index.insert(0x2000, 3);
        index.insert(0x2500, 4);

        let range = index.get_range(0x1000, 0x2000);
        // Range [0x1000, 0x2000) includes 0x1000 and 0x1500
        assert!(range.contains(&1));
        assert!(range.contains(&2));
        assert!(!range.contains(&3));
    }

    #[test]
    fn test_len_and_is_empty() {
        let mut index = ShadowIndex::new();
        assert!(index.is_empty());
        assert_eq!(index.len(), 0);

        index.insert(0x1000, 1);
        index.insert(0x1000, 2);
        assert!(!index.is_empty());
        assert_eq!(index.len(), 2);
        assert_eq!(index.unique_addresses(), 1);
    }

    #[test]
    fn test_many_addresses() {
        let mut index = ShadowIndex::new();
        for i in 0..1000 {
            index.insert(i as u64 * 8, i);
        }
        assert_eq!(index.unique_addresses(), 1000);
        assert_eq!(index.len(), 1000);
    }
}
