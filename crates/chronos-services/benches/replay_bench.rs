//! Replay throughput bench — closes CAP-GAP-CHRONOS-H1.5-REPLAY-BENCH.
//!
//! ## Why this exists
//!
//! Per H1.5 §6.2: `SegmentedExecutionLog::replay()` (the lower-level
//! replay primitive that backs `chronos_cli::replay::run_replay`) had no
//! throughput bench. With no bench, capacity planning for replay-heavy
//! workloads (CI re-validations, cross-host reconstruction) had no
//! empirical anchor.
//!
//! ## What it measures
//!
//! Wall-clock duration of `SegmentedExecutionLog::replay()` over a
//! synthetic log with `N` records appended beforehand. Three sizes are
//! bench'd (`N = 1k, 10k, 100k`) so a single curve gives a coarse sense
//! of replay scalability.
//!
//! ## Why not larger N?
//!
//! 100k is the largest size that fits in a default criterion measurement
//! budget without saturating the dev box's I/O. Larger N is the
//! territory of integration tests with `tempfile`-backed persistence
//! and is covered by `chronos-log`'s own integration suite, not this
//! bench.
//!
//! ## Caveats
//!
//! - In-memory `InMemoryExecutionLog` (the replay target) holds all
//!   records. Memory consumption scales linearly; do NOT increase N
//!   beyond 100k without first verifying that the bench target host
//!   has the RAM headroom.
//! - The synthetic `NewExecutionRecord` payload is empty
//!   (`ExecutionPayload::Empty`). Real workloads with `Blob(Vec<u8>)`
//!   payloads will be slower; this bench establishes the **floor**
//!   for replay throughput, not the realistic ceiling.
//! - Wall-clock is a coarse metric; for a deeper analysis use
//!   `cargo bench --bench replay_bench -- --verbose` to see per-iteration
//!   dispersion.
//!
//! ## Out-of-scope
//!
//! - Replay integrity validation (`build_replay_plan`) is measured
//!   separately by `chronos-log`'s own integration suite.
//! - Cross-host reconstruction (CAP-CARGO-X-HOST-REPLAY) is not a
//!   `SegmentedExecutionLog` operation and is deferred per M10-CLOSE.

use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
use uuid::Uuid;

use chronos_domain::evidence::{ExecutionKind, ExecutionPayload, NewExecutionRecord};
use chronos_domain::SessionId;
use chronos_log::segmented::{SegmentedConfig, SegmentedExecutionLog};

/// Build a synthetic `SegmentedExecutionLog` with `n` empty payloads and
/// return the log (to call `replay()` on).
fn build_log(n: usize, segment_dir: &std::path::Path) -> SegmentedExecutionLog {
    let session_id = SessionId::new(format!("bench-replay-{}-{}", std::process::id(), n));
    let config = SegmentedConfig::with_dir(segment_dir.to_path_buf());
    let log = SegmentedExecutionLog::open(session_id.clone(), config).expect("log open");
    for i in 0..n as u64 {
        let rec = NewExecutionRecord {
            session_id: session_id.clone(),
            kind: ExecutionKind::Raw,
            monotonic_ns: i.saturating_mul(1_000),
            payload: ExecutionPayload::default(),
            invocation_id: None,
            parent_invocation_id: None,
            symbol_id: None,
            captured_at_unix_ns: None,
        };
        log.append(rec).expect("append");
    }
    log
}

fn bench_replay_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("replay_throughput");
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(10);

    for &n in &[1_000usize, 10_000, 100_000] {
        let tmp = std::env::temp_dir().join(format!(
            "chronos-replay-bench-{}-{}",
            std::process::id(),
            Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        let log = build_log(n, &tmp);

        group.bench_function(format!("replay_{n}_records"), |b| {
            b.iter(|| {
                let replayed = log.replay().expect("replay");
                criterion::black_box(replayed)
            })
        });

        std::fs::remove_dir_all(&tmp).ok();
    }
    group.finish();
}

criterion_group!(replay_benches, bench_replay_throughput);
criterion_main!(replay_benches);
