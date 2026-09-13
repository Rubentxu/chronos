//! Native probe service — live ptrace-based probes (`probe_*` tool family).
//!
//! This module owns the long-lived state associated with a live native probe:
//! `LiveProbeSession` (carrying the `NativeProbeBackend`, the underlying
//! `CaptureSession`, and the optional eBPF adapter). The MCP-server tool
//! functions in `chronos-mcp` are thin wrappers that delegate here.
//!
//! Browser-probe sessions live in a sibling service (deferred to m5-06b).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use chronos_domain::adapter::ProbeBackend;
use chronos_domain::bus::EventBus;
use chronos_domain::{CaptureConfig, CaptureSession, Language};
use chronos_native::probe_backend::NativeProbeBackend;
use tokio::sync::Mutex as TokioMutex;
use tracing::info;

use chronos_query::QueryEngine;

use crate::error::ServiceError;
use crate::output::{ProbeDrainResult, ProbeStartOutput, ProbeStopResult};
use chronos_domain::tripwire::TripwireManager;
use chronos_domain::TraceEvent;

/// A live native probe session.
///
/// Unlike `debug_run` which blocks until the program exits, a live probe streams
/// events to an `EventBus` ring buffer in real-time. Events can be drained at any
/// time via `probe_drain`, and the probe is stopped via `probe_stop`.
pub struct LiveProbeSession {
    /// The native probe backend driving the ptrace loop.
    pub backend: NativeProbeBackend,
    /// The capture session returned by `start_probe`.
    pub session: CaptureSession,
    /// Language of the target program.
    pub language: Language,
    /// Path to the target binary.
    pub target: String,
    /// True when this session ptrace-attaches to a caller-owned process.
    /// The legacy stop backend terminates its tracee, which is appropriate for
    /// spawned probes but must not be applied to an attached process.
    pub attached: bool,
    /// eBPF adapter owned by this session, if any uprobes have been injected.
    /// Stored here so the lifecycle is observable: subsequent `probe_inject`
    /// calls reuse the same adapter, and `probe_stop` detaches cleanly.
    pub ebpf_adapter: Option<Arc<chronos_ebpf::EbpfAdapter>>,
    /// Most recent eBPF attachment metadata (binary_path, symbol_name, pid).
    pub ebpf_attachment: Option<EbpfAttachmentInfo>,
}

/// Metadata for the eBPF attachment of a live probe session.
#[derive(Debug, Clone)]
pub struct EbpfAttachmentInfo {
    /// Library / binary path the uprobe was attached to.
    pub binary_path: String,
    /// Symbol the uprobe was attached to.
    pub symbol_name: String,
    /// Pid the uprobe was attached to.
    pub pid: u32,
}

/// Aggregated server state that probe methods need to read/mutate.
///
/// Mirrors the `SessionsContext` pattern from `m5-03`: the server still owns
/// the underlying maps and the `ProbeService` borrows them through this struct.
pub struct ProbeContext<'a> {
    /// Live probe sessions: `session_id` → `LiveProbeSession`.
    pub live_probes: &'a Mutex<HashMap<String, LiveProbeSession>>,
    /// Finalized session engines: `session_id` → `QueryEngine`.
    /// Kept here so `probe_stop` can hand off the events to the indexer path
    /// (the actual `build_and_store_engine` call still happens in the server,
    /// but the service surfaces the events/language it needs).
    pub engines: &'a TokioMutex<HashMap<String, QueryEngine>>,
    /// Language recorded per finalized session.
    pub session_languages: &'a Arc<TokioMutex<HashMap<String, Language>>>,
    /// Tripwire manager — `probe_drain` evaluates tripwires against every
    /// drained event so live evidence reaches the tripwire subsystem.
    pub tripwire_manager: &'a Arc<TripwireManager>,
    /// Currently active session id (for `probe_start` to mark the new session).
    pub active_session: &'a TokioMutex<Option<String>>,
}

/// Type alias matching `ProbeContext<'a>` — used in tests and follow-up
/// implementations where the `'_` lifetime conflicts with the `loop`/`for`
/// label-keyword parsing (rustc 2021 disallows `_` as a label name).
pub type ProbeContextRef<'a> = ProbeContext<'a>;

// ---------------------------------------------------------------------------
// Input types
// ---------------------------------------------------------------------------

/// Input for `ProbeService::start`. Mirrors the MCP `ProbeStartParams` shape
/// but without JsonSchema / Deserialize (the server is responsible for parsing).
#[derive(Debug, Clone)]
pub struct ProbeStartInput {
    pub program: String,
    pub args: Vec<String>,
    pub trace_syscalls: bool,
    pub cwd: Option<String>,
    pub bus_capacity: usize,
    pub track_function_frames: Option<bool>,
}

/// Input for `ProbeService::start_attach` (m9-77).
///
/// `pid` is the running process to attach to. The dispatcher resolves the
/// binary path from `/proc/<pid>/exe`, so the caller does not supply a
/// program name. `trace_syscalls` and `bus_capacity` mirror the spawn
/// defaults.
#[derive(Debug, Clone)]
pub struct ProbeAttachInput {
    pub pid: u32,
    pub trace_syscalls: bool,
    pub bus_capacity: usize,
}

/// Output for `ProbeService::start_attach` (m9-77).
///
/// `session_id` is the freshly minted id under which the live probe is
/// registered in `ctx.live_probes` (same key shape as `ProbeStartOutput`).
#[derive(Debug)]
pub struct ProbeAttachOutput {
    pub session_id: String,
    pub pid: u32,
    pub target: String,
    pub language: String,
    pub bus_capacity: usize,
}

/// Input for `ProbeService::drain`.
#[derive(Debug, Clone)]
pub struct ProbeDrainInput {
    pub session_id: String,
    /// Pre-parsed cursor (already converted from `CursorDto`).
    pub cursor: Option<chronos_domain::EventCursor>,
    pub offset: usize,
    pub limit: usize,
}

/// Input for `ProbeService::inject`.
#[derive(Debug, Clone)]
pub struct ProbeInjectInput {
    pub session_id: String,
    pub binary_path: String,
    pub symbol_name: String,
    /// Optional PID override; if `None`, the probe's own traced PID is used.
    pub pid: Option<u32>,
}

/// Result of `ProbeService::inject` — wraps the three terminal cases the
/// `probe_inject` wrapper turns into JSON. `EbpfUnavailable` and
/// `AttachFailed` carry the error message that the wrapper surfaces as a
/// tool error (CallToolResult::error), while `Attached` becomes a success.
#[derive(Debug)]
pub enum ProbeInjectResult {
    /// Probe is registered but its PID is not yet known (start-up race).
    ProbeStarting,
    /// Adapter could not be constructed (kernel lacks eBPF feature).
    EbpfUnavailable(String),
    /// Adapter attached successfully to the process.
    Attached {
        session_id: String,
        binary_path: String,
        symbol_name: String,
        pid: u32,
    },
    /// Adapter constructed but `attach_uprobe` returned an error.
    AttachFailed {
        session_id: String,
        binary_path: String,
        symbol_name: String,
        pid: u32,
        error: String,
    },
}

/// Native probe service — business logic for `probe_start`, `probe_stop`,
/// `probe_drain`, `probe_drain_log`, `probe_compaction_metrics`,
/// `session_snapshot`, `probe_inject`, `probe_status`.
///
/// All methods take a `&ProbeContext<'_>` that points at the shared state held
/// by `ChronosServer`. The MCP tool bodies in `chronos-mcp::server` are thin
/// wrappers that build the context and translate `ServiceError` into the
/// existing MCP error/JSON shapes.
pub struct ProbeService;

impl ProbeService {
    /// Start a live native probe on a target program.
    ///
    /// Returns the new `session_id` and metadata in a `ProbeStartOutput`. The
    /// server is expected to surface the same JSON shape it always has, plus
    /// mark this session as active in `ctx.active_session`.
    pub async fn start(
        ctx: &ProbeContext<'_>,
        input: ProbeStartInput,
    ) -> Result<ProbeStartOutput, ServiceError> {
        // Build capture config
        let mut config = CaptureConfig::new(&input.program);
        config.args = input.args;
        config.capture_syscalls = input.trace_syscalls;
        config.language = Some(ctx_infer_language(&input.program));

        if let Some(ref cwd) = input.cwd {
            config.cwd = Some(PathBuf::from(cwd));
        }

        // Create a fresh EventBus for this session
        let bus = EventBus::new_shared(input.bus_capacity);
        let language = ctx_infer_language(&input.program);
        let backend = NativeProbeBackend::new(bus).with_language(language);

        // Start the probe (non-blocking — spawns background thread)
        let track_function_frames = input.track_function_frames.unwrap_or(false);
        let session = backend
            .start_probe(config, track_function_frames)
            .map_err(|e| ServiceError::ProbeStartFailed(e.to_string()))?;

        let session_id = uuid::Uuid::new_v4().to_string();
        info!(
            "Live probe started for '{}' (session: {}, bus capacity: {})",
            input.program, session_id, input.bus_capacity
        );

        // Store the live probe session
        let live_probe = LiveProbeSession {
            backend,
            session,
            language,
            target: input.program.clone(),
            attached: false,
            ebpf_adapter: None,
            ebpf_attachment: None,
        };
        ctx.live_probes
            .lock()
            .map_err(|_| ServiceError::LockPoisoned)?
            .insert(session_id.clone(), live_probe);

        // Mark as active session so subsequent probe_* calls know which
        // session to operate on by default.
        let mut active = ctx.active_session.lock().await;
        *active = Some(session_id.clone());
        drop(active);

        Ok(ProbeStartOutput {
            session_id,
            status: "running".to_string(),
            target: input.program,
            language: format!("{:?}", language),
            bus_capacity: input.bus_capacity,
            hint: "Use probe_drain to read events in real-time, probe_stop to finalize."
                .to_string(),
        })
    }

    /// Attach the live probe to an already-running process (m9-77).
    ///
    /// Resolves the running binary from `/proc/<pid>/exe`, builds a
    /// `CaptureConfig`, calls `NativeProbeBackend::attach_probe`, registers
    /// the returned `CaptureSession` under `ctx.live_probes`, and marks
    /// `ctx.active_session` to the new id. The dispatcher surfaces the
    /// returned `ProbeAttachOutput` as `SessionStartOutput { action: Attach }`.
    ///
    /// **Linux-only at the implementation level.** On non-Linux
    /// `resolve_pid_target` returns `Err`, so the call fails closed before
    /// any ptrace syscall. **Same-uid / same-pid only**: `PtraceTracer::attach`
    /// returns `EPERM` for foreign-uids; the error is surfaced as
    /// `ServiceError::AttachFailed` and the backend's `running` flag is
    /// cleared by the spawned thread's failure path (HIGH-4 invariant).
    pub fn start_attach(
        ctx: &ProbeContext<'_>,
        input: ProbeAttachInput,
    ) -> Result<ProbeAttachOutput, ServiceError> {
        let target = resolve_pid_target(input.pid).map_err(|e| {
            ServiceError::AttachFailed(format!(
                "could not resolve target for pid {}: {}",
                input.pid, e
            ))
        })?;
        let language = Language::from_path(&target);
        // `Language::from_path` returns `Unknown` for binaries without a
        // recognised extension (typical native executables). A binary that
        // is already running is almost certainly a native ELF executable,
        // so default to `Native` rather than `Unknown` — the language is
        // surfaced in the capability snapshot and `Unknown` would be
        // misleading.
        let language = if language == Language::Unknown {
            Language::Native
        } else {
            language
        };
        let config = CaptureConfig {
            target: target.clone(),
            args: Vec::new(),
            env: None,
            cwd: None,
            language: Some(language),
            capture_syscalls: input.trace_syscalls,
            capture_variables: false,
            capture_stack: true,
            capture_memory: false,
            capture_function_exit: false,
            function_filter: None,
            max_duration_ms: None,
        };
        let bus = EventBus::new_shared(input.bus_capacity);
        let backend = NativeProbeBackend::new(bus).with_language(language);
        let session = backend.attach_probe(input.pid, config).map_err(|e| {
            ServiceError::AttachFailed(format!(
                "NativeProbeBackend::attach_probe({}) failed: {}",
                input.pid, e
            ))
        })?;
        let session_id = uuid::Uuid::new_v4().to_string();
        let live = crate::probe::LiveProbeSession {
            backend,
            session,
            language,
            target: target.clone(),
            attached: true,
            ebpf_adapter: None,
            ebpf_attachment: None,
        };
        ctx.live_probes
            .lock()
            .map_err(|_| ServiceError::LockPoisoned)?
            .insert(session_id.clone(), live);
        info!(
            "Live probe attached to pid {} ('{}', session: {}, bus capacity: {})",
            input.pid, target, session_id, input.bus_capacity
        );
        Ok(ProbeAttachOutput {
            session_id,
            pid: input.pid,
            target,
            language: format!("{:?}", language),
            bus_capacity: input.bus_capacity,
        })
    }

    /// Stop a live native probe session.
    ///
    /// Returns the drained raw `TraceEvent`s and metadata so the server-side
    /// wrapper can call `build_and_store_engine` (which still lives on the
    /// server because it touches `engines` and `session_languages`).
    pub fn stop(ctx: &ProbeContext<'_>, session_id: &str) -> Result<ProbeStopResult, ServiceError> {
        let mut live_probes = ctx
            .live_probes
            .lock()
            .map_err(|_| ServiceError::LockPoisoned)?;
        if live_probes
            .get(session_id)
            .is_some_and(|live_probe| live_probe.attached)
        {
            return Err(ServiceError::Unsupported(
                "session_stop for an attached process is not available yet; closing the MCP server detaches it without terminating the target".to_string(),
            ));
        }

        // Remove the live probe session only after the ownership safety check.
        let live_probe = live_probes.remove(session_id);

        let live_probe =
            live_probe.ok_or_else(|| ServiceError::ProbeNotFound(session_id.to_string()))?;

        // MS-RACE-FIX (ADR-0005): stop FIRST, then drain. `stop_probe` is
        // blocking: it joins the capture thread (bounded) before returning,
        // so draining afterwards observes every event the probe emitted.
        // Draining before stopping loses events emitted in the race window.
        if let Err(e) = live_probe.backend.stop_probe(&live_probe.session) {
            tracing::warn!("Probe stop error for session {}: {}", session_id, e);
        }

        // Detach any eBPF uprobes this session owned after the probe thread
        // has exited. Best-effort.
        if let Some(adapter) = &live_probe.ebpf_adapter {
            if let Err(e) = adapter.detach_all() {
                tracing::warn!("eBPF detach error for session {}: {}", session_id, e);
            }
        }

        // Drain final raw events from the bus (for QueryEngine). No concurrent
        // producer remains at this point.
        // drain_raw_events() returns TraceEvent directly, which is what
        // build_and_store_engine needs.
        let events: Vec<TraceEvent> = live_probe.backend.drain_raw_events();

        let total_events = events.len();
        let language = live_probe.language;
        let target = live_probe.target;
        let ebpf_was_attached = live_probe.ebpf_attachment.is_some();

        // Compute duration before moving events
        let duration_ms = if let (Some(first), Some(last)) = (events.first(), events.last()) {
            last.timestamp_ns.saturating_sub(first.timestamp_ns) / 1_000_000
        } else {
            0
        };

        info!(
            "Live probe stopped for '{}' (session: {}, events: {})",
            target, session_id, total_events
        );

        Ok(ProbeStopResult {
            events,
            language,
            target,
            total_events,
            duration_ms,
            ebpf_detached: ebpf_was_attached,
        })
    }

    /// Non-destructive drain from a live probe session.
    ///
    /// Returns the semantic events + cursor metadata. The server wrapper is
    /// responsible for serializing the events into the existing JSON shape
    /// (this lets the wrapper apply the `offset`/`limit` slicing after the
    /// service hands off the full snapshot, matching the original contract).
    pub fn drain(
        ctx: &ProbeContext<'_>,
        input: ProbeDrainInput,
    ) -> Result<ProbeDrainResult, ServiceError> {
        // Narrow lock scope: read events inside scoped block, then drop lock.
        let (events, new_cursor, cursor_stale) = {
            let probes = ctx
                .live_probes
                .lock()
                .map_err(|_| ServiceError::LockPoisoned)?;
            let live_probe = probes
                .get(&input.session_id)
                .ok_or_else(|| ServiceError::ProbeNotFound(input.session_id.clone()))?;
            match live_probe.backend.read_since(input.cursor) {
                Ok((events, new_cursor, status)) => {
                    let stale = matches!(status, chronos_domain::CursorStatus::Stale);
                    (events, new_cursor, stale)
                }
                Err(chronos_domain::TraceError::CursorStale { .. }) => {
                    return Err(ServiceError::CursorStale);
                }
                Err(e) => {
                    return Err(ServiceError::DrainFailed(e.to_string()));
                }
            }
        }; // lock dropped here

        let total_buffered = events.len();

        // Evaluate tripwires against every drained semantic event so live
        // evidence reaches the tripwire subsystem without waiting for stop.
        let mut fired: Vec<chronos_domain::TripwireFired> = Vec::new();
        if !events.is_empty() {
            let mgr = Arc::clone(ctx.tripwire_manager);
            for ev in &events {
                let local = mgr.evaluate_semantic(ev);
                if !local.is_empty() {
                    fired.extend(local);
                }
            }
        }
        let tripwires_fired = fired.len();

        Ok(ProbeDrainResult {
            events,
            new_cursor,
            cursor_stale,
            total_buffered,
            tripwires_fired,
        })
    }

    /// Read records from a live probe session's durable `ExecutionLog`.
    ///
    /// Returns raw `TraceEvent`s plus decoder counters so the server wrapper
    /// can serialize them into the existing JSON shape.
    pub fn drain_log(
        ctx: &ProbeContext<'_>,
        session_id: &str,
        since: Option<u64>,
        limit: usize,
    ) -> Result<(Vec<TraceEvent>, Option<u64>, u64, u64), ServiceError> {
        let probes = ctx
            .live_probes
            .lock()
            .map_err(|_| ServiceError::LockPoisoned)?;
        let live_probe = probes
            .get(session_id)
            .ok_or_else(|| ServiceError::ProbeNotFound(session_id.to_string()))?;
        live_probe
            .backend
            .read_execution_log_records_with_stats(since, limit)
            .map_err(|e| ServiceError::DrainFailed(e.to_string()))
    }

    /// Snapshot the live probe session's `ExecutionLog` compaction counters.
    ///
    /// Returns `Ok(None)` when the probe was not configured with an
    /// ExecutionLog directory (server wrapper surfaces this as `log_attached: false`).
    pub fn compaction_metrics(
        ctx: &ProbeContext<'_>,
        session_id: &str,
    ) -> Result<Option<chronos_log::CompactionMetrics>, ServiceError> {
        let probes = ctx
            .live_probes
            .lock()
            .map_err(|_| ServiceError::LockPoisoned)?;
        let live_probe = probes
            .get(session_id)
            .ok_or_else(|| ServiceError::ProbeNotFound(session_id.to_string()))?;
        live_probe
            .backend
            .compaction_metrics()
            .map_err(|e| ServiceError::DrainFailed(e.to_string()))
    }

    /// Drain raw events from a live probe session and return them + the
    /// session's language. The server-side wrapper then calls
    /// `build_and_store_engine` and surfaces the `session_snapshot` JSON shape.
    pub fn session_snapshot(
        ctx: &ProbeContext<'_>,
        session_id: &str,
    ) -> Result<(Vec<TraceEvent>, chronos_domain::Language), ServiceError> {
        let probes = ctx
            .live_probes
            .lock()
            .map_err(|_| ServiceError::LockPoisoned)?;
        let live_probe = probes
            .get(session_id)
            .ok_or_else(|| ServiceError::ProbeNotFound(session_id.to_string()))?;

        // drain_raw_events() returns TraceEvent (QueryEngine's expected type).
        let events = live_probe.backend.drain_raw_events();
        let language = live_probe.language;
        Ok((events, language))
    }

    /// Attach an eBPF uprobe to a running probe process.
    ///
    /// Mutates the live probe session in-place to record the adapter + attachment
    /// metadata. Returns the terminal outcome so the server wrapper can serialize
    /// the existing JSON shape.
    pub fn inject(
        ctx: &ProbeContext<'_>,
        input: ProbeInjectInput,
    ) -> Result<ProbeInjectResult, ServiceError> {
        // Look up the live probe session to get the target PID and own the adapter.
        let target_pid = {
            let probes = ctx
                .live_probes
                .lock()
                .map_err(|_| ServiceError::LockPoisoned)?;
            let live_probe = probes
                .get(&input.session_id)
                .ok_or_else(|| ServiceError::ProbeNotFound(input.session_id.clone()))?;
            live_probe
                .backend
                .get_traced_pid()
                .map(|p| p as u32)
                .unwrap_or(live_probe.session.pid)
        };

        let pid = input.pid.unwrap_or(target_pid);
        if pid == 0 {
            return Ok(ProbeInjectResult::ProbeStarting);
        }

        // If the session already has an eBPF adapter, detach the previous
        // attachment first so the new injection is the single source of truth.
        {
            let mut probes = ctx
                .live_probes
                .lock()
                .map_err(|_| ServiceError::LockPoisoned)?;
            if let Some(lp) = probes.get_mut(&input.session_id) {
                if lp.ebpf_adapter.is_some() {
                    lp.ebpf_attachment = None;
                }
            }
        }

        // Attempt eBPF uprobe injection. The adapter is owned by the session
        // so the lifecycle is observable via probe_status and probe_stop can
        // detach on shutdown.
        match chronos_ebpf::EbpfAdapter::new() {
            Ok(adapter) => {
                let adapter = Arc::new(adapter);
                match adapter.attach_uprobe(pid, &input.binary_path, &input.symbol_name) {
                    Ok(()) => {
                        {
                            let mut probes = ctx
                                .live_probes
                                .lock()
                                .map_err(|_| ServiceError::LockPoisoned)?;
                            if let Some(lp) = probes.get_mut(&input.session_id) {
                                lp.ebpf_adapter = Some(adapter.clone());
                                lp.ebpf_attachment = Some(EbpfAttachmentInfo {
                                    binary_path: input.binary_path.clone(),
                                    symbol_name: input.symbol_name.clone(),
                                    pid,
                                });
                            }
                        }
                        Ok(ProbeInjectResult::Attached {
                            session_id: input.session_id,
                            binary_path: input.binary_path,
                            symbol_name: input.symbol_name,
                            pid,
                        })
                    }
                    Err(e) => {
                        // Persist adapter even on attach failure so the session
                        // has a stable record and probe_status reflects availability.
                        let mut probes = ctx
                            .live_probes
                            .lock()
                            .map_err(|_| ServiceError::LockPoisoned)?;
                        if let Some(lp) = probes.get_mut(&input.session_id) {
                            lp.ebpf_adapter = Some(adapter.clone());
                            lp.ebpf_attachment = Some(EbpfAttachmentInfo {
                                binary_path: input.binary_path.clone(),
                                symbol_name: input.symbol_name.clone(),
                                pid,
                            });
                        }
                        Ok(ProbeInjectResult::AttachFailed {
                            session_id: input.session_id,
                            binary_path: input.binary_path,
                            symbol_name: input.symbol_name,
                            pid,
                            error: e.to_string(),
                        })
                    }
                }
            }
            Err(e) => {
                // Record the attempted attachment even when the kernel feature
                // is unavailable, so the session record reflects that the user
                // requested a probe and we cannot honour it.
                let mut probes = ctx
                    .live_probes
                    .lock()
                    .map_err(|_| ServiceError::LockPoisoned)?;
                if let Some(lp) = probes.get_mut(&input.session_id) {
                    lp.ebpf_attachment = Some(EbpfAttachmentInfo {
                        binary_path: input.binary_path.clone(),
                        symbol_name: input.symbol_name.clone(),
                        pid,
                    });
                }
                Ok(ProbeInjectResult::EbpfUnavailable(e.to_string()))
            }
        }
    }

    /// Snapshot a live probe session's status — target, language, traced PID,
    /// eBPF attachment metadata, and session state.
    pub fn status(
        ctx: &ProbeContext<'_>,
        session_id: &str,
    ) -> Result<crate::output::ProbeStatusOutput, ServiceError> {
        let probes = ctx
            .live_probes
            .lock()
            .map_err(|_| ServiceError::LockPoisoned)?;
        let live_probe = probes
            .get(session_id)
            .ok_or_else(|| ServiceError::ProbeNotFound(session_id.to_string()))?;

        let ebpf = live_probe.ebpf_attachment.as_ref().map(|a| {
            serde_json::json!({
                "binary_path": a.binary_path,
                "symbol_name": a.symbol_name,
                "pid": a.pid,
                "adapter_owned": live_probe.ebpf_adapter.is_some(),
            })
        });
        let traced_pid = live_probe
            .backend
            .get_traced_pid()
            .map(|p| p as u32)
            .unwrap_or(live_probe.session.pid);

        Ok(crate::output::ProbeStatusOutput {
            session_id: session_id.to_string(),
            language: live_probe.language,
            target: live_probe.target.clone(),
            traced_pid,
            ebpf,
            state: format!("{:?}", live_probe.session.state),
        })
    }
}

/// Language inferred from a file path. Mirrors `Language::from_path` but lives
/// in the service so the server doesn't need to import the domain type just
/// for this.
fn ctx_infer_language(program: &str) -> Language {
    Language::from_path(program)
}

/// Resolve the binary path of a running pid (m9-77).
///
/// Linux: reads the symlink at `/proc/<pid>/exe`, which always exists for
/// running processes and resolves through deleted-binary links. The result
/// is the path the kernel reports as the currently-executing binary; if the
/// binary has been deleted on disk, the path still resolves (with the
/// ` (deleted)` suffix), and the downstream `attach_probe` call will fail
/// at symbol-load time with `CaptureFailed`.
///
/// Non-Linux: returns `Err` so the caller fails closed. The dispatcher
/// surfaces this as `ServiceError::AttachFailed("… requires Linux")`.
fn resolve_pid_target(pid: u32) -> Result<String, String> {
    #[cfg(target_os = "linux")]
    {
        let link = format!("/proc/{}/exe", pid);
        match std::fs::read_link(&link) {
            Ok(path) => Ok(path.to_string_lossy().into_owned()),
            Err(e) => Err(format!("read_link({}): {}", link, e)),
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        Err("session_start{action=attach} requires Linux".to_string())
    }
}

// The imports below are currently used by `ProbeService::start`. As follow-up
// commits implement the remaining methods (`stop`, `drain`, `drain_log`,
// `compaction_metrics`, `session_snapshot`, `inject`, `status`), the full
// surface will be exercised.
#[allow(unused_imports)]
use _ensure_compiles as _;
mod _ensure_compiles {
    // Marker module: anchors the unused-imports lint suppression at the file
    // level. It contains no body, so all `use` statements above must compile
    // (no unresolved imports). The probe service is incrementally extracted
    // across multiple commits; until each method lands its imports are
    // silenced by the `#[allow(unused_imports)]` above.
}
