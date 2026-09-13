# Spec: m9-77 chronos_domain::attach — make session_start{action=attach} real

> Spec is the contract. Implementation, layering and file-level diffs live in
> `proposal.md` and `design.md`; tasks live in `tasks.md`.

## REQ-M9-77-01 — Wire shape

`session_start{action=attach}` SHALL accept a `pid: u32` parameter (no other
parameter is read for the attach case) and SHALL return the standard
`SessionStartOutput` envelope with `action = "attach"`, mirroring the spawn
case.

**Scenarios:**
- `attach_to_self_with_pid_returns_session_id` — call `session_start{action=attach}`
  with `pid = std::process::id()`; expect a successful response carrying a
  non-empty `session_id`, `target` equal to the resolved binary path, and
  `language` non-`Unknown`.
- `attach_with_zero_pid_returns_invalid_input_error` — call with `pid = 0`;
  expect an error response carrying `Invalid session_start input: requires pid`.
- `attach_with_missing_pid_returns_invalid_input_error` — call with no `pid`
  field; expect an error response carrying `requires pid`.

## REQ-M9-77-02 — Domain API

`NativeProbeBackend::start_probe_attach(pid, config, track_function_frames)` SHALL
exist, take a `CaptureConfig` (with `target` resolved from `/proc/<pid>/exe`),
spawn the existing ptrace-attach thread, populate `traced_pid`, mark
`running = true`, and return a `CaptureSession` with `state = Active`. On
non-Linux the method SHALL be a `compile_error!` and the dispatcher SHALL fail
closed before it gets there.

**Scenarios:**
- `start_probe_attach_to_self_sets_running_and_traced_pid` — backend unit test
  calling `start_probe_attach(std::process::id(), config, false)`; assert
  `running == true`, `traced_pid == Some(pid)` after success, and that
  `stop_probe` brings `running` back to `false`.
- `start_probe_attach_to_unknown_pid_returns_capture_failed` — pid
  `0xfffffffe`; expect `TraceError::CaptureFailed` mentioning `PTRACE_ATTACH`.

## REQ-M9-77-03 — Service layer

`ProbeService::start_attach(ctx, input)` SHALL exist, resolve the running
binary from `/proc/<pid>/exe`, build a `CaptureConfig`, call
`NativeProbeBackend::start_probe_attach`, register the returned `CaptureSession`
under `ctx.live_probes`, and mark `ctx.active_session` to the new id.

**Scenarios:**
- `probe_service_start_attach_resolves_target_from_proc_exe` — service test
  with a real `ProbeContext` (test fixture) and a pid that resolves to a
  binary on disk; assert the `LiveProbeSession` entry has the resolved target
  string and a non-`Unknown` language.
- `probe_service_start_attach_to_unknown_pid_returns_attach_failed` — same
  fixture, pid `0xfffffffe`; expect `ServiceError::AttachFailed` mentioning
  the pid.

## REQ-M9-77-04 — Dispatcher wiring

`ChronosSessionLifecycleService::attach(input)` SHALL call
`ProbeService::start_attach` instead of returning `Unsupported`. The
`CapabilitySnapshot` returned SHALL match the spawn case in shape (same field
set, same field types) but with `probe_type = "ebpf_user"` for now (the
existing convention; an `attach_native` discriminator is out of scope for
this cycle and would require a `CapabilitySnapshot` schema bump).

**Scenarios:**
- `start_attach_to_running_self_returns_capability_snapshot` — dispatcher
  test with a real `ProbeContext`, `pid = std::process::id()`; assert
  `out.session_id` non-empty, `out.action == Attach`, `out.capability_snapshot.language`
  non-`None`, `out.capability_snapshot.bus_capacity` non-`None`.
- `start_attach_to_unknown_pid_returns_attach_failed` — pid `0xfffffffe`;
  expect `ServiceError::AttachFailed`.
- `start_attach_without_pid_returns_invalid_input` — already covered by
  the m7-04 guard; carried forward unchanged.

## REQ-M9-77-05 — MCP wire

The `session_start` tool description SHALL no longer say "stub (m7+)". The
tool error mapping SHALL add a `ServiceError::AttachFailed(msg)` arm that
surfaces as `CallToolResult::error(text_content(format!("attach failed: {}", msg)))`.

**Scenarios:**
- `session_start_attach_to_self_returns_session_id` — sandbox test
  `test_session_start_attach_to_running_self`; assert the response is a
  success `CallToolResult`, not an error, and `session_id` is present.
- `session_start_attach_to_self_then_session_stop_succeeds` — same fixture,
  follow-up `session_stop(session_id)`; expect the stop to acknowledge
  the session exists (no `ProbeNotFound`).

## REQ-M9-77-06 — Out of scope (explicit non-goals)

- eBPF attach. The existing `probe_inject` flow already covers uprobe attach
  to a running session; eBPF attach as a lifecycle entry-point is a
  separate feature and is out of scope here.
- Cross-platform attach. The implementation uses Linux `ptrace` exclusively;
  on macOS / Windows the dispatcher SHALL return
  `ServiceError::AttachFailed("session_start{action=attach} requires Linux")`.
- Permission check in the dispatcher. The `ptrace` call fails with EPERM
  for foreign-uid pids; the dispatcher surfaces the error rather than
  pre-validating. A friendlier error message is a future cycle.
- Backwards-compat shim for any v1 attach tool. There is no v1 attach tool;
  `chronos_domain::attach` is net-new since v1 had no attach concept.

## REQ-M9-77-07 — Documentation surface

The `session_start` tool description in `chronos-mcp::server` SHALL be
updated to remove the "(m7+)" stub hint. The `AGENTS.md` document SHALL
record the ptrace-permissions caveat under §3. The
`docs/manual-ai/{en,es}/08-session-management.md` files SHALL grow a
"Attach a session" section.

**Scenarios:** verified by code inspection (no behavioural test).

## REQ-M9-77-08 — Gate contract

T0 (fmt + clippy `-D warnings`), T1 (`-p chronos-services --lib`,
`-p chronos-native --lib --test-threads=1`), T3 (workspace lib minus
sandbox/native/e2e), and T4-smoke (`e2e_connectivity`, the renamed
`session_lifecycle::test_session_start_attach_to_running_self`,
`store_open_failure`) SHALL all pass. The vault drift gate (`scripts/check_vault_drift.sh`)
SHALL remain PASS. The CC smoke (`scripts/smoke_test_ccs.sh`) SHALL remain 6/6
or higher.

## Cross-checks

- The pre-existing pre-existing ptrace flake (`chronos-native::ptrace_tracer::tests::test_launch_with_syscall_tracing`)
  SHALL not move: this cycle does not modify `PtraceTracer` itself, only the
  `NativeProbeBackend` surface that calls into it.
- `FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION` is partially mitigated
  by the m9-76 regen tool; this cycle adds one more manifest, regen tool
  runs to fixpoint before commit.
- The `Unsupported` arm is **removed** from `session_lifecycle::attach`; any
  test, sandbox client, or downstream agent relying on the stub will need
  to handle the new error path. Audit: the only direct caller of
  `ServiceError::Unsupported` for attach is the sandbox test
  `test_session_start_attach_returns_unsupported`, which is renamed and
  tightened in REQ-M9-77-05.
