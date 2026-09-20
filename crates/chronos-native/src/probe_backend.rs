//! Native ptrace probe backend that persists events to a `SegmentedExecutionLog`
//! via the accepted-Raw seam.
//!
//! This backend no longer constructs an `EventBus` mirror. Every accepted
//! observation is appended to the session's `ExecutionLog` first, and the
//! `accepted_raw_observer` runs **before** anything is fanned out (see
//! `accept_and_publish`). The probe loops take only the `AcceptanceSeam`
//! (`{ log, observer }`) — there is no parallel sink left to keep in sync.
//!
//! ## m1-03: durable write via the accepted-Raw seam
//!
//! Every `TraceEvent` is appended to the attached `SegmentedExecutionLog`
//! through `accept_raw` before anything is observed or fanned out. REC-C2.3
//! retired the parallel `EventBus` mirror: the canonical log is the only
//! sink, and consumers read it directly.

use crate::capture_runner::run_function_frame_capture_with_callback;
use crate::native_adapter::NativeAdapter;
use crate::ptrace_tracer::{PtraceConfig, PtraceTracer};
use crate::symbol_resolver::SymbolResolver;
use chronos_domain::ports::execution_log::ExecutionLogProvider;
use chronos_domain::semantic::{ResolveContext, ResolverPipeline, SemanticResolver};
use chronos_domain::{
    CaptureConfig, CaptureSession, Language, ProbeBackend, SourceLocation, TraceError, TraceEvent,
};
use chronos_log::{ExecutionPayload, NewExecutionRecord, SegmentedConfig, SegmentedExecutionLog};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use tracing::{debug, error, info, warn};

/// Serialize a `TraceEvent` into a `NewExecutionRecord` suitable for
/// `ExecutionLogProvider::append`. The `tag` is set to
/// `trace_event.category` so consumers can filter by trace type.
///
/// The `session_id` argument is supplied by the caller (the live
/// accepted-Raw path uses the provider's own session identity; the
/// `persist_events_to_execution_log` batch helper passes the session
/// id from its own arguments).
///
/// When the event carries function identity — its `data` is
/// `EventData::Function` with `symbol_id` / `invocation_id` /
/// `parent_invocation_id` populated (m2 frame tracking) — those fields are
/// copied onto the record so the v2 segment persists them (m2-07,
/// REQ-BridgeProjectsEventIdentity). Non-Function payloads (syscalls,
/// signals, v1-style events) keep all three identity fields `None`, matching
/// the pre-m2-07 behaviour.
fn trace_event_to_log_record(
    session_id: &str,
    monotonic_ns: u64,
    event: &TraceEvent,
) -> NewExecutionRecord {
    // Use serde_json as the body format — TraceEvent already derives
    // Serialize via chronos-domain's schemars feature.
    let payload_bytes = serde_json::to_vec(event).unwrap_or_default();
    let (invocation_id, parent_invocation_id, symbol_id) = match &event.data {
        chronos_domain::EventData::Function {
            symbol_id,
            invocation_id,
            parent_invocation_id,
            ..
        } => (*invocation_id, *parent_invocation_id, *symbol_id),
        _ => (None, None, None),
    };
    NewExecutionRecord {
        kind: chronos_log::ExecutionKind::Raw,
        session_id: chronos_log::SessionId::new(session_id),
        monotonic_ns,
        payload: ExecutionPayload::new(payload_bytes, format!("{:?}", event.event_type)),
        invocation_id,
        parent_invocation_id,
        symbol_id,
        captured_at_unix_ns: None,
    }
}

/// REC-C3.3.2 — build a `NewExecutionRecord` whose `session_id`
/// comes from the provider. The live accepted-Raw path uses this so
/// the record's identity matches the provider's identity by
/// construction (no second string of session id is transported).
fn trace_event_to_log_record_for_provider(
    log: &dyn ExecutionLogProvider,
    monotonic_ns: u64,
    event: &TraceEvent,
) -> NewExecutionRecord {
    let mut rec = trace_event_to_log_record(log.session_id().as_str(), monotonic_ns, event);
    rec.session_id = log.session_id().clone();
    rec
}

/// Public re-export so integration tests can exercise the
/// `TraceEvent → NewExecutionRecord` mapping the producer uses.
/// Mirrors the private helper used by `dual_push`; stays in lock-step
/// because both call sites live in this module.
#[doc(hidden)]
pub fn trace_event_to_log_record_for_test(
    session_id: &str,
    monotonic_ns: u64,
    event: &TraceEvent,
) -> NewExecutionRecord {
    trace_event_to_log_record(session_id, monotonic_ns, event)
}

/// Persist a slice of captured trace events into a durable `SegmentedExecutionLog`
/// v2 (segment files written under `base_dir/session_id`), using the **same**
/// producer mapping the live-probe dual-write uses (`trace_event_to_log_record`).
///
/// This is the production seam that turns a real INT3 function-frame capture
/// (`CaptureRunner` + `PtraceConfig::track_function_frames = true`) into a
/// queryable ExecutionLog: `FunctionEntry` events keep their `symbol_id` /
/// `invocation_id` / `parent_invocation_id` on the v2 records. The blockingly
/// captured events from a `run_to_completion` are not timestamped, so the
/// in-log ordering is the event index (monotonic_ns = `index`).
///
/// Returns the number of records appended, after a flush makes them durable.
pub fn persist_events_to_execution_log(
    session_id: &str,
    base_dir: &Path,
    events: &[TraceEvent],
) -> Result<usize, String> {
    let log_dir = base_dir.join(session_id);
    std::fs::create_dir_all(&log_dir)
        .map_err(|e| format!("create ExecutionLog dir {}: {e:?}", log_dir.display()))?;
    let log = SegmentedExecutionLog::open(
        chronos_log::SessionId::new(session_id),
        SegmentedConfig::with_dir(&log_dir),
    )
    .map_err(|e| format!("open ExecutionLog for {session_id}: {e:?}"))?;

    let mut appended = 0usize;
    for (idx, event) in events.iter().enumerate() {
        let record = trace_event_to_log_record(session_id, idx as u64, event);
        log.append(record)
            .map_err(|e| format!("append record {idx}: {e:?}"))?;
        appended += 1;
    }
    log.flush()
        .map_err(|e| format!("flush ExecutionLog for {session_id}: {e:?}"))?;
    Ok(appended)
}

/// REC-C2.2.0 — notified **after** a `Raw` event is accepted as durable
/// evidence, and **before** the live fan-out.
///
/// This is the accepted-Raw seam. Application policy (tripwire derivation,
/// counters, anything that must not see unpersisted observations) subscribes
/// here; `chronos-native` only captures and persists, it does not decide what
/// a firing means.
///
/// `source_seq` is the authoritative identity of the accepted source. An
/// observer is never called for an event whose append failed.
pub type AcceptedRawObserver =
    std::sync::Arc<dyn Fn(chronos_log::EventSeq, &TraceEvent) + Send + Sync>;

/// The accepted-Raw seam: the durable log to write through, plus the
/// application hook to notify once the record is durable.
///
/// Bundled so the probe loops take one parameter instead of two and the
/// canonical path is threaded identically for spawn and attach.
///
/// REC-C3.3.2: `log` is an opaque `Arc<dyn ExecutionLogProvider>`; the
/// native backend no longer names `SegmentedExecutionLog` on the live
/// path. The canonical seam is the provider port, not a concrete
/// adapter.
#[derive(Clone, Default)]
pub struct AcceptanceSeam {
    /// Durable log. `None` means "no canonical sink attached".
    pub log: Option<Arc<dyn ExecutionLogProvider>>,
    /// Application hook. `None` means "capture only".
    pub observer: Option<AcceptedRawObserver>,
}

/// Native ptrace probe backend for real-time event bus feeding.
///
/// REC-C3.3.2: `execution_log` and `AcceptanceSeam.log` carry
/// `Arc<dyn ExecutionLogProvider>`. The native backend no longer
/// names `SegmentedExecutionLog` on the live path. Maintenance
/// (`flush`, `compaction_metrics`, `compact_up_to`, `retain_up_to`)
/// is the responsibility of the canonical
/// `chronos_services::session_log::SessionExecutionLog` wrapper that
/// owns the provider; this backend only writes through it.
pub struct NativeProbeBackend {
    /// Language being traced.
    language: Language,
    /// Semantic resolver pipeline.
    resolver_pipeline: ResolverPipeline,
    /// Flag to signal the background thread to stop.
    running: Arc<AtomicBool>,
    /// Handle to the polling thread (if running).
    thread_handle: std::sync::Mutex<Option<thread::JoinHandle<()>>>,
    /// The PID of the currently traced process (for stop_probe to kill).
    traced_pid: std::sync::Arc<std::sync::Mutex<Option<i32>>>,
    /// Whether the current tracee is caller-owned through `attach_probe`.
    /// Such targets must be woken and detached, never terminated.
    attached_target: Arc<AtomicBool>,
    /// REC-C2.2.0: application hook invoked at the accepted-Raw seam.
    accepted_raw_observer: Option<AcceptedRawObserver>,
    /// REC-C3.3.2 — the canonical `ExecutionLogProvider` for the
    /// running session, attached by the caller through
    /// [`NativeProbeBackend::attach_execution_log`]. The backend
    /// only writes through it; it never opens or constructs a
    /// concrete adapter on its own (REC-C1.2a: the session owns
    /// the log, the backend holds a writer clone).
    execution_log: std::sync::Arc<std::sync::Mutex<Option<Arc<dyn ExecutionLogProvider>>>>,
}

impl Default for NativeProbeBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeProbeBackend {
    /// Create a new native probe backend.
    ///
    /// REC-C2.3: no longer takes an `EventBusHandle` — the canonical sink is
    /// the session-owned `ExecutionLog`, attached by the caller through
    /// [`NativeProbeBackend::attach_execution_log`].
    ///
    /// REC-C3.3.2: there is no `with_execution_log_dir` path. The
    /// backend never opens or constructs a concrete adapter on its
    /// own; if no provider was attached, `start_probe` fails closed.
    pub fn new() -> Self {
        Self {
            language: Language::C,
            resolver_pipeline: ResolverPipeline::new(),
            running: Arc::new(AtomicBool::new(false)),
            thread_handle: std::sync::Mutex::new(None),
            traced_pid: std::sync::Arc::new(std::sync::Mutex::new(None)),
            attached_target: Arc::new(AtomicBool::new(false)),
            accepted_raw_observer: None,
            execution_log: std::sync::Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Configure a directory where `ExecutionLog` segment files
    /// will be written for each new session. Pass `None` to disable
    /// the dual-write to the log.
    /// REC-C2.2.2 — project a durable `TraceEvent` into its `SemanticEvent` view.
    ///
    /// A **pure projection**: it runs the resolver pipeline and never touches
    /// any state outside the inputs it received. `probe_drain` builds its wire
    /// events from `ExecutionLog` `Raw` records through this, so
    /// semanticisation stops being a second decision about what occurred and
    /// becomes a *view* of the durable evidence.
    ///
    /// The caller supplies the session's resolution context, so a replay of the
    /// same durable `Raw` produces the same projection the producer produced.
    /// Reconstructing `ResolveContext` from ambient state is exactly how a
    /// projection silently diverges between capture and replay.
    pub fn project_semantic(
        &self,
        event: &TraceEvent,
        ctx: &ResolveContext,
    ) -> chronos_domain::SemanticEvent {
        self.resolver_pipeline.resolve(event, ctx)
    }

    /// A clone of the resolver pipeline.
    ///
    /// Lets a consumer project durable `Raw` records without holding a lock on
    /// the live-probe registry per event. The pipeline is stateless per call.
    pub fn clone_resolver_pipeline(&self) -> ResolverPipeline {
        self.resolver_pipeline.clone()
    }

    /// The resolution context this backend captured with.
    ///
    /// A deterministic projection needs this context to be reconstructible
    /// from session metadata.
    pub fn resolve_context(&self, binary_path: Option<String>) -> ResolveContext {
        let pid = u32::try_from(
            self.traced_pid
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .unwrap_or(0),
        )
        .unwrap_or(0);
        ResolveContext { pid, binary_path }
    }

    /// REC-C2.2.0: install the accepted-Raw observer (canonical path).
    pub fn with_accepted_raw_observer(mut self, observer: AcceptedRawObserver) -> Self {
        self.accepted_raw_observer = Some(observer);
        self
    }

    /// Attach an `ExecutionLogProvider` that the CALLER owns
    /// (REC-C1.2a + REC-C3.3.2).
    ///
    /// This is the canonical path: the composition root creates the
    /// provider, the session owns the wrapper, and the backend holds
    /// a clone of the writer. The backend never names the concrete
    /// adapter and never opens one on its own. `start_probe` fails
    /// closed if this was never called.
    ///
    /// `Arc<dyn ExecutionLogProvider>` (not `&dyn`) because the
    /// backend keeps the writer for the entire probe lifetime; the
    /// capture thread needs the object to outlive the constructor
    /// call.
    pub fn attach_execution_log(&self, log: Arc<dyn ExecutionLogProvider>) {
        *self.execution_log.lock().unwrap_or_else(|e| e.into_inner()) = Some(log);
    }

    /// Currently-attached `ExecutionLogProvider`, if any.
    ///
    /// Returns a clone of the `Arc<dyn ExecutionLogProvider>` so the
    /// caller owns a strong reference. Used by the canonical probe
    /// loop (`AcceptanceSeam::log`) and by `stop_probe` to surface
    /// the final `tail_seq` to the service layer.
    pub fn execution_log(&self) -> Option<Arc<dyn ExecutionLogProvider>> {
        self.execution_log
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Snapshot of the attached provider, kept as `Arc<dyn ...>` for
    /// the auto-compaction daemon in `chronos-mcp`. The daemon reads
    /// maintenance counters via the canonical
    /// `SessionExecutionLog` wrapper, which lives on the
    /// `LiveProbeSession`; this accessor is only here for tests that
    /// want to verify the writer is attached.
    #[doc(hidden)]
    pub fn execution_log_slot_for_test(
        &self,
    ) -> &std::sync::Arc<std::sync::Mutex<Option<Arc<dyn ExecutionLogProvider>>>> {
        &self.execution_log
    }

    /// REC-C2.1/C2.2.0 — **persist first, observe, fan-out last**.
    ///
    /// Named for what it does; the old name (`dual_push`) described the
    /// dual-write shape that REC-C2 retired.
    ///
    /// When an ExecutionLog provider is attached, the authoritative
    /// append happens FIRST and the application observer runs BEFORE
    /// anything is fanned out. If the append fails, no observation
    /// happens: Chronos must not observe as having happened something
    /// it refused to record.
    ///
    /// REC-C2.3 — no longer takes or pushes to an `EventBusHandle`. The
    /// canonical log is the only sink, and there is no mirror to keep in sync.
    ///
    /// Returns the accepted `EventSeq` when the log took the record, or the
    /// append error. REC-C2.1's derivation step uses the returned seq as the
    /// `source_seq` of any firing this event causes.
    ///
    /// REC-C3.3.2 — error type is `ExecutionLogError`, not the legacy
    /// `LogError`. The "no sink" branch maps to
    /// `ExecutionLogError::Unavailable` rather than a brand-new variant.
    pub fn accept_and_publish(
        log: Option<&Arc<dyn ExecutionLogProvider>>,
        trace_event: &TraceEvent,
        timestamp_ns: u64,
        observer: Option<&AcceptedRawObserver>,
    ) -> Result<
        Option<chronos_log::EventSeq>,
        chronos_domain::ports::execution_log::ExecutionLogError,
    > {
        match log {
            Some(log) => {
                let seq = Self::accept_raw(log.as_ref(), trace_event, timestamp_ns)?;
                // Accepted as durable evidence. The application hook runs
                // BEFORE any fan-out, so no observer can act on an unpersisted
                // observation.
                if let Some(observer) = observer {
                    observer(seq, trace_event);
                }
                Ok(Some(seq))
            }
            None => {
                // No canonical sink attached. The only honest answer
                // is to refuse — the live path does not silently drop
                // events. `Unavailable` reuses the existing variant so
                // we do not have to grow the error surface for what is
                // currently a transitional state (the composition root
                // always wires a sink).
                Err(
                    chronos_domain::ports::execution_log::ExecutionLogError::Unavailable {
                        detail:
                            "REC-C3.3.2: no canonical execution-log provider attached; refusing to \
                         publish an unpersisted observation"
                                .to_string(),
                    },
                )
            }
        }
    }

    /// REC-C2.2.0 — persist a `Raw` record and return its authoritative seq.
    ///
    /// Persistence only. The live fan-out and the application hook are the
    /// caller's business, which is what lets `services` interpose policy
    /// between acceptance and observation.
    ///
    /// REC-C3.3.2 — consumes `&dyn ExecutionLogProvider`. The
    /// `session_id` on the record comes from the provider by
    /// construction, so the canonical-evidence identity is single-sourced.
    fn accept_raw(
        log: &dyn ExecutionLogProvider,
        trace_event: &TraceEvent,
        timestamp_ns: u64,
    ) -> Result<chronos_log::EventSeq, chronos_domain::ports::execution_log::ExecutionLogError>
    {
        let rec = trace_event_to_log_record_for_provider(log, timestamp_ns, trace_event);
        log.append(rec)
    }

    /// Set the language to trace.
    pub fn with_language(mut self, language: Language) -> Self {
        self.language = language;
        self
    }

    /// Add a semantic resolver to the pipeline.
    pub fn with_resolver(mut self, resolver: Box<dyn SemanticResolver>) -> Self {
        self.resolver_pipeline.add_resolver(resolver);
        self
    }

    /// Start a probe for a new process.
    ///
    /// Spawns the target binary via `PtraceTracer::launch()` and starts a background
    /// thread that runs the ptrace event loop. Each ptrace event is converted to a
    /// `TraceEvent` and appended to the session's `ExecutionLog` via the
    /// accepted-Raw seam (REC-C2.3 retired the parallel legacy mirror).
    ///
    /// When `track_function_frames=true` (and a [`SymbolResolver`] was loaded from
    /// the spawned binary), the live thread drives the function-capture branch
    /// (`run_function_frame_capture_with_callback`) so that real `FunctionEntry`
    /// events reach the attached `SegmentedExecutionLog` v2 through the same
    /// accepted-Raw seam as syscall/registers events. See
    /// `m2-native-live-probe-frame-capture` design for the contract.
    ///
    /// Returns a `CaptureSession` immediately (non-blocking).
    pub fn start_probe(
        &self,
        config: CaptureConfig,
        track_function_frames: bool,
    ) -> Result<CaptureSession, TraceError> {
        // HIGH-4: Guard against double-start
        if self.running.load(Ordering::SeqCst) {
            return Err(TraceError::CaptureFailed(
                "A probe is already running on this backend. Call stop_probe first.".into(),
            ));
        }
        self.attached_target.store(false, Ordering::SeqCst);

        let program_path = PathBuf::from(&config.target);

        if !program_path.exists() {
            return Err(TraceError::CaptureFailed(format!(
                "Target binary not found: {}",
                config.target
            )));
        }

        let language = config
            .language
            .unwrap_or_else(|| Language::from_path(&config.target));

        let running = self.running.clone();
        let resolver_pipeline = self.resolver_pipeline.clone();

        // Pre-load symbols from the binary
        let symbol_resolver = {
            let mut resolver = SymbolResolver::new();
            match resolver.load_from_binary(&program_path) {
                Ok(()) => {
                    info!(
                        "Loaded {} symbols from {}",
                        resolver.symbol_count(),
                        config.target
                    );
                    Some(resolver)
                }
                Err(e) => {
                    warn!("Could not load symbols from {}: {}", config.target, e);
                    None
                }
            }
        };

        let ptrace_config = PtraceConfig {
            trace_syscalls: config.capture_syscalls,
            capture_registers: true,
            follow_children: true,
            track_function_frames,
        };

        // Build the (placeholder) session up front so we have a
        // stable id for the ExecutionLog directory.
        let session = CaptureSession::new(0, language, config.clone());

        // REC-C1.2a + REC-C3.3.2: an `ExecutionLogProvider` attached
        // by the caller (the composition root → session → wrapper)
        // is the ONLY canonical sink for this probe. The backend
        // does NOT invent a log from a path and does NOT open
        // `SegmentedExecutionLog` on its own.
        let log_for_thread: Option<Arc<dyn ExecutionLogProvider>> = match self
            .execution_log
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
        {
            Some(arc) => {
                info!(
                    "REC-C1.2a: using caller-owned ExecutionLog for session {}",
                    arc.session_id().as_str()
                );
                Some(arc)
            }
            None => {
                return Err(TraceError::CaptureFailed(
                    "REC-C3.3.2: no canonical execution-log provider attached. Call \
                     NativeProbeBackend::attach_execution_log(...) with the session-owned \
                     provider before start_probe."
                        .into(),
                ));
            }
        };

        let accepted_raw_observer_for_thread = self.accepted_raw_observer.clone();

        // Spawn background thread to run the event loop
        let target = config.target.clone();
        let args = config.args.clone();

        // Shared slot so the thread can publish its PID back for stop_probe to kill.
        let traced_pid_thread = self.traced_pid.clone();
        // Clone for the closure - original `running` stays available for error handling
        let running_clone = running.clone();

        // CRIT-1: Set running=true BEFORE spawn so the thread never sees a stale false
        running.store(true, Ordering::SeqCst);

        let handle = thread::Builder::new()
            .name("chronos-native-probe".into())
            .spawn(move || {
                Self::run_probe_loop_with_pid_cb(
                    &target,
                    args,
                    &ptrace_config,
                    &running_clone,
                    symbol_resolver.as_ref(),
                    resolver_pipeline,
                    language,
                    AcceptanceSeam {
                        log: log_for_thread,
                        observer: accepted_raw_observer_for_thread,
                    },
                    move |pid: i32| {
                        *traced_pid_thread.lock().unwrap_or_else(|e| e.into_inner()) = Some(pid);
                    },
                );
            })
            .map_err(|e| {
                // CRIT-1: Reset running flag on spawn failure
                running.store(false, Ordering::SeqCst);
                TraceError::CaptureFailed(format!("Failed to spawn probe thread: {}", e))
            })?;

        // Store the handle - we need to get the PID first
        // Since the thread manages its own PID, we'll store a placeholder for now
        // The actual PID tracking happens inside the thread
        *self.thread_handle.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle);

        Ok(session)
    }

    /// Attach a probe to an existing process.
    ///
    /// Similar to `start_probe` but uses `PtraceTracer::attach()` instead of `launch()`
    /// to attach to an already-running process.
    pub fn attach_probe(
        &self,
        pid: u32,
        config: CaptureConfig,
    ) -> Result<CaptureSession, TraceError> {
        // HIGH-4: Guard against double-start
        if self.running.load(Ordering::SeqCst) {
            return Err(TraceError::CaptureFailed(
                "A probe is already running on this backend. Call stop_probe first.".into(),
            ));
        }
        self.attached_target.store(true, Ordering::SeqCst);

        let language = config.language.unwrap_or(Language::C);
        let running = self.running.clone();
        let resolver_pipeline = self.resolver_pipeline.clone();
        // REC-C2.2.1: attach accepts through the same seam as spawn.
        let attach_log_for_thread = self.execution_log();
        let attach_observer_for_thread = self.accepted_raw_observer.clone();

        let ptrace_config = PtraceConfig {
            trace_syscalls: config.capture_syscalls,
            capture_registers: true,
            follow_children: true,
            track_function_frames: false,
        };

        // Shared slot so the thread can publish its PID back for stop_probe to kill.
        let traced_pid_thread = self.traced_pid.clone();
        // Clone for the closure - original `running` stays available for error handling
        let running_clone = running.clone();

        // CRIT-1: Set running=true BEFORE spawn so the thread never sees a stale false
        running.store(true, Ordering::SeqCst);

        // Spawn background thread to run the event loop in attach mode
        let handle = thread::Builder::new()
            .name("chronos-native-probe-attach".into())
            .spawn(move || {
                // Set traced_pid at START of thread (before attaching),
                // since we know the PID upfront for attach.
                *traced_pid_thread.lock().unwrap_or_else(|e| e.into_inner()) = Some(pid as i32);
                Self::run_probe_loop_attach(
                    pid,
                    &ptrace_config,
                    &running_clone,
                    resolver_pipeline,
                    language,
                    AcceptanceSeam {
                        log: attach_log_for_thread,
                        observer: attach_observer_for_thread,
                    },
                );
            })
            .map_err(|e| {
                // CRIT-1: Reset running flag on spawn failure
                running.store(false, Ordering::SeqCst);
                TraceError::CaptureFailed(format!("Failed to spawn probe thread: {}", e))
            })?;

        *self.thread_handle.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle);

        let mut session = CaptureSession::new(pid, language, config);
        session.activate();

        Ok(session)
    }

    /// Stop an active probe session.
    ///
    /// Sets the running flag to false and kills the traced process to
    /// interrupt any blocking waitpid, then waits (bounded, 10 s timeout)
    /// for the probe thread to exit before returning (blocking semantics,
    /// MS-RACE-FIX / ADR-0005, bounded by HIGH-5). This guarantees a
    /// subsequent `drain_raw_events()` call observes every event the probe
    /// emitted, while refusing to deadlock the caller if the thread is wedged.
    pub fn stop_probe(&self, session: &CaptureSession) -> Result<(), TraceError> {
        // CRIT-2: Signal the thread to stop (no spin-wait — the thread will exit
        // naturally when it checks running=false after the next wait_event returns).
        self.running.store(false, Ordering::SeqCst);

        // Best-effort: interrupt waitpid. Spawned tracees are owned by Chronos
        // and can be killed. For an attached target, SIGSTOP creates a ptrace
        // stop event; the loop observes `running=false` and PTRACE_DETACHes it.
        // If the PID isn't published yet, the thread will exit when it checks
        // running=false after launch.
        let pid_to_kill = self
            .traced_pid
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if let Some(pid) = pid_to_kill {
            let signal = if self.attached_target.swap(false, Ordering::SeqCst) {
                info!("Stopping attached PID {} for safe ptrace detach", pid);
                nix::sys::signal::Signal::SIGSTOP
            } else {
                info!("Killing spawned process PID {} to stop probe", pid);
                nix::sys::signal::Signal::SIGKILL
            };
            let _ = nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid), signal);
        } else {
            debug!("stop_probe: traced_pid not yet set, relying on running=false to stop thread");
        }

        // BLOCKING: join the probe thread inline before returning so that drain_raw_events
        // called immediately after sees a fully-stopped producer. HIGH-5 timeout guards
        // against a stuck thread so we never deadlock the caller: see
        // [`bounded_join_with_timeout`] for the pattern (spawn a waiter that joins
        // the thread and signals via channel, then recv_timeout on the caller's
        // stack). If the timeout elapses, we detach the waiter and warn (the MCP
        // server response path must not block indefinitely).
        let session_id = session.session_id.clone();
        if let Some(handle) = self
            .thread_handle
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            match bounded_join_with_timeout(handle, std::time::Duration::from_secs(10)) {
                BoundedJoinResult::Joined => {
                    info!("Probe thread exited cleanly for session {}", session_id)
                }
                BoundedJoinResult::Timeout => {
                    warn!(
                        "Probe thread did not exit within 10s for session {} — abandoning",
                        session_id
                    );
                }
                BoundedJoinResult::Panicked => {
                    warn!(
                        "Probe thread panicked during shutdown for session {}",
                        session_id
                    )
                }
            }
        }

        Ok(())
    }

    /// Internal wrapper: calls run_probe_loop with a PID callback.
    #[allow(clippy::too_many_arguments)]
    fn run_probe_loop_with_pid_cb(
        program_path: &str,
        args: Vec<String>,
        ptrace_config: &PtraceConfig,
        running: &Arc<AtomicBool>,
        symbol_resolver: Option<&SymbolResolver>,
        resolver_pipeline: ResolverPipeline,
        language: Language,
        seam: AcceptanceSeam,
        on_pid_launched: impl FnOnce(i32),
    ) {
        Self::run_probe_loop(
            program_path,
            args,
            ptrace_config,
            running,
            symbol_resolver,
            resolver_pipeline,
            language,
            seam,
            on_pid_launched,
        );
    }

    /// Internal: Run the probe event loop for a spawned process.
    #[allow(clippy::too_many_arguments)]
    fn run_probe_loop(
        program_path: &str,
        args: Vec<String>,
        ptrace_config: &PtraceConfig,
        running: &Arc<AtomicBool>,
        symbol_resolver: Option<&SymbolResolver>,
        resolver_pipeline: ResolverPipeline,
        _language: Language,
        seam: AcceptanceSeam,
        on_pid_launched: impl FnOnce(i32),
    ) {
        let mut tracer = PtraceTracer::new(ptrace_config.clone());
        let adapter = NativeAdapter::new();

        // Check running flag before entering launch
        if !running.load(Ordering::Relaxed) {
            return;
        }

        // Launch the target process
        let pid = match tracer.launch(PathBuf::from(program_path).as_path(), &args) {
            Ok(p) => {
                info!("Probe started for PID {}", p);
                // Notify caller of the launched PID so stop_probe can kill it.
                on_pid_launched(p);
                p
            }
            Err(e) => {
                error!("Failed to launch {}: {}", program_path, e);
                return;
            }
        };

        let mut event_id: u64 = 0;

        // Check running flag before entering main loop
        if !running.load(Ordering::Relaxed) {
            if pid > 0 {
                let _ = tracer.kill(pid);
            }
            return;
        }

        // m2-native-live-probe-frame-capture: when the live probe was launched
        // with track_function_frames=true AND we resolved symbols for the
        // spawned binary, drive the function-capture branch in place. The
        // helper streams each TraceEvent (FunctionEntry or InvocationIncomplete)
        // through `on_event`, which we pipe through the same accepted-Raw
        // seam the flat loop uses so frames reach the attached
        // SegmentedExecutionLog v2 (REC-C2.3 retired the legacy half). On
        // any helper error we kill the tracee, fall through to cleanup, and
        // let the thread exit normally.
        if ptrace_config.track_function_frames {
            if let Some(resolver) = symbol_resolver {
                let timestamp_ns = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos() as u64;
                // The live backend exposes an `Arc<AtomicBool>` named `running`
                // (true = continue, false = stop). The capture helper expects a
                // `stop_flag` (true = stop). Bridge the two with a mirror
                // thread that keeps the helper's flag inverse-synchronised
                // with `running`; `stop_probe` (which sets `running=false`)
                // is observed by the helper within at most ~20 ms.
                let stop_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
                    !running.load(Ordering::SeqCst),
                ));
                let stop_flag_for_helper = stop_flag.clone();
                let running_for_refresh = running.clone();
                let _stop_mirror = std::thread::spawn(move || {
                    while running_for_refresh.load(Ordering::SeqCst) {
                        stop_flag_for_helper.store(false, Ordering::SeqCst);
                        std::thread::sleep(std::time::Duration::from_millis(20));
                    }
                    stop_flag_for_helper.store(true, Ordering::SeqCst);
                });
                let helper_result = run_function_frame_capture_with_callback(
                    &mut tracer,
                    pid,
                    PathBuf::from(program_path).as_path(),
                    resolver,
                    &stop_flag,
                    None,
                    |trace_event: TraceEvent| {
                        let accepted = Self::accept_and_publish(
                            seam.log.as_ref(),
                            &trace_event,
                            timestamp_ns,
                            seam.observer.as_ref(),
                        )
                        .is_ok();
                        let ctx = ResolveContext {
                            pid: pid as u32,
                            binary_path: Some(program_path.to_string()),
                        };
                        let _semantic_event = resolver_pipeline.resolve(&trace_event, &ctx);
                        // REC-C2.3: no live fan-out sink — the accepted-Raw
                        // seam is the only producer. Acceptance itself is the
                        // signal.
                        if !accepted {
                            debug!("frame capture: rejected observation (no canonical sink)");
                        }
                        event_id += 1;
                    },
                );
                if let Err(err) = helper_result {
                    warn!(
                        "Live function-frame capture exited with error for PID {}: {} — \
                         killing tracee and returning",
                        pid, err
                    );
                    let _ = tracer.kill(pid);
                }
                // fall through to cleanup
            }
        }

        // Main event loop
        while running.load(Ordering::Relaxed) {
            let ptrace_event = match tracer.wait_event() {
                Ok(Some(event)) => event,
                Ok(None) => {
                    // None from blocking waitpid means ECHILD (no more children) OR
                    // the process was killed/exited. Either way, stop the loop.
                    debug!("Probe: no more traced processes, exiting event loop");
                    break;
                }
                Err(e) => {
                    debug!("wait_event error: {}", e);
                    break;
                }
            };

            let timestamp_ns = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64;

            // Convert to TraceEvent and push to bus
            if let Some(mut trace_event) =
                adapter.ptrace_event_to_trace_event(&ptrace_event, event_id, timestamp_ns)
            {
                // Resolve symbol if available
                if let Some(resolver) = symbol_resolver {
                    let addr = trace_event.location.address;
                    if addr > 0 {
                        if let Some(sym) = resolver.resolve(addr) {
                            trace_event.location = SourceLocation::new(
                                sym.file.as_deref().unwrap_or(""),
                                sym.line.unwrap_or(0),
                                &sym.name,
                                addr,
                            );
                        }
                    }
                }

                // REC-C2.3: the accepted-Raw seam is the only sink; no
                // EventBus fallback. The log (if attached) takes the record
                // and the application observer runs synchronously before the
                // next event is processed.
                let accepted = Self::accept_and_publish(
                    seam.log.as_ref(),
                    &trace_event,
                    timestamp_ns,
                    seam.observer.as_ref(),
                )
                .is_ok();

                // Resolve to semantic event via the pipeline. Kept for the
                // pipeline's own accounting (and so future fan-out can
                // attach without re-resolving); the canonical view is the
                // session's ExecutionLog.
                let ctx = ResolveContext {
                    pid: pid as u32,
                    binary_path: Some(program_path.to_string()),
                };
                let _semantic_event = resolver_pipeline.resolve(&trace_event, &ctx);
                if !accepted {
                    debug!("probe loop: rejected observation (no canonical sink)");
                }

                event_id += 1;
            }

            // Continue the traced process
            let event_pid = ptrace_event.pid();
            if event_pid > 0
                && !matches!(
                    ptrace_event,
                    crate::ptrace_tracer::PtraceEvent::Exited { .. }
                )
                && !matches!(
                    ptrace_event,
                    crate::ptrace_tracer::PtraceEvent::Signaled { .. }
                )
            {
                let continue_result = if ptrace_config.trace_syscalls {
                    tracer.syscall_continue(event_pid)
                } else {
                    tracer.continue_execution(event_pid)
                };
                if let Err(e) = continue_result {
                    debug!("Failed to continue PID {}: {}", event_pid, e);
                }
            }
        }

        // Cleanup
        // CRIT-3: Kill ALL traced PIDs (root + clone children) to prevent zombies
        // when follow_children=true.
        for &child_pid in tracer.traced_pids().iter() {
            if child_pid != pid {
                let _ = nix::sys::signal::kill(
                    nix::unistd::Pid::from_raw(child_pid),
                    nix::sys::signal::Signal::SIGKILL,
                );
                // Reap the zombie
                let _ = nix::sys::wait::waitpid(
                    nix::unistd::Pid::from_raw(child_pid),
                    Some(nix::sys::wait::WaitPidFlag::WNOHANG),
                );
            }
        }
        if let Err(e) = tracer.kill(pid) {
            debug!("Failed to kill root PID {}: {}", pid, e);
        }

        info!("Probe loop ended for PID {}", pid);
    }

    /// Internal: Run the probe event loop for an attached process.
    fn run_probe_loop_attach(
        pid: u32,
        ptrace_config: &PtraceConfig,
        running: &Arc<AtomicBool>,
        resolver_pipeline: ResolverPipeline,
        _language: Language,
        seam: AcceptanceSeam,
    ) {
        let mut tracer = PtraceTracer::new(ptrace_config.clone());
        let adapter = NativeAdapter::new();

        if let Err(e) = tracer.attach(pid as i32) {
            error!("Failed to attach to PID {}: {}", pid, e);
            // HIGH-4: clear `running` so the backend can be reused. The
            // attach thread set it true before the ptrace call; if ptrace
            // fails we must release that flag or every subsequent
            // attach_probe returns the "already running" guard.
            running.store(false, Ordering::SeqCst);
            return;
        }

        info!("Probe attached to PID {}", pid);

        let mut event_id: u64 = 0;

        // Main event loop
        while running.load(Ordering::Relaxed) {
            let ptrace_event = match tracer.wait_event() {
                Ok(Some(event)) => event,
                Ok(None) => {
                    debug!("No more traced processes");
                    break;
                }
                Err(e) => {
                    debug!("wait_event error: {}", e);
                    break;
                }
            };

            let timestamp_ns = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64;

            // Convert to TraceEvent and push to bus
            if let Some(trace_event) =
                adapter.ptrace_event_to_trace_event(&ptrace_event, event_id, timestamp_ns)
            {
                // REC-C2.3: the accepted-Raw seam is the only sink (the
                // attach loop already shared this with spawn in C2.2.1; the
                // EventBus fallback is retired here).
                let accepted = Self::accept_and_publish(
                    seam.log.as_ref(),
                    &trace_event,
                    timestamp_ns,
                    seam.observer.as_ref(),
                )
                .is_ok();

                let ctx = ResolveContext {
                    pid,
                    binary_path: None,
                };
                let _semantic_event = resolver_pipeline.resolve(&trace_event, &ctx);
                if !accepted {
                    debug!("attach loop: rejected observation (no canonical sink)");
                }

                event_id += 1;
            }

            // Continue the traced process
            let event_pid = ptrace_event.pid();
            if event_pid > 0
                && !matches!(
                    ptrace_event,
                    crate::ptrace_tracer::PtraceEvent::Exited { .. }
                )
                && !matches!(
                    ptrace_event,
                    crate::ptrace_tracer::PtraceEvent::Signaled { .. }
                )
            {
                let continue_result = if ptrace_config.trace_syscalls {
                    tracer.syscall_continue(event_pid)
                } else {
                    tracer.continue_execution(event_pid)
                };
                if let Err(e) = continue_result {
                    debug!("Failed to continue PID {}: {}", event_pid, e);
                }
            }
        }

        // Cleanup - detach instead of kill for attached processes
        if let Err(e) = tracer.detach(pid as i32) {
            debug!("Failed to detach from PID {}: {}", pid, e);
        }

        info!("Probe loop ended for attached PID {}", pid);
    }
}

impl ProbeBackend for NativeProbeBackend {
    /// Always returns true on Linux (ptrace is available).
    fn is_available(&self) -> bool {
        cfg!(target_os = "linux")
    }

    /// Returns "native-ptrace".
    fn name(&self) -> &str {
        "native-ptrace"
    }

    fn stop_probe(&self, session: &CaptureSession) -> Result<(), TraceError> {
        self.stop_probe(session)
    }
}

impl NativeProbeBackend {
    /// Get the PID of the currently traced process.
    ///
    /// Returns `None` if the probe hasn't started yet or has already stopped.
    /// For spawned probes, this is set once the child process is launched.
    /// For attached probes, this is set immediately before the event loop starts.
    pub fn get_traced_pid(&self) -> Option<i32> {
        *self.traced_pid.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// REC-C3.3.3 (Tren B slice E) — advance the traced target.
    ///
    /// Resolves the PID via [`Self::get_traced_pid`] and calls
    /// [`PtraceTracer::continue_execution`]. Mirrors the PID
    /// resolution pattern used by [`Self::stop_probe`].
    ///
    /// Returns `TraceError::ProbeNotRunning` when no PID is currently
    /// published (probe has not started or has already stopped). All
    /// other errors propagate from `PtraceTracer::continue_execution`.
    pub fn advance(&self, _session: &CaptureSession) -> Result<(), TraceError> {
        let pid = self
            .get_traced_pid()
            .ok_or_else(|| TraceError::capture_failed("advance called with no traced PID"))?;
        let tracer = PtraceTracer::new(PtraceConfig::default());
        tracer.continue_execution(pid)
    }

    /// REC-C3.3.3 (Tren B slice E) — single-step the traced target by one
    /// instruction.
    ///
    /// Same PID resolution as [`Self::advance`]; delegates to
    /// [`PtraceTracer::step`]. Used by the `probe_step` MCP tool.
    pub fn step(&self, _session: &CaptureSession) -> Result<(), TraceError> {
        let pid = self
            .get_traced_pid()
            .ok_or_else(|| TraceError::capture_failed("step called with no traced PID"))?;
        let tracer = PtraceTracer::new(PtraceConfig::default());
        tracer.step(pid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // REC-C2.0 / REC-C2.1 / REC-C2.3 characterizations
    // (measure reality; not aspirational).
    // ------------------------------------------------------------------

    /// REC-C2.3 + REC-C3.3.2 — the accepted-Raw seam is the only producer.
    ///
    /// Replaces the previous "the bus must be empty when append is refused"
    /// characterization (which only made sense while a `EventBus` mirror was
    /// still part of the canonical flow). The same invariant now reads as:
    /// an accepted observation lands in the log, and a refused observation
    /// does not.
    ///
    /// Uses an `InMemoryExecutionLogProvider` so the test exercises the
    /// canonical port path. Native no longer needs a concrete
    /// `SegmentedExecutionLog` to wire a writer.
    #[test]
    fn c2_3_persist_first_accepted_lands_in_log_refused_does_not() {
        use chronos_log::provider::SegmentedExecutionLogProvider;
        let session = chronos_log::SessionId::new("c23-persist-first");
        let dir = std::env::temp_dir().join(format!(
            "chronos-c23-persistfirst-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("mkdir");
        let log_dir = dir.join(session.as_str());
        let concrete = std::sync::Arc::new(
            chronos_log::SegmentedExecutionLog::open(
                session.clone(),
                chronos_log::SegmentedConfig::with_dir(&log_dir),
            )
            .expect("open log"),
        );
        let provider: Arc<dyn ExecutionLogProvider> = Arc::new(SegmentedExecutionLogProvider::new(
            session.clone(),
            concrete.clone(),
        ));
        let log: Arc<dyn ExecutionLogProvider> = Arc::clone(&provider);

        let event = TraceEvent::signal(1, 100, 1, 11, "SIGSEGV", 0);

        // Accepted: the log assigned a seq, and a read returns the event.
        let accepted = NativeProbeBackend::accept_and_publish(Some(&log), &event, 123, None)
            .expect(
                "accept_and_publish returns Ok(Some(seq)) on success; the error channel \
                 surfaces ExecutionLogError::Unavailable when no provider is attached",
            );
        assert!(accepted.is_some(), "the log assigned a seq");
        let (_events, _tail, _unparseable, _seen) =
            read_log_with_stats(provider.as_ref(), None, 16).expect("read log");
        assert_eq!(_events.len(), 1, "accepted ⇒ one record in the log");

        // Refused: seal the log and push again. The append must fail, and a
        // second read must NOT report a new record.
        provider.seal().expect("seal");
        let refused = NativeProbeBackend::accept_and_publish(Some(&log), &event, 124, None);
        assert!(refused.is_err(), "the append is refused");
        let (events_after, _tail_after, _unparseable_after, _seen_after) =
            read_log_with_stats(provider.as_ref(), None, 16).expect("read log after refusal");
        assert_eq!(
            events_after.len(),
            1,
            "refused ⇒ no record appended (no observation without acceptance)"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// REC-C2.2.2 / REC-C2.3 — semanticisation is a projection of durable
    /// evidence, not a second decision about occurrence.
    ///
    /// The previous form consulted an `EventBus` mirror and asserted the bus
    /// stayed empty. With the bus gone (REC-C2.3) the property collapses to
    /// "the projection is a pure function of its inputs and the resolver
    /// pipeline" — which is what this test now asserts.
    #[test]
    fn c2_3_projecting_semantics_is_a_pure_function_of_inputs() {
        let backend = NativeProbeBackend::new();
        let event = TraceEvent::signal(1, 100, 1, 11, "SIGSEGV", 0);

        let ctx = backend.resolve_context(None);
        let a = backend.project_semantic(&event, &ctx);
        let b = backend.project_semantic(&event, &ctx);
        // Same inputs ⇒ same outputs.
        assert_eq!(a.source_event_id, b.source_event_id);
        assert_eq!(a.timestamp_ns, b.timestamp_ns);
        assert_eq!(a.thread_id, b.thread_id);
        assert_eq!(a.description, b.description);
    }

    /// REC-C2.3 + REC-C3.3.2 — the accepted-Raw seam: persist, then observe,
    /// then refuse on no-sink.
    ///
    /// The observer runs exactly once for an accepted observation, and the
    /// `source_seq` it sees is the record's. A refused append (sealed log)
    /// never notifies. With the bus retired, the only signal is "the observer
    /// was called" and "the seq is the record's".
    ///
    /// Uses an `InMemoryExecutionLogProvider` to exercise the canonical port
    /// path; native now needs no concrete `SegmentedExecutionLog` to wire
    /// a writer.
    #[test]
    fn c2_3_accepted_raw_seam_observer_runs_after_persist_and_refused_does_not_notify() {
        use chronos_log::provider::SegmentedExecutionLogProvider;
        let session = chronos_log::SessionId::new("c23-seam");
        let dir = std::env::temp_dir().join(format!(
            "chronos-c23-seam-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("mkdir");
        let log_dir = dir.join(session.as_str());
        let concrete = std::sync::Arc::new(
            chronos_log::SegmentedExecutionLog::open(
                session.clone(),
                chronos_log::SegmentedConfig::with_dir(&log_dir),
            )
            .expect("open log"),
        );
        let provider: Arc<dyn ExecutionLogProvider> = Arc::new(SegmentedExecutionLogProvider::new(
            session.clone(),
            concrete.clone(),
        ));
        let log: Arc<dyn ExecutionLogProvider> = Arc::clone(&provider);

        let event = TraceEvent::signal(1, 100, 1, 11, "SIGSEGV", 0);

        let seen: std::sync::Arc<std::sync::Mutex<Vec<u64>>> = Default::default();
        let seen_for_obs = seen.clone();
        let observer: AcceptedRawObserver = std::sync::Arc::new(move |seq, _ev| {
            seen_for_obs.lock().unwrap().push(seq.0);
        });

        NativeProbeBackend::accept_and_publish(Some(&log), &event, 123, Some(&observer))
            .expect("accepted");

        let observations = seen.lock().unwrap().clone();
        assert_eq!(observations.len(), 1, "the observer ran exactly once");
        assert_eq!(observations[0], 0, "the source seq is the durable record's");

        // A refused append must not notify at all.
        let seen_after = seen.clone();
        let observer2: AcceptedRawObserver = std::sync::Arc::new(move |seq, _ev| {
            seen_after.lock().unwrap().push(seq.0);
        });
        provider.seal().expect("seal");
        assert!(
            NativeProbeBackend::accept_and_publish(Some(&log), &event, 124, Some(&observer2))
                .is_err()
        );
        assert_eq!(
            seen.lock().unwrap().len(),
            1,
            "no notification for an observation that was never accepted"
        );

        // The no-sink branch must refuse as Unavailable
        // (REC-C3.3.2 — no concrete ExecutionLogBackend to fall back to).
        assert!(matches!(
            NativeProbeBackend::accept_and_publish(None, &event, 125, None),
            Err(chronos_domain::ports::execution_log::ExecutionLogError::Unavailable { .. })
        ));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// REC-C2.3 — there is no `EventBus`, so no `EventBus::read_since` to
    /// characterize. The characterization retires with the type.
    ///
    /// The invariant that outlives CHAR-C2-06 is now: an observation the
    /// log accepted is observable through the canonical reader, and the
    /// reader is non-destructive. This is asserted by
    /// `c2_3_persist_first_accepted_lands_in_log_refused_does_not` (above)
    /// and by the canonical drain tests in `chronos-services`. The test is
    /// kept here as a stub that documents the retirement.
    #[test]
    fn char_c2_06_retired_with_eventbus() {
        // The bus is gone; the test exists to mark the retirement.
        // The behaviour CHAR-C2-06 used to characterize — that a read of the
        // ring could empty it for other consumers — was the live-mirror's
        // defining failure mode. Without a live mirror there is no ring to
        // empty, and the canonical reader (chronos-services::canonical_drain)
        // is asserted non-destructive in its own suite.
    }

    #[test]
    fn test_native_probe_backend_creation() {
        let backend = NativeProbeBackend::new();
        assert_eq!(backend.name(), "native-ptrace");
    }

    #[test]
    fn test_native_probe_backend_is_available() {
        let backend = NativeProbeBackend::new();
        // Should be true on Linux
        #[cfg(target_os = "linux")]
        assert!(backend.is_available());
    }

    // ------------------------------------------------------------------
    // REC-C3.3.2 — port-independence regression.
    //
    // The canonical native producer must behave identically when the
    // attached provider is `InMemoryExecutionLogProvider` instead of
    // the default `SegmentedExecutionLog`. This proves the writer path
    // is independent of the storage mechanism (the whole point of the
    // C3.3.2 inversion).
    // ------------------------------------------------------------------

    /// `accept_and_publish` lands an event in an `InMemoryExecutionLogProvider`,
    /// the observer fires exactly once, sealing refuses further appends, and
    /// reading back through the port returns the same record. Mirrors the
    /// `c2_3_*` tests but against a non-default adapter.
    #[test]
    fn c33_native_works_through_in_memory_provider() {
        use chronos_log::provider::InMemoryExecutionLogProvider;
        let session = chronos_log::SessionId::new("c33-inmem");
        let inner = std::sync::Arc::new(chronos_log::memory::InMemoryExecutionLog::new());
        let provider: Arc<dyn ExecutionLogProvider> = Arc::new(InMemoryExecutionLogProvider::new(
            session.clone(),
            inner.clone(),
        ));
        let log: Arc<dyn ExecutionLogProvider> = Arc::clone(&provider);

        let ev = TraceEvent::signal(1, 100, 1, 11, "SIGSEGV", 0);

        // Accepted append ⇒ Some(seq).
        let accepted =
            NativeProbeBackend::accept_and_publish(Some(&log), &ev, 123, None).expect("accepted");
        assert!(accepted.is_some(), "the in-memory provider assigned a seq");

        // Read back through the port — the InMemory adapter implements
        // `read_from_seq` like every other adapter on the port.
        let page = provider
            .read_from_seq(chronos_log::EventSeq::ZERO, 16)
            .expect("read_from_seq");
        assert_eq!(page.records.len(), 1, "exactly one record was appended");
        assert_eq!(page.records[0].session_id, session);

        // Sealed ⇒ refused.
        provider.seal().expect("seal in-memory");
        let refused = NativeProbeBackend::accept_and_publish(Some(&log), &ev, 124, None);
        assert!(
            refused.is_err(),
            "the in-memory provider refuses after seal"
        );

        // No sink ⇒ ExecutionLogError::Unavailable.
        let no_sink = NativeProbeBackend::accept_and_publish(None, &ev, 125, None);
        assert!(matches!(
            no_sink,
            Err(chronos_domain::ports::execution_log::ExecutionLogError::Unavailable { .. })
        ));
    }

    #[test]
    fn test_native_probe_backend_with_language() {
        let backend = NativeProbeBackend::new().with_language(Language::Rust);
        assert!(backend.is_available());
    }

    #[test]
    fn bridge_projects_function_identity_onto_record() {
        // REQ-BridgeProjectsEventIdentity: an EventData::Function carrying
        // symbol/invocation identity must flow onto the NewExecutionRecord.
        let sym = chronos_domain::SymbolId::new("factorial", None, Language::C);
        let inv = chronos_domain::InvocationId::now();
        let parent = chronos_domain::InvocationId::now();
        let event = TraceEvent {
            event_id: 1,
            timestamp_ns: 1000,
            thread_id: 1,
            event_type: chronos_domain::EventType::FunctionEntry,
            location: Default::default(),
            data: chronos_domain::EventData::Function {
                name: "factorial".into(),
                signature: None,
                symbol_id: Some(sym),
                invocation_id: Some(inv),
                parent_invocation_id: Some(parent),
            },
        };
        let rec = super::trace_event_to_log_record_for_test("s", 1000, &event);
        assert_eq!(rec.symbol_id, Some(sym));
        assert_eq!(rec.invocation_id, Some(inv));
        assert_eq!(rec.parent_invocation_id, Some(parent));
    }

    #[test]
    fn bridge_keeps_identity_empty_for_non_function_events() {
        // REQ-BridgeProjectsEventIdentity: non-Function payloads (here a
        // syscall) keep all three identity fields None.
        let event = TraceEvent::syscall_enter(1, 1000, 1, "read", 0, vec![], 0x4000);
        let rec = super::trace_event_to_log_record_for_test("s", 1000, &event);
        assert_eq!(rec.symbol_id, None);
        assert_eq!(rec.invocation_id, None);
        assert_eq!(rec.parent_invocation_id, None);
    }

    // --- bounded_join_with_timeout tests (m9-69) ---
    //
    // The bounded-join pattern used by `stop_probe` is HARD to exercise in a
    // unit test because the production timeout is 10 seconds. By extracting
    // the pattern into `bounded_join_with_timeout`, we can test the timeout
    // branch with a 100ms budget — proving boundedness without a 10s wait.

    /// Happy path: a thread that exits quickly must produce `Joined` within
    /// the timeout window.
    #[test]
    fn bounded_join_returns_joined_when_thread_exits_quickly() {
        let handle = std::thread::spawn(|| {
            // Trivial work; thread exits immediately.
        });
        let started = std::time::Instant::now();
        let result = super::bounded_join_with_timeout(handle, std::time::Duration::from_secs(2));
        let elapsed = started.elapsed();
        assert_eq!(result, super::BoundedJoinResult::Joined);
        assert!(
            elapsed < std::time::Duration::from_millis(500),
            "joined thread should return well under 2s, took {:?}",
            elapsed
        );
    }

    /// Timeout path: a thread that sleeps longer than the timeout must
    /// produce `Timeout` within the timeout window — never block the caller.
    /// This is the test that proves the boundedness of `stop_probe`.
    #[test]
    fn bounded_join_returns_timeout_when_thread_overruns() {
        let handle = std::thread::spawn(|| {
            // Sleep 10 seconds — much longer than the 100ms timeout.
            // We expect the bounded_join to abandon us at ~100ms.
            std::thread::sleep(std::time::Duration::from_secs(10));
        });
        let started = std::time::Instant::now();
        let result =
            super::bounded_join_with_timeout(handle, std::time::Duration::from_millis(100));
        let elapsed = started.elapsed();
        assert_eq!(result, super::BoundedJoinResult::Timeout);
        // Must return within ~250ms (100ms timeout + slack for thread spawn +
        // channel send overhead). The whole point of this assertion is to prove
        // we don't wait 10 seconds here.
        assert!(
            elapsed < std::time::Duration::from_millis(250),
            "bounded_join must return within ~timeout, took {:?}",
            elapsed
        );
        // The waiter thread is now detached and will eventually finish the
        // 10s sleep. No way to assert it from here without a second
        // bounded_join (which we already proved works). The drop of the
        // JoinHandle inside the waiter thread is what makes the test leak
        // the sleeper — acceptable for a unit test that runs in <1s.
    }

    // ---- attach_probe (m9-77) ----
    //
    // attach_probe uses PTRACE_ATTACH which is Linux-only. On non-Linux
    // platforms the unit tests are compiled out and contribute zero coverage;
    // the dispatcher path fails closed before reaching the backend.

    #[cfg(target_os = "linux")]
    #[test]
    fn attach_probe_to_self_sets_running_and_traced_pid() {
        use chronos_domain::CaptureConfig;

        let backend = NativeProbeBackend::new();
        let pid = std::process::id();
        let config = CaptureConfig::new("/usr/bin/true");
        let session = backend.attach_probe(pid, config).expect("attach self");
        // HIGH-4 invariant: attach must have flipped `running` to true.
        assert!(
            backend.running.load(std::sync::atomic::Ordering::SeqCst),
            "attach_probe must mark running=true"
        );
        // traced_pid is set inside the attach thread before any ptrace call,
        // but the test cannot observe it deterministically until the thread
        // has scheduled. Poll briefly.
        let mut traced = None;
        for _ in 0..50 {
            traced = *backend.traced_pid.lock().unwrap_or_else(|e| e.into_inner());
            if traced == Some(pid as i32) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(
            traced,
            Some(pid as i32),
            "traced_pid must equal the attached pid"
        );
        // CaptureSession state must be Active.
        assert!(matches!(
            session.state,
            chronos_domain::SessionState::Active
        ));
        // Tidy: stop_probe brings `running` back to false. Bounded join via
        // the existing helper.
        let handle = backend
            .thread_handle
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        let session_clone = session.clone();
        backend
            .running
            .store(false, std::sync::atomic::Ordering::SeqCst);
        if let Some(h) = handle {
            let _ = super::bounded_join_with_timeout(h, std::time::Duration::from_secs(2));
        }
        // After stop: running=false.
        assert!(
            !backend.running.load(std::sync::atomic::Ordering::SeqCst),
            "running must be cleared after stop"
        );
        // Reference the local to keep the compiler honest about unused
        // warnings if the tidy-up paths above are removed in future edits.
        let _ = session_clone;
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn attach_probe_to_unknown_pid_clears_running_after_ptrace_failure() {
        use chronos_domain::CaptureConfig;

        let backend = NativeProbeBackend::new();
        // 0xfffffffe is a deliberately non-existent pid (it sits in the
        // unmapped range; ESRCH on Linux). The attach thread's ptrace call
        // fails and `run_probe_loop_attach` must clear `running` to release
        // the HIGH-4 guard so the backend is reusable.
        let pid = 0xfffffffe_u32;
        let config = CaptureConfig::new("/usr/bin/true");
        // attach_probe itself returns Ok — the ptrace call happens
        // asynchronously inside the spawned thread.
        let _ = backend
            .attach_probe(pid, config)
            .expect("spawn attach thread");
        // Poll for the HIGH-4 invariant: a failed attach must leave
        // `running=false` so the backend can be reused.
        let mut cleared = false;
        for _ in 0..200 {
            if !backend.running.load(std::sync::atomic::Ordering::SeqCst) {
                cleared = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(
            cleared,
            "failed attach must clear `running` (HIGH-4 invariant)"
        );
    }
}

/// Result of [`bounded_join_with_timeout`].
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum BoundedJoinResult {
    /// The thread completed within the timeout.
    Joined,
    /// The thread did not complete within the timeout; the waiter is detached.
    Timeout,
    /// The thread panicked during shutdown.
    Panicked,
}

/// BLOCKING: wait for `handle` to complete, bounded by `timeout`.
///
/// Mirrors the HIGH-5 design pattern (spawn a waiter that joins the thread
/// and signals via channel, then `recv_timeout` on the caller's stack).
/// If the timeout elapses, the waiter thread is **abandoned** (not joined)
/// because we cannot block the caller indefinitely. The waiter will eventually
/// finish when the thread exits (clean exit or panic) and is detached via
/// the standard `JoinHandle` drop semantics — no zombie risk.
///
/// Used by [`NativeProbeBackend::stop_probe`] with `Duration::from_secs(10)`
/// to enforce the bounded join contract (MS-RACE-FIX / ADR-0005).
///
/// Exposed as `pub(crate)` for unit tests that need to exercise the timeout
/// branch without waiting 10 seconds.
pub(crate) fn bounded_join_with_timeout(
    handle: std::thread::JoinHandle<()>,
    timeout: std::time::Duration,
) -> BoundedJoinResult {
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    std::thread::spawn(move || {
        let _ = handle.join();
        let _ = tx.send(());
    });
    match rx.recv_timeout(timeout) {
        Ok(()) => BoundedJoinResult::Joined,
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => BoundedJoinResult::Timeout,
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => BoundedJoinResult::Panicked,
    }
}

/// REC-C3.3.2 — decode `TraceEvent`s out of an `ExecutionLogProvider`
/// with the m1-04 decoder counters.
///
/// Stateless page walk over `ExecutionLogProvider::read_from_seq`.
/// No `read_after`, no `LogConsumerId`, no `ExecutionLogBackend`: the
/// provider port is the only source of truth.
///
/// **`total_records_seen` is incremented BEFORE applying `since`**, to
/// preserve the m1-04 metric exactly. C3.3.2 is an architectural
/// inversion; redefining the metric is a deliberate, separate
/// decision.
///
/// `CHUNK` is the page size for the stateless loop. Small enough to
/// keep memory bounded on large logs, large enough to avoid one I/O
/// round-trip per record.
const READ_LOG_WITH_STATS_CHUNK: usize = 256;

/// Returns `(events, max_seq, unparseable_payload_count, total_records_seen)`.
///
/// The helper is `pub(crate)` because it is a private decode/aggregate
/// utility, not an authoritative read model. The only legitimate
/// caller inside the native crate is internal diagnostics; production
/// readers go through `chronos_services::session_log`. Sandbox UAT
/// uses this decoder to validate the live capture path end-to-end.
pub fn read_log_with_stats(
    log: &dyn ExecutionLogProvider,
    since: Option<u64>,
    limit: usize,
) -> Result<(Vec<TraceEvent>, Option<u64>, u64, u64), TraceError> {
    let mut out = Vec::new();
    let mut max_seq: Option<u64> = None;
    let mut unparseable = 0u64;
    let mut total_seen = 0u64;

    let mut position = chronos_log::EventSeq::ZERO;
    let chunk = READ_LOG_WITH_STATS_CHUNK.max(limit.max(1));

    loop {
        let page = log
            .read_from_seq(position, chunk)
            .map_err(|e| TraceError::CaptureFailed(format!("log read_from_seq: {}", e)))?;

        for record in page.records {
            // m1-04 metric: count BEFORE applying `since` to preserve
            // the legacy counter exactly.
            total_seen += 1;
            if let Some(since) = since {
                if record.seq.0 <= since {
                    continue;
                }
            }
            max_seq = Some(match max_seq {
                Some(prev) if prev >= record.seq.0 => prev,
                _ => record.seq.0,
            });
            match serde_json::from_slice::<TraceEvent>(&record.payload.bytes) {
                Ok(ev) => out.push(ev),
                Err(_) => {
                    // m1-04: surface the count instead of silently dropping.
                    unparseable += 1;
                }
            }
            if out.len() >= limit {
                return Ok((out, max_seq, unparseable, total_seen));
            }
        }

        if page.exhausted {
            break;
        }
        position = page.position_after;
    }

    Ok((out, max_seq, unparseable, total_seen))
}
