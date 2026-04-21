//! Criterion benchmarks for symbol resolution timing.
//!
//! Measures the time to look up and resolve debug target configurations
//! that simulate symbol resolution latency patterns.

use chronos_sandbox::targets::rust::RustTarget;
use chronos_sandbox::targets::DebugTarget;

/// Benchmark: target creation and introspection (simulates symbol resolution config lookup).
pub fn bench_target_container_options(c: &mut criterion::Criterion) {
    let mut group = c.benchmark_group("symbol_resolution");

    group.bench_function("rust_target_creation", |b| {
        b.iter(|| {
            let _ = RustTarget::new();
        })
    });

    group.bench_function("rust_target_is_attached_check", |b| {
        let target = RustTarget::new();
        b.iter(|| {
            let _ = target.is_attached();
        })
    });

    group.finish();
}

/// Benchmark: manifest loading from YAML bytes (simulates config parse latency).
pub fn bench_manifest_parse(c: &mut criterion::Criterion) {
    let yaml = r#"
name: rust-ptrace
language: rust
image: rust:1.77
debug_interface: ptrace
ports: []
caps:
  - SYS_PTRACE
security_opt:
  - seccomp=unconfined
workload:
  type: cpu_loop
  duration_secs: 30
kpi_thresholds:
  cpu_overhead_pct: 5.0
  memory_overhead_mb: 50.0
  capture_latency_p99_ms: 100.0
  events_per_sec_min: 1000
scenarios:
  - name: attach_and_capture
    description: Attach and capture events
    timeout_secs: 60
"#;

    let mut group = c.benchmark_group("manifest_parse");

    group.bench_function("parse_rust_manifest", |b| {
        b.iter(|| {
            let _: Result<serde_yaml::Value, _> = serde_yaml::from_str(yaml);
        })
    });

    group.finish();
}

criterion::criterion_group!(
    benches,
    bench_target_container_options,
    bench_manifest_parse
);
criterion::criterion_main!(benches);
