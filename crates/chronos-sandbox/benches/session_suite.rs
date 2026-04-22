//! Criterion benchmarks for SandboxSession operations.
//!
//! Measures the performance of session creation, lifecycle state transitions,
//! and serialization operations.

use chronos_sandbox::session::SandboxSession;

/// Benchmark: measure time to create a SandboxSession.
pub fn bench_session_creation(c: &mut criterion::Criterion) {
    let mut group = c.benchmark_group("session_operations");

    group.bench_function("session_new", |b| {
        b.iter(|| {
            let session = SandboxSession::new("bench-session".to_string());
            criterion::black_box(session);
        })
    });

    group.finish();
}

/// Benchmark: measure time to transition through session lifecycle states.
pub fn bench_session_lifecycle(c: &mut criterion::Criterion) {
    let mut group = c.benchmark_group("session_lifecycle");

    group.bench_function("session_start_stop", |b| {
        b.iter(|| {
            let mut session = SandboxSession::new("bench-lifecycle".to_string());
            session.start();
            criterion::black_box(session.state);
            session.stop();
            criterion::black_box(session.state);
            criterion::black_box(session.duration());
        })
    });

    group.bench_function("session_pause_resume", |b| {
        b.iter(|| {
            let mut session = SandboxSession::new("bench-pause".to_string());
            session.start();
            session.pause();
            criterion::black_box(session.state);
            session.resume();
            criterion::black_box(session.state);
            session.stop();
        })
    });

    group.finish();
}

/// Benchmark: measure time to check session status repeatedly.
pub fn bench_session_status_check(c: &mut criterion::Criterion) {
    let mut group = c.benchmark_group("session_status");

    group.bench_function("session_status_idle", |b| {
        let session = SandboxSession::new("bench-status".to_string());
        b.iter(|| {
            let _ = criterion::black_box(session.status());
        })
    });

    group.bench_function("session_status_running", |b| {
        let mut session = SandboxSession::new("bench-status".to_string());
        session.start();
        b.iter(|| {
            let _ = criterion::black_box(session.status());
        })
    });

    group.finish();
}

criterion::criterion_group!(
    benches,
    bench_session_creation,
    bench_session_lifecycle,
    bench_session_status_check
);
criterion::criterion_main!(benches);
