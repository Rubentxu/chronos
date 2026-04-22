use criterion::{black_box, criterion_group, criterion_main, Criterion};
use chronos_domain::{EventData, EventType, SourceLocation, TraceEvent, TraceQuery, VariableInfo, VariableScope};
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

/// Benchmark arithmetic expression evaluation with simple expressions.
fn bench_query_engine_eval_simple(c: &mut Criterion) {
    // Create events with local variables for evaluation
    let locals = vec![
        VariableInfo::new("x", "10", "i32", 0x1000, VariableScope::Local),
        VariableInfo::new("y", "3", "i32", 0x2000, VariableScope::Local),
    ];
    let events: Vec<TraceEvent> = (0..1000u64)
        .map(|i| {
            TraceEvent::python_call_with_locals(
                i,
                i * 100,
                1,
                "my_module.my_func",
                "/path/to/script.py",
                10,
                locals.clone(),
            )
        })
        .collect();
    let engine = QueryEngine::new(events);

    c.bench_function("query_engine_eval_simple", |b| {
        b.iter(|| engine.evaluate_expression(black_box(500), black_box("x + 1")))
    });
}

/// Benchmark arithmetic expression evaluation with complex expressions.
fn bench_query_engine_eval_complex(c: &mut Criterion) {
    // Create events with local variables for evaluation
    let locals = vec![
        VariableInfo::new("a", "100", "i32", 0x1000, VariableScope::Local),
        VariableInfo::new("b", "5", "i32", 0x2000, VariableScope::Local),
        VariableInfo::new("c", "10", "i32", 0x3000, VariableScope::Local),
        VariableInfo::new("d", "3", "i32", 0x4000, VariableScope::Local),
        VariableInfo::new("e", "7", "i32", 0x5000, VariableScope::Local),
    ];
    let events: Vec<TraceEvent> = (0..1000u64)
        .map(|i| {
            TraceEvent::python_call_with_locals(
                i,
                i * 100,
                1,
                "my_module.my_func",
                "/path/to/script.py",
                10,
                locals.clone(),
            )
        })
        .collect();
    let engine = QueryEngine::new(events);

    c.bench_function("query_engine_eval_complex", |b| {
        b.iter(|| engine.evaluate_expression(black_box(500), black_box("(a * b + c) / d - e")))
    });
}

/// Benchmark event lookup by timestamp range from a TraceSession with 1000 events.
fn bench_session_event_lookup(c: &mut Criterion) {
    let events: Vec<TraceEvent> = (0..1000u64)
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

    c.bench_function("session_event_lookup_1000_by_timestamp_range", |b| {
        // Query a timestamp range that should return ~100 events (timestamps 25000 to 75000)
        let query = TraceQuery::new("bench")
            .time_range(black_box(25000), black_box(75000));
        b.iter(|| engine.execute(black_box(&query)))
    });
}

criterion_group!(
    benches,
    bench_get_event_by_id,
    bench_execute_query,
    bench_execute_query_with_pagination,
    bench_query_engine_eval_simple,
    bench_query_engine_eval_complex,
    bench_session_event_lookup
);
criterion_main!(benches);
