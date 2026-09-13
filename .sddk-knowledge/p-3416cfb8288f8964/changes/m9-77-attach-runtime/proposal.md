# Proposal: m9-77 chronos_domain::attach — make session_start{action=attach} real

## Subject

- cycle: m9-77
- path: A-lite (Decision Model v2: C1, bounded, architectural fork on what `attach` does at the domain layer)
- branch: `feat/m9-77-attach-runtime`
- base: `1cb515b21871cde0de53d67c081208ab6e57caca` (post-m9-76 main)
- tag target: `v0.7.79`
- findings closed: `FIND-M7-04-ATTACH-DOMAIN-API-STUB` (re-opened through m9-backlog), opens as new explicit finding row `FIND-M9-77-ATTACH-PTRACE-OWNERSHIP-NOT-DECIDED` (low, see Residual risk)
- findings introduced (deferred): none
- new tests: 5 unit + 1 sandbox smoke replacement

## Why this is the next cycle

The v2 spec for `session_start` (`docs/chronos-agentic-reconstruction/docs/specs/AGENT_API_V2.md` line 11) names "start/attach and return `SessionId` + capability snapshot". The m7-04 scoping (`docs/milestones/m7-04-session-start-stop-capabilities-scoping.md`) decided the **wire shape** and rejected the **implementation**:

- `session_start{action=attach}` parameter set: `pid: u32` (no v1 analogue; net-new for m7-04)
- The dispatcher entry-point `ChronosSessionLifecycleService::attach(input)` exists at
  `crates/chronos-services/src/session_lifecycle.rs:181` and **returns
  `ServiceError::Unsupported("attach (m7+) — no domain-layer attach API yet")`**.
- The m7-close report (`docs/milestones/m7-close-report.md` §7) and the m8-close
  report (`docs/milestones/M8-CLOSE.md` §7) both list "implement
  `chronos_domain::attach`" as the first item on the m9+ backlog.
- This handoff (`m9-backlog-blocked-2026-09-12.md` Session 2026-09-13T17:20Z) has
  named m9-77 as the slot for this work in three sessions in a row.

The natural slot is now `m9-77`. This proposal is the **implementation** that
m7-04 deferred, not a redesign.

## What the cycle delivers

`chronos_domain::attach` becomes a real function on the same axis as the existing
`chronos_domain` primitives (`CaptureConfig`, `CaptureSession`, `Language`). The
v2 dispatcher wires to it. The existing stub returns
`ServiceError::Unsupported("attach (m7+)")` and is replaced.

Scope boundary, decided up front:

- **One backend** (native / Linux ptrace). eBPF attach is a separate concern
  (it is `probe_inject`'s job, not lifecycle's) and stays out.
- **Attach succeeds or fails closed**. The probe either reaches the running
  process and starts streaming events to the bus, or the call fails with
  `ServiceError::AttachFailed(reason)`. No `ServiceError::Unsupported` for the
  valid-pid case after this cycle.
- **One session_id per attach**. `start_probe_attach` returns a `CaptureSession`
  with a fresh uuid, and the dispatcher registers it under `live_probes` exactly
  like `start_probe` does, so the existing `probe_drain` / `probe_stop` /
  `session_stop` paths work unmodified.
- **Symbol resolution uses the running binary** read from `/proc/<pid>/exe`,
  mirroring the existing `SymbolResolver::from_pid` helper
  (`crates/chronos-native/src/symbol_resolver.rs:127`). Function-frame tracking,
  syscall tracing, and bus capacity mirror `start_probe`.
- **Linux-only** at the implementation level (`PtraceTracer::attach` already
  uses `nix::unistd::Pid::from_raw`). On non-Linux the call fails with a
  named error — no platform gate at the dispatcher layer.

## The architectural fork (and its decision)

`m7-04` defined the wire shape and pointed at "no domain-layer attach API yet".
The fork this cycle decides is **what the domain-layer attach API does at the
domain layer**. Three options considered, one chosen:

| Option | Where it lives | What it does | Verdict |
|---|---|---|---|
| A. Use existing `chronos_capture::TraceAdapter::attach_to_process` | `chronos-services` | Calls the trait; relies on the captured adapter | **rejected** — the trait is the language-adapter interface, not the live-probe streaming surface; `NativeAdapter::attach_to_process` returns a `CaptureSession` and exits, no event bus |
| B. Wire the existing `NativeProbeBackend::attach_probe` (the live-probe attach primitive at `probe_backend.rs:502`) into the dispatcher | `chronos-native` (already there) + `chronos-services` (new wire) | The primitive already exists, opens no ExecutionLog (deliberate: attach has no segmenting seam yet), spawns the existing `run_probe_loop_attach` thread, returns `CaptureSession`, marks `running=true` and records `traced_pid` for `stop_probe` to find | **chosen** — it is already 90% of what the dispatcher needs; the cycle adds two unit tests for it, the service wrapper, and the dispatcher arm |
| C. Add a new `start_probe_attach` mirroring `start_probe` exactly (including ExecutionLog) | `chronos-native` | Open ExecutionLog, spawn the existing `run_probe_loop_attach` thread, return `CaptureSession` | **rejected** — ExecutionLog is a m1-03 concern; the existing `attach_probe` deliberately does not open one because attach has no segmenting seam yet, and adding it here would expand scope without changing what the dispatcher can do |

Option B is the smallest change that reuses the existing primitive
(`run_probe_loop_attach` at `probe_backend.rs:887`), populates the
`LiveProbeSession` exactly like `start_probe` does, and surfaces the new
probe through the same `probe_drain` / `probe_stop` machinery. The
"domain-layer attach API" the m7-04 docs called out is therefore **already
present** — `attach_probe` exists since before m7 — but unreferenced; this
cycle is the wiring, not the primitive.

## API surface added

```rust
// crates/chronos-native/src/probe_backend.rs
//   attach_probe(pid, config) already exists at line 502; this cycle adds
//   two unit tests for it and a doc comment upgrade. NO new backend method.

impl NativeProbeBackend {
    /// Attach to an already-running process and stream events through
    /// the EventBus. Mirrors `start_probe` for the attach case; the
    /// `run_probe_loop_attach` background thread is the same one used
    /// internally by `attach_probe`.
    pub fn attach_probe(
        &self,
        pid: u32,
        config: CaptureConfig,
    ) -> Result<CaptureSession, TraceError> { /* already implemented */ }
}

// crates/chronos-services/src/probe.rs — net-new
#[derive(Debug, Clone)]
pub struct ProbeAttachInput {
    pub pid: u32,
    pub trace_syscalls: bool,
    pub bus_capacity: usize,
}

impl ProbeService {
    /// Live attach: resolve the running binary from /proc/<pid>/exe,
    /// call `NativeProbeBackend::attach_probe`, register the returned
    /// session under `ctx.live_probes`, and mark `ctx.active_session`.
    pub fn start_attach(
        ctx: &ProbeContext<'_>,
        input: ProbeAttachInput,
    ) -> Result<ProbeAttachOutput, ServiceError> { /* new */ }
}

#[derive(Debug)]
pub struct ProbeAttachOutput {
    pub session_id: String,
    pub pid: u32,
    pub target: String,
    pub language: String,
    pub bus_capacity: usize,
}
```

`ChronosSessionLifecycleService::attach` rewires from
`Err(ServiceError::Unsupported(...))` to:

1. Validate `pid` is non-zero (already done today).
2. Call `ProbeService::start_attach(ctx, ProbeAttachInput { pid, ... })`.
3. Build the same `CapabilitySnapshot` shape the spawn path produces.
4. Return a `SessionStartOutput` with `action=Attach`.

## Test plan

Five unit tests in `crates/chronos-services/src/session_lifecycle.rs` and
`crates/chronos-native/src/probe_backend.rs`:

1. `attach_to_zero_pid_returns_invalid_input` — dispatcher-level guard, kept
   unchanged.
2. `attach_to_known_pid_resolves_target_from_proc_exe` — service-level happy
   path against `/proc/self/exe` (the test binary's own pid); asserts
   `target` field is the resolved binary path, `language` is non-`Unknown`,
   `event_count: None`, `query_engine_ready: false`.
3. `attach_to_unknown_pid_returns_attach_failed` — pid 0xfffffffe (deliberately
   non-existent); asserts `ServiceError::AttachFailed(reason)` with the pid in
   the message.
4. `native_probe_backend_start_probe_attach_rejects_unparseable_config` — backend
   test, validates that a missing `/proc/<pid>/exe` is surfaced as
   `TraceError::CaptureFailed` (not a panic).
5. `native_probe_backend_start_probe_attach_sets_running_and_traced_pid` — backend
   test, asserts `running == true` and `traced_pid == Some(pid)` after success,
   `false` / `None` after the call returns the error.

One sandbox suite replacement:

- `chronos-sandbox/tests/session_lifecycle.rs::test_session_start_attach_returns_unsupported`
  is renamed to `test_session_start_attach_to_running_self` and asserts: the
  server does **not** return `Unsupported`, the `session_id` matches the
  documented field, and a follow-up `session_stop` on the returned session
  succeeds (proving the live session is registered in `live_probes`).

## Gates

- T0: `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` clean.
- T1: `-p chronos-services --lib` green; `-p chronos-native --lib --test-threads=1` green (the existing pre-existing ptrace flake does not move).
- T3: workspace lib minus sandbox/native/e2e green.
- T4-smoke: `e2e_connectivity` 1/1, `session_lifecycle` (the renamed attach test) passes, `store_open_failure` 2/2. `CHRONOS_MCP_PATH` set, binary rebuilt.
- Vault drift: PASS (no CC drift introduced); CC smoke 6/6.
- CC#12: `main_sha == head_sha == remote_tag_peel` for `v0.7.79`.

## Residual risk (deliberately recorded, not papered over)

- **PID ownership / permissions**: `PtraceTracer::attach` will fail with EPERM
  if the calling process does not have `CAP_SYS_PTRACE` or is not running as
  the same user (or as root). The dispatcher surfaces this as
  `ServiceError::AttachFailed("PTRACE_ATTACH failed: …")`. This is the
  expected failure mode, not a bug; the cycle does not introduce a soft
  fallback.
- **Detached-process semantics**: an attached probe's `session_stop` calls
  `PtraceTracer::detach` rather than killing the target. The target keeps
  running. This is the correct ptrace semantics and matches what `probe_stop`
  does for spawned sessions, but it differs from what a casual reader of
  `session_start{action=attach}` might assume. Documented in
  `docs/manual-ai/{en,es}/08-session-management.md` follow-up (one paragraph;
  not in scope for this cycle, filed as `FIND-M9-77-DETACHED-LIFECYCLE-DOCS`).
- **Symlink resolution**: `/proc/<pid>/exe` is read with `read_link`, which
  resolves through symlinks. If a pid has been re-execed, the resolved path is
  the current binary, which is the right thing. If the binary has been deleted
  on disk, `read_link` returns the deleted path; `start_probe_attach` will
  fail at `load_from_binary`. Same failure mode as `start_probe` for a missing
  binary; surfaced as `ServiceError::AttachFailed`.

## Findings / open follow-ups (after this cycle)

- `FIND-M9-77-DETACHED-LIFECYCLE-DOCS` (new, low): one-paragraph addition to
  `docs/manual-ai/{en,es}/08-session-management.md` clarifying that
  `session_stop` detaches the probe and the target keeps running.
- Existing inherited findings (unchanged): `FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE`,
  `FIND-M9-74-NATIVE-PTRACE-TESTS-NEED-SERIAL`,
  `FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION`,
  `FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION` (partially mitigated by m9-76).

## Cross-checks

- CC#1..CC#56: PASS, counts unchanged (no new CC introduced; CC#48 / CC#54
  meta-checks unchanged).
- CC#4: regenerated to fixpoint by the m9-76 tool — 76 → 77 manifests at the
  post-merge commit.
- CC#36 / CC#55 / CC#39: verify-report carries `Path` at the head and a
  `## Files Inventory` section; verify-findings carries `lens_summary` after
  `subject`; this manifest and `release-report.md` carry `## Cross-checks`.
- CC#33: `## Subject` is present in `change-entry.md`, `verify-report.md` and
  `verify-findings.json`.
- CC#12: `main_sha == head_sha == remote_tag_peel` for `v0.7.79` (filled in
  the post-release commit, because the tag points at the artifacts commit).
- No `docs/propuestas/` file touched; no test deleted or weakened.

## Files touched

- (modified) `crates/chronos-native/src/probe_backend.rs` — `start_probe_attach`
  + 2 backend unit tests
- (modified) `crates/chronos-services/src/probe.rs` — `ProbeAttachInput`,
  `ProbeAttachOutput`, `ProbeService::start_attach`
- (modified) `crates/chronos-services/src/session_lifecycle.rs` —
  `attach` body rewired, `Unsupported` arm removed, 2 unit tests rewritten
- (modified) `crates/chronos-mcp/src/server.rs` — `session_start` tool
  description no longer says "stub", error mapping gains an `AttachFailed`
  arm
- (modified) `chronos-sandbox/tests/session_lifecycle.rs` — one test renamed
  and tightened
- (modified) `AGENTS.md` — §3 records the ptrace-permissions caveat for
  attach, §7 lists the new tool surface
- (modified) `docs/manual-ai/en/08-session-management.md`,
  `docs/manual-ai/es/08-gestion-sesiones.md` — "Attach a session" section
  added (one paragraph; the deeper docs follow-up is `FIND-M9-77-DETACHED-LIFECYCLE-DOCS`)
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-77-attach-runtime/*`
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-77-attach-runtime/change-entry.md`
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-77-attach-runtime/archive-manifest.md`
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` (m9-77 row, Total cycles 76→77)
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` (one new deferred row)
