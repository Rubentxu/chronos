//! Tests for KPI modules (latency, overhead, throughput, scoring).
//!
//! These tests verify KPI collection, metric calculation,
//! and scoring breakdown without requiring real sandbox execution.

use chronos_sandbox::kpi::latency::{LatencyCollector, LatencyMetrics};
use chronos_sandbox::kpi::overhead::OverheadCollector;
use chronos_sandbox::kpi::throughput::{ThroughputCollector, ThroughputMetrics};
use chronos_sandbox::manifest::scoring::score_result;

/// Tests basic latency recording and percentile calculation.
#[test]
fn test_latency_recorder_basic() {
    let mut collector = LatencyCollector::new();

    // Record some sample latencies (in microseconds)
    for i in 1..=100 {
        collector.record_startup(i * 100); // 100us, 200us, ..., 10000us
    }

    let metrics = collector.startup_metrics();
    assert_eq!(metrics.samples, 100);

    // p50 should be around 5000us (the median)
    assert!((metrics.p50_us as i64 - 5000).abs() <= 500);
    // p95 should be around 9500us
    assert!((metrics.p95_us as i64 - 9500).abs() <= 500);
    // p99 should be around 9900us
    assert!((metrics.p99_us as i64 - 9900).abs() <= 500);
}

/// Tests latency recording with empty samples.
#[test]
fn test_latency_empty_samples() {
    let collector = LatencyCollector::new();
    let metrics = collector.startup_metrics();
    assert_eq!(metrics.samples, 0);
    assert_eq!(metrics.p50_us, 0);
    assert_eq!(metrics.p95_us, 0);
    assert_eq!(metrics.p99_us, 0);
}

/// Tests OverheadCollector creation and baseline recording.
#[test]
fn test_overhead_collector_creation() {
    let mut collector = OverheadCollector::new();

    // Record baseline measurements
    collector.record_baseline(100.0, 50_000_000u64); // 100% CPU, 50MB memory

    // CPU overhead calculation: (105 - 100) / 100 * 100 = 5%
    let cpu_overhead = collector.cpu_overhead(105.0);
    assert!(cpu_overhead.is_some());
    assert!((cpu_overhead.unwrap() - 5.0).abs() < 0.01);

    // Memory overhead: 60MB - 50MB = 10MB
    let mem_overhead = collector.memory_overhead(60_000_000);
    assert_eq!(mem_overhead, Some(10_000_000));
}

/// Tests overhead acceptability checks.
#[test]
fn test_overhead_acceptability() {
    let mut collector = OverheadCollector::new();
    collector.record_baseline(100.0, 50_000_000u64);

    // CPU overhead within 5% limit
    assert!(collector.is_cpu_acceptable(104.9));
    assert!(!collector.is_cpu_acceptable(106.0)); // > 5%

    // Memory overhead within 20MB limit
    assert!(collector.is_memory_acceptable(69_000_000)); // ~19MB overhead
    assert!(!collector.is_memory_acceptable(71_000_000)); // ~21MB overhead
}

/// Tests ThroughputCounter basic operation.
#[test]
fn test_throughput_counter() {
    let mut counter = ThroughputCollector::new();
    counter.start();

    // Record some events
    for _ in 0..1000 {
        counter.record_event();
    }

    counter.stop();

    let metrics = counter.metrics();
    assert_eq!(metrics.total_events, 1000);
    assert!(metrics.duration_sec > 0.0);
    assert!(metrics.raw_events_per_sec >= 0);
}

/// Tests throughput calculation with multiple events recorded at once.
#[test]
fn test_throughput_record_batch() {
    let mut counter = ThroughputCollector::new();
    counter.start();
    counter.record_events(5000);
    counter.stop();

    let metrics = counter.metrics();
    assert_eq!(metrics.total_events, 5000);
}

/// Tests scoring formula with equal weights summing to requirements.
#[test]
fn test_scoring_formula() {
    // All perfect scores
    let breakdown = score_result(1.0, 1.0, 1.0, 1.0, 1.0);
    assert!((breakdown.total - 1.0).abs() < 0.001);
    assert_eq!(breakdown.grade(), 'A');

    // Verify weights sum to 1.0
    let weight_sum: f64 = 0.35 + 0.20 + 0.15 + 0.15 + 0.15;
    assert!((weight_sum - 1.0).abs() < 0.001);
}

/// Tests ScoringBreakdown with various score combinations.
#[test]
fn test_scoring_breakdown_various_scores() {
    // Test B grade
    let breakdown = score_result(0.8, 0.8, 0.8, 0.8, 0.8);
    assert!((breakdown.total - 0.8).abs() < 0.001);
    assert_eq!(breakdown.grade(), 'B');

    // Test F grade (very low scores)
    let breakdown = score_result(0.3, 0.3, 0.3, 0.3, 0.3);
    assert!((breakdown.total - 0.3).abs() < 0.001);
    assert_eq!(breakdown.grade(), 'F');
}

/// Tests LatencyMetrics calculation with sorted samples.
#[test]
fn test_latency_metrics_sorted() {
    // Pre-sorted samples
    let samples: Vec<u64> = (10..=60).step_by(10).collect(); // 10, 20, 30, 40, 50, 60
    let metrics = LatencyMetrics::from_samples(&samples);

    assert_eq!(metrics.samples, 6);
    // p50 should be around 35 (middle of 30 and 40)
    assert!((metrics.p50_us as i64 - 35).abs() <= 5);
}

/// Tests ThroughputMetrics zero duration handling.
#[test]
fn test_throughput_zero_duration() {
    let metrics = ThroughputMetrics::new(1000, 0.0);
    assert_eq!(metrics.raw_events_per_sec, 0);
    assert_eq!(metrics.duration_sec, 0.0);
}

/// Tests ThroughputMetrics acceptable thresholds.
#[test]
fn test_throughput_acceptable_thresholds() {
    // 100k events/sec - meets raw requirement
    let high = ThroughputMetrics::new(100_000, 1.0);
    assert!(high.is_raw_throughput_acceptable());
    assert!(high.is_compressed_throughput_acceptable());

    // 50k events/sec - below requirements
    let low = ThroughputMetrics::new(50_000, 1.0);
    assert!(!low.is_raw_throughput_acceptable());
    assert!(!low.is_compressed_throughput_acceptable());
}
