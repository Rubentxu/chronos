//! Native ptrace probe backend that feeds events to an EventBus in real-time.
//!
//! This backend replaces the "record everything then analyze" model of `CaptureRunner`
//! with a live event bus model. Events are pushed to an `EventBus` ring buffer
//! as they occur, allowing real-time monitoring and querying.
//!
//! ## m1-03: dual-write to `ExecutionLog`
//!
//! When a segment-log directory is configured via
//! [`NativeProbeBackend::with_execution_log_dir`], every `TraceEvent`
//! is also pushed to a per-session `SegmentedExecutionLog`. The
//! legacy EventBus path stays intact for callers that have not
//! opted into the new persistence backend (m1-01 / m0-01 UATs
//! continue to pass).

use crate::native_adapter::NativeAdapter;
use crate::ptrace_tracer::{PtraceConfig, PtraceTracer};
use crate::symbol_resolver::SymbolResolver;
use chronos_domain::bus::EventBusHandle;
use chronos_domain::semantic::{ResolveContext, ResolverPipeline, SemanticEvent, SemanticResolver};
use chronos_domain::{
    CaptureConfig, CaptureSession, Language, ProbeBackend, SourceLocation, TraceError, TraceEvent,
};
use chronos_log::{ExecutionPayload, NewExecutionRecord, SegmentedConfig, SegmentedExecutionLog};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use tracing::{debug, error, info, warn};

/// Serialize a `TraceEvent` into a `NewExecutionRecord` suitable for
/// `SegmentedExecutionLog::append`. The `tag` is set to
/// `trace_event.category` so consumers can filter by trace type.
fn trace_event_to_log_record(
    session_id: &str,
    monotonic_ns: u64,
    event: &TraceEvent,
) -> NewExecutionRecord {
    // Use serde_json as the body format — TraceEvent already derives
    // Serialize via chronos-domain's schemars feature.
    let payload_bytes = serde_json::to_vec(event).unwrap_or_default();
    NewExecutionRecord {
        session_id: chronos_log::SessionId::new(session_id),
        monotonic_ns,
        payload: ExecutionPayload::new(payload_bytes, format!("{:?}", event.event_type)),
    }
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

/// Native ptrace probe backend for real-time event bus feeding.
pub struct NativeProbeBackend {
    /// Shared event bus handle.
    event_bus: EventBusHandle,
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
    /// Optional `ExecutionLog` for the running session. Populated by
    /// `start_probe` so the ptrace thread can record events to a
    /// durable, segmented log alongside the legacy EventBus.
    /// m1-03 migration: read path is dual — see `read_since`.
    execution_log: std::sync::Arc<std::sync::Mutex<Option<std::sync::Arc<SegmentedExecutionLog>>>>,
    /// Directory where segment files are written. `None` means the
    /// `ExecutionLog` is disabled (only the legacy EventBus is used).
    execution_log_dir: std::sync::Arc<std::sync::Mutex<Option<PathBuf>>>,
}

impl NativeProbeBackend {
    /// Create a new native probe backend with the given event bus handle.
    pub fn new(event_bus: EventBusHandle) -> Self {
        Self {
            event_bus,
            language: Language::C,
            resolver_pipeline: ResolverPipeline::new(),
            running: Arc::new(AtomicBool::new(false)),
            thread_handle: std::sync::Mutex::new(None),
            traced_pid: std::sync::Arc::new(std::sync::Mutex::new(None)),
            execution_log: std::sync::Arc::new(std::sync::Mutex::new(None)),
            execution_log_dir: std::sync::Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Configure a directory where `ExecutionLog` segment files
    /// will be written for each new session. Pass `None` to disable
    /// the dual-write to the log.
    pub fn with_execution_log_dir(self, dir: Option<PathBuf>) -> Self {
        if let Some(d) = dir {
            *self
                .execution_log_dir
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = Some(d);
        } else {
            *self
                .execution_log_dir
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = None;
        }
        self
    }

    /// Currently-attached `ExecutionLog`, if any.
    pub fn execution_log(&self) -> Option<std::sync::Arc<SegmentedExecutionLog>> {
        self.execution_log
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Test-only accessor that returns the underlying `Arc<Mutex<…>>`
    /// holding the optional `ExecutionLog`. Lets integration tests
    /// attach a pre-built log so they can exercise
    /// `read_execution_log_records` without spawning a real probe
    /// (which would need root + a target binary). Marked
    /// `#[doc(hidden)]` because it exposes internal mutable state.
    #[doc(hidden)]
    pub fn execution_log_slot_for_test(
        &self,
    ) -> &std::sync::Arc<std::sync::Mutex<Option<std::sync::Arc<SegmentedExecutionLog>>>> {
        &self.execution_log
    }

    /// Read a snapshot of the on-disk log and decode the
    /// `TraceEvent`s it contains. Requires a configured log
    /// directory (see `with_execution_log_dir`).
    ///
    /// On success returns `(records, tail_seq)` where `records` is
    /// a `Vec<TraceEvent>` (deserialized from the log's payload)
    /// and `tail_seq` is the seq counter of the latest record. The
    /// optional `since` arg filters to records with seq strictly
    /// greater than the given value (m1-03 incremental read
    /// support). Use `read_execution_log_records_with_stats` if you
    /// also need the decoder counters surfaced in m1-04.
    pub fn read_execution_log_records(
        &self,
        since: Option<u64>,
        limit: usize,
    ) -> Result<(Vec<TraceEvent>, Option<u64>), TraceError> {
        let (events, tail, _unparseable, _total_seen) =
            self.read_execution_log_records_with_stats(since, limit)?;
        Ok((events, tail))
    }

    /// Variant of `read_execution_log_records` that also returns
    /// decoder counters: `(events, tail_seq, unparseable_payload_count,
    /// total_records_seen)`. `unparseable_payload_count` is the
    /// number of records in the log whose JSON payload did not
    /// decode as a `TraceEvent`. These are still durable on disk;
    /// the counter is the signal that "something else wrote to
    /// this log" (schema drift, alternate producer, corruption).
    pub fn read_execution_log_records_with_stats(
        &self,
        since: Option<u64>,
        limit: usize,
    ) -> Result<(Vec<TraceEvent>, Option<u64>, u64, u64), TraceError> {
        let log = self
            .execution_log
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let log = match log {
            Some(l) => l,
            None => {
                return Err(TraceError::CaptureFailed(
                    "ExecutionLog not configured for this backend".into(),
                ));
            }
        };

        // Read all records from the log via read_after on a
        // fresh consumer. We carry a tiny tag-format skipper
        // because the log payload is JSON-encoded TraceEvent.
        let consumer = chronos_log::LogConsumerId::new("m1-03-query");
        let read = log
            .read_after(&consumer, None)
            .map_err(|e| TraceError::CaptureFailed(format!("log read: {}", e)))?;
        let mut out = Vec::new();
        let mut max_seq: Option<u64> = None;
        let mut unparseable = 0u64;
        let mut total_seen = 0u64;
        if let chronos_log::ReadResult::Ok { records, .. } = read {
            for r in records {
                total_seen += 1;
                if let Some(since) = since {
                    if r.seq.0 <= since {
                        continue;
                    }
                }
                if let Some(prev) = max_seq {
                    if r.seq.0 > prev {
                        max_seq = Some(r.seq.0);
                    }
                } else {
                    max_seq = Some(r.seq.0);
                }
                match serde_json::from_slice::<TraceEvent>(&r.payload.bytes) {
                    Ok(ev) => out.push(ev),
                    Err(_) => {
                        // m1-04: surface the count instead of
                        // silently dropping. The record stays
                        // durable on disk; we just don't try to
                        // decode it.
                        unparseable += 1;
                    }
                }
                if out.len() >= limit {
                    break;
                }
            }
        }
        Ok((out, max_seq, unparseable, total_seen))
    }

    /// Push a `TraceEvent` to the legacy EventBus and, if an
    /// ExecutionLog is attached, also to it. Errors from the log
    /// path are logged but never abort the probe loop.
    fn dual_push(
        event_bus: &EventBusHandle,
        log: Option<&SegmentedExecutionLog>,
        session_log_id: &str,
        trace_event: &TraceEvent,
        timestamp_ns: u64,
    ) {
        event_bus.push_raw(trace_event.clone());
        if let Some(log) = log {
            let rec = trace_event_to_log_record(session_log_id, timestamp_ns, trace_event);
            if let Err(e) = log.append(rec) {
                debug!("m1-03: ExecutionLog append failed (continuing): {}", e);
            }
        }
    }

    /// Create a new native probe backend with a default event bus.
    pub fn with_default_bus() -> Self {
        let bus = chronos_domain::bus::EventBus::new_shared(10000); // 10k event capacity
        Self::new(bus)
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
    /// `TraceEvent` and pushed to the `EventBus` in real-time.
    ///
    /// Returns a `CaptureSession` immediately (non-blocking).
    pub fn start_probe(&self, config: CaptureConfig) -> Result<CaptureSession, TraceError> {
        // HIGH-4: Guard against double-start
        if self.running.load(Ordering::SeqCst) {
            return Err(TraceError::CaptureFailed(
                "A probe is already running on this backend. Call stop_probe first.".into(),
            ));
        }

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
        let event_bus = self.event_bus.clone();
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
        };

        // Build the (placeholder) session up front so we have a
        // stable id for the ExecutionLog directory.
        let session = CaptureSession::new(0, language, config.clone());

        // m1-03: open the ExecutionLog first so the spawned thread
        // can move a clone of the Arc into the closure.
        let log_session_id = format!("native-{}", session.session_id);
        let log_for_thread: Option<std::sync::Arc<SegmentedExecutionLog>> = match self
            .execution_log_dir
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
        {
            Some(base_dir) => {
                let log_dir = base_dir.join(&log_session_id);
                match SegmentedExecutionLog::open(
                    chronos_log::SessionId::new(&log_session_id),
                    SegmentedConfig::with_dir(&log_dir),
                ) {
                    Ok(log) => {
                        let arc = std::sync::Arc::new(log);
                        *self.execution_log.lock().unwrap_or_else(|e| e.into_inner()) =
                            Some(arc.clone());
                        info!(
                            "m1-03: ExecutionLog attached at {:?} for session {}",
                            log_dir, log_session_id
                        );
                        Some(arc)
                    }
                    Err(e) => {
                        warn!(
                            "m1-03: failed to open ExecutionLog at {:?}: {}. \
                             Continuing with legacy EventBus only.",
                            log_dir, e
                        );
                        None
                    }
                }
            }
            None => None,
        };

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
                    event_bus,
                    resolver_pipeline,
                    language,
                    log_for_thread,
                    log_session_id,
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

        let language = config.language.unwrap_or(Language::C);
        let event_bus = self.event_bus.clone();
        let running = self.running.clone();
        let resolver_pipeline = self.resolver_pipeline.clone();

        let ptrace_config = PtraceConfig {
            trace_syscalls: config.capture_syscalls,
            capture_registers: true,
            follow_children: true,
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
                    event_bus,
                    resolver_pipeline,
                    language,
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
    /// interrupt any blocking waitpid. Returns immediately without waiting
    /// for the probe thread to exit (non-blocking).
    pub fn stop_probe(&self, session: &CaptureSession) -> Result<(), TraceError> {
        // CRIT-2: Signal the thread to stop (no spin-wait — the thread will exit
        // naturally when it checks running=false after the next wait_event returns).
        self.running.store(false, Ordering::SeqCst);

        // Best-effort: kill the traced process to unblock waitpid.
        // If the PID isn't published yet, the thread will exit when it checks
        // running=false after launch.
        let pid_to_kill = self
            .traced_pid
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if let Some(pid) = pid_to_kill {
            info!("Killing traced process PID {} to stop probe", pid);
            let _ = nix::sys::signal::kill(
                nix::unistd::Pid::from_raw(pid),
                nix::sys::signal::Signal::SIGKILL,
            );
        } else {
            debug!("stop_probe: traced_pid not yet set, relying on running=false to stop thread");
        }

        // Detach the thread handle without joining — the probe thread will exit
        // on its own once it sees running=false and/or the process is dead.
        // We do NOT join here to avoid blocking the MCP server's response path.
        if let Some(handle) = self
            .thread_handle
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            // HIGH-5: Use a channel to implement join-with-timeout so we don't
            // leak if the probe thread is stuck in waitpid.
            let session_id = session.session_id.clone();
            std::thread::spawn(move || {
                let (tx, rx) = std::sync::mpsc::channel::<()>();
                std::thread::spawn(move || {
                    let _ = handle.join();
                    let _ = tx.send(());
                });
                match rx.recv_timeout(std::time::Duration::from_secs(10)) {
                    Ok(()) => {
                        info!("Probe thread exited cleanly for session {}", session_id)
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        warn!(
                            "Probe thread did not exit within 10s for session {} — abandoning",
                            session_id
                        );
                    }
                    Err(_) => {
                        warn!("Probe thread panicked during shutdown for {}", session_id)
                    }
                }
            });
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
        event_bus: EventBusHandle,
        resolver_pipeline: ResolverPipeline,
        language: Language,
        execution_log: Option<std::sync::Arc<SegmentedExecutionLog>>,
        log_session_id: String,
        on_pid_launched: impl FnOnce(i32),
    ) {
        Self::run_probe_loop(
            program_path,
            args,
            ptrace_config,
            running,
            symbol_resolver,
            event_bus,
            resolver_pipeline,
            language,
            execution_log,
            log_session_id,
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
        event_bus: EventBusHandle,
        resolver_pipeline: ResolverPipeline,
        _language: Language,
        execution_log: Option<std::sync::Arc<SegmentedExecutionLog>>,
        log_session_id: String,
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

                // Push raw event to raw buffer for QueryEngine AND to the
                // m1-03 ExecutionLog if one was attached. dual_push
                // is no-op on the log side when no log is
                // configured, so the EventBus path stays intact
                // for callers that opt out.
                Self::dual_push(
                    &event_bus,
                    execution_log.as_deref(),
                    &log_session_id,
                    &trace_event,
                    timestamp_ns,
                );

                // Resolve to semantic event via the pipeline
                let ctx = ResolveContext {
                    pid: pid as u32,
                    binary_path: Some(program_path.to_string()),
                };
                let semantic_event = resolver_pipeline.resolve(&trace_event, &ctx);
                event_bus.push(semantic_event);

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
        event_bus: EventBusHandle,
        resolver_pipeline: ResolverPipeline,
        _language: Language,
    ) {
        let mut tracer = PtraceTracer::new(ptrace_config.clone());
        let adapter = NativeAdapter::new();

        if let Err(e) = tracer.attach(pid as i32) {
            error!("Failed to attach to PID {}: {}", pid, e);
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
                // Push raw event to raw buffer for QueryEngine
                event_bus.push_raw(trace_event.clone());

                // Resolve to semantic event via the pipeline
                let ctx = ResolveContext {
                    pid,
                    binary_path: None,
                };
                let semantic_event = resolver_pipeline.resolve(&trace_event, &ctx);
                event_bus.push(semantic_event);

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

    /// Drain all buffered semantic events from the event bus.
    fn drain_events(&self) -> Result<Vec<SemanticEvent>, TraceError> {
        Ok(self.event_bus.snapshot())
    }

    /// Non-destructive read on the underlying EventBus (m0-01-live-pagination).
    fn read_since(
        &self,
        cursor: Option<chronos_domain::EventCursor>,
    ) -> chronos_domain::ReadResult {
        self.event_bus.read_since(cursor)
    }

    fn stop_probe(&self, session: &CaptureSession) -> Result<(), TraceError> {
        self.stop_probe(session)
    }

    fn drain_raw_events(&self) -> Vec<TraceEvent> {
        self.drain_raw_events()
    }
}

impl NativeProbeBackend {
    /// Drain all buffered raw trace events from the event bus.
    ///
    /// Used by MCP tools (probe_stop, session_snapshot) to build QueryEngine
    /// which requires the original TraceEvent data.
    pub fn drain_raw_events(&self) -> Vec<TraceEvent> {
        self.event_bus.snapshot_raw()
    }

    /// Get the PID of the currently traced process.
    ///
    /// Returns `None` if the probe hasn't started yet or has already stopped.
    /// For spawned probes, this is set once the child process is launched.
    /// For attached probes, this is set immediately before the event loop starts.
    pub fn get_traced_pid(&self) -> Option<i32> {
        *self.traced_pid.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_probe_backend_creation() {
        let bus = chronos_domain::bus::EventBus::new_shared(100);
        let backend = NativeProbeBackend::new(bus);
        assert_eq!(backend.name(), "native-ptrace");
    }

    #[test]
    fn test_native_probe_backend_is_available() {
        let bus = chronos_domain::bus::EventBus::new_shared(100);
        let backend = NativeProbeBackend::new(bus);
        // Should be true on Linux
        #[cfg(target_os = "linux")]
        assert!(backend.is_available());
    }

    #[test]
    fn test_native_probe_backend_with_language() {
        let bus = chronos_domain::bus::EventBus::new_shared(100);
        let backend = NativeProbeBackend::new(bus).with_language(Language::Rust);
        assert!(backend.is_available());
    }
}
