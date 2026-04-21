//! Criterion benchmarks for throughput measurement.
//!
//! These benchmarks measure the performance of ThroughputCounter operations.

use chronos_sandbox::kpi::throughput::ThroughputCollector;

/// Benchmark: measure ThroughputCounter::record_event() throughput.
pub fn bench_throughput_count(c: &mut criterion::Criterion) {
    let mut group = c.benchmark_group("throughput_operations");

    group.bench_function("record_single_event", |b| {
        let mut counter = ThroughputCollector::new();
        counter.start();
        b.iter(|| {
            counter.record_event();
        })
    });

    group.bench_function("record_100_events", |b| {
        let mut counter = ThroughputCollector::new();
        counter.start();
        b.iter(|| {
            for _ in 0..100 {
                counter.record_event();
            }
        })
    });

    group.bench_function("record_1000_events", |b| {
        let mut counter = ThroughputCollector::new();
        counter.start();
        b.iter(|| {
            for _ in 0..1000 {
                counter.record_event();
            }
        })
    });

    group.bench_function("record_events_batch_1000", |b| {
        let mut counter = ThroughputCollector::new();
        counter.start();
        b.iter(|| {
            counter.record_events(1000);
        })
    });

    group.bench_function("stop_and_get_metrics", |b| {
        let mut counter = ThroughputCollector::new();
        counter.start();
        for _ in 0..10000 {
            counter.record_event();
        }
        counter.stop();
        b.iter(|| {
            let mut c2 = ThroughputCollector::new();
            c2.start();
            for _ in 0..10000 {
                c2.record_event();
            }
            c2.stop();
            let _ = c2.metrics();
        })
    });

    group.finish();
}

criterion::criterion_group!(benches, bench_throughput_count);
criterion::criterion_main!(benches);
