# Tasks: m9-77 chronos_domain::attach

> Five commit-sized tasks. Each is reviewable on its own and ends green.
> Falsification lives in `design.md` §7 and is replayed in §0.6 of each task.

## Task 1 — `chronos_domain::attach` primitive (`chronos-native`)

**Branch commit**: `feat(m9-77): native probe backend start_probe_attach`
**Files**: `crates/chronos-native/src/probe_backend.rs` (+~80 LoC, +2 tests)
**Size**: ~120 LoC diff

### 1.1 Add `start_probe_attach` to `NativeProbeBackend`

Mirror `start_probe` for the attach case:

- HIGH-4 guard (`running` already true → `CaptureFailed`).
- `PtraceConfig` with `capture_registers: true`, `follow_children: true`,
  `trace_syscalls: config.capture_syscalls`, `track_function_frames: track_function_frames`.
- Open ExecutionLog (same shape as `start_probe`).
- Set `running = true`, record `traced_pid = Some(pid)`, spawn the
  `run_probe_loop_attach` thread (already implemented at
  `probe_backend.rs:887`).
- Return `CaptureSession::new(pid, language, config)`.

### 1.2 Two unit tests (`#[cfg(target_os = "linux")]`)

- `start_probe_attach_to_self_sets_running_and_traced_pid`
- `start_probe_attach_to_unknown_pid_returns_capture_failed`

### 1.3 Falsification (replayed from `design.md` §7)

- Disable HIGH-4 guard → second call test fails (the backend accepts a
  second attach).
- Skip `traced_pid` set → `stop_probe` cannot kill the process; test fails
  with a `kill: ESRCH` race; restore and re-run green.

### 1.4 Gate

`cargo test -p chronos-native --lib --test-threads=1` green; existing
`test_launch_with_syscall_tracing` flake does not move.

## Task 2 — Service-layer wire (`chronos-services::probe`)

**Branch commit**: `feat(m9-77): probe service start_attach`
**Files**: `crates/chronos-services/src/probe.rs` (+~80 LoC, +0 tests at
this layer), `crates/chronos-services/src/error.rs` (+5 LoC).
**Size**: ~85 LoC diff

### 2.1 `ServiceError::AttachFailed(String)` variant

```rust
#[error("attach failed: {0}")]
AttachFailed(String),
```

### 2.2 `ProbeAttachInput`, `ProbeAttachOutput`, `ProbeService::start_attach`

### 2.3 `resolve_pid_target(pid: u32) -> Result<String, String>`

Linux: `read_link("/proc/<pid>/exe")`. Non-Linux: error string.

### 2.4 Gate

`cargo check -p chronos-services --all-targets` green (compile-time).

## Task 3 — Dispatcher rewiring (`chronos-services::session_lifecycle`)

**Branch commit**: `feat(m9-77): session lifecycle attach wired to probe service`
**Files**: `crates/chronos-services/src/session_lifecycle.rs` (rewrite of
`Self::attach` and three tests, ~80 LoC net).
**Size**: ~80 LoC diff

### 3.1 Rewrite `attach`

`Self::attach` becomes `async fn attach(ctx, input)`. Validate `pid` (kept),
call `ProbeService::start_attach`, build `CapabilitySnapshot`, return
`SessionStartOutput { action: Attach, … }`.

### 3.2 Update `start` dispatcher

`SessionStartAction::Attach => Self::attach(ctx, input).await`.

### 3.3 Three unit tests

- `start_attach_without_pid_returns_invalid_input` — kept.
- `start_attach_to_running_self_returns_capability_snapshot` — replaces
  `start_attach_with_pid_returns_unsupported`; happy-path against
  `std::process::id()`.
- `start_attach_to_unknown_pid_returns_attach_failed` — new; pid
  `0xfffffffe`.

### 3.4 Falsification (replayed)

- Re-introduce the `Unsupported` return → `start_attach_to_running_self`
  test fails with the `Unsupported` arm.
- Skip the `live_probes` insert → sandbox test fails on follow-up
  `session_stop` (covered in Task 5).

### 3.5 Gate

`cargo test -p chronos-services --lib` green; `-p chronos-services --tests`
green.

## Task 4 — MCP wire (`chronos-mcp::server`)

**Branch commit**: `feat(m9-77): mcp session_start attach error mapping`
**Files**: `crates/chronos-mcp/src/server.rs` (~10 LoC), tool description
text.
**Size**: ~10 LoC diff

### 4.1 Add `Err(ServiceError::AttachFailed(msg))` arm

Place between `LoadFailed` and the catch-all `Err(other)`.

### 4.2 Update tool description

Drop "currently a stub (m7+)" from the `session_start` tool description.

### 4.3 Gate

`cargo check -p chronos-mcp --all-targets` green.

## Task 5 — Sandbox test rewiring + docs + AGENTS

**Branch commit**: `feat(m9-77): sandbox test for session_start attach`
**Files**: `chronos-sandbox/tests/session_lifecycle.rs` (1 test rewritten),
`AGENTS.md` (§3 caveat + §7 vault commands), `docs/manual-ai/{en,es}/08-*.md`
(one paragraph each).
**Size**: ~50 LoC diff

### 5.1 Rewrite `test_session_start_attach_returns_unsupported` → `test_session_start_attach_to_running_self`

Asserts: success response, `session_id` non-empty, follow-up `session_stop`
succeeds.

### 5.2 `AGENTS.md`

- §3 (Test topology): add a paragraph under "Bucket C — chronos-sandbox"
  noting that the attach path requires Linux ptrace permissions and that
  the test attaches to `std::process::id()` (always same uid).
- §7 (Quick reference): add a one-liner to the `cargo test -p
  chronos-sandbox` block reminding to set `CHRONOS_MCP_PATH`.

### 5.3 Manual-ai docs

- `en/08-session-management.md`: add "## Attach a session" section
  (`session_start{action=attach, pid=<pid>}`; ptrace permission caveat;
  follow-up `session_stop` detaches; target keeps running).
- `es/08-gestion-sesiones.md`: same section in Spanish.

### 5.4 Gate

`cargo test -p chronos-sandbox --test session_lifecycle
--test-threads=1` with `CHRONOS_MCP_PATH` set, green.

## Ordering

Tasks 1–4 are sequential: each builds on the prior (Task 1 → Task 2 → Task 3
→ Task 4). Task 5 is the final wiring and docs commit; it can be folded into
Task 4's commit if a reviewer prefers fewer commits, but the default is five
commits matching the proposal's "Files touched" list.

## Falsification schedule

After Task 1, replay the design's mutation table for the backend. After
Task 3, replay for the dispatcher (most regressions will land here). After
Task 5, run the full sandbox smoke (`session_lifecycle` + `e2e_connectivity`
+ `store_open_failure`) and confirm no false positives.

## Out-of-scope follow-ups (recorded for m9-78+)

- `FIND-M9-77-DETACHED-LIFECYCLE-DOCS` — docs follow-up if reviewer wants
  more than one paragraph; in current scope as Task 5.3.
- `probe_type = "ptrace_attach"` for the capability snapshot — separate
  cycle that touches `CapabilitySnapshot` schema.
- `bus_capacity` parameter on the attach wire — separate cycle that bumps
  `SessionStartInput`.
- eBPF attach as a lifecycle entry-point — separate cycle that wires
  `chronos_ebpf::attach_uprobe` into the dispatcher.
