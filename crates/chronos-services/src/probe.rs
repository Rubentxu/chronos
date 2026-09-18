//! Native probe service — live ptrace-based probes (`probe_*` tool family).
//!
//! This module owns the long-lived state associated with a live native probe:
//! `LiveProbeSession` (carrying the `NativeProbeBackend`, the underlying
//! `CaptureSession`, and the optional eBPF adapter). The MCP-server tool
//! functions in `chronos-mcp` are thin wrappers that delegate here.
//!
//! Browser-probe sessions live in a sibling service (deferred to m5-06b).
//!
//! REC-C2.3 — the canonical author no longer constructs an `EventBus`: the
//! accepted-Raw seam (`accept_raw`) is the only producer, and consumers read
//! the session's `ExecutionLog`. There is no mirror and no parallel source of
//! truth left to keep in sync.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use chronos_domain::{CaptureConfig, CaptureSession, Language};
use chronos_native::probe_backend::NativeProbeBackend;
use tokio::sync::Mutex as TokioMutex;
use tracing::info;

use chronos_query::QueryEngine;

use crate::error::ServiceError;
use crate::output::{ProbeDrainResult, ProbeSnapshotResult, ProbeStartOutput, ProbeStopResult};
use chronos_domain::semantic::{ResolveContext, SemanticEvent};
use chronos_domain::tripwire::TripwireManager;
use chronos_domain::TraceEvent;

/// A live native probe session.
///
/// Unlike `debug_run` which blocks until the program exits, a live probe persists
/// each `TraceEvent` through the accepted-Raw seam (`accept_raw`) and consumers
/// read the session's `ExecutionLog` via `probe_drain` and `probe_stop`.
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
    /// REC-C1.2a: the session OWNS its ExecutionLog, mandatorily.
    ///
    /// Not `Option`: an agentic session that cannot own a log is not
    /// constructed at all (the entrypoint fails instead). The native backend
    /// keeps a clone only for writing, and there is no backend read fallback.
    pub execution_log: crate::session_log::SessionExecutionLog,
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
    /// Session-scoped ExecutionLog registry (REC-C1.3). Outlives `live_probes`
    /// so a stopped session's log stays readable.
    pub execution_logs: &'a crate::session_log::SessionExecutionLogRegistry,
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
    pub track_function_frames: Option<bool>,
    /// Root directory for this session's ExecutionLog (REC-C1.2a). The log is
    /// mandatory: when this is `None` a default root is used, and if the log
    /// cannot be opened the start fails.
    pub execution_log_dir: Option<PathBuf>,
}

/// Input for `ProbeService::start_attach` (m9-77).
///
/// `pid` is the running process to attach to. The dispatcher resolves the
/// binary path from `/proc/<pid>/exe`, so the caller does not supply a
/// program name. `trace_syscalls` mirrors the spawn defaults.
#[derive(Debug, Clone)]
pub struct ProbeAttachInput {
    pub pid: u32,
    pub trace_syscalls: bool,
    /// See [`ProbeStartInput::execution_log_dir`].
    pub execution_log_dir: Option<PathBuf>,
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
}

/// Input for `ProbeService::drain`.
#[derive(Debug, Clone)]
pub struct ProbeDrainInput {
    pub session_id: String,
    /// Canonical `EventsCursorV1` token (`ecv1:...`).
    ///
    /// REC-C2.2.2: the legacy ring cursor (`total_pushed`/`snapshot_len`) is a
    /// different coordinate space and is NEVER converted into an EventSeq.
    /// `None` starts at the log's retained boundary.
    pub evidence_cursor: Option<String>,
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

/// Publish a session's log handle in the registry (REC-C1.3).
///
/// The registry stores a clone of the very handle the session owns, so there is
/// one log and one identity, never a copy.
fn register_session_log(
    ctx: &ProbeContext<'_>,
    log: &crate::session_log::SessionExecutionLog,
) -> Result<(), ServiceError> {
    ctx.execution_logs.register(log.clone())
}

/// Resolve the directory for a session-owned ExecutionLog.
///
/// `explicit` wins when the caller configured one; otherwise the session gets a
/// per-session directory under a stable root. Every agentic session owns a log,
/// so this always returns a path — there is no "log disabled" branch.
///
/// REC-C1.5 closure: the default root comes from
/// [`chronos_log::resolve_execution_log_root`], the single canonical resolver
/// shared by session start, bootstrap, and durable delete.
fn execution_log_dir(explicit: Option<&std::path::Path>, session_id: &str) -> PathBuf {
    let root = match explicit {
        Some(dir) => dir.to_path_buf(),
        None => chronos_log::resolve_execution_log_root(),
    };
    root.join(session_id)
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

        let language = ctx_infer_language(&input.program);

        // REC-C1.2a: canonical ownership order.
        //   1. mint the canonical SessionId,
        //   2. open the ExecutionLog the session will own,
        //   3. hand the backend only a clone for writing.
        // REC-C2.3: there is no longer a parallel `EventBus` to construct; the
        // accepted-Raw seam is the only producer.
        let session_id = uuid::Uuid::new_v4().to_string();
        let owned_log = crate::session_log::SessionExecutionLog::create(
            execution_log_dir(input.execution_log_dir.as_deref(), &session_id),
            chronos_log::SessionId::new(session_id.clone()),
        )?;
        let backend = NativeProbeBackend::new()
            .with_language(language)
            .with_accepted_raw_observer(Self::accepted_raw_observer(
                owned_log.clone(),
                Arc::clone(ctx.tripwire_manager),
            ))
            .attach_execution_log(
                owned_log
                    .legacy_segmented_backend_for_native_bridge()
                    .expect("native probe backend requires a segmented adapter (C33-DEBT-NATIVE-LOG-BRIDGE-01)"),
            );

        // Start the probe (non-blocking — spawns background thread)
        let track_function_frames = input.track_function_frames.unwrap_or(false);
        let session = backend
            .start_probe(config, track_function_frames)
            .map_err(|e| ServiceError::ProbeStartFailed(e.to_string()))?;

        info!(
            "Live probe started for '{}' (session: {})",
            input.program, session_id,
        );

        // Store the live probe session. The log is mandatory and session-owned.
        let live_probe = LiveProbeSession {
            backend,
            session,
            language,
            target: input.program.clone(),
            attached: false,
            ebpf_adapter: None,
            ebpf_attachment: None,
            execution_log: owned_log,
        };
        // Publish the SAME handle in the registry before the session becomes
        // visible to callers: a reader can then resolve the log for the whole
        // logical life of the session, live or stopped.
        crate::probe::register_session_log(ctx, &live_probe.execution_log)?;

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
        // REC-C1.2a: same canonical order as `start` — mint id, own the log,
        // then let the backend write through a clone.
        // REC-C2.3: no EventBus to construct.
        let session_id = uuid::Uuid::new_v4().to_string();
        let owned_log = crate::session_log::SessionExecutionLog::create(
            execution_log_dir(input.execution_log_dir.as_deref(), &session_id),
            chronos_log::SessionId::new(session_id.clone()),
        )?;
        let backend = NativeProbeBackend::new()
            .with_language(language)
            .with_accepted_raw_observer(Self::accepted_raw_observer(
                owned_log.clone(),
                Arc::clone(ctx.tripwire_manager),
            ))
            .attach_execution_log(
                owned_log
                    .legacy_segmented_backend_for_native_bridge()
                    .expect("native probe backend requires a segmented adapter (C33-DEBT-NATIVE-LOG-BRIDGE-01)"),
            );
        let session = backend.attach_probe(input.pid, config).map_err(|e| {
            ServiceError::AttachFailed(format!(
                "NativeProbeBackend::attach_probe({}) failed: {}",
                input.pid, e
            ))
        })?;
        let live = crate::probe::LiveProbeSession {
            backend,
            session,
            language,
            target: target.clone(),
            attached: true,
            ebpf_adapter: None,
            ebpf_attachment: None,
            execution_log: owned_log,
        };
        ctx.live_probes
            .lock()
            .map_err(|_| ServiceError::LockPoisoned)?
            .insert(session_id.clone(), live);
        info!(
            "Live probe attached to pid {} ('{}', session: {})",
            input.pid, target, session_id,
        );
        Ok(ProbeAttachOutput {
            session_id,
            pid: input.pid,
            target,
            language: format!("{:?}", language),
        })
    }

    /// Stop a live native probe session.
    ///
    /// Returns the drained raw `TraceEvent`s and metadata so the server-side
    /// wrapper can call `build_and_store_engine` (which still lives on the
    /// server because it touches `engines` and `session_languages`).
    /// REC-C2.2.0 — build the accepted-Raw observer for a session.
    ///
    /// This is where application policy meets the durable seam: `chronos-native`
    /// persists the `Raw` record and hands over its `source_seq`; the
    /// derivation into durable `TripwireFired` evidence happens here, in the
    /// services layer. Matches `chronos-services` -> `chronos-native`, never
    /// the reverse.
    pub fn accepted_raw_observer(
        log: crate::session_log::SessionExecutionLog,
        manager: Arc<TripwireManager>,
    ) -> chronos_native::probe_backend::AcceptedRawObserver {
        Arc::new(
            move |source_seq, event| match crate::tripwire_evidence::derive_firings_from_event(
                &log, &manager, source_seq, event,
            ) {
                Ok(report) => {
                    for failure in report.failures {
                        tracing::warn!(
                            "tripwire derivation failed for source seq {} (tripwire {}): {}",
                            failure.source_seq.0,
                            failure.tripwire_id,
                            failure.reason
                        );
                    }
                }
                Err(e) => tracing::warn!(
                    "tripwire derivation error at source seq {}: {}",
                    source_seq.0,
                    e
                ),
            },
        )
    }

    pub fn stop(ctx: &ProbeContext<'_>, session_id: &str) -> Result<ProbeStopResult, ServiceError> {
        // REC-C2.2.3: the session's ExecutionLog is the authority for "what was
        // captured". Resolve it BEFORE taking the live-probe lock so a missing
        // log is reported as such rather than as a missing probe.
        let log = ctx.execution_logs.get(session_id)?;

        let mut live_probes = ctx
            .live_probes
            .lock()
            .map_err(|_| ServiceError::LockPoisoned)?;
        // Attached sessions are stopped by the backend with SIGSTOP followed by
        // PTRACE_DETACH, while spawned sessions retain the historical SIGKILL
        // behavior. Both can therefore use the same lifecycle entrypoint.
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

        // Read the session's durable evidence. The probe thread has been joined
        // by `stop_probe`, and every accepted observation was appended BEFORE
        // it was fanned out, so the log now holds everything the capture will
        // ever hold. This is a read, not a drain: nothing is consumed, and the
        // result no longer depends on how much the EventBus ring happened to
        // still be holding.
        let scan = crate::canonical_drain::read_all_raw_events(&log)?;
        let events: Vec<TraceEvent> = scan.events;

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
            completeness: scan.completeness,
            examined_records: scan.examined_records,
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
        // REC-C2.2.2: the canonical route reads the session's ExecutionLog.
        // The EventBus is not consulted and `TripwireManager` is not a
        // parameter, so neither transport nor runtime subscription state can
        // change what a page reports.
        let log = ctx.execution_logs.get(&input.session_id)?;

        // Decode the canonical cursor with the session expectation: a
        // malformed token or one minted for another session is a typed error,
        // never a silent re-anchor.
        let cursor = match input.evidence_cursor.as_deref() {
            Some(token) => Some(
                crate::events_cursor::EventsCursorV1::decode_for_session(token, log.session_id())
                    .map_err(|e| ServiceError::InvalidInput(format!("probe_drain cursor: {e}")))?,
            ),
            None => None,
        };

        // The projection context AND the pipeline come from the session, so
        // replaying the same durable Raw produces the same semantic view the
        // producer saw. Both are taken under a short lock and then the lock is
        // dropped: projection runs per event and must not hold it.
        let (ctx_obj, pipeline) = {
            let probes = ctx
                .live_probes
                .lock()
                .map_err(|_| ServiceError::LockPoisoned)?;
            let live_probe = probes
                .get(&input.session_id)
                .ok_or_else(|| ServiceError::ProbeNotFound(input.session_id.clone()))?;
            (
                live_probe
                    .backend
                    .resolve_context(Some(live_probe.target.clone())),
                live_probe.backend.clone_resolver_pipeline(),
            )
        };

        let project = |event: &chronos_domain::TraceEvent, rc: &ResolveContext| -> SemanticEvent {
            // Pure projection through the same resolver pipeline the producer
            // used; no EventBus access.
            pipeline.resolve(event, rc)
        };

        let page = crate::canonical_drain::read_canonical_drain_page(
            &log,
            cursor.as_ref(),
            // Scan enough raw events that `offset`/`limit` slicing still has the
            // window it was asked for. The cursor is derived from what was
            // EXAMINED, not from the slice that is returned.
            input.offset.saturating_add(input.limit).max(1),
            crate::canonical_drain::DEFAULT_MAX_EXAMINED_RECORDS,
            crate::canonical_drain::DEFAULT_MAX_DERIVED_PER_SOURCE,
            &ctx_obj,
            &project,
        )?;

        Ok(ProbeDrainResult {
            events: page.events,
            evidence_cursor: Some(page.next_cursor.encode()),
            completeness: page.completeness,
            exhausted: page.exhausted,
            total_buffered: page.raw_events,
            tripwires_fired: page.tripwires_fired,
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
        // REC-C1.2/C1.2a: read through the session-owned log, not the backend.
        // NOTE (C1.3 trap): this helper reads fresh with a fixed consumer and
        // filters `since` by hand. It does NOT implement EventsCursorV1 and must
        // not become the canonical cursor reader.
        let owned = &live_probe.execution_log;
        chronos_native::read_log_with_stats(
            &owned.legacy_segmented_backend_for_native_bridge().expect(
                "read_log_with_stats requires a segmented adapter (C33-DEBT-NATIVE-LOG-BRIDGE-01)",
            ),
            since,
            limit,
        )
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
        // REC-C1.2a: counters come from the session-owned log, not the backend.
        // REC-C3.3.1: compaction_metrics is a maintenance capability — the
        // SessionExecutionLog already gates it on segmented providers and
        // returns ServiceError on mismatch.
        Ok(Some(live_probe.execution_log.compaction_metrics()?))
    }

    /// Drain raw events from a live probe session and return them + the
    /// session's language. The server-side wrapper then calls
    /// `build_and_store_engine` and surfaces the `session_snapshot` JSON shape.
    pub fn session_snapshot(
        ctx: &ProbeContext<'_>,
        session_id: &str,
    ) -> Result<ProbeSnapshotResult, ServiceError> {
        let log = ctx.execution_logs.get(session_id)?;
        let language = {
            let probes = ctx
                .live_probes
                .lock()
                .map_err(|_| ServiceError::LockPoisoned)?;
            probes
                .get(session_id)
                .ok_or_else(|| ServiceError::ProbeNotFound(session_id.to_string()))?
                .language
        };

        // REC-C2.2.3: the snapshot is a READ of durable evidence, so it is
        // non-destructive and repeatable. The old path called
        // `drain_raw_events()` against the EventBus ring, which meant a second
        // snapshot saw nothing and a large capture saw only the tail.
        let scan = crate::canonical_drain::read_all_raw_events(&log)?;
        Ok(ProbeSnapshotResult {
            events: scan.events,
            language,
            completeness: scan.completeness,
        })
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

#[cfg(test)]
mod rec_c1_2_tests {
    //! REC-C1.2 / C1.2a — the session owns its ExecutionLog mandatorily, the
    //! log carries one identity, and there is no backend read fallback.

    use super::*;
    use chronos_domain::{CaptureConfig, CaptureSession};
    use chronos_log::{SegmentedConfig, SegmentedExecutionLog};

    fn test_log(tag: &str) -> crate::session_log::SessionExecutionLog {
        let dir = std::env::temp_dir().join(format!(
            "rec-c1-2a-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        crate::session_log::SessionExecutionLog::create(&dir, chronos_log::SessionId::new(tag))
            .expect("test log")
    }

    fn stub_session(execution_log: crate::session_log::SessionExecutionLog) -> LiveProbeSession {
        // REC-C2.3: no EventBus — the backend's only sink is the
        // caller-attached ExecutionLog.
        let backend = NativeProbeBackend::new().attach_execution_log(
            execution_log
                .legacy_segmented_backend_for_native_bridge()
                .expect("stub session uses a segmented adapter"),
        );
        let session = CaptureSession::new(0, Language::Rust, CaptureConfig::new("noop"));
        LiveProbeSession {
            backend,
            session,
            language: Language::Rust,
            target: "noop".to_string(),
            attached: false,
            ebpf_adapter: None,
            ebpf_attachment: None,
            execution_log,
        }
    }

    #[test]
    fn session_log_is_mandatory_and_owned_by_construction() {
        // The type is not `Option`, so "a session without a log" is not even
        // representable. This test pins the invariant that the canonical
        // constructor hands the session a log whose identity matches.
        let live = stub_session(test_log("mandatory"));
        assert_eq!(live.execution_log.session_id().as_str(), "mandatory");
        assert_eq!(
            live.execution_log.cursor_start().session_id().as_str(),
            "mandatory"
        );
    }

    #[test]
    fn backend_and_session_share_one_log_identity() {
        let live = stub_session(test_log("shared"));
        let backend_log = live.backend.execution_log().expect("backend holds a clone");
        // The backend holds the SAME `Arc<SegmentedExecutionLog>` the session
        // adopted through `legacy_segmented_backend_for_native_bridge`. That
        // bridge is the only place where the concrete backend reaches into
        // services for now (C33-DEBT-NATIVE-LOG-BRIDGE-01).
        let session_segmented = live
            .execution_log
            .legacy_segmented_backend_for_native_bridge()
            .expect("session log is segmented");
        assert!(
            std::sync::Arc::ptr_eq(&backend_log, &session_segmented),
            "backend must write to the very log the session owns"
        );
        assert_eq!(
            backend_log.session_id(),
            live.execution_log.session_id(),
            "writer and owner must agree on identity"
        );
    }

    #[test]
    fn log_record_identity_comes_from_the_log_not_a_second_string() {
        // C1.2a removed the redundant `session_log_id` parameter from dual_push;
        // the record identity is `log.session_id()`. This asserts the property
        // that mattered: the log's own identity is authoritative.
        let live = stub_session(test_log("identity"));
        // Read identity through the port, not through the concrete bridge.
        assert_eq!(live.execution_log.session_id().as_str(), "identity");
    }

    #[test]
    fn identity_mismatch_is_rejected_at_adoption() {
        let dir = std::env::temp_dir().join(format!("rec-c1-2a-mm-{}", std::process::id()));
        let handle = std::sync::Arc::new(
            SegmentedExecutionLog::open(
                chronos_log::SessionId::new("native-9999"),
                SegmentedConfig::with_dir(&dir),
            )
            .unwrap(),
        );
        let err = crate::session_log::SessionExecutionLog::try_adopt(
            None,
            chronos_log::SessionId::new("service-uuid"),
            handle,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ServiceError::ExecutionLogIdentityMismatch { .. }
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn session_creation_with_an_unopenable_log_root_fails_loudly() {
        // A directory that cannot be created must fail session creation, not
        // silently fall back to a sink-less session.
        let bad_root = "/proc/definitely-not-creatable/chronos";
        let err = crate::session_log::SessionExecutionLog::create(
            std::path::Path::new(bad_root),
            chronos_log::SessionId::new("sess-fail"),
        )
        .unwrap_err();
        assert!(
            matches!(err, ServiceError::ProbeStartFailed(_)),
            "expected a loud failure, got {err:?}"
        );
    }
}
