# REC-C3.5-residual-inversion — proposal

## Goal

Close the **residual production edges** that the REC-C3-hexagonal-closure
initiative did not touch, so `reconstruction-contracts.toml` can drop the
three `known_dependency_violations` entries and `check_architecture_contracts.py`
returns clean under the convergence baseline.

## Scope

Three residual production edges remain in `chronos-services` after the
REC-C3-hexagonal-closure initiative (Etapas A..D landed):

1. **`chronos-services::diff.rs → chronos_store::TraceDiff`**
   - `services::ChronosDiffService::compare_sessions` delegates to the
     zero-size struct `chronos_store::TraceDiff::compare(...)`.
   - 1 production callsite + 1 bench + 4 unit tests in `chronos_store::diff`.
   - BLAKE3 hash-based symmetric set diff, pure function.

2. **`chronos-services::{probe.rs, observe.rs} → chronos_native::{NativeProbeControllerImpl, NativeProbeBackend}`**
   - Three production callsites in `services::probe.rs` (start, attach,
     start_native_noop) and one test helper in `services::observe.rs`.
   - The service layer constructs the concrete backend **and** the port
     adapter (`NativeProbeControllerImpl::new(...)`), which is the audit
     §4.5 S4 composition-in-application-layer smell.
   - There is no composition-root helper (`chronos_mcp::composition`) for
     this port. The composition helper pattern exists for every other
     port (`default_execution_log_factory`, `default_uprobe_injector`,
     `default_browser_probe_factory`, `default_session_archive`,
     `default_counterexample_repository`) — the native probe port is the
     only gap.

3. **`chronos-services::counterexample.rs → chronos_store::counterexample_storage`**
   - 4 wire types imported as `cs::{...}`:
     `MinimisedPayload`, `HypothesisInputWire`, `ExistencePredicateWire`,
     `CounterexampleBundleSummaryWire`.
   - R-roadmap refinement B'.1 from the C3.5 cycle: deferred until a
     consumer outside services forced the move. The wider scope now
     confirms `chronos_cli::replay::replay_counterexample_bundle` also
     imports `chronos_store::counterexample_storage::{...}`, so the move
     has a second consumer and is no longer speculative.

Plus a stale-waiver finding that the C3.5 cycle introduced:

4. **`chronos-store → chronos-native` waiver is stale**
   - `check_architecture_contracts.py` reports:
     `stale architecture debt baseline: chronos-store -> chronos-native
     no longer exists; remove the waiver`.
   - Etapa C (REC-C3-hexagonal-closure) dropped the optional
     `chronos-native` dep and the `address_normalization` feature in
     `chronos-store/Cargo.toml`. The waiver line is dead code in the
     baseline.

## What this cycle delivers

### R.1 — Stale-waiver removal

Drop `"chronos-store -> chronos-native"` from
`reconstruction-contracts.toml[architecture].known_dependency_violations`.
Mechanical, single line.

### R.2 — `DiffEngine` port

- New port `DiffEngine` in
  `crates/chronos-domain/src/ports/diff.rs`:
  ```rust
  #[async_trait]
  pub trait DiffEngine: Send + Sync {
      fn compare(
          &self,
          session_a_id: &str,
          session_b_id: &str,
          events_a: &[TraceEvent],
          events_b: &[TraceEvent],
          meta_a: &SessionMetadata,
          meta_b: &SessionMetadata,
      ) -> DiffReport;
  }
  ```
- New struct `Blake3DiffEngine` impl in
  `crates/chronos-store/src/diff_engine_adapter.rs` containing the
  body currently in `chronos_store::diff::TraceDiff::compare`.
- `services::ChronosDiffService::compare_sessions` takes
  `Arc<dyn DiffEngine>` (added to `ProbeContext`-equivalent or a new
  `DiffContext`).
- Composition helper `default_diff_engine() -> Arc<dyn DiffEngine>` in
  `chronos-mcp::composition`.
- `chronos_store::diff` is left in place as a deprecated re-export
  pointing at the adapter (kept until the bench is updated to use the
  adapter directly).

### R.3 — `NativeProbeControllerFactory` port + composition helper

- New port `NativeProbeControllerFactory` in
  `crates/chronos-domain/src/ports/native_probe_factory.rs`:
  ```rust
  pub trait NativeProbeControllerFactory: Send + Sync {
      fn build(
          &self,
          session_id: SessionId,
          session: CaptureSession,
          language: Language,
          accepted_raw_observer: Option<Arc<dyn AcceptedRawObserver>>,
      ) -> Result<Box<dyn NativeProbeController>, FactoryBuildError>;
  }
  ```
- New struct `ChronosNativeProbeControllerFactory` in
  `crates/chronos-native/src/native_probe_factory.rs` containing the
  body currently spread across
  `services::probe.rs` (`backend` construction + `attach_execution_log`
  + `start_probe` + `NativeProbeControllerImpl::new`).
- `services::probe.rs` adds `native_probe_factory: &Arc<dyn NativeProbeControllerFactory>`
  to `ProbeContext`. Three callsites in
  `services::probe.rs` and one in `services::observe.rs` switch from
  constructing the backend + impl to calling `factory.build(...)`.
- Composition helper `default_native_probe_controller_factory() -> Arc<dyn NativeProbeControllerFactory>`
  in `chronos-mcp::composition`.
- `chronos-services` removes `chronos-native` from its
  `[dependencies]`.

### R.4 — Move 4 wire types to `chronos-domain::ports::counterexample::wire`

- New module `crates/chronos-domain/src/ports/counterexample/wire.rs`
  declaring `MinimisedPayload`, `HypothesisInputWire`,
  `ExistencePredicateWire`, `CounterexampleBundleSummaryWire` with
  the same `#[derive(...)]` shape they had in
  `chronos_store::counterexample_storage`.
- `chronos_store::counterexample_storage` keeps the type aliases
  `pub use chronos_domain::ports::counterexample::wire::{...}` so the
  redb encoding path doesn't churn.
- `services::counterexample.rs` switches to
  `use chronos_domain::ports::counterexample::wire as cs;`.
- `cli::replay::replay_counterexample_bundle` switches to the same.
- This unlocks the next iteration where `chronos-services` can drop
  `chronos-store` entirely (or keep it only for the redb
  `SessionStore` adapter, never for types).

### R.5 — Drop remaining waivers + verify gate

After R.1..R.4 land:

- `reconstruction-contracts.toml[architecture].known_dependency_violations = []`.
- `python3 scripts/check_architecture_contracts.py` returns 0 findings.
- `python3 scripts/check_hex_boundary.py` still passes (no change to
  HEX-C32).
- `cargo test --workspace --lib --no-fail-fast` still passes.
- Sandbox smoke `e2e_connectivity` + `analytics_tools` still pass.

## Acceptance criteria

| ID | Criterion | How to verify |
|---|---|---|
| AC-1 | `check_architecture_contracts.py` returns 0 findings | `python3 scripts/check_architecture_contracts.py` |
| AC-2 | `check_hex_boundary.py` still 0 errors / N notes | `python3 scripts/check_hex_boundary.py` |
| AC-3 | `services::diff.rs` has no `use chronos_store::` for `TraceDiff` | `grep -n 'use chronos_store::TraceDiff' crates/chronos-services/src/diff.rs` → 0 matches |
| AC-4 | `services::probe.rs` and `services::observe.rs` have no `use chronos_native::` | `grep -n 'use chronos_native' crates/chronos-services/src/{probe,observe}.rs` → 0 matches |
| AC-5 | `services::counterexample.rs` has no `use chronos_store::counterexample_storage` | `grep -n 'use chronos_store::counterexample_storage' crates/chronos-services/src/counterexample.rs` → 0 matches |
| AC-6 | `cli::replay::replay_counterexample_bundle` consumes wire types via `chronos_domain` | `grep -n 'use chronos_store::counterexample_storage' crates/chronos-cli/src/replay.rs` → 0 matches (only type re-exports in `chronos_store::counterexample_storage` remain) |
| AC-7 | `chronos-services` does not declare `chronos-store` or `chronos-native` in `[dependencies]` | `grep -E 'chronos-(store|native)' crates/chronos-services/Cargo.toml` → 0 matches |
| AC-8 | Sandbox smoke green | `e2e_connectivity` + `analytics_tools` both pass |
| AC-9 | Unit tests green | `cargo test --workspace --lib --no-fail-fast` 0 failures |

## R-roadmap refinements (this cycle)

- **R.1** is straightforward (1 line removal in contracts.toml).
- **R.2** mirrors the established `SessionReader`/`LifecycleStore`/`CounterexampleRepository`
  port + adapter pattern. The diff function is **already pure** and
  synchronous; the port needs no `async_trait`.
- **R.3** is the composition-leak fix that audit §4.5 S4 called out
  originally. The `factory.build(observer)` signature matches the
  production code: the service currently builds the backend with the
  observer, attaches the ExecutionLog provider, calls `start_probe`,
  and wraps in `NativeProbeControllerImpl`. All four steps move into
  the factory impl.
- **R.4** was an R-roadmap refinement **B'.1** from the C3.5 cycle
  that we now have the second consumer (`cli::replay`) to justify.

## Routing

- **A-lite**: bounded cross-cutting refactor, no architectural fork,
  follows established patterns from C3.5 (SessionReader, LifecycleStore,
  CounterexampleRepository). No new domain concepts, only structural
  moves.
- **Sequence**: propose (this file) → spec → tasks → apply → verify →
  release → archive. SDDK in auto mode, no per-step human gate.

## Out of scope

- The `chronos-store` → `chronos-domain` move for the redb `SessionStore`
  itself (the database adapter). That is owned by future work, not by
  this cycle — services will keep using `SessionStore` indirectly via
  the `SessionArchive`/`SessionReader`/`LifecycleStore`/`CounterexampleRepository`
  ports.
- The `chronos-domain` → `chronos_native` direction in the
  `provider.rs:167` doc comment is **not** an actual code edge (it is a
  `///` doc note about a transitional seam). No code change.
- The `chronos-cli::replay` is touched only at the type-import level
  (R.4); its behaviour is unchanged.
