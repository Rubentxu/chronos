# Proposal: REC-C3.3.3 (Tren B) — native probe MCP tool surface (capture / advance / step) + service-port expansion

> Cycle: `p-3416cfb8288f8964/rec-c3.3-train-b`
> Path: **A-min** (operator-set 2026-09-18; route unchanged from explore)
> Phase: **Propose**
> Author: sddk-propose (MiniMax-M3)
> Updated: 2026-09-19T11:22Z
> Base SHA: `fa5eb582` · Current HEAD: `9a68e9f1`
> Evidence base: `cycle-artifacts/p-3416cfb8288f8964/rec-c3.3-train-b/exploration-report.md` (sha256 `0aaae8777ff0ad6e90b9c42b9d09eda4d450b2b4a7c5fcfec334bc36b920b9b1`)

## Intent

Close the remaining gap between what AI agents can do today through the chronos
MCP surface (capture a probe, query it, extract counterexamples) and what
operators expect from a "drive the native probe" workflow: single-shot capture
orchestration, advance past a paused state, and single-step the target. At the
same time, finish the first leg of `C31-DEBT-03` by introducing the
`SessionArchive` and `CounterexampleRepository` service ports (so
`chronos-services` stops calling `chronos_store::SessionStore::*` directly for
content operations) and lifting `SessionMetadata` from `chronos-store` to
`chronos-domain` (mirroring the C3.3.1 `SessionId` lift). The
`store→native` direction remains out of scope (B3 → REC-C3.4).

This is **Tren B** of the REC-C3.3 milestone: Tren A (REC-C3.3.2) closed with
composition-root integration and did **not** add `capture_session` (zero hits
in `crates/` — the README/code mismatch is resolved here: Tren B introduces it).

## Scope

### In Scope

- 3 new MCP tools in `crates/chronos-mcp/src/server.rs`:
  - `capture_session` — single-shot orchestrator: `probe_start` → wait for
    exit / stop condition → `probe_stop` → `save_session`, persisted via the
    new `Arc<dyn SessionArchive>` port.
  - `probe_advance` — continue the target past its paused state, delegating to
    `NativeProbeBackend::advance` → `PtraceTracer::continue_execution`
    (already at `ptrace_tracer.rs:607`).
  - `probe_step` — single-step the target, delegating to
    `NativeProbeBackend::step` → `PtraceTracer::step` (already at
    `ptrace_tracer.rs:619`).
- 2 new ports in `chronos-domain::ports`:
  - `SessionArchive` — sibling of `SessionRepository` (lifecycle stays readonly).
    `save(metadata, events)`, `load(id) -> (SessionMetadata, Vec<TraceEvent>)`,
    `list() -> Vec<SessionMetadata>`, `delete(id)`, `count_events(id)`.
    Concrete impl wraps `chronos_store::SessionStore::{save_session,
    load_session, list_sessions, delete_session}`.
  - `CounterexampleRepository` — `save_bundle(bundle_id, events)`,
    `load_bundle(bundle_id)`, `list_bundles(filter)`,
    `count_bundle_events(bundle_id)`. Concrete impl wraps
    `chronos_store::SessionStore::{save_counterexample_bundle,
    load_counterexample_bundle, list_counterexample_bundles,
    count_counterexample_bundle_events,
    load_counterexample_bundle_events}`. `CounterexampleBundleFilter` moves
    here too.
- 1 mechanical lift: `SessionMetadata` (`chronos-store/src/storage.rs:22`)
  → `chronos-domain::session::SessionMetadata` (mirrors C3.3.1 `SessionId`
  lift). `chronos-store` re-exports via `pub use`. 17 call-sites in
  `chronos-services` swap imports.
- 1 composition-root extension in `crates/chronos-mcp/src/composition.rs`:
  - Bootstrap-scoped: `default_session_archive()`,
    `default_counterexample_repository()`, and `in_memory_*` test variants
    (parallel `InMemorySessionRepository` pattern). Underlying
    `SessionStore` is the process-wide singleton owned by `ChronosServer`.
  - Session-scoped: **none new**. `probe_advance` / `probe_step` go through
    the existing `ProbeService`, which already receives `&ProbeContext<'_>`
    per call (B4 satisfied; no new session-scoped factory needed).
- 1 new sandbox test file: `chronos-sandbox/tests/native_probe_tools.rs`
  (created in the first apply slice — Slice B — so the existing
  `apply-checkpoint.smoke_subset` is honest).
- 1 new `NativeProbeBackend` method pair: `advance(&self, session) -> Result<()>`,
  `step(&self, session) -> Result<()>` (both delegate to existing
  `PtraceTracer::continue_execution` / `PtraceTracer::step`).
- 1 new `ProbeService` method pair: `advance(session_id)`,
  `step(session_id)` (no new port — B1).
- 8 service files rewire `chronos_store::SessionStore::save_counterexample_bundle`
  → `Arc<dyn CounterexampleRepository>` (no behavior change).

### Out of Scope

- `store → native` direction (REC-C3.4 owns it; B3).
- New `NativeProbeServicePort` (B1). `services → native` edge stays where
  it is today via `LiveProbeSession` + `ProbeContext`.
- Wiring `ProbeFactory` / `ProbeRegistry` ports (declared but unused) — out
  of A-min budget; logged as `no_action_in_current_cycle` per B8.
- Production schema changes unrelated to native-probe capture.
- REC-C1.5.2 strict replay surfaces — untouched (B6).
- Production behavior of `probe_start` / `probe_stop` / `session_start` /
  `session_stop` / `save_session` / `load_session` — untouched; we add new
  tools, we do not re-tune existing ones.

## Capabilities

> CONTRACT with `sddk-spec`. Existing capability names researched in
> `.sddk-knowledge/p-3416cfb8288f8964/changes/` (no prior
> `SessionArchive`, `CounterexampleRepository`, `capture_session`,
> `probe_advance`, `probe_step`, or `SessionMetadata` capability exists;
> all are **new**).

### New Capabilities

- `session-archive-port`: domain-side `SessionArchive` port +
  `InMemorySessionArchive` test fake; persists `SessionMetadata` + event
  stream for save/load/list/delete/count; siblings `SessionRepository`
  (lifecycle stays readonly).
- `counterexample-repository-port`: domain-side `CounterexampleRepository`
  port + `InMemoryCounterexampleRepository` test fake + lift of
  `CounterexampleBundleFilter` from `chronos-store`.
- `session-metadata-domain`: lift `SessionMetadata` from
  `chronos-store/src/storage.rs:22` to `chronos-domain::session` (mirrors
  C3.3.1 `SessionId` lift); 17 call-sites swap imports.
- `native-probe-advance-step`: new `NativeProbeBackend::advance` /
  `NativeProbeBackend::step` + `ProbeService::advance` / `ProbeService::step`;
  no new port (B1).
- `mcp-tool-capture-session`: new MCP tool `capture_session` that
  composes `probe_start` → wait → `probe_stop` → `save_session` through
  `Arc<dyn SessionArchive>`; honors B6 replay integrity via the existing
  takeover path.
- `mcp-tool-probe-advance`: new MCP tool `probe_advance` that calls
  `ProbeService::advance(session_id)` → `NativeProbeBackend::advance` →
  `PtraceTracer::continue_execution`.
- `mcp-tool-probe-step`: new MCP tool `probe_step` that calls
  `ProbeService::step(session_id)` → `NativeProbeBackend::step` →
  `PtraceTracer::step`.
- `mcp-tool-session-counterexample-rewrite`: existing
  `save_session` / `load_session` / `list_sessions` / `delete_session` /
  `counterexample_*` tools now route through `Arc<dyn SessionArchive>` and
  `Arc<dyn CounterexampleRepository>` (composition-root injected).
- `composition-root-default-archive-factories`: bootstrap-scoped
  `default_session_archive()` / `default_counterexample_repository()` in
  `chronos-mcp::composition`, with `in_memory_*` test variants.
- `sandbox-native-probe-tools`: new `chronos-sandbox/tests/native_probe_tools.rs`
  exercising `capture_session`, `probe_advance`, `probe_step` against the
  live `chronos-mcp` binary via the CIH-B `McpTestClient::start` harness.

### Modified Capabilities

- `chronos-domain::SessionRepository`: unchanged in shape (still
  lifecycle/registry only); explicitly reaffirmed as **not** the home for
  `save` / `load`. The new `SessionArchive` is the sibling port. (No
  delta spec — explicit reaffirmation in this proposal only.)

## Approach

Approach **1** from the exploration report (single-shot `capture_session`
plus `probe_advance` / `probe_step`, no factory port). All three new MCP
tools land in `crates/chronos-mcp/src/server.rs`; all persistence routes
through the new ports; the native ABI gains two methods on
`NativeProbeBackend` that delegate to existing `PtraceTracer` calls.

The `SessionMetadata` lift is mechanical: copy the type definition verbatim,
delete it from `chronos-store`, add `pub use chronos_domain::SessionMetadata`
in `chronos-store` (mirrors the C3.3.1 `SessionId` pattern at
`rec-c3-3-1-identity-storage-seam/specification.md` §D1). 17 imports swap.
Wire format (bincode for storage, JSON for wire) roundtrips identically
because the type definition is byte-for-byte the same.

Composition-root split (B4): bootstrap-scoped factories wrap the existing
`SessionStore` (already process-scoped via `ChronosServer`). Session-scoped
factories are **not** added — `advance` / `step` are per-call operations
that flow through `&ProbeContext<'_>` on the existing `ProbeService`, which
is already invoked per MCP request.

Replay integrity (B6): `capture_session`'s persistence step uses the new
`SessionArchive::save`, which wraps `SessionStore::save_session`; load goes
through the existing `register_takeover` path that REC-C1.5.2 production
code uses. No change to `ReplayPlan` validation.

C3.3.2 stop rule still binds: **no** `dyn Any`, downcast, `chronos_store`
helper, or `chronos_store` DTO crosses the new ports. `SessionArchive` and
`CounterexampleRepository` only take domain-side types (`SessionMetadata`,
`Vec<TraceEvent>`, `CounterexampleBundleFilter`).

## Quality Intent

- **Production surfaces**:
  - `crates/chronos-mcp/src/server.rs` (3 new tool entries + rewiring of
    5 session tools + 5 counterexample tools).
  - `crates/chronos-mcp/src/composition.rs` (4 new factory functions +
    4-6 new in-module tests).
  - `crates/chronos-domain/src/ports/session.rs` (lifted `SessionMetadata`
    + new `SessionArchive` port + `InMemorySessionArchive`).
  - `crates/chronos-domain/src/ports/counterexample.rs` (new file:
    `CounterexampleRepository` port + `InMemoryCounterexampleRepository` +
    `CounterexampleBundleFilter`).
  - `crates/chronos-domain/src/ports/mod.rs` (`pub use` of the new
    `counterexample` module).
  - `crates/chronos-domain/src/lib.rs` (re-export `SessionMetadata`).
  - `crates/chronos-store/src/storage.rs` (`SessionMetadata` deletion +
    `pub use` re-export).
  - `crates/chronos-services/src/{session_explain,session_export,diff,
    output,sessions,session_compare,session_lifecycle,counterexample}.rs`
    (8 production files: import swaps + repository rewiring).
  - `crates/chronos-native/src/probe_backend.rs` (new `advance` / `step`
    methods).
  - `crates/chronos-services/src/probe.rs` (new `ProbeService::advance` /
    `step`).
  - `chronos-sandbox/tests/native_probe_tools.rs` (new file).
- **Changed public APIs**: `chronos-domain::SessionMetadata` (lifted) is
  identity-equivalent (same fields, same derives) — `chronos-store` re-exports
  via `pub use` for back-compat at the dependency boundary. Net new public
  surface: `chronos-domain::SessionArchive`,
  `chronos-domain::CounterexampleRepository`,
  `chronos-domain::ports::counterexample::CounterexampleBundleFilter`,
  `NativeProbeBackend::advance` / `step`, `ProbeService::advance` / `step`,
  and the 3 MCP tools.
- **Readiness dimensions**:
  - **Type-level ratchet**: 17 call-sites compile after import swap.
  - **Wire-format ratchet**: bincode (storage) and JSON (wire) roundtrip
    identical (verified by `session_persistence_extended.rs` /
    `save_load_roundtrip`).
  - **Replay-integrity ratchet**: `capture_session` load path goes through
    existing `register_takeover`; `ReplayIntegrityError` variants unchanged.
  - **Sandbox ratchet**: `native_probe_tools.rs` exercises the live
    `chronos-mcp` binary against `capture_session` / `probe_advance` /
    `probe_step`.
  - **Composition-root ratchet**: `composition_tests` module gains 4-6
    cases covering `default_session_archive` / `default_counterexample_repository`
    + in-memory variants.
- **Required real boundaries**:
  - Live `chronos-mcp` binary (the only place the MCP tools surface).
  - Real `SessionStore` roundtrip (in-memory is a fake; production goes
    through redb-backed `SessionStore`).
  - Real ptrace-traced target (sandbox test needs a controllable long-running
    binary; reuse the m9-77 fixture pattern).
  - `McpTestClient::start` (CIH-B harness) — no inline
    `Command::new("cargo")` reinvented.

## Architecture Impact

- **Level**: **boundary** (chronos-domain gains two new ports and a lifted
  type; chronos-services rewires 8 files to the new ports; chronos-mcp
  adds 4 composition factories; chronos-native gains 2 backend methods).
- **Evidence**:
  - `crates/chronos-domain/src/ports/` (10 ports today; gains 2 new ports
    + 1 lifted type).
  - `crates/chronos-services/src/` (8 files rewire to the new ports).
  - `crates/chronos-mcp/src/composition.rs` (243 lines today; gains 4
    factories + 4-6 tests).
  - `crates/chronos-store/src/storage.rs:22` (`SessionMetadata` lift site;
    mirrors the C3.3.1 `SessionId` lift at `rec-c3-3-1-identity-storage-seam/
    specification.md` §D1).
  - `crates/chronos-native/src/probe_backend.rs` (new
    `advance` / `step`; delegates to `PtraceTracer::{continue_execution,
    step}` at `ptrace_tracer.rs:607` and `:619`).
- **Architecture intent**: `cycle-artifacts/p-3416cfb8288f8964/
  rec-c3.3-train-b/exploration-report.md` §Affected Areas + §Approaches §1;
  this proposal is the **planned** side of the boundary intent.
  `sddk-c4-likec4` render is **deferred** to the design phase (the propose
  phase notes that the renderer absence uses its fallback and does not
  invent evidence — there is no LikeC4 `.c4` file yet).

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/chronos-mcp/src/server.rs` | Modified | Add `capture_session`, `probe_advance`, `probe_step` (3 new `#[tool]` handlers). Rewire `save_session` / `load_session` / `list_sessions` / `delete_session` through `Arc<dyn SessionArchive>`. Rewire `counterexample_shrink` / `counterexample_get` / `counterexample_list` / `counterexample_events_count` / `counterexample_bundle_events` through `Arc<dyn CounterexampleRepository>`. |
| `crates/chronos-mcp/src/composition.rs` | Modified | Add `default_session_archive()` / `default_counterexample_repository()` (bootstrap-scoped) + `in_memory_session_archive()` / `in_memory_counterexample_repository()` test variants. 4-6 new in-module tests in `composition_tests`. |
| `crates/chronos-domain/src/ports/session.rs` | Modified | Lift `SessionMetadata` from `chronos-store`. Add `SessionArchive` port (object-safe, sync, sibling of `SessionRepository`). Add `InMemorySessionArchive` test fake. |
| `crates/chronos-domain/src/ports/counterexample.rs` | New | New module: `CounterexampleRepository` port + `InMemoryCounterexampleRepository` + `CounterexampleBundleFilter` + `CounterexampleBundleSummary` + `CounterexampleBundleRecord`. |
| `crates/chronos-domain/src/ports/mod.rs` | Modified | `pub use counterexample;` (parallel to `pub use browser_probe;` etc.). |
| `crates/chronos-domain/src/lib.rs` | Modified | Re-export `SessionMetadata` from the crate root. |
| `crates/chronos-store/src/storage.rs` | Modified | Delete `SessionMetadata` (line 22). Replace with `pub use chronos_domain::SessionMetadata;` (mirrors C3.3.1 `SessionId` lift). |
| `crates/chronos-store/src/lib.rs` | Modified | Re-export the lifted `SessionMetadata`. |
| `crates/chronos-services/src/session_explain.rs` | Modified | `use chronos_domain::SessionMetadata;` (was `chronos_store::SessionMetadata`). |
| `crates/chronos-services/src/session_export.rs` | Modified | Same import swap. |
| `crates/chronos-services/src/diff.rs` | Modified | Same import swap. |
| `crates/chronos-services/src/output.rs` | Modified | Same import swap. |
| `crates/chronos-services/src/sessions.rs` | Modified | Same import swap; route through `Arc<dyn SessionArchive>` for `save_session` / `load_session` / `list_sessions` / `delete_session`. |
| `crates/chronos-services/src/session_compare.rs` | Modified | Same import swap. |
| `crates/chronos-services/src/session_lifecycle.rs` | Modified | Same import swap. |
| `crates/chronos-services/src/counterexample.rs` | Modified | Same import swap; route through `Arc<dyn CounterexampleRepository>` for all 5 counterexample operations. |
| `crates/chronos-services/src/probe.rs` | Modified | Add `ProbeService::advance(session_id)` and `ProbeService::step(session_id)` (per-call; no new port). |
| `crates/chronos-native/src/probe_backend.rs` | Modified | Add `advance(&self, session)` and `step(&self, session)` on `NativeProbeBackend`; delegate to `PtraceTracer::continue_execution` / `PtraceTracer::step`. |
| `chronos-sandbox/tests/native_probe_tools.rs` | New | Sandbox file exercising `capture_session` / `probe_advance` / `probe_step` against the live `chronos-mcp` binary. Uses CIH-B `McpTestClient::start` harness. First-apply-slice deliverable. |
| `chronos-sandbox/tests/session_persistence.rs` | Modified | No behavioral change; assertions read metadata via the `SessionArchive` port (reconciles field identity after the `SessionMetadata` move). |
| `chronos-sandbox/tests/session_lifecycle.rs` | Modified | Same — assertions read metadata via the port. |
| `chronos-sandbox/tests/counterexample_tools.rs` | Modified | Same — assertions read counterexample bundles via the port. |
| `cycle-artifacts/p-3416cfb8288f8964/rec-c3.3-train-b/proposal.md` | New | This file. |
| `cycle-artifacts/p-3416cfb8288f8964/rec-c3.3-train-b/specification.md` | New | Next phase — `sddk-spec` writes the requirements / scenarios. |
| `cycle-artifacts/p-3416cfb8288f8964/rec-c3.3-train-b/task-graph.json` | New | Next phase — `sddk-tasks` writes the slice decomposition. |
| `cycle-artifacts/p-3416cfb8288f8964/rec-c3.3-train-b/apply-checkpoint.json` | Modified | Add `phase.propose.complete` ledger entry after this phase closes. |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| `dyn Any` / downcast / `chronos_store` DTO creeps across `SessionArchive` or `CounterexampleRepository` (C3.3.2 stop rule) | Low | Port trait signatures reviewed at apply slice boundaries; abort and replan per C3.3.2 stop rule if any leakage appears. |
| `SessionMetadata` wire-format drift after the lift | Low | Same field order, same derives, same bincode / JSON shape (byte-for-byte copy). Ratchets: `session_persistence_extended.rs::save_load_roundtrip` and the m7-04 `tail_sealed` / `sealed_at` field identity checks. |
| `probe_advance` / `probe_step` race with `probe_start` (HIGH-4 `running.load(SeqCst)` double-start guard) | Low | `ProbeService::advance` / `step` consult `live_probes` + `attached_target` / `running` state under the same guard pattern as `ProbeService::start`; re-checked at apply slice boundaries. |
| `capture_session` skips the `register_takeover` path (B6 violation) | Low | Implementation explicitly routes load through the existing `SegmentedExecutionLog::register_takeover`; verified by a sandbox test that reopens a sealed session and asserts `ReplayIntegrityError` variants are unchanged. |
| `composition_tests` module becomes the only test for the new ports (no real-binary smoke) | Med | `native_probe_tools.rs` exercises the live binary for all 3 new tools; `composition_tests` covers the factory wiring only. |
| `chronos-store` retains a circular dependency on `chronos-domain` for `SessionMetadata` (mirror of the C3.3.1 `SessionId` re-export) | Low | Re-export only — `chronos-store::SessionMetadata` is `pub use chronos_domain::SessionMetadata`. No newtype, no wrapper. Same pattern as `chronos-log::SessionId`. |
| Smoke subset list contains `native_probe_tools` before the file exists | Med | `native_probe_tools.rs` is a first-apply-slice deliverable (Slice B); the file lands before the first T4-smoke run. apply-checkpoint `notes[].smoke_subset` is honest from commit one. |
| Carry-forward debt `C31-DEBT-03` (services → ports inversion not finished) remains open after Tren B | Confirmed | Partially addressed (SessionArchive + CounterexampleRepository). The remaining `services → native` edge is logged as `no_action_in_current_cycle` per B8 and owned by REC-C3.4. |
| `ProbeFactory` / `ProbeRegistry` declared-but-unused ports stay unused | Confirmed | Out of A-min budget; logged as `no_action_in_current_cycle` per B8. Wiring them is a future cycle if a port is needed. |

## Rollback Plan

Each slice is independently revertable because the proposal introduces
additive surfaces only:

1. **`SessionMetadata` lift**: revert by re-adding the deleted struct in
   `chronos-store/src/storage.rs` and removing the `pub use` re-export.
   Call-site imports swap back to `use chronos_store::SessionMetadata;`.
   The lift is mechanical; reverting is also mechanical. No wire-format
   drift because the type is byte-for-byte identical.
2. **`SessionArchive` + `CounterexampleRepository` ports**: revert by
   re-pointing `chronos-services` at `chronos_store::SessionStore::*`
   directly (the original call sites still exist after the rewire; we
   only swap the abstraction). The new ports and their `InMemory*` fakes
   become unused but do not break compilation because they are
   self-contained new modules.
3. **`capture_session` / `probe_advance` / `probe_step` MCP tools**:
   revert by removing the 3 `#[tool]` handlers from
   `crates/chronos-mcp/src/server.rs`. The supporting
   `ProbeService::{advance,step}` and `NativeProbeBackend::{advance,step}`
   stay (they are additive and do not break anything).
4. **Composition-root factories**: revert by removing the 4 new functions
   from `crates/chronos-mcp/src/composition.rs`. `ChronosServer::try_new`
   keeps its existing signature.
5. **`native_probe_tools.rs`**: revert by deleting the file. Smoke subset
   in `apply-checkpoint.notes[].smoke_subset` reverts to
   `e2e_connectivity + analytics_tools + program_scenarios`.

No `--no-fail-fast` / `#[ignore]` / `--skip` introduced (B5). No schema
migration runs on persisted data; no version bump. Branch `feat/
rec-c3.3-train-b` is reverted by `git revert` of the slice commits (B7:
no squash, no rebase).

## Dependencies

- `PtraceTracer::continue_execution` (`crates/chronos-native/src/
  ptrace_tracer.rs:607`) and `PtraceTracer::step` (`:619`) — already exist;
  no native ABI churn.
- `SessionStore::save_session` / `load_session` / `list_sessions` /
  `delete_session` (`crates/chronos-store/src/storage.rs`) — already exist;
  `SessionArchive` impl wraps them.
- `SessionStore::save_counterexample_bundle` / `load_counterexample_bundle` /
  `list_counterexample_bundles` / `count_counterexample_bundle_events` /
  `load_counterexample_bundle_events` (`crates/chronos-store/src/ce_write.rs:34`,
  `ce_read.rs:65`, `:100`, `:127`, `:205`) — already exist;
  `CounterexampleRepository` impl wraps them.
- `InMemorySessionRepository` pattern (`crates/chronos-domain/src/ports/
  session.rs`) — already exists; `InMemorySessionArchive` /
  `InMemoryCounterexampleRepository` follow the same shape.
- `McpTestClient::start` (`crates/chronos-sandbox/src/client/tools.rs`) —
  CIH-B binary resolution is in place; `native_probe_tools.rs` inherits
  the harness.
- `reconstruction-contracts.toml::active_gate = REC-C3` — already set.
- `chronos-mcp::composition` (B4 composition root) — already established
  by Tren A; this cycle extends it, does not relocate it.
- `chronos-domain::ports` — already the canonical home for service ports;
  this cycle adds 2 new ports and lifts 1 type.

## Success Criteria

- [ ] `capture_session`, `probe_advance`, `probe_step` are present in
      `crates/chronos-mcp/src/server.rs` (verified by
      `grep -n "name = \"capture_session\"\|name = \"probe_advance\"\|name = \"probe_step\""`).
- [ ] `NativeProbeBackend::advance` and `NativeProbeBackend::step` exist
      and delegate to `PtraceTracer::continue_execution` /
      `PtraceTracer::step`.
- [ ] `ProbeService::advance(session_id)` and `ProbeService::step(session_id)`
      exist; route through `&ProbeContext<'_>` per call (no new port).
- [ ] `chronos-domain::ports::counterexample` module exists with
      `CounterexampleRepository`, `InMemoryCounterexampleRepository`,
      `CounterexampleBundleFilter`.
- [ ] `chronos-domain::SessionMetadata` exists and matches
      `chronos-store::SessionMetadata` field-for-field (re-exported).
- [ ] All 17 `chronos-services` call-sites import `SessionMetadata` from
      `chronos_domain` (verified by
      `grep -rn "chronos_store::SessionMetadata" crates/chronos-services`).
- [ ] `chronos-mcp::composition` has `default_session_archive`,
      `default_counterexample_repository`, `in_memory_session_archive`,
      `in_memory_counterexample_repository`; 4-6 new in-module tests pass.
- [ ] `chronos-sandbox/tests/native_probe_tools.rs` exists and exercises
      the 3 new tools against the live `chronos-mcp` binary.
- [ ] **T0** (`cargo fmt --all -- --check && cargo clippy --workspace
      --all-targets -- -D warnings`) green.
- [ ] **T1** (`cargo test --workspace --lib --no-fail-fast`) green — 634
      unit tests + any new ones.
- [ ] **T4-smoke** (`chronos-sandbox --test e2e_connectivity +
      analytics_tools + program_scenarios + native_probe_tools`) green
      against the new binary.
- [ ] `cargo test --workspace --tests --exclude chronos-sandbox
      --exclude chronos-e2e` green.
- [ ] No `#[ignore]`, no `--skip`, no permanent waivers (B5, B8).
- [ ] REC-C1.5.2 strict replay surfaces untouched (`chronos-log/src/
      replay.rs` `git diff` is empty for Tren B; B6).
- [ ] `apply-checkpoint.json::commits_since_base` is non-empty at close
      (skeleton-only at proposal phase is the starting state, not the
      closing state).
- [ ] `findings_introduced.no_action` logs the remaining
      `services → native` edge as `no_action_in_current_cycle` for REC-C3.4
      (B8).
- [ ] B7 preserved: full SHA chain, no squash, no rebase, no merge commit.
- [ ] B9 preserved: comments match what the code verifies (e.g., the
      `SessionMetadata` lift comment explicitly states "same field shape,
      same derives, re-export only").
- [ ] `sddk artifact store` returns a receipt whose `content_digest`
      equals the proposal SHA-256 (CAS contract per
      `prompts/sddk/phases/propose.md` §Artifact Contract).
- [ ] `sddk ledger verify` returns `ok` after the `phase.propose.complete`
      transition (orchestrator-run; this phase writes the entry into
      `apply-checkpoint.json::ledger_progression` so the orchestrator
      can run the transition).