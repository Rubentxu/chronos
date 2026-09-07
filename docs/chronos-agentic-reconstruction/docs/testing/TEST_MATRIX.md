# Test matrix

| Area | Unit | Integration | Real UAT | Special runtime |
|---|---:|---:|---:|---:|
| ExecutionLog | yes | yes | SCALE-LOG-001 | no |
| Persistence | yes | yes | SCALE-LOG-001 | no |
| Invocation projection | yes | yes | RUST-STATE-001 | maybe |
| Properties | yes | yes | RUST-STATE-001 | no |
| Causal slice | yes | yes | RUST-STATE-001 | no |
| MCP v2 | yes | yes | agent UAT | no |
| eBPF native | yes | yes | RUST-TIMING-003 | privileged |
| OBI | adapter | yes | GO-DISCOUNT-001 | privileged |
| Go compile-time | adapter | yes | GO-DISCOUNT-001 | Go toolchain |
| Rust XRay | spike | spike | RUST-STATE-001 | nightly |
| Rust USDT | spike | spike | targeted | Linux/toolchain |
| OTel correlation | yes | yes | DIST-TRACE-001 | maybe |
| Concurrency | yes | yes | CONC-* | maybe |
| GUI | component | API | SCALE-LOG-001 | no |

## Stable core quality gates

```text
cargo fmt --check
cargo clippy --workspace --all-targets
cargo test --workspace
```

Reconcile commands with platform/feature constraints instead of pretending privileged backends can run everywhere.
