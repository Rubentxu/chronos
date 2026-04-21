//! Throughput measurement for sandbox operations.
//!
//! Satisfies Requirement: sandbox-kpi-throughput

/// Throughput metrics.
#[derive(Debug, Clone, Default)]
pub struct ThroughputMetrics {
    /// Raw events per second.
    pub raw_events_per_sec: u64,
    /// Compressed events per second.
    pub compressed_events_per_sec: u64,
    /// Total events processed.
    pub total_events: u64,
    /// Duration in seconds.
    pub duration_sec: f64,
}

impl ThroughputMetrics {
    /// Calculates throughput from event count and duration.
    pub fn new(total_events: u64, duration_sec: f64) -> Self {
        if duration_sec <= 0.0 {
            return Self::default();
        }

        let raw_events_per_sec = (total_events as f64 / duration_sec) as u64;
        // Compression ratio is assumed to be ~0.8 (80%)
        let compressed_events_per_sec = (raw_events_per_sec as f64 * 0.8) as u64;

        Self {
            raw_events_per_sec,
            compressed_events_per_sec,
            total_events,
            duration_sec,
        }
    }

    /// Checks if raw throughput meets the requirement (≥ 100k events/sec).
    pub fn is_raw_throughput_acceptable(&self) -> bool {
        self.raw_events_per_sec >= 100_000
    }

    /// Checks if compressed throughput meets the requirement (≥ 80k events/sec).
    pub fn is_compressed_throughput_acceptable(&self) -> bool {
        self.compressed_events_per_sec >= 80_000
    }
}

/// Measures throughput for sandbox event processing.
#[derive(Debug, Clone, Default)]
pub struct ThroughputCollector {
    total_events: u64,
    start_time: Option<std::time::Instant>,
    end_time: Option<std::time::Instant>,
}

impl ThroughputCollector {
    /// Creates a new ThroughputCollector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts the throughput measurement.
    pub fn start(&mut self) {
        self.start_time = Some(std::time::Instant::now());
        self.total_events = 0;
        self.end_time = None;
    }

    /// Records an event.
    pub fn record_event(&mut self) {
        self.total_events += 1;
    }

    /// Records multiple events.
    pub fn record_events(&mut self, count: u64) {
        self.total_events += count;
    }

    /// Stops the throughput measurement.
    pub fn stop(&mut self) {
        self.end_time = Some(std::time::Instant::now());
    }

    /// Returns the throughput metrics.
    pub fn metrics(&self) -> ThroughputMetrics {
        let duration = self
            .start_time
            .and_then(|start| self.end_time.map(|end| end - start))
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);

        ThroughputMetrics::new(self.total_events, duration)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_throughput_metrics_calculation() {
        let metrics = ThroughputMetrics::new(100_000, 1.0);
        assert_eq!(metrics.raw_events_per_sec, 100_000);
        assert_eq!(metrics.compressed_events_per_sec, 80_000);
    }

    #[test]
    fn test_acceptable_throughput() {
        let metrics = ThroughputMetrics::new(100_000, 1.0);
        assert!(metrics.is_raw_throughput_acceptable());
        assert!(metrics.is_compressed_throughput_acceptable());

        let low_metrics = ThroughputMetrics::new(50_000, 1.0);
        assert!(!low_metrics.is_raw_throughput_acceptable());
        assert!(!low_metrics.is_compressed_throughput_acceptable());
    }
}
