//! Latency measurement for sandbox operations.
//!
//! Satisfies Requirement: sandbox-kpi-latency

use std::collections::BTreeMap;

/// Latency metrics (p50, p95, p99).
#[derive(Debug, Clone, Default)]
pub struct LatencyMetrics {
    /// p50 latency in microseconds.
    pub p50_us: u64,
    /// p95 latency in microseconds.
    pub p95_us: u64,
    /// p99 latency in microseconds.
    pub p99_us: u64,
    /// Sample count.
    pub samples: usize,
}

impl LatencyMetrics {
    /// Calculates latency metrics from a collection of samples (in microseconds).
    pub fn from_samples(samples: &[u64]) -> Self {
        if samples.is_empty() {
            return Self::default();
        }

        let mut sorted: BTreeMap<u64, usize> = BTreeMap::new();
        for &sample in samples {
            *sorted.entry(sample).or_insert(0) += 1;
        }

        let len = samples.len();
        let p50_idx = (len as f64 * 0.50) as usize;
        let p95_idx = (len as f64 * 0.95) as usize;
        let p99_idx = (len as f64 * 0.99) as usize;

        let mut cumulative = 0;
        let mut p50 = 0;
        let mut p95 = 0;
        let mut p99 = 0;

        for (&value, &count) in sorted.iter() {
            cumulative += count;
            if p50 == 0 && cumulative >= p50_idx {
                p50 = value;
            }
            if p95 == 0 && cumulative >= p95_idx {
                p95 = value;
            }
            if p99 == 0 && cumulative >= p99_idx {
                p99 = value;
                break;
            }
        }

        Self {
            p50_us: p50,
            p95_us: p95,
            p99_us: p99,
            samples: len,
        }
    }
}

/// Measures latency for various sandbox operations.
#[derive(Debug, Clone, Default)]
pub struct LatencyCollector {
    startup_samples: Vec<u64>,
    symbol_resolution_samples: Vec<u64>,
    query_samples: Vec<u64>,
}

impl LatencyCollector {
    /// Creates a new LatencyCollector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a startup latency sample (in microseconds).
    pub fn record_startup(&mut self, latency_us: u64) {
        self.startup_samples.push(latency_us);
    }

    /// Records a symbol resolution latency sample (in microseconds).
    pub fn record_symbol_resolution(&mut self, latency_us: u64) {
        self.symbol_resolution_samples.push(latency_us);
    }

    /// Records a query latency sample (in microseconds).
    pub fn record_query(&mut self, latency_us: u64) {
        self.query_samples.push(latency_us);
    }

    /// Returns the startup latency metrics.
    pub fn startup_metrics(&self) -> LatencyMetrics {
        LatencyMetrics::from_samples(&self.startup_samples)
    }

    /// Returns the symbol resolution latency metrics.
    pub fn symbol_resolution_metrics(&self) -> LatencyMetrics {
        LatencyMetrics::from_samples(&self.symbol_resolution_samples)
    }

    /// Returns the query latency metrics.
    pub fn query_metrics(&self) -> LatencyMetrics {
        LatencyMetrics::from_samples(&self.query_samples)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latency_metrics_calculation() {
        let samples: Vec<u64> = (1..=100).collect();
        let metrics = LatencyMetrics::from_samples(&samples);

        assert_eq!(metrics.samples, 100);
        // p50 should be around 50
        assert!((metrics.p50_us as i64 - 50).abs() <= 1);
        // p95 should be around 95
        assert!((metrics.p95_us as i64 - 95).abs() <= 1);
        // p99 should be around 99
        assert!((metrics.p99_us as i64 - 99).abs() <= 1);
    }

    #[test]
    fn test_empty_samples() {
        let metrics: LatencyMetrics = LatencyMetrics::from_samples(&[]);
        assert_eq!(metrics.samples, 0);
    }
}
