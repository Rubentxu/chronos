//! Causality index: tracks memory/variable write mutations for causal queries.

use crate::trace::{EventId, ThreadId, TimestampNs};
use std::collections::HashMap;

/// A single write mutation recorded in the causality index.
#[derive(Debug, Clone)]
pub struct CausalityEntry {
    /// ID of the trace event that caused this write.
    pub event_id: EventId,
    /// Timestamp (nanoseconds) when the write occurred.
    pub timestamp: TimestampNs,
    /// Thread that performed the write.
    pub thread_id: ThreadId,
    /// Value before the write, if known.
    pub value_before: Option<String>,
    /// Value after the write.
    pub value_after: String,
    /// Function that performed the write.
    pub function: String,
    /// Source file, if available.
    pub file: Option<String>,
    /// Source line, if available.
    pub line: Option<u32>,
}

/// Index that tracks memory/variable write mutations for causality queries.
///
/// Supports two query patterns:
/// - By address: `find_last_mutation(addr, before_ts)` — what wrote to this address last?
/// - By name: `trace_lineage(name)` — full write history for a named variable.
#[derive(Debug, Clone, Default)]
pub struct CausalityIndex {
    /// Memory address → list of write mutations (ordered by insertion = chronological).
    write_events: HashMap<u64, Vec<CausalityEntry>>,
    /// Variable name → memory addresses (for name-based lookups).
    name_to_addr: HashMap<String, Vec<u64>>,
}

impl CausalityIndex {
    /// Create a new empty causality index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a write mutation at a memory address.
    ///
    /// Optionally associates the address with a variable name for name-based queries.
    pub fn record_write(
        &mut self,
        addr: u64,
        entry: CausalityEntry,
        var_name: Option<&str>,
    ) {
        self.write_events.entry(addr).or_default().push(entry);

        if let Some(name) = var_name {
            let addrs = self.name_to_addr.entry(name.to_string()).or_default();
            if !addrs.contains(&addr) {
                addrs.push(addr);
            }
        }
    }

    /// Find the last write to `addr` that occurred strictly before `before_ts`.
    ///
    /// Returns `None` if no writes exist or all writes are at/after `before_ts`.
    pub fn find_last_mutation(
        &self,
        addr: u64,
        before_ts: TimestampNs,
    ) -> Option<&CausalityEntry> {
        self.write_events.get(&addr)?.iter()
            .filter(|e| e.timestamp < before_ts)
            .max_by_key(|e| e.timestamp)
    }

    /// Return all write entries for a named variable (exact name match), ordered by timestamp.
    ///
    /// Collects entries across all addresses associated with the variable name.
    pub fn trace_lineage(&self, name: &str) -> Vec<&CausalityEntry> {
        let Some(addrs) = self.name_to_addr.get(name) else {
            return Vec::new();
        };

        let mut entries: Vec<&CausalityEntry> = addrs
            .iter()
            .filter_map(|addr| self.write_events.get(addr))
            .flat_map(|v| v.iter())
            .collect();

        entries.sort_by_key(|e| e.timestamp);
        entries
    }

    /// Return all write entries for a specific address.
    pub fn writes_at(&self, addr: u64) -> &[CausalityEntry] {
        self.write_events
            .get(&addr)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Number of distinct addresses tracked.
    pub fn address_count(&self) -> usize {
        self.write_events.len()
    }

    /// Total number of write entries across all addresses.
    pub fn entry_count(&self) -> usize {
        self.write_events.values().map(Vec::len).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(event_id: u64, timestamp: u64, thread_id: u64, value: &str) -> CausalityEntry {
        CausalityEntry {
            event_id,
            timestamp,
            thread_id,
            value_before: None,
            value_after: value.to_string(),
            function: "test_fn".to_string(),
            file: None,
            line: None,
        }
    }

    #[test]
    fn test_record_write_and_find_last_mutation() {
        let mut idx = CausalityIndex::new();
        let addr = 0x1000;

        idx.record_write(addr, make_entry(1, 100, 1, "10"), None);
        idx.record_write(addr, make_entry(2, 200, 1, "20"), None);
        idx.record_write(addr, make_entry(3, 300, 1, "30"), None);

        // Last write before ts=250 is the one at ts=200
        let entry = idx.find_last_mutation(addr, 250).unwrap();
        assert_eq!(entry.timestamp, 200);
        assert_eq!(entry.value_after, "20");

        // Last write before ts=100 — nothing (ts=100 is not strictly before 100)
        assert!(idx.find_last_mutation(addr, 100).is_none());

        // Last write before ts=400 is the one at ts=300
        let entry = idx.find_last_mutation(addr, 400).unwrap();
        assert_eq!(entry.timestamp, 300);
    }

    #[test]
    fn test_trace_lineage_exact_match() {
        let mut idx = CausalityIndex::new();
        let addr1 = 0x2000;
        let addr2 = 0x2008;

        idx.record_write(addr1, make_entry(1, 100, 1, "hello"), Some("my_var"));
        idx.record_write(addr2, make_entry(2, 200, 1, "world"), Some("my_var"));
        idx.record_write(addr1, make_entry(3, 300, 1, "updated"), Some("my_var"));

        let lineage = idx.trace_lineage("my_var");
        assert_eq!(lineage.len(), 3);
        // Ordered by timestamp
        assert_eq!(lineage[0].timestamp, 100);
        assert_eq!(lineage[1].timestamp, 200);
        assert_eq!(lineage[2].timestamp, 300);

        // Exact match only — partial name returns empty
        assert!(idx.trace_lineage("my").is_empty());
        assert!(idx.trace_lineage("my_var_extra").is_empty());
    }

    #[test]
    fn test_writes_at() {
        let mut idx = CausalityIndex::new();
        let addr = 0x3000;

        assert!(idx.writes_at(addr).is_empty());

        idx.record_write(addr, make_entry(1, 50, 1, "a"), None);
        idx.record_write(addr, make_entry(2, 60, 2, "b"), None);

        let writes = idx.writes_at(addr);
        assert_eq!(writes.len(), 2);
        assert_eq!(writes[0].value_after, "a");
        assert_eq!(writes[1].value_after, "b");
    }

    #[test]
    fn test_name_to_addr_tracking() {
        let mut idx = CausalityIndex::new();
        let addr = 0x4000;

        // Same address, same name — should not duplicate the address in name_to_addr
        idx.record_write(addr, make_entry(1, 10, 1, "v1"), Some("counter"));
        idx.record_write(addr, make_entry(2, 20, 1, "v2"), Some("counter"));

        // Two distinct writes at same addr
        let lineage = idx.trace_lineage("counter");
        assert_eq!(lineage.len(), 2);

        // address_count / entry_count
        assert_eq!(idx.address_count(), 1);
        assert_eq!(idx.entry_count(), 2);
    }

    #[test]
    fn test_find_last_mutation_unknown_addr() {
        let idx = CausalityIndex::new();
        assert!(idx.find_last_mutation(0xDEAD, 9999).is_none());
    }
}
