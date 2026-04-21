use criterion::{black_box, criterion_group, criterion_main, Criterion};
use chronos_domain::{EventData, EventType, SourceLocation, TraceEvent, TraceQuery};
use chronos_query::QueryEngine;

fn bench_get_event_by_id(c: &mut Criterion) {
    let events: Vec<TraceEvent> = (0..10_000u64)
        .map(|i| {
            TraceEvent::new(
                i,
                i * 100,
                i % 4,
                EventType::FunctionEntry,
                SourceLocation::new("test.rs", 10, &format!("fn_{}", i), 0x1000 + i),
                EventData::Empty,
            )
        })
        .collect();
    let engine = QueryEngine::new(events);

    c.bench_function("get_event_by_id_10k", |b| {
        b.iter(|| engine.get_event_by_id(black_box(5000)))
    });
}

fn bench_execute_query(c: &mut Criterion) {
    let events: Vec<TraceEvent> = (0..100_000u64)
        .map(|i| {
            TraceEvent::new(
                i,
                i * 100,
                i % 8,
                EventType::FunctionEntry,
                SourceLocation::new(
                    "test.rs",
                    10,
                    &format!("fn_{}", i % 100),
                    0x1000 + i,
                ),
                EventData::Empty,
            )
        })
        .collect();
    let engine = QueryEngine::new(events);
    let query = TraceQuery::new("bench");

    c.bench_function("execute_query_100k_all", |b| {
        b.iter(|| engine.execute(black_box(&query)))
    });
}

fn bench_execute_query_with_pagination(c: &mut Criterion) {
    let events: Vec<TraceEvent> = (0..100_000u64)
        .map(|i| {
            TraceEvent::new(
                i,
                i * 100,
                i % 8,
                EventType::FunctionEntry,
                SourceLocation::new(
                    "test.rs",
                    10,
                    &format!("fn_{}", i % 100),
                    0x1000 + i,
                ),
                EventData::Empty,
            )
        })
        .collect();
    let engine = QueryEngine::new(events);

    c.bench_function("execute_query_100k_paginated", |b| {
        let query = TraceQuery::new("bench").pagination(100, 0);
        b.iter(|| engine.execute(black_box(&query)))
    });
}

criterion_group!(
    benches,
    bench_get_event_by_id,
    bench_execute_query,
    bench_execute_query_with_pagination
);
criterion_main!(benches);
