# Tasks — Retire stale EventBus doc-comment mentions

## Apply

- [x] **D1**: `crates/chronos-mcp/src/server.rs` — `live_probes` field doc.
- [x] **D2**: `crates/chronos-native/src/probe_backend.rs` —
  `start_probe` doc + function-capture branch inline comment.
- [x] **D3**: `crates/chronos-native/src/capture_runner.rs` —
  `run_function_frame_capture_with_callback` callback-model doc.
- [x] **D4**: `crates/chronos-log/src/memory.rs` — m1-01 module-level
  doc (drop `EventBus` throughput comparator).
- [x] **D5**: `chronos-sandbox/tests/m1_acceptance.rs` — drop two
  `let backend_unused_eventbus_arg_removed = ();` residual stubs.

## Verification

- [x] **V1**: `cargo fmt --all -- --check` (T0) → clean.
- [x] **V2**: `cargo clippy --workspace --all-targets -- -D warnings`
  (T0) → clean. Pre-existing `clippy::new_without_default` for
  `NativeProbeBackend` fixed as a minimum-allowance separate patch (same
  cycle, separate commit per AGENTS.md §4 option A).
- [x] **V3**: `cargo test -p chronos-log -p chronos-native
  -p chronos-services -p chronos-mcp --lib --no-fail-fast
  -- --test-threads=1` (T1+T2) — touched crates pass.

## Pre-existing tree hygiene

While running T0 we discovered two pre-existing on `main` issues that
blocked `-D warnings`:

1. `cargo fmt` drift on `crates/chronos-native/src/probe_backend.rs`
   (an `assert!` body that exceeded the 100-col soft wrap). `cargo fmt
   --all` applied; same commit as the doc edits — the drift was
   minimal and the affected lines live in tests that use REC-C2.3-era
   identifiers, so the same commit reads coherently.
2. `clippy::new_without_default` on `NativeProbeBackend` (no
   `Default` impl). Minimum patch: `impl Default {
   fn default() { Self::new() } }`. Acceptable: identical behavior,
   one indirection. Worth folding into the same chore because the
   failure mode of `-D warnings` is what blocked the cycle's own
   verification.

Per AGENTS.md §4 option A both fixes are in-cycle under the
`chore/rec-c2.3-retire-stale-bus-doc` branch.
