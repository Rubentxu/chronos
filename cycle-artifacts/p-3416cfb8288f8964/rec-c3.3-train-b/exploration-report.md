# SDDK Exploration: REC-C3.3.3 (Tren B) — native probe MCP tool surface

**Cycle**: `p-3416cfb8288f8964/rec-c3.3-train-b`
**Path**: A-min (operator-set 2026-09-18)
**Phase**: Explore
**Updated**: 2026-09-19T10:47Z
**Base SHA**: `fa5eb582` (post-Tren-A merge, post-`rec-c3-ci-hygiene` close at `3fdcbf13`)
**Scope** (operator-locked): expand chronos MCP tool surface so AI agents can drive the native probe (capture / query / extract / advance / step), reusing existing service-port abstractions (B1), introducing `SessionArchive` + `CounterexampleRepository` ports and moving `SessionMetadata` to `chronos-domain` (B2), keeping `store→native` direction out (B3 → REC-C3.4), with mandatory composition-root bootstrap/session-scoped split (B4).

## Context Quality
- Level: **C2** (clear with bounded scope; rules B1–B9 are explicit; phase artifacts from C3.3.0/1/2 are available and consistent).
- Evidence Present:
  - Operator rules in `cycle-artifacts/p-3416cfb8288f8964/rec-c3.3-train-b/apply-checkpoint.json::scope_decisions.constraints` (B1..B9).
  - Tren A recon `cycle-artifacts/p-3416cfb8288f8964/rec-c3-3-2-composition-integration-inversion/exploration-report.md` (composition-inversion decisions, native-bridge retirement, port inventory).
  - Tren A close artifacts (apply-checkpoint + close-receipt) and `findings_introduced.no_action_in_current_cycle` (FIND-Tren-A-01, FIND-CI-PRE-EXISTING-TEST-FAILURES) closed by `rec-c3-ci-hygiene`.
  - Hygiene close: `5bcb2b63` (CIH-C vault drift reconciliation) + `3fdcbf13` (CIH final SHA sync). HEAD of `rec-c3-ci-hygiene` is `3fdcbf13`.
  - Composition root module `crates/chronos-mcp/src/composition.rs` (243 lines, 6 bootstrap-scoped factories).
  - Service-port catalog `crates/chronos-domain/src/ports/` (10 modules, all object-safe, all sync).
  - Strict replay surfaces `crates/chronos-log/src/replay.rs` (`ReplayPlan` / `ReplayIntegrityError`).
- Missing Context:
  - The Tren-B README says "Tren A closed with `capture_session` finalized". No `name = "capture_session"` exists in `crates/chronos-mcp/src/server.rs` (confirmed: zero hits across all `crates/`). The likely mapping is the composition `probe_start` + `probe_stop` + `save_session` workflow, not a single MCP tool. This is a documentation-vs-code mismatch that the orchestrator must resolve before proposal work (likely option: introduce a high-level `capture_session` tool that wraps the existing trio as part of Tren B — see Affected Areas / §Approaches).
  - No `chronos-sandbox/tests/native_probe_tools.rs` file exists (smoke_subset cites it). The smoke subset will need that file produced as part of Tren B (or the subset must be edited).
- Recommended Effort: **deepen**. The locked rules reduce scope ambiguity but the open questions (capture_session vs probe_start, SessionMetadata migration surface, native advance/step semantics, composition-root placement of session-scoped factories) all warrant targeted reads before proposal.

## Current State

### Native probe ABI (`crates/chronos-native`)

Two coordination layers, two surfaces:

1. `crates/chronos-native/src/capture_runner.rs` (1131+ lines):
   - `CaptureRunner` (line 166): builder for `new(config) | attach_to(pid, config) | with_ptrace_config | start | run_to_completion | stop_and_collect`.
   - `CaptureResult` (line 43): `events: Vec<TraceEvent>`, `end_reason`, `total_events`.
   - `CaptureEndReason`: `Exited(i32) | Signaled { signal, signal_name } | StoppedByUser | Failed(String)`.
   - `AttachMode`: `Spawn { program, args } | Attach(u32)`.
   - `run_function_frame_capture_with_callback` (line 804): callback hook used by the FunctionFrame integration tests.
2. `crates/chronos-native/src/probe_backend.rs` (1100+ lines):
   - `NativeProbeBackend::new` (line 224), `attach_execution_log` (line 302), `start_probe` (line 438), `attach_probe` (line 577), `stop_probe` (line 653), `drain` (line 570 in services proxy), `drain_log` (line 650), `project_semantic`, `clone_resolver_pipeline`, `resolve_context`, `with_language`, `with_resolver`, `with_accepted_raw_observer`, `with_config`.
   - **No `advance` or `step` method on `NativeProbeBackend`**. The ptrace tracer's `PtraceTracer` (used internally) has `PTRACE_SINGLESTEP` support in `ptrace_tracer.rs`, but no service-level wrapper exposes it as `advance` / `step`.
3. `crates/chronos-services/src/probe.rs`:
   - `ProbeService::start` (line 264) — mint session id, open log via factory, build backend, start probe, register in `live_probes`, mark `active_session`.
   - `ProbeService::start_attach` (line 365) — `/proc/<pid>/exe` resolution + `attach_probe`.
   - `ProbeService::stop` (line 491), `drain` (line 570), `drain_log` (line 650), `compaction_metrics` (line 711), `session_snapshot` (line 735), `inject` (line 768), `status` (line 908).
   - `ProbeContext<'_>` (line 90) — eight fields: `live_probes`, `execution_logs`, `engines`, `session_languages`, `tripwire_manager`, `active_session`, `uprobe_injector`. **No `probe_factory: Arc<dyn ProbeFactory>` field yet** — the existing `ProbeFactory` / `ProbeRegistry` ports in `chronos-domain::ports::probe` are not wired into `ProbeContext` (the registry pair was imported but never consumed by `ProbeService::start`, which calls `NativeProbeBackend::new()` directly — Tren A made composition-root exposure only for `UprobeInjector` and `BrowserProbeFactory`, leaving the `ProbeFactory` factory port still unwired at the `ProbeContext` level).

### Existing service-port abstractions reusable per B1

Already-canonical ports in `crates/chronos-domain/src/ports/`:

| Port | Trait file | Methods | Used by |
|---|---|---|---|
| `ExecutionLogProvider` | `execution_log.rs` | evidence-only, `pages(from, kind)` | `SessionExecutionLog::provider`, `probe_drain_log`, `SessionStartOutput` paths |
| `ExecutionLogRetention` | `execution_log_retention.rs` | `advance_retained_from(port)` | `SessionExecutionLog::retention` |
| `ExecutionLogMaintenance` | `execution_log_maintenance.rs` | `flush`, `compact_retired`, `metrics` | `SessionExecutionLog::maintenance` |
| `ExecutionLogFactory` | `execution_log_factory.rs` | `create(root, session_id)` | `SessionExecutionLogRegistry::register_create` |
| `SessionRepository` | `session.rs` | `get`, `upsert`, `remove`, `list`, `len`, `is_empty` (lifecycle/registry only — B2 forbids inflation) | `ServiceContext.sessions_registry` (in-memory `InMemorySessionRepository` in tests) |
| `ProbeFactory` | `probe.rs` | `create(session_id, config, requirements) -> Box<dyn ProbeController>` | **not wired** into `ProbeContext` today |
| `ProbeController` | `probe.rs` | `session_id`, `stop`, `detach`, `backend` | none (factory returns it but services do not consume) |
| `ProbeRegistry` | `probe.rs` | `attach`, `detach`, `list_active`, `is_active` | none (no `ProbeContext` field) |
| `UprobeInjector` | `uprobe.rs` | `acquire(...)`, returns `Arc<dyn UprobeHandle>` | `ProbeContext::uprobe_injector` (C3.3.2.3) |
| `BrowserProbeFactory` | `browser_probe.rs` | `create(...)` (per-session) | `BrowserProbeContext::factory` (C3.3.2.4) |
| Notification sink | `notification.rs` | fire-and-forget | webhook emission |

**Tren B intent**: tools do not introduce `NativeProbeServicePort`. The wire surface in `chronos-mcp::server.rs` already calls `probe_start`, `probe_stop`, `probe_drain`, `probe_drain_log`, `probe_inject`, `probe_status`, `probe_compaction_metrics`, `session_start`, `session_stop`, `session_snapshot`, `save_session`, `load_session`, `list_sessions`, `delete_session`, `drop_session`, `capabilities`, plus the query/debug/observe family. Composition-root exposes only the three already-wired factories.

**Note**: `ProbeFactory` / `ProbeRegistry` are declared but not wired. If Tren B decides to widen the surface (advance / step), the natural place to do it is to wire these ports — but that would be a B1 violation. The least-invasive path is to add `advance(session_id)` and `step(session_id)` as new methods on `NativeProbeBackend` and add service-level `ProbeService::advance` / `ProbeService::step` that the new MCP tools call (no new port). That stays inside B1 because no `NativeProbeServicePort` is introduced; the existing `services → native` composition edge stays where it is today.

### `SessionArchive` / `CounterexampleRepository` / `SessionMetadata` split (B2)

| B2 piece | Current location | Tren B target |
|---|---|---|
| `SessionMetadata` | `crates/chronos-store/src/storage.rs:22-43` (concrete bincode-shaped record with `session_id, created_at, language, target, event_count, duration_ms, tail_sealed, sealed_at`) | New `chronos_domain::session::SessionMetadata` (or `chronos_domain::value::session`) with the same fields; `chronos_store::SessionMetadata` becomes `pub use chronos_domain::SessionMetadata` (mirrors the C3.3.1 `SessionId` pattern). All 17 call-sites in `chronos-services` (see `grep -rn "chronos_store::SessionMetadata"`) change to `chronos_domain::SessionMetadata`. |
| `SessionArchive` | **does not exist** | New port in `chronos-domain::ports::session` (sibling of `SessionRepository`) — `save(metadata, events): SessionId`, `load(session_id) -> Result<(SessionMetadata, Vec<TraceEvent>), TraceError>`, `list() -> Vec<SessionMetadata>`, `delete(session_id)`, `count_events(session_id) -> usize`. Implemented by `chronos_store::SessionStore::save_session / load_session / list_sessions / delete_session` (which currently live at `crates/chronos-store/src/storage.rs` lines ~328, ~600, etc.). `SessionRepository` stays lifecycle-only (no `save` / `load` inflation per B2). |
| `CounterexampleRepository` | **does not exist** | New port in `chronos-domain::ports::counterexample` — `save_bundle(bundle_id, events)`, `load_bundle(bundle_id)`, `list_bundles(filter)`, `count_bundle_events(bundle_id)`. Implemented by `chronos_store::SessionStore::save_counterexample_bundle`, `load_counterexample_bundle`, `list_counterexample_bundles`, `count_counterexample_bundle_events`, `load_counterexample_bundle_events` (`crates/chronos-store/src/ce_write.rs:34`, `ce_read.rs:65`, `:100`, `:127`, `:205`). 8 production files in `chronos-services` today use `chronos_store::SessionStore` directly for counterexample operations (per `grep -rn "save_counterexample_bundle\|load_counterexample_bundle" crates/chronos-services`); these need to switch to `Arc<dyn CounterexampleRepository>`. |

**Stop rule (Tren A scope decision, line 257 of Tren A apply-checkpoint)**: if removing `chronos-store` from `services/Cargo.toml` requires `dyn Any` / downcast / store helper / `chronos_store` DTO crossing the port, the inversion is wrong; abort and replan. **This rule still binds Tren B.**

### Composition root (`chronos-mcp::composition`, B4)

Lines 1–243 of `crates/chronos-mcp/src/composition.rs`:

- **Bootstrap-scoped factories** (already there):
  - `default_execution_log_factory() -> Arc<dyn ExecutionLogFactory>` (line 51).
  - `default_uprobe_injector() -> Arc<dyn UprobeInjector>` (line 71).
  - `default_browser_probe_factory() -> Arc<dyn BrowserProbeFactory>` (line 89).
  - `default_store_path(db_path, home) -> PathBuf` (line 97).
  - `allow_in_memory_fallback(raw) -> bool` (line 114).
  - `open_session_store_at(path, allow_in_memory_fallback) -> Result<SessionStore, StoreOpenError>` (line 127) — the only `SessionStore::try_open` / `in_memory` site in the binary.
- **Module-level invariant** (line 7 of the file): "the only place in `chronos-mcp` where `::new()` / `try_open()` on infrastructure types is called".

**Tren B additions to this module** (bootstrap-scoped candidates):
- `default_session_archive() -> Arc<dyn SessionArchive>` — wraps the existing `SessionStore` (composed via `open_session_store_at`) and exposes the new port.
- `default_counterexample_repository() -> Arc<dyn CounterexampleRepository>` — same wrapping.
- `in_memory_session_archive()` and `in_memory_counterexample_repository()` for tests (parallels `InMemorySessionRepository` pattern).
- These are bootstrap-scoped because the underlying `SessionStore` is a process-wide singleton owned by `ChronosServer`.

**Tren B additions to this module** (session-scoped candidates):
- `native_capture_runner_factory(config) -> Arc<dyn NativeCaptureRunnerFactory>` (or similar) — produces a fresh `CaptureRunner` per `capture_session` call. This is exactly the bootstrap-vs-session distinction B4 calls out. If Tren B decides to compose a capture runner per-call rather than reuse the per-session `NativeProbeBackend`, the factory goes here.

**Server bootstrap** (`crates/chronos-mcp/src/server.rs::ChronosServer::try_new`): takes the bootstrap outputs, builds the `Arc<ServerContext>` (which feeds `ProbeContext`, `SessionsContext`, etc.). The Tren B proposal must extend the constructor signature so the new factory outputs reach the relevant contexts without breaking the existing `try_new() -> Result<Arc<Self>, InitError>` contract.

### Existing MCP tool surface (item 5)

`crates/chronos-mcp/src/server.rs` (9428 lines) declares **65 `#[tool(name = "...")]` handlers** (confirmed by `grep -n "name = \""`):

```
query_events, get_event, get_call_stack, get_execution_summary, execution_query,
state_diff, state_query, list_threads, debug_call_graph, debug_find_variable_origin,
debug_find_crash, debug_detect_races, inspect_causality, debug_expand_hotspot,
debug_get_saliency_scores, save_session, load_session, list_sessions, delete_session,
drop_session, evaluate_expression, debug_get_variables, debug_get_memory,
debug_get_registers, debug_diff, debug_analyze_memory, forensic_memory_audit,
tripwire_create, tripwire_list, tripwire_delete, tripwire_query,
probe_start, probe_stop, session_start, session_stop, capabilities, probe_drain,
probe_drain_log, probe_compaction_metrics, session_snapshot, probe_inject,
probe_status, browser_probe_start, browser_probe_stop, browser_probe_drain,
performance_regression_audit, compare_sessions, session_compare, session_explain,
mutation_lens, causal_slice, hypothesis_test, session_export, trace_slice,
events_read, observe, counterexample_shrink, counterexample_get, counterexample_list,
counterexample_events_count, counterexample_bundle_events
```

Mapping to Tren B scope ("capture / query / extract / advance / step"):

| Verb | Today | Tren B action |
|---|---|---|
| **capture** | `probe_start` + `session_start` + (wait) + `probe_stop` + `save_session` — five-tool sequence | Add `capture_session` (one-shot orchestrator: start probe, wait for completion / stop condition, persist metadata+events via the new `SessionArchive` port). README says "capture_session finalized" but no such tool exists — this is the README/code mismatch flagged in §Missing Context. |
| **query** | already extensive: `query_events`, `get_event`, `state_query`, `state_diff`, `list_threads`, `get_execution_summary`, `execution_query`, debug and observe family | **No new tool required** for the verb; only need to make sure the new `SessionArchive::load` is the read path behind `load_session` (not the concrete store). |
| **extract** | `counterexample_shrink`, `counterexample_get`, `counterexample_list`, `counterexample_events_count`, `counterexample_bundle_events` | **No new tool required**; switch the service layer behind these tools to consume the new `CounterexampleRepository` port instead of `chronos_store::SessionStore` directly. |
| **advance** | **none** | Add `probe_advance` (continue capture past current breakpoint / resume after pause). Native ABI: `NativeProbeBackend::advance` is **not** implemented today. Requires new method + `PtraceTracer::cont` (already exists). |
| **step** | **none** | Add `probe_step` (single-step target). Native ABI: `NativeProbeBackend::step` is **not** implemented today. Requires new method + `PtraceTracer::single_step` (already exists in `ptrace_tracer.rs`). |

### REC-C1.5.2 strict replay surfaces (item 6 — confirm Tren B does NOT touch)

`crates/chronos-log/src/replay.rs`:
- `build_replay_plan()` (line 153), `apply_replay_plan()` (line 153 area), `plan_gaps()`.
- `ReplayPlan`, `PlannedSegment`, `ReplayAction` types.
- `ReplayIntegrityError` (line 67, 9 variants — `CorruptSegment`, `HeaderRangeMismatch`, `FilenameHeaderMismatch`, `PayloadRangeMismatch`, `RecordSessionMismatch`, `EntryCountMismatch`, `MissingRange`, `OverlappingSegments`, `EmptySegment`).

Used by:
- `chronos-log/src/segmented.rs` (canonical reopen path, REC-C1.5.2 production semantics).
- Reconciled test: `chronos-sandbox/tests/m1_acceptance.rs` Case 6 — corrected in CIH-A (commit `533304b4`) to assert `ReplayIntegrity::CorruptSegment`, not lenient skip+recover.

**Tren B's obligations**:
- Do not modify `replay.rs`, `segmented.rs`, `ReplayIntegrityError`, or any production code in `chronos-log`.
- Do not relax the production semantics to make tests green.
- If a new MCP tool (`capture_session`) reopens an existing session, it must call the existing `register_takeover` path that the production code already uses — same fail-closed surface.

### ReplayIntegrity / LogError / ReplayIntegrityError taxonomy (item 7)

| Taxonomy | Location | Variants | Notes |
|---|---|---|---|
| `ReplayIntegrityError` | `chronos-log/src/replay.rs:67` | 9 (see §above) | `thiserror::Error`, raised by `build_replay_plan` only. No implicit conversion. |
| `LogError` | `chronos-log/src/error.rs:13` | separate enum (full surface in `error.rs`) | Per-segment decode/encoding error surface; differs in kind from replay validation. |
| `ExecutionLogError` | `chronos-domain/src/ports/execution_log.rs` | mapped from `LogError` (`ExecutionLogError::Open`, etc., added in C3.3.1) | The domain-side port type. |
| `TraceError` | `chronos-domain/src/error.rs` | top-level error that services and the wire surface consume. `SessionNotFound`, `CaptureFailed`, `AttachFailed`, etc. | The bridge from `ReplayIntegrityError` to the wire goes via `LogError -> ExecutionLogError -> TraceError` only if the application layer needs to surface it. None of Tren B's tools force a new error variant. |

**Observation**: `ReplayIntegrityError` and `ReplayIntegrity` are distinct types. Only `ReplayIntegrityError` exists. The Tren B proposal does not need to introduce `ReplayIntegrity` (the README phrase "ReplayIntegrity / LogError / ReplayIntegrityError" in the framing prompt is shorthand for the three error surfaces above).

### Ownership boundaries (item 8)

| Cycle | State | SHA | Scope |
|---|---|---|---|
| `rec-c0-*`, `rec-c1-*`, `rec-c2-*` | CLOSED | historical | prior milestones |
| `rec-c3-1-application-ports` | CLOSED | historical | declared the port taxonomy |
| `rec-c3-2-webhook-reconciliation` | CLOSED | historical | webhook sink port + adapter |
| `rec-c3-3-services-inversion` | CLOSED | `2f5e06f9` | recon + 4-edge inventory + composition-root placement |
| `rec-c3-3-1-identity-storage-seam` | CLOSED | `25420e5d` (later in `rec-c3.3-train-a`) | `SessionId` single owner, `ExecutionLogProvider` real, lifted types to domain |
| `rec-c3-3-2-composition-integration-inversion` (Tren A) | RELEASED on `rec-c3.3-train-a` | `25420e5d` | capability-bundle split for execution log + native bridge elimination + browser/ebpf composition wiring. **Tren A did NOT add any new MCP tool.** |
| `rec-c3-ci-hygiene` | CLOSED on `rec-c3-ci-hygiene` | `3fdcbf13` | CIH-A m1_02 reconciliation to REC-C1.5.2; CIH-B `McpTestClient::start` binary resolution; CIH-C vault drift reconciliation |
| **`rec-c3.3-train-b`** (Tren B, **THIS CYCLE**) | OPEN | base `fa5eb582`, head `acda8447` (skeleton only) | expand MCP tool surface (capture/query/extract/advance/step); introduce `SessionArchive` + `CounterexampleRepository` ports; move `SessionMetadata` to `chronos-domain` |
| `rec-c3-4-store-native-direction` (REC-C3.4) | NOT YET OPENED | n/a | store→native direction; Tren B leaves this as a no-touch edge per B3 |

**Boundary discipline for Tren B**:
- Tren B's `apply-checkpoint` (current head `acda8447`) is **skeleton-only** (1 commit, no `commits_since_base`). No slice has shipped.
- Tren A's released head (`25420e5d` on `rec-c3.3-train-a`) is the boundary for `services → execution-log` edges. Tren B does not regress.
- `rec-c3-ci-hygiene` is closed and merged-equivalent (the branch is still open but HEAD `3fdcbf13` is the closure commit, and Tren B branched off `fa5eb582` which sits AFTER all the hygiene commits in `git log`).

## Affected Areas

- `crates/chronos-mcp/src/server.rs` — add `capture_session`, `probe_advance`, `probe_step` tools; modify `save_session` / `load_session` / `delete_session` / `list_sessions` to route through `Arc<dyn SessionArchive>` (composition-root injected); modify `counterexample_*` tools to route through `Arc<dyn CounterexampleRepository>`.
- `crates/chronos-mcp/src/composition.rs` — add `default_session_archive()`, `default_counterexample_repository()`, in-memory variants for tests, and (if scope warrants) a session-scoped `native_capture_runner_factory()`. Tests in the in-file `composition_tests` module gain 4–6 cases.
- `crates/chronos-domain/src/ports/session.rs` — add `SessionMetadata` (lifted from `chronos-store`) and `SessionArchive` port + `InMemorySessionArchive` + `pub use` re-export if needed.
- `crates/chronos-domain/src/ports/counterexample.rs` (new file) — `CounterexampleRepository` port + `InMemoryCounterexampleRepository` + types: `CounterexampleBundleFilter`, `CounterexampleBundleSummary`, `CounterexampleBundleRecord`.
- `crates/chronos-domain/src/ports/mod.rs` — `pub use` of the new modules (parallel to `pub use browser_probe`, `pub use execution_log`).
- `crates/chronos-domain/src/lib.rs` — re-export `SessionMetadata` from the crate root so `services` can use it without a sub-module path.
- `crates/chronos-store/src/storage.rs` — replace `pub struct SessionMetadata` with `pub use chronos_domain::SessionMetadata;` (mirrors C3.3.1 `SessionId` pattern); add `save_session_via_archive(&self, archive: Arc<dyn SessionArchive>, ...)` adapter or refactor so the existing `save_session` / `load_session` / `list_sessions` / `delete_session` methods are reachable as a single concrete `SessionArchive` impl.
- `crates/chronos-store/src/lib.rs` — re-export the lifted `SessionMetadata`.
- `crates/chronos-services/src/session_explain.rs`, `session_export.rs`, `diff.rs`, `output.rs`, `sessions.rs`, `session_compare.rs`, `session_lifecycle.rs`, `counterexample.rs` (8 files) — switch `use chronos_store::SessionMetadata` → `use chronos_domain::SessionMetadata`; switch direct `chronos_store::SessionStore::*` calls to `Arc<dyn SessionArchive>` / `Arc<dyn CounterexampleRepository>` consumption (mediated by `ServiceContext`).
- `crates/chronos-services/Cargo.toml` — verify `chronos-store` is still present (it must stay for the concrete `SessionStore` to exist as an adapter — what gets removed is the *direct use* of `SessionStore` from services). Per B2 the dependency may need to stay if the new port adapter lives in `chronos-store` and services depend on the port only — net effect is that services no longer call `SessionStore::save_session` directly; they call `SessionArchive::save_session`.
- `crates/chronos-services/src/probe.rs` — add `ProbeService::advance` and `ProbeService::step` (or fold into existing `inject` family; advance/step are MCP-side primitives, distinct from uprobe inject).
- `crates/chronos-native/src/probe_backend.rs` — add `NativeProbeBackend::advance(&self, session: &CaptureSession) -> Result<(), TraceError>` and `step(&self, session: &CaptureSession) -> Result<(), TraceError>` methods. Both delegate to `PtraceTracer::cont` / `single_step` (already implemented in `ptrace_tracer.rs`).
- `crates/chronos-native/src/capture_runner.rs` — possibly extend `CaptureRunner` with `step(&mut self)` / `resume(&mut self)` to compose with the new service methods (defer to proposal — may not be necessary if `ProbeService::advance` talks to `NativeProbeBackend` directly).
- `chronos-sandbox/tests/native_probe_tools.rs` (new) — covers `probe_advance`, `probe_step`, `capture_session` end-to-end against the live binary (this file does NOT exist; smoke_subset cites it).
- `chronos-sandbox/tests/session_persistence.rs`, `session_lifecycle.rs`, `counterexample_tools.rs` — switch assertions to read metadata via the `SessionArchive` port (no behavioral change expected; reconciles field identity if `SessionMetadata` field order / default changes during the move).
- `cycle-artifacts/p-3416cfb8288f8964/rec-c3.3-train-b/` — `exploration-report.md` (this file), `specification.md` (Tren B requirements), `task-graph.json` (slice decomposition), `apply-checkpoint.json` (updated head after each slice), `verification-report.md` (lens summary), `release-receipt.md`, `merge-receipt.md`, `archive-manifest.md`.

## Approaches

1. **Single-shot `capture_session` plus `probe_advance` / `probe_step`, no factory port** (recommended)
   - Add `capture_session` as a high-level MCP tool that composes `probe_start` → wait-for-target-exit → `probe_stop` → `save_session`. Wire through the new `SessionArchive` port (so the tool's persistence step is the first consumer of `Arc<dyn SessionArchive>`).
   - Add `probe_advance` and `probe_step` as new MCP tools; backend gains `NativeProbeBackend::advance` / `step`. Service layer gets `ProbeService::advance` / `step`. No new port (B1).
   - `SessionMetadata` moves to `chronos-domain`. All 17 call-sites swap imports.
   - `SessionArchive` + `CounterexampleRepository` ports introduced. Concrete impls wrap the existing `SessionStore` methods.
   - Pros: smallest code churn; no `NativeProbeServicePort` (B1); clear bootstrap/session split (B4) because `capture_session` is bootstrap-only orchestration and `advance` / `step` are session-scoped but live entirely behind the existing service / backend surface; `SessionMetadata` lift is mechanical and mirrors C3.3.1's `SessionId` lift.
   - Cons: `capture_session` is a "superset" of `probe_start` + `save_session` — agents that already script the five-tool sequence may not switch; no behavior gate that forces callers to use the new tool.
   - Effort: **Medium** (3 new MCP tools, 2 new backend methods, 2 new ports, 17 call-site imports, 8 services call-site rewrites, 1 new sandbox test file).

2. **`capture_session` only; defer `advance` / `step` to a follow-up**
   - Add only `capture_session`. Defer `probe_advance` and `probe_step` to `rec-c3-4-store-native-direction` (or a new micro-cycle).
   - Pros: smaller blast radius; faster close.
   - Cons: violates the operator scope ("expand the surface for capture / query / extract / advance / step"); Tren B becomes capture-only.
   - Effort: **Low**, but **not in scope**.

3. **New `NativeProbeServicePort` that bundles advance/step/drain**
   - Honors a "native probe is its own port" framing. Violates **B1** (operator rule). Not viable.

4. **Wire `ProbeFactory` / `ProbeRegistry` ports** as part of the surface expansion
   - Closes the existing declared-but-unused ports. Violates **B1** if done to introduce a new `NativeProbeServicePort`, but is consistent if done to retire direct `NativeProbeBackend::new()` calls in `ProbeService::start`.
   - Pros: finishes C3.3.0 intent; closes `C31-DEBT-03` carry-forward debt.
   - Cons: pulls in scope that the operator framed as "REC-C3.3.x roadmap" not "Tren B". Risks exceeding A-min budget.
   - Effort: **High**. **Recommend deferring to a follow-up cycle** and noting it as `no_action_in_current_cycle` per B8.

## Recommendation

**Approach 1** is the only viable path that satisfies B1–B9 and the operator scope. The README/code mismatch on `capture_session` is resolved by adding the tool as part of approach 1. The composition root must split clearly:

- **Bootstrap-scoped** in `composition.rs`: `default_session_archive`, `default_counterexample_repository`, `in_memory_*` test variants, and the session-scoped factory for native capture runners only if proposal decides a `CaptureRunner` is needed per-call (likely NOT — `NativeProbeBackend` is already per-session via `LiveProbeSession`).
- **Session-scoped**: none new required; `advance` / `step` go through existing `ProbeService` which already gets `&ProbeContext<'_>` per call.

The `SessionMetadata` lift to `chronos-domain` mirrors the C3.3.1 `SessionId` lift (mechanical, low-risk). The new `SessionArchive` and `CounterexampleRepository` ports MUST NOT be inflated to include lifecycle state — `SessionRepository` stays the lifecycle port per B2.

## Risks

- **README/code mismatch on `capture_session`**: The Tren-B README claims capture_session is "in". No such tool exists. If the operator's intent was that Tren A added it, the assumption is wrong and the orchestrator must clarify before proposal. (See Open Questions.)
- **Missing smoke file**: `chronos-sandbox/tests/native_probe_tools.rs` does not exist. If the proposal adopts approach 1, the file is produced as part of apply. The smoke subset list must be edited to drop it from `notes[].smoke_subset` until then, or the proposal must commit to producing it in the first apply slice.
- **`store→native` leakage**: B3 forbids it. The `SessionArchive` / `CounterexampleRepository` ports must NOT re-introduce it. If the new ports' concrete impls need to call `chronos_native::*` for any reason, the proposal is wrong; abort per B8 and the C3.3.2 stop rule.
- **`SessionMetadata` field-shape migration**: 17 call-sites and 8 production files in `chronos-services` reference `chronos_store::SessionMetadata`. The `serde_json` shape (bincode for storage, JSON for wire) must roundtrip identically after the move. Test `session_persistence_extended.rs` and `save_load_roundtrip` are the ratchets.
- **C3.3.2 stop rule still binds**: any `dyn Any` / downcast / `chronos_store` DTO crossing the port is an abort condition. Proposal must explicitly state none of these appear in the new ports' signatures.
- **Replay integrity invariants**: `capture_session` reopens a session via `SessionArchive::load` — that path goes through `SegmentedExecutionLog::register_takeover` which goes through `ReplayPlan` validation. If the new tool skips that path, it violates C1.5.2. Proposal must explicitly route load through the existing takeover.
- **Probe advance / step semantics**: the existing `PtraceTracer` has both `cont` and `single_step`, but neither is exposed as a service-level method. If proposal adds `advance` / `step`, it must decide: (a) resume from a paused state (after `probe_inject` set a breakpoint), (b) single-step from current state. State management (`attached_target`, `running`) must be re-checked under HIGH-4 (`running.load(Ordering::SeqCst)` double-start guard).
- **Carry-forward debt**: `C31-DEBT-03` (services → ports inversion not finished) is still open. Tren B partially addresses it via `SessionArchive` + `CounterexampleRepository`. The remaining gap is `services → native` (only addressed by approach 4 above). Per B8, log the remaining gap as `no_action_in_current_cycle` and let REC-C3.4 own it.
- **`SessionRepository` inflation risk**: B2 forbids adding `save` / `load` to `SessionRepository`. Proposal must explicitly keep it lifecycle-only. The new `SessionArchive` is a **sibling** port, not a widening of `SessionRepository`.
- **Bootstrapping race in tests**: the `McpTestClient::start` fix in CIH-B resolves the binary via cargo metadata + lazy `cargo build`. New sandbox tests for `capture_session` / `probe_advance` / `probe_step` must inherit this harness (no inline `Command::new("cargo")` reinvented in the new test file).

## Open Questions

1. **`capture_session` was never added as a tool in Tren A. Is the operator's claim that "Tren A closed with capture_session finalized" referring to a different artifact (e.g., the capability-bundle composition root), or was the README aspirational and Tren B is actually the cycle that introduces it?**
2. **Should `probe_advance` and `probe_step` be wired through the existing `ProbeService` path (recommended, no new port) or through the unused `ProbeFactory` / `ProbeRegistry` ports (which closes `C31-DEBT-03` but is out-of-scope per B1 if introduced as a new port)?**
3. **Where does `SessionMetadata`'s `serde_json::Value` representation for the wire (currently produced in `session_explain.rs:118`, `session_export.rs:74`) live after the move — does the JSON shape stay identical, or does Tren B get the chance to drop the legacy fields?**
4. **The `StoreKind` enum in `chronos-store/src/storage.rs:67` is `pub(crate)`. After the `SessionMetadata` move, the `SessionArchive` port needs a way to expose whether the underlying store is persistent or in-memory (m9-82 disclosure). Does the port gain a `is_persistent()` method, or is it a separate `DegradedStoreDisclosure` capability port?**
5. **`SessionArchive::delete` semantics**: today `SessionsService::delete_session` refuses to delete a live session (m1.5 lifecycle-safe delete). Does the port take a `Force` flag, or does the refusal live at the service layer?**
6. **`CounterexampleRepository` filter shape**: `CounterexampleBundleFilter` is currently a `chronos_store::counterexample_storage` type. Does it move to `chronos_domain::ports::counterexample` as part of the port (and what derives does it gain), or does the port accept a `HashMap<String, Value>`-like filter?**
7. **Sandbox smoke file `native_probe_tools.rs` does not exist. Is producing it in the first apply slice acceptable, or should it be excluded from `notes[].smoke_subset` until a later slice?**
8. **Tren B's `apply-checkpoint.json::commits_since_base` is currently empty (skeleton only). Does the operator want the proposal phase to commit any planning artifacts (e.g., a `proposal.md` / `specification.md`) before any source code, or is the spec the first commit?**
9. **What is the test gate for the new tools? `program_scenarios.rs` already exercises `probe_start` / `probe_stop`. Does `native_probe_tools.rs` add tests for `capture_session`, `probe_advance`, `probe_step` against a long-running binary, or against a controlled fixture that exits deterministically?**

## Ready for Proposal

**Yes**, with the precondition that the orchestrator confirms with the operator that:

- (a) `capture_session` is correctly described as a **new** Tren B tool (not a finished Tren A artifact);
- (b) `probe_advance` and `probe_step` stay inside the existing `ProbeService` path (no `NativeProbeServicePort` — B1 is preserved);
- (c) `native_probe_tools.rs` is produced in the first apply slice (or removed from `notes[].smoke_subset`).

Approach 1 (single-shot `capture_session` + `probe_advance` / `probe_step` + `SessionArchive` + `CounterexampleRepository` + `SessionMetadata` move, no `NativeProbeServicePort`, composition-root split per B4) is the recommended proposal shape. The orchestrator should hand off to `sddk-propose` (MiniMax-M3) with this exploration report as the evidence base, then `sddk-spec` for the requirements / scenarios, then `sddk-tasks` for the slice decomposition.

## Standard Envelope (for `phase.explore.complete` ledger transition)

```yaml
status: passed
executive_summary: |
  Tren B (REC-C3.3.3) exploration confirms the locked rules B1..B9 and maps
  every affected area. Two new MCP tools (capture_session, probe_advance, probe_step)
  plus the SessionMetadata move to chronos-domain and the introduction of
  SessionArchive + CounterexampleRepository ports satisfy the operator scope.
  No NativeProbeServicePort is introduced. Composition-root split stays inside
  chronos-mcp::composition with bootstrap-scoped factories. REC-C1.5.2 strict
  replay surfaces are explicitly not touched. Three open questions require
  operator confirmation before sddk-propose.
artifacts:
  - path: cycle-artifacts/p-3416cfb8288f8964/rec-c3.3-train-b/exploration-report.md
    sha256: 2039e77d9f75cc3c17ff218cbc78f7d40a5798fcb31d4bd5689cbae61f1e2649
next_recommendation: sddk-propose
context_quality: C2
taxonomy:
  - dominant_axis: port_inversion_completion (B1, B2, B3, B4)
  - dominant_axis: tool_surface_expansion (capture / advance / step verbs)
  - dominant_axis: composition_root_growth (bootstrap vs session-scoped split)
  - dominant_axis: legacy_migration (SessionMetadata lift)
model_substitution_recorded: true
model_substitution_from: deepseek-chat
model_substitution_to: MiniMax-M3
model_substitution_reason: api.deepseek.com chat completions auth failed in this env
risks:
  - capture_session README/code mismatch (resolved: Tren B adds the tool)
  - missing native_probe_tools.rs sandbox file (created in apply slice)
  - store->native direction must remain unaddressed (B3 -> REC-C3.4)
  - C3.3.2 stop rule still binds: dyn Any / downcast / chronos_store DTO across the port is abort
  - replay integrity invariant: capture_session load path goes through existing takeover
```

## CLI Ledger Contract

Transition reference:

```
Transition:   phase.explore.complete
Matrix row:   lifecycle.cycle.transition.explore
Artifact:     cycle-artifacts/p-3416cfb8288f8964/rec-c3.3-train-b/exploration-report.md
On failure:   blocked — runtime remains OPEN/explore; do not retry from cache
```

The orchestrator runs:

1. `sddk cycle status --root . --scope . --cycle rec-c3.3-train-b --format json` (record phase).
2. Build `{evidence_json}` with report path/SHA-256 (after final write), inspected scope, context quality `C2`, unresolved gaps (9 open questions), and one result per exploration criterion (8 items + 8 affected-area categories).
3. `sddk cycle evaluate-gate --root . --scope . --cycle rec-c3.3-train-b --transition phase.explore.complete --gate exploration-sufficient --outcome passed --evaluator sddk.cli --evidence {evidence_json} --timestamp {now} --actor sddk --format json`
4. `sddk cycle transition --root . --scope . --cycle rec-c3.3-train-b --transition phase.explore.complete --artifact exploration-report={path} --gate-receipt {receipt_id} --lease-owner sddk-explore --fencing-token {token} --format json`
5. `sddk ledger verify --root . --scope . --format json`

The CLAUDE.md only runs the CLI ledger on **explicit** operator request (the framing prompt is read-only). If the orchestrator wants the transition recorded, run the above and confirm `gate: exploration-sufficient` passed before invoking `sddk-propose`.