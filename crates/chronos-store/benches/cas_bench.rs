use criterion::{black_box, criterion_group, criterion_main, Criterion};
use chronos_domain::{EventData, EventType, SourceLocation, TraceEvent};
use chronos_store::{SessionMetadata, SessionStore, TraceDiff};
use tempfile::tempdir;

fn bench_session_store_save_single_event(c: &mut Criterion) {
    let dir = tempdir().unwrap();
    let store = SessionStore::open(dir.path().join("bench.redb").as_path()).unwrap();

    let event = TraceEvent::new(
        0,
        1000,
        1,
        EventType::FunctionEntry,
        SourceLocation::new("main.rs", 10, "bench_fn", 0x1000),
        EventData::Function {
            name: "bench_fn".to_string(),
            signature: None,
        },
    );

    let metadata = SessionMetadata {
        session_id: "bench-session".to_string(),
        created_at: 0,
        language: "native".to_string(),
        target: "/bin/bench".to_string(),
        event_count: 1,
        duration_ms: 100,
    };

    c.bench_function("session_store_save_single_event", |b| {
        b.iter(|| {
            store.save_session(black_box(metadata.clone()), black_box(&[event.clone()]))
        })
    });
}

fn bench_content_store_put(c: &mut Criterion) {
    use chronos_store::ContentStore;
    use redb::Database;

    let dir = tempdir().unwrap();
    let db = Database::create(dir.path().join("cas_bench.redb").as_path()).unwrap();
    let store = ContentStore::new(std::sync::Arc::new(db));

    let event = TraceEvent::new(
        0,
        1000,
        1,
        EventType::FunctionEntry,
        SourceLocation::new("main.rs", 10, "bench_fn", 0x1000),
        EventData::Function {
            name: "bench_fn".to_string(),
            signature: None,
        },
    );

    c.bench_function("content_store_put_single_event", |b| {
        b.iter(|| store.put(black_box(&event)))
    });
}

fn bench_session_store_load(c: &mut Criterion) {
    let dir = tempdir().unwrap();
    let store = SessionStore::open(dir.path().join("bench.redb").as_path()).unwrap();

    let event = TraceEvent::new(
        0,
        1000,
        1,
        EventType::FunctionEntry,
        SourceLocation::new("main.rs", 10, "bench_fn", 0x1000),
        EventData::Function {
            name: "bench_fn".to_string(),
            signature: None,
        },
    );

    let metadata = SessionMetadata {
        session_id: "load-bench-session".to_string(),
        created_at: 0,
        language: "native".to_string(),
        target: "/bin/bench".to_string(),
        event_count: 1,
        duration_ms: 100,
    };

    store
        .save_session(metadata.clone(), &[event])
        .unwrap();

    c.bench_function("session_store_load_single_event", |b| {
        b.iter(|| store.load_session(black_box("load-bench-session")))
    });
}

fn bench_trace_diff(c: &mut Criterion) {
    let events_a: Vec<TraceEvent> = (0..1_000)
        .map(|i| TraceEvent::function_entry(i, i * 100, 1, format!("fn_{}", i), 0x1000 + i))
        .collect();
    let events_b: Vec<TraceEvent> = (500..1_500)
        .map(|i| TraceEvent::function_entry(i, i * 100, 1, format!("fn_{}", i), 0x1000 + i))
        .collect();
    let meta_a = SessionMetadata {
        session_id: "a".into(),
        created_at: 0,
        language: "native".into(),
        target: "prog".into(),
        event_count: 1000,
        duration_ms: 1000,
    };
    let meta_b = meta_a.clone();
    c.bench_function("trace_diff_1k_50pct_overlap", |b| {
        b.iter(|| TraceDiff::compare("a", "b", black_box(&events_a), black_box(&events_b), &meta_a, &meta_b))
    });
}

criterion_group!(
    benches,
    bench_session_store_save_single_event,
    bench_content_store_put,
    bench_session_store_load,
    bench_trace_diff
);
criterion_main!(benches);
