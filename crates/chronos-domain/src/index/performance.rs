//! Performance index: hardware counter data per function via perf_event_open.
//!
//! When CAP_PERFMON is unavailable, all counter fields remain `None` and
//! the index degrades gracefully — it still tracks call counts.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Hardware performance counters snapshot.
///
/// Fields are `Option` because counters may be unavailable without
/// `CAP_PERFMON` (Linux 5.8+) or `CAP_SYS_ADMIN`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PerfCounters {
    /// CPU cycles.
    pub cycles: Option<u64>,
    /// Instructions retired.
    pub instructions: Option<u64>,
    /// Last-level cache misses.
    pub cache_misses: Option<u64>,
    /// Cache references (hits + misses).
    pub cache_references: Option<u64>,
}

impl PerfCounters {
    /// Returns true if at least one counter has a value.
    pub fn has_data(&self) -> bool {
        self.cycles.is_some()
            || self.instructions.is_some()
            || self.cache_misses.is_some()
            || self.cache_references.is_some()
    }

    /// IPC (instructions per cycle), if both counters are available.
    pub fn ipc(&self) -> Option<f64> {
        match (self.instructions, self.cycles) {
            (Some(instr), Some(cycles)) if cycles > 0 => Some(instr as f64 / cycles as f64),
            _ => None,
        }
    }
}

/// Per-function performance statistics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FunctionPerf {
    /// Function entry address.
    pub address: u64,
    /// Function name (if resolved).
    pub name: Option<String>,
    /// Number of times this function was called.
    pub call_count: u64,
    /// Total estimated cycles spent in this function.
    pub total_cycles: u64,
}

impl FunctionPerf {
    pub fn new(address: u64, name: Option<String>) -> Self {
        Self {
            address,
            name,
            call_count: 0,
            total_cycles: 0,
        }
    }

    /// Average cycles per call.
    pub fn avg_cycles(&self) -> f64 {
        if self.call_count == 0 {
            0.0
        } else {
            self.total_cycles as f64 / self.call_count as f64
        }
    }
}

/// Index for hardware performance counter data collected during a trace session.
///
/// Built by `IndexBuilder` during `finalize()`. Counter fields are `None`
/// if `perf_event_open` was unavailable at capture time.
#[derive(Debug, Clone, Default)]
pub struct PerformanceIndex {
    /// Global counters for the entire session.
    pub counters: PerfCounters,
    /// Per-function statistics (address → stats).
    function_stats: HashMap<u64, FunctionPerf>,
}

impl PerformanceIndex {
    /// Create a new empty performance index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a function call with optional cycle count.
    pub fn record_call(&mut self, address: u64, name: Option<String>, cycles: Option<u64>) {
        let entry = self
            .function_stats
            .entry(address)
            .or_insert_with(|| FunctionPerf::new(address, name));
        entry.call_count += 1;
        if let Some(c) = cycles {
            entry.total_cycles += c;
        }
    }

    /// Set global counters (called once when trace session ends).
    pub fn set_counters(&mut self, counters: PerfCounters) {
        self.counters = counters;
    }

    /// Get current global counters snapshot.
    pub fn read_counters(&self) -> &PerfCounters {
        &self.counters
    }

    /// Get per-function stats for a specific address.
    pub fn function_perf(&self, address: u64) -> Option<&FunctionPerf> {
        self.function_stats.get(&address)
    }

    /// Return all function stats sorted by call count descending.
    pub fn top_functions_by_calls(&self, limit: usize) -> Vec<&FunctionPerf> {
        let mut sorted: Vec<&FunctionPerf> = self.function_stats.values().collect();
        sorted.sort_by(|a, b| b.call_count.cmp(&a.call_count));
        sorted.truncate(limit);
        sorted
    }

    /// Return all function stats sorted by total cycles descending.
    pub fn top_functions_by_cycles(&self, limit: usize) -> Vec<&FunctionPerf> {
        let mut sorted: Vec<&FunctionPerf> = self.function_stats.values().collect();
        sorted.sort_by(|a, b| b.total_cycles.cmp(&a.total_cycles));
        sorted.truncate(limit);
        sorted
    }

    /// Total number of tracked functions.
    pub fn function_count(&self) -> usize {
        self.function_stats.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_call_and_read_counters() {
        let mut idx = PerformanceIndex::new();

        idx.record_call(0x1000, Some("main".to_string()), Some(500));
        idx.record_call(0x1000, Some("main".to_string()), Some(300));
        idx.record_call(0x2000, Some("helper".to_string()), Some(100));

        let perf = idx.function_perf(0x1000).unwrap();
        assert_eq!(perf.call_count, 2);
        assert_eq!(perf.total_cycles, 800);
        assert!((perf.avg_cycles() - 400.0).abs() < f64::EPSILON);

        let helper = idx.function_perf(0x2000).unwrap();
        assert_eq!(helper.call_count, 1);

        // No counters set yet
        assert!(!idx.read_counters().has_data());
    }

    #[test]
    fn test_function_counts() {
        let mut idx = PerformanceIndex::new();

        for i in 0..5u64 {
            for _ in 0..(i + 1) {
                idx.record_call(0x1000 + i, None, None);
            }
        }

        let top = idx.top_functions_by_calls(3);
        assert_eq!(top.len(), 3);
        // Most called first (addr 0x1004 was called 5 times)
        assert_eq!(top[0].call_count, 5);
        assert_eq!(top[1].call_count, 4);
        assert_eq!(top[2].call_count, 3);
    }

    #[test]
    fn test_set_and_read_global_counters() {
        let mut idx = PerformanceIndex::new();

        let counters = PerfCounters {
            cycles: Some(1_000_000),
            instructions: Some(2_000_000),
            cache_misses: Some(500),
            cache_references: Some(10_000),
        };
        idx.set_counters(counters.clone());

        let read = idx.read_counters();
        assert_eq!(read.cycles, Some(1_000_000));
        assert_eq!(read.instructions, Some(2_000_000));
        assert!(read.has_data());

        // IPC = 2_000_000 / 1_000_000 = 2.0
        assert!((read.ipc().unwrap() - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_graceful_fallback_no_counters() {
        // Without perf_event_open, counters remain None
        let idx = PerformanceIndex::new();
        let counters = idx.read_counters();

        assert!(!counters.has_data());
        assert!(counters.cycles.is_none());
        assert!(counters.ipc().is_none());
    }

    #[test]
    fn test_top_functions_by_cycles() {
        let mut idx = PerformanceIndex::new();
        idx.record_call(0xA000, Some("hot".to_string()), Some(9000));
        idx.record_call(0xB000, Some("cold".to_string()), Some(100));
        idx.record_call(0xA000, Some("hot".to_string()), Some(1000));

        let top = idx.top_functions_by_cycles(1);
        assert_eq!(top[0].address, 0xA000);
        assert_eq!(top[0].total_cycles, 10000);
    }
}
