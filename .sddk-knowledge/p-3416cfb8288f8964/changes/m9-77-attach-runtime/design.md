# Design: m9-77 chronos_domain::attach

> Design ties spec to code: signatures, layering, error mapping, ordering.
> Tasks in `tasks.md`.

## 1. Layering

```
MCP wire (chronos-mcp::server::session_start)
   └─ SessionLifecycleContext + SessionStartInput { action: Attach, pid }
       └─ chronos-services::session_lifecycle::ChronosSessionLifecycleService::attach
           ├─ validate pid != 0                       (kept)
           └─ ProbeService::start_attach(ctx, ProbeAttachInput { pid, … })
               ├─ readlink /proc/<pid>/exe            (linux only)
               ├─ Language::from_path(resolved)
               ├─ NativeProbeBackend::start_probe_attach(pid, config, false)
               │     ├─ HIGH-4 guard: running? → return Err
               │     ├─ capture_registers / follow_children defaults
               │     ├─ open ExecutionLog (mirror start_probe)
               │     ├─ spawn thread running run_probe_loop_attach
               │     └─ return CaptureSession { pid, language, config, state=Active }
               ├─ ctx.live_probes.lock().insert(session_id, LiveProbeSession{…})
               └─ ctx.active_session = Some(session_id)
       └─ build CapabilitySnapshot (mirrors spawn path)
       └─ SessionStartOutput { action: Attach, session_id, capability_snapshot, … }
```

`chronos_domain` itself gains no new types: `CaptureConfig` already takes a
`target: String` (which we resolve from `/proc/<pid>/exe`), and `CaptureSession`
already holds `pid` + `language` + `state`. The "domain-layer attach API" the
m7-04 docs called out is therefore the **live-probe attach primitive**, not
a new free function on `chronos_domain`.

## 2. Signatures

### 2.1 `crates/chronos-native/src/probe_backend.rs`

```rust
impl NativeProbeBackend {
    /// Attach the live probe to an already-running process.
    ///
    /// Mirrors `start_probe` for the attach case: opens the configured
    /// `ExecutionLog` (if any), spawns the existing `run_probe_loop_attach`
    /// background thread, records `traced_pid` for `stop_probe`, and
    /// returns a fresh `CaptureSession` with `state = Active`.
    ///
    /// Linux only. On non-Linux this method is gated by `cfg(target_os)`
    /// and the dispatcher fails closed before calling it.
    pub fn start_probe_attach(
        &self,
        pid: u32,
        config: CaptureConfig,
        track_function_frames: bool,
    ) -> Result<CaptureSession, TraceError> {
        // 1. HIGH-4 guard (same as start_probe)
        if self.running.load(Ordering::SeqCst) {
            return Err(TraceError::CaptureFailed(
                "A probe is already running on this backend. Call stop_probe first.".into(),
            ));
        }
        // 2. ptrace_config defaults mirror start_probe
        let ptrace_config = PtraceConfig {
            trace_syscalls: config.capture_syscalls,
            capture_registers: true,
            follow_children: true,
            track_function_frames,
        };
        // 3. Pre-build the session so we have a stable id for the log dir.
        let session = CaptureSession::new(pid, language, config.clone());
        let log_session_id = format!("native-{}", session.session_id);
        // 4. ExecutionLog open (same shape as start_probe)
        let log_for_thread: Option<std::sync::Arc<SegmentedExecutionLog>> = …;
        // 5. Record the pid in `traced_pid` BEFORE spawning the thread.
        *self.traced_pid.lock().unwrap() = Some(pid as i32);
        // 6. Set running=true before spawn.
        self.running.store(true, Ordering::SeqCst);
        // 7. Spawn the attach loop.
        let handle = thread::Builder::new()
            .name("chronos-native-probe-attach".into())
            .spawn(move || {
                Self::run_probe_loop_attach(
                    pid,
                    &ptrace_config,
                    &running,
                    event_bus,
                    resolver_pipeline,
                    language,
                );
            })?;
        *self.thread_handle.lock().unwrap() = Some(handle);
        Ok(session)
    }
}
```

The ExecutionLog wiring is the same as `start_probe`. The thread calls into
the **already-existing** `Self::run_probe_loop_attach` at
`crates/chronos-native/src/probe_backend.rs:887`, which is currently
implemented but unreferenced. We do **not** add a new loop; we call the
existing one and only wire its handle / pid storage.

### 2.2 `crates/chronos-services/src/probe.rs`

```rust
#[derive(Debug, Clone)]
pub struct ProbeAttachInput {
    pub pid: u32,
    pub trace_syscalls: bool,
    pub bus_capacity: usize,
    pub track_function_frames: bool,
}

#[derive(Debug)]
pub struct ProbeAttachOutput {
    pub session_id: String,
    pub pid: u32,
    pub target: String,
    pub language: String,
    pub bus_capacity: usize,
}

impl ProbeService {
    pub fn start_attach(
        ctx: &ProbeContext<'_>,
        input: ProbeAttachInput,
    ) -> Result<ProbeAttachOutput, ServiceError> {
        // Resolve the binary the running pid is executing.
        let target = resolve_pid_target(input.pid).map_err(|e| {
            ServiceError::AttachFailed(format!(
                "could not resolve target for pid {}: {}",
                input.pid, e
            ))
        })?;
        let language = Language::from_path(&target);
        let config = CaptureConfig {
            target,
            args: vec![],
            env: None,
            cwd: None,
            language: Some(language),
            capture_syscalls: input.trace_syscalls,
            capture_variables: false, // default; mirrors start_probe
            capture_stack: true,
            capture_memory: false,
            capture_function_exit: false,
            function_filter: None,
            max_duration_ms: None,
        };
        let bus = EventBus::new_shared(input.bus_capacity);
        let backend = NativeProbeBackend::new(bus).with_language(language);
        let session = backend
            .start_probe_attach(input.pid, config, input.track_function_frames)
            .map_err(|e| ServiceError::AttachFailed(e.to_string()))?;
        let session_id = uuid::Uuid::new_v4().to_string();
        let live = LiveProbeSession {
            backend,
            session,
            language,
            target: backend_target.clone(), // populated below
            ebpf_adapter: None,
            ebpf_attachment: None,
        };
        ctx.live_probes
            .lock()
            .map_err(|_| ServiceError::LockPoisoned)?
            .insert(session_id.clone(), live);
        let mut active = ctx.active_session.lock().await;
        *active = Some(session_id.clone());
        drop(active);
        Ok(ProbeAttachOutput {
            session_id,
            pid: input.pid,
            target: …,           // resolved path
            language: format!("{:?}", language),
            bus_capacity: input.bus_capacity,
        })
    }
}

fn resolve_pid_target(pid: u32) -> Result<String, String> {
    #[cfg(target_os = "linux")]
    {
        let link = std::fs::read_link(format!("/proc/{}/exe", pid))
            .map_err(|e| format!("/proc/{}/exe: {}", pid, e))?;
        Ok(link.to_string_lossy().into_owned())
    }
    #[cfg(not(target_os = "linux"))]
    Err("session_start{action=attach} requires Linux".to_string())
}
```

`ServiceError` gains one new variant:

```rust
// crates/chronos-services/src/error.rs
#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    // … existing variants …
    /// session_start{action=attach} failed (could not resolve pid,
    /// ptrace returned EPERM, or the backend rejected the config).
    #[error("attach failed: {0}")]
    AttachFailed(String),
}
```

### 2.3 `crates/chronos-services/src/session_lifecycle.rs`

```rust
async fn attach(
    ctx: &SessionLifecycleContext<'_>,
    input: SessionStartInput,
) -> Result<SessionStartOutput, ServiceError> {
    let pid = input.pid.ok_or_else(|| {
        ServiceError::InvalidInput(
            "session_start{action=attach} requires `pid`".to_string(),
        )
    })?;
    if pid == 0 {
        return Err(ServiceError::InvalidInput(
            "session_start{action=attach} requires a non-zero `pid`".to_string(),
        ));
    }
    let out = ProbeService::start_attach(
        ctx.probe,
        ProbeAttachInput {
            pid,
            trace_syscalls: false,            // default; matches m7-04 spawn default
            bus_capacity: 4096,
            track_function_frames: false,
        },
    )?;
    let snapshot = CapabilitySnapshot {
        probe_type: Some("ebpf_user".to_string()),
        language: Some(out.language.clone()),
        bus_capacity: Some(out.bus_capacity),
        bus_fill: Some(0),
        query_engine_ready: false,
        active_subscriptions: vec![],
        tail_sealed: false,
        sealed_at: None,
    };
    Ok(SessionStartOutput {
        session_id: out.session_id,
        action: SessionStartAction::Attach,
        target: Some(out.target),
        language: Some(out.language),
        event_count: None,
        duration_ms: None,
        bus_capacity: Some(out.bus_capacity),
        capability_snapshot: snapshot,
        provenance: lifecycle_provenance("session_start:attach"),
    })
}
```

The dispatcher `start` arm flips from `Self::attach(input)` (a static
method that returned `Unsupported`) to `Self::attach(ctx, input)` (an
async method that calls `ProbeService::start_attach`). All other dispatch
arms unchanged.

## 3. Error mapping

| Origin | `ServiceError` | Wire outcome (call_tool error) |
|---|---|---|
| `pid == 0` / missing | `InvalidInput` | "Invalid session_start input: requires pid" (existing arm) |
| `/proc/<pid>/exe` unreadable | `AttachFailed` | "attach failed: /proc/<pid>/exe: …" (new arm) |
| `ptrace::attach` fails (EPERM, ESRCH) | `AttachFailed` | "attach failed: PTRACE_ATTACH failed: …" (new arm) |
| Non-Linux | `AttachFailed` | "attach failed: session_start{action=attach} requires Linux" (new arm) |
| `LiveProbeSession` insert fails | `LockPoisoned` | "internal error: …" (existing arm) |

The MCP tool body adds:

```rust
Err(ServiceError::AttachFailed(msg)) => Ok(CallToolResult::error(text_content(
    format!("attach failed: {}", msg),
))),
```

before the catch-all `Err(other)` arm.

## 4. Test fixture reuse

The existing `start_spawn_requires_spawn_fields` and
`start_load_returns_metadata_snapshot` tests in
`crates/chronos-services/src/session_lifecycle.rs` already build a
`SessionLifecycleContext` with a real (empty) `SessionStore` and a
`ProbeContext` wrapping the same empty maps. The new tests reuse
`build_minimal_ctx(store)` and call `start_attach` against:

- `std::process::id()` — happy path; the test binary always exists, and
  `/proc/self/exe` always resolves.
- `0xfffffffe` — deliberately non-existent; `ptrace::attach` returns
  ESRCH, the backend returns `CaptureFailed`, the service maps to
  `AttachFailed`.

For the `NativeProbeBackend` test, we keep the same pattern as
`bridge_projects_function_identity_onto_record`: build a backend, call
`start_probe_attach(std::process::id(), config, false)`, assert
`running == true` and `traced_pid == Some(pid)`, then call `stop_probe`
to bring `running` back to false. The test is `#[cfg(target_os = "linux")]`
because the whole attach path is Linux-only; on macOS the test file is
gated and contributes zero coverage, which the gate accepts.

## 5. Sandbox test rewiring

`chronos-sandbox/tests/session_lifecycle.rs::test_session_start_attach_returns_unsupported`
→ `test_session_start_attach_to_running_self`:

```rust
#[tokio::test]
async fn test_session_start_attach_to_running_self() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Attach to the test process itself (always exists, always owned by the same uid).
    let self_pid = std::process::id();
    let response = client
        .session_start_attach(self_pid)
        .await
        .expect("session_start{action=attach} should not return Err for a valid pid");

    // Success: the response carries a session_id we can stop.
    let session_id = response
        .get("session_id")
        .and_then(|v| v.as_str())
        .expect("attach response missing session_id")
        .to_string();
    assert!(!session_id.is_empty());

    // Follow-up: stop the session. ProbeNotFound would indicate the live
    // session was never registered, which is the bug we are closing.
    client
        .session_stop(session_id, true, false)
        .await
        .expect("session_stop on the attached session must succeed");

    client.shutdown().await.ok();
}
```

This replaces the existing test (which was a smoke test for the stub
behaviour). The new test exercises both the attach path and the live
session registration.

## 6. Open questions / known limits

- **Function-frame tracking on attach** is `track_function_frames: false`
  by default, matching the m7-04 default. The dispatcher takes the
  spawn-shape default; an explicit override can come later via a v2
  parameter.
- **`bus_capacity`** defaults to `4096` (same as spawn). An explicit
  override is out of scope (the m7-04 wire shape did not include a
  `bus_capacity` parameter for attach).
- **Cap snapshot `probe_type = "ebpf_user"`** is a placeholder that
  matches the spawn path's current value. A future cycle that adds
  `attach_ebpf` (separate `chronos_ebpf` plumbing) can introduce
  `probe_type = "ptrace_attach"`. Filed as a follow-up.

## 7. Failure-mode inventory (falsification targets)

| Mutation | Expected failure |
|---|---|
| Disable `/proc/<pid>/exe` resolution | `start_attach_to_unknown_pid_returns_attach_failed` passes for the wrong reason; `attach_to_self` test fails with `AttachFailed("/proc/.../exe: …")` |
| Re-introduce the `Unsupported` return in the dispatcher | `start_attach_to_running_self_returns_capability_snapshot` fails with `Unsupported`; sandbox test fails with the same |
| Skip the `live_probes` insert | Sandbox test fails on the follow-up `session_stop` with `ProbeNotFound` |
| Forget to set `running = true` | `start_probe_attach_to_unknown_pid` test fails: the second call to `start_probe_attach` no longer rejects with the HIGH-4 guard message |
| Set `traced_pid = None` after spawn | `stop_probe` cannot find the pid; sandbox test still passes (because the test does not check post-stop state), but the dispatcher test that calls `stop_probe` directly fails |
| Use `Language::Unknown` regardless of resolved path | `attach_to_self_with_pid_returns_session_id` fails the `language non-Unknown` assertion |
| Set `probe_type = "ptrace_attach"` | Sandbox test passes; the `start_attach_to_running_self_returns_capability_snapshot` test fails on the snapshot shape assertion |
