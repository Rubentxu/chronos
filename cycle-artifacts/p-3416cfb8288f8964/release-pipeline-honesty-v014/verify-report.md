# Verify Report — release-pipeline-honesty

- **Cycle:** `p-3416cfb8288f8964/release-pipeline-honesty`
- **Path:** B-direct (lens: `direct-acceptance`)
- **Work item:** `a7695ce6-d1f4-4def-9a1d-5a5224cf79fc` (Done)
- **Subject:** commit `3d935b6a3d022f8ddc85b511e7dca414de010f2d`
- **Verdict:** **PASS**

## Acceptance evidence (OBSERVED)

| # | Check | Result |
|---|-------|--------|
| V1 | Non-Linux public probe surface fails closed | 4 guards return `TraceError::UnsupportedOperation` (`probe_backend.rs:456,607,1161,1184`); no fake-success path |
| V2 | `stop_probe` remains portable | not gated; nix signal APIs exist on macOS |
| V3 | `is_available()` precedent intact | still `cfg!(target_os = "linux")` (`probe_backend.rs:1124`) |
| V4 | Bridge stubs are structurally identical to the Linux types | mechanical parse: `PtraceConfig` 4/4 fields MATCH; `PtraceEvent` 6/6 variants with identical field sets MATCH |
| V5 | Consumers compile on both targets | `cargo check -p chronos-mcp --target x86_64-apple-darwin` 0 errors; `cargo check --workspace` (linux) 0 errors |
| V6 | No behaviour change on the only supported platform | `cargo test -p chronos-native --lib -- --test-threads=1` → 109 passed, 0 failed (baseline preserved) |
| V7 | `release.yml` fail-loud guard actually fails | negative test: guard exits 1 with `::error::` when the binary is absent |
| V8 | `docker.yml` credential guard both branches | simulated: secrets absent → skip with notice; secrets present → publish |
| V9 | Static hygiene | `cargo clippy -p chronos-native --all-targets` 0; darwin clippy 0; `cargo fmt --check` clean |
| V10 | Consumer suites | `chronos-capture` 26/26; `chronos-services` 539/539 |

## Gates

- **anti-placeholder:** PASS. Off-Linux callers get an explicit `UnsupportedOperation`; the bridge types are documented as never constructed and are never presented as a working tracer.
- **production-readiness:** PASS. No stub is reachable as a success path; `is_available()` already prevented backend selection off Linux before this change.
- **regression:** PASS. Linux behaviour is byte-identical in behaviour terms (impl body untouched, only moved into `mod imp`); 109/109 preserved.
- **SOLID / duplication:** PASS. No new abstraction layer was introduced; the mechanism reuses the existing `cfg` + fail-closed error pattern already present in `NativeProbeBackend::is_available`.

## Out-of-scope (recorded, not blocking)

- `cargo test -p chronos-native --lib` in **parallel** mode still hangs/fails (2 failures in `ptrace_tracer::tests`, 3 tests hung >60 s). Pre-existing, reproduced before this change, unrelated to platform gating. Serial mode is the supported invocation.
- `ring`/`blake3` native build scripts cannot run in a local Linux→macOS cross-check for unrelated workspace crates. Local cross-cc artifact only; CI mac runners have a working `cc`.
- Real macOS runner verification pending until the push lands (see the release/merge receipt).
