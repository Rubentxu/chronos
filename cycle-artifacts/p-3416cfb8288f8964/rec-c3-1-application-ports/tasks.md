# Tasks — rec-c3-1-application-ports

> Generated for the build phase. Tasks are ordered to minimize
> churn: ports first (no callers), then `ports/mod.rs` wiring,
> then lib.rs re-exports, then unit tests, then the boundary check
> script, then contracts update, then docs.

## Apply

- [ ] **B1**: `crates/chronos-domain/src/ports/execution_log.rs` — re-export module
  - Re-export `ExecutionLog`, `ExecutionLogBackend as ExecutionLogProvider`,
    `NewExecutionRecord` from `chronos_log`.
  - Add module-level doc explaining the re-export rationale (AD-3).
  - Add a compile-only `#[cfg(test)] mod tests` for the `dyn` bound.

- [ ] **B2**: `crates/chronos-domain/src/ports/probe.rs` — new port module
  - Declare `ProbeController` trait with 4 methods (session_id, stop,
    detach, backend) per AD-2 and AD-6.
  - Declare `ProbeFactory` trait with `create` method per REQ-1.
  - Declare `ProbeRegistry` trait with attach/detach/list_active per REQ-1.
  - Provide `NullProbeFactory` and `NullProbeRegistry` no-op impls
    per AD-5.

- [ ] **B3**: `crates/chronos-domain/src/ports/session.rs` — new port module
  - Declare `SessionRepository` trait per REQ-3.
  - Declare `SessionHandle` struct with 4 fields, all port-typed
    per AD-4.
  - Provide `InMemorySessionRepository` smoke impl per AD-5.

- [ ] **B4**: `crates/chronos-domain/src/ports/telemetry.rs` — new port module
  - Declare `TelemetryReceiver` trait with metric/event methods per REQ-4.
  - Provide `NoopTelemetry` struct + impl per AD-5.

- [ ] **B5**: `crates/chronos-domain/src/ports/mod.rs` — wire the new modules
  - Add `mod` declarations for `execution_log`, `probe`, `session`,
    `telemetry` (keeping `notification`).
  - Add `pub use` re-exports for each module's public surface.

- [ ] **B6**: `crates/chronos-domain/src/lib.rs` — re-export at crate root
  - Add `pub use ports::*;` line (already present for ports, but
    ensure all new symbols are exposed).

- [ ] **B7**: `crates/chronos-domain/tests/ports/main.rs` + 5 sub-files
  - `probe.rs`: NullProbeFactory / NullProbeRegistry smoke.
  - `session.rs`: InMemorySessionRepository insert/find/list.
  - `telemetry.rs`: NoopTelemetry drop-in.
  - `execution_log.rs`: `Arc<dyn ExecutionLogProvider>` compile check.
  - `notification.rs`: NullNotificationSink.send() no-op.

- [ ] **B8**: `scripts/check_hex_boundary.py` (NEW) per AD-7
  - Reads `cargo tree -p chronos-domain --depth 1`.
  - Asserts no `reqwest`, `hyper`, `tokio`, `tokio-retry`,
    `async-http`, etc.
  - Exit 0 on clean, exit 2 on violation.

- [ ] **B9**: `reconstruction-contracts.toml` — HEX-001 → verified
  - Flip `status = "verified"`.
  - Add evidence, uat, verify fields per AD-8.
  - Add `notes` referencing the new port files and the boundary script.

- [ ] **B10**: `docs/ROADMAP.md` — REC-C3 entry update
  - Add the C3.1 in-progress note in the Active milestone section.

## Verification

- [ ] **V1**: T0 lint gate — `cargo fmt --all -- --check` and
  `cargo clippy --workspace --all-targets -- -D warnings` clean.

- [ ] **V2**: T1 — `cargo test -p chronos-domain --lib --tests
  --no-fail-fast` green; new port tests pass.

- [ ] **V3**: T2 — `cargo test -p chronos-domain -p chronos-services
  -p chronos-mcp --lib --no-fail-fast -- --test-threads=1`
  green (no regression in callers).

- [ ] **V4**: Hex boundary check —
  `python3 scripts/check_hex_boundary.py` PASS.

- [ ] **V5**: Architecture contract gate —
  `python3 scripts/check_architecture_contracts.py --strict-legacy`
  still PASS.

- [ ] **V6**: `cargo tree -p chronos-domain --depth 1` manual
  inspection — no HTTP/async deps.

## Out-of-scope gates (not run)

- T3 (full workspace) — likely passes but not required for this cycle.
- T4-smoke (sandbox subset) — no MCP wire change.
- T5 (full sandbox) — out of scope.

## Sub-cycle commit plan (recommended)

Reviewable work units, in order:

- **C1**: B1 + B5 (partial — only `mod execution_log`) +
  `mod.rs` re-export — re-export of `ExecutionLogProvider`.
- **C2**: B2 (probe.rs with NullProbeFactory + NullProbeRegistry).
- **C3**: B3 (session.rs with InMemorySessionRepository).
- **C4**: B4 (telemetry.rs with NoopTelemetry).
- **C5**: B5 (complete — wire all modules) + B6 (lib.rs re-export).
- **C6**: B7 (5 test files).
- **C7**: B8 (boundary check script).
- **C8**: B9 (contracts flip) + B10 (roadmap note) + implementation
  receipt + verification report.

Each commit reviewable independently. The cycle's
implementation-receipt.md references the commit chain.

## Today's session outcome

Tasks B1..B10 NOT executed today. Tomorrow's session picks up
here. Today's deliverables:

- `cycle-artifacts/p-3416cfb8288f8964/rec-c3-1-application-ports/exploration-report.md`
- `cycle-artifacts/p-3416cfb8288f8964/rec-c3-1-application-ports/specification.md`
- `cycle-artifacts/p-3416cfb8288f8964/rec-c3-1-application-ports/design.md`
- `cycle-artifacts/p-3416cfb8288f8964/rec-c3-1-application-ports/tasks.md` (this file)

Cycle is in `phase: plan` with lease active and the design gate
passed. To resume tomorrow:

```bash
sddk cycle resume --cycle p-3416cfb8288f8964/rec-c3-1-application-ports \
  --root /var/mnt/DiscoChino2-fast/Proyectos/rust/chronos --scope .
```