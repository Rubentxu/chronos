//! CPU and memory overhead measurement.
//!
//! Satisfies Requirement: sandbox-kpi-overhead

/// Measures CPU and memory overhead of sandbox operations.
///
/// # Requirements
/// - CPU overhead ≤ 5%
/// - Memory overhead ≤ 20MB
#[derive(Debug, Clone, Default)]
pub struct OverheadCollector {
    baseline_cpu: Option<f64>,
    baseline_memory: Option<u64>,
}

impl OverheadCollector {
    /// Creates a new OverheadCollector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records the baseline (without sandbox) measurements.
    pub fn record_baseline(&mut self, cpu: f64, memory: u64) {
        self.baseline_cpu = Some(cpu);
        self.baseline_memory = Some(memory);
    }

    /// Returns the CPU overhead as a percentage.
    pub fn cpu_overhead(&self, current: f64) -> Option<f64> {
        self.baseline_cpu.map(|baseline| ((current - baseline) / baseline) * 100.0)
    }

    /// Returns the memory overhead in bytes.
    pub fn memory_overhead(&self, current: u64) -> Option<u64> {
        self.baseline_memory.map(|baseline| current - baseline)
    }

    /// Checks if CPU overhead is within acceptable limits (≤ 5%).
    pub fn is_cpu_acceptable(&self, current: f64) -> bool {
        self.cpu_overhead(current)
            .map(|overhead| overhead <= 5.0)
            .unwrap_or(true)
    }

    /// Checks if memory overhead is within acceptable limits (≤ 20MB).
    pub fn is_memory_acceptable(&self, current: u64) -> bool {
        self.memory_overhead(current)
            .map(|overhead| overhead <= 20 * 1024 * 1024)
            .unwrap_or(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overhead_calculation() {
        let mut collector = OverheadCollector::new();
        collector.record_baseline(100.0, 50_000_000); // 50MB baseline

        assert!((collector.cpu_overhead(105.0).unwrap() - 5.0).abs() < 0.01);
        assert_eq!(collector.memory_overhead(60_000_000), Some(10_000_000)); // 10MB overhead
    }

    #[test]
    fn test_acceptability_checks() {
        let mut collector = OverheadCollector::new();
        collector.record_baseline(100.0, 50_000_000);

        assert!(collector.is_cpu_acceptable(104.9)); // ≤ 5%
        assert!(!collector.is_cpu_acceptable(110.0)); // > 5%
        assert!(collector.is_memory_acceptable(70_000_000)); // ≤ 20MB
        assert!(!collector.is_memory_acceptable(100_000_000)); // > 20MB
    }
}
