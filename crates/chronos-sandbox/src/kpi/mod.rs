//! KPI (Key Performance Indicator) collection module.
//!
//! Provides interfaces and implementations for measuring sandbox performance metrics.

pub mod overhead;
pub mod latency;
pub mod throughput;

/// KPI collector trait for sandbox metrics.
pub trait KpiCollector: Send + Sync {
    /// Collects CPU overhead metrics.
    fn collect_cpu_overhead(&self) -> Result<f64, crate::error::SandboxError>;

    /// Collects memory overhead metrics.
    fn collect_memory_overhead(&self) -> Result<u64, crate::error::SandboxError>;
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_kpi_collector_placeholder() {
        // Placeholder test
        assert!(true);
    }
}
