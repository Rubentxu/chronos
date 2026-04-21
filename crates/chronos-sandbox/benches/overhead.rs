//! Criterion benchmarks for KPI overhead measurement.
//!
//! These benchmarks measure the performance of KPI collection operations.

use chronos_sandbox::kpi::overhead::OverheadCollector;
use chronos_sandbox::kpi::latency::LatencyCollector;

/// Benchmark: measure time to call OverheadCollector::measure().
/// Note: OverheadCollector uses cpu_overhead() and memory_overhead() methods
/// instead of a single measure() method.
pub fn bench_overhead_calculation(c: &mut criterion::Criterion) {
    let mut collector = OverheadCollector::new();
    collector.record_baseline(100.0, 50_000_000u64);

    let mut group = c.benchmark_group("kpi_overhead");

    group.bench_function("cpu_overhead_calculation", |b| {
        b.iter(|| {
            let _ = collector.cpu_overhead(105.0);
        })
    });

    group.bench_function("memory_overhead_calculation", |b| {
        b.iter(|| {
            let _ = collector.memory_overhead(60_000_000);
        })
    });

    group.bench_function("is_cpu_acceptable_check", |b| {
        b.iter(|| {
            let _ = collector.is_cpu_acceptable(104.0);
        })
    });

    group.bench_function("is_memory_acceptable_check", |b| {
        b.iter(|| {
            let _ = collector.is_memory_acceptable(65_000_000);
        })
    });

    group.finish();
}

/// Benchmark: measure time to record 1000 latency samples.
pub fn bench_latency_record(c: &mut criterion::Criterion) {
    let mut group = c.benchmark_group("latency_operations");

    group.bench_function("record_single_startup_sample", |b| {
        let mut collector = LatencyCollector::new();
        b.iter(|| {
            collector.record_startup(1000);
        })
    });

    group.bench_function("record_1000_startup_samples", |b| {
        let mut collector = LatencyCollector::new();
        b.iter(|| {
            for i in 1..=1000 {
                collector.record_startup(i * 100);
            }
        })
    });

    group.bench_function("compute_metrics_after_1000_samples", |b| {
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

criterion::criterion_group!(
    benches,
    bench_overhead_calculation,
    bench_latency_record
);
criterion::criterion_main!(benches);
