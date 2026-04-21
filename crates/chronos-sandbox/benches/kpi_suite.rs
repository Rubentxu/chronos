//! Criterion benchmark suite grouping all KPI benchmarks.
//!
//! This module combines all KPI-related benchmarks into a single suite
/// for comprehensive performance testing.

use chronos_sandbox::kpi::latency::LatencyCollector;
use chronos_sandbox::kpi::overhead::OverheadCollector;
use chronos_sandbox::kpi::throughput::ThroughputCollector;
use chronos_sandbox::manifest::scoring::score_result;

/// Group: latency_operations
pub mod latency_operations {
    use super::*;

    pub fn bench_latency_record(c: &mut criterion::Criterion) {
        let mut group = c.benchmark_group("latency_operations");

        group.bench_function("record_startup_sample", |b| {
            let mut collector = LatencyCollector::new();
            b.iter(|| {
                collector.record_startup(1000);
            })
        });

        group.bench_function("record_symbol_resolution_sample", |b| {
            let mut collector = LatencyCollector::new();
            b.iter(|| {
                collector.record_symbol_resolution(500);
            })
        });

        group.bench_function("record_query_sample", |b| {
            let mut collector = LatencyCollector::new();
            b.iter(|| {
                collector.record_query(200);
            })
        });

        group.bench_function("compute_startup_metrics", |b| {
            let mut collector = LatencyCollector::new();
            for i in 1..=1000 {
                collector.record_startup(i * 100);
            }
            b.iter(|| {
                let _ = collector.startup_metrics();
            })
        });

        group.finish();
    }
}

/// Group: throughput_operations
pub mod throughput_operations {
    use super::*;

    pub fn bench_throughput_operations(c: &mut criterion::Criterion) {
        let mut group = c.benchmark_group("throughput_operations");

        group.bench_function("record_event", |b| {
            let mut counter = ThroughputCollector::new();
            counter.start();
            b.iter(|| {
                counter.record_event();
            })
        });

        group.bench_function("record_batch_100", |b| {
            let mut counter = ThroughputCollector::new();
            counter.start();
            b.iter(|| {
                counter.record_events(100);
            })
        });

        group.bench_function("stop_and_metrics", |b| {
            let mut counter = ThroughputCollector::new();
            counter.start();
            for _ in 0..10000 {
                counter.record_event();
            }
            counter.stop();
            b.iter(|| {
                let _ = counter.metrics();
            })
        });

        group.finish();
    }
}

/// Group: scoring_computation
pub mod scoring_computation {
    use super::*;

    pub fn bench_scoring_operations(c: &mut criterion::Criterion) {
        let mut group = c.benchmark_group("scoring_computation");

        group.bench_function("score_result_calculation", |b| {
            b.iter(|| {
                let _ = score_result(0.95, 0.90, 0.85, 0.88, 0.92);
            })
        });

        group.bench_function("score_result_perfect", |b| {
            b.iter(|| {
                let _ = score_result(1.0, 1.0, 1.0, 1.0, 1.0);
            })
        });

        group.finish();
    }
}

/// Overhead measurement benchmarks
pub mod overhead_operations {
    use super::*;

    pub fn bench_overhead_operations(c: &mut criterion::Criterion) {
        let mut group = c.benchmark_group("overhead_operations");

        group.bench_function("cpu_overhead_calc", |b| {
            let mut collector = OverheadCollector::new();
            collector.record_baseline(100.0, 50_000_000u64);
            b.iter(|| {
                let _ = collector.cpu_overhead(105.0);
            })
        });

        group.bench_function("memory_overhead_calc", |b| {
            let mut collector = OverheadCollector::new();
            collector.record_baseline(100.0, 50_000_000u64);
            b.iter(|| {
                let _ = collector.memory_overhead(60_000_000);
            })
        });

        group.finish();
    }
}

/// Main benchmark group that runs all KPI benchmarks.
pub fn kpi_suite(c: &mut criterion::Criterion) {
    latency_operations::bench_latency_record(c);
    throughput_operations::bench_throughput_operations(c);
    scoring_computation::bench_scoring_operations(c);
    overhead_operations::bench_overhead_operations(c);
}

criterion::criterion_group!(
    benches,
    kpi_suite,
    latency_operations::bench_latency_record,
    throughput_operations::bench_throughput_operations,
    scoring_computation::bench_scoring_operations,
    overhead_operations::bench_overhead_operations
);
criterion::criterion_main!(benches);
