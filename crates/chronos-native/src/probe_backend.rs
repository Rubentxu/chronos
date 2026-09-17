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
/// `SegmentedExecutionLog::append`. The `tag` is set to
/// `trace_event.category` so consumers can filter by trace type.
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
#[derive(Clone, Default)]
pub struct AcceptanceSeam {
    /// Durable log. `None` is the legacy COMPATIBILITY path (bus only).
    pub log: Option<std::sync::Arc<SegmentedExecutionLog>>,
    /// Application hook. `None` means "capture only".
    pub observer: Option<AcceptedRawObserver>,
}

/// Native ptrace probe backend for real-time event bus feeding.
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
    /// Optional `ExecutionLog` for the running session. Populated by
    /// `start_probe` so the ptrace thread can record events to a
    /// durable, segmented log.
    /// REC-C2.3: read path is canonical (the session-owned log); there is
    /// no EventBus fallback.
    execution_log: std::sync::Arc<std::sync::Mutex<Option<std::sync::Arc<SegmentedExecutionLog>>>>,
    /// Directory where segment files are written. `None` means the
    /// `ExecutionLog` is disabled (legacy callers; deprecated — REC-C2.3
    /// makes the canonical seam mandatory).
    execution_log_dir: std::sync::Arc<std::sync::Mutex<Option<PathBuf>>>,
}

impl NativeProbeBackend {
    /// Create a new native probe backend.
    ///
    /// REC-C2.3: no longer takes an `EventBusHandle` — the canonical sink is
    /// the session-owned `ExecutionLog`, attached by the caller through
    /// [`NativeProbeBackend::attach_execution_log`] or opened by `start_probe`
    /// from `with_execution_log_dir`.
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
            execution_log_dir: std::sync::Arc::new(std::sync::Mutex::new(None)),
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

    /// Snapshot of the attached `ExecutionLog`'s compaction counters
    /// (m1-07). Returns `Ok(None)` if no log is attached; `Ok(Some(zeros))`
    /// if a log is attached but no compaction runs have happened yet.
    pub fn compaction_metrics(&self) -> Result<Option<chronos_log::CompactionMetrics>, TraceError> {
        let log = self
            .execution_log
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let log = match log {
            Some(l) => l,
            None => return Ok(None),
        };
        Ok(Some(log.compaction_metrics()))
    }

    /// Attach an ExecutionLog that the CALLER owns (REC-C1.2a).
    ///
    /// This is the canonical path: the session creates the log, keeps
    /// ownership, and hands the backend only a clone for writing. The backend
    /// therefore never invents a second identity for the canonical log, and the
    /// record `session_id` comes from `log.session_id()`.
    pub fn attach_execution_log(self, log: std::sync::Arc<SegmentedExecutionLog>) -> Self {
        *self.execution_log.lock().unwrap_or_else(|e| e.into_inner()) = Some(log);
        self
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
        read_log_with_stats(&log, since, limit)
    }

    /// REC-C2.1/C2.2.0 — **persist first, observe, fan-out last**.
    ///
    /// Named for what it does; the old name (`dual_push`) described the
    /// dual-write shape that REC-C2 retired.
    ///
    /// When an ExecutionLog is attached, the authoritative append happens
    /// FIRST and the application observer runs BEFORE anything is fanned out.
    /// If the append fails, no observation happens: Chronos must not observe
    /// as having happened something it refused to record.
    ///
    /// REC-C2.3 — no longer takes or pushes to an `EventBusHandle`. The
    /// canonical log is the only sink, and there is no mirror to keep in sync.
    ///
    /// Returns the accepted `EventSeq` when the log took the record, or the
    /// append error. REC-C2.1's derivation step uses the returned seq as the
    /// `source_seq` of any firing this event causes.
    fn accept_and_publish(
        log: Option<&SegmentedExecutionLog>,
        trace_event: &TraceEvent,
        timestamp_ns: u64,
        observer: Option<&AcceptedRawObserver>,
    ) -> Result<Option<chronos_log::EventSeq>, chronos_log::LogError> {
        match log {
            Some(log) => {
                let seq = Self::accept_raw(log, trace_event, timestamp_ns)?;
                // Accepted as durable evidence. The application hook runs
                // BEFORE any fan-out, so no observer can act on an unpersisted
                // observation.
                if let Some(observer) = observer {
                    observer(seq, trace_event);
                }
                Ok(Some(seq))
            }
            None => {
                // Legacy path: no log attached. Without a sink, the only
                // honest answer is to refuse — there is no `EventBus` to
                // absorb the observation. REC-C2.3 retired the bus, so this
                // branch now errors instead of silently dropping the event.
                Err(chronos_log::LogError::AppendFailed {
                    session: "unknown".to_string(),
                    reason: "REC-C2.3: no canonical sink attached; refusing to publish an \
                             unpersisted observation"
                        .to_string(),
                })
            }
        }
    }

    /// REC-C2.2.0 — persist a `Raw` record and return its authoritative seq.
    ///
    /// Persistence only. The live fan-out and the application hook are the
    /// caller's business, which is what lets `services` interpose policy
    /// between acceptance and observation.
    fn accept_raw(
        log: &SegmentedExecutionLog,
        trace_event: &TraceEvent,
        timestamp_ns: u64,
    ) -> Result<chronos_log::EventSeq, chronos_log::LogError> {
        let rec = trace_event_to_log_record(log.session_id().as_str(), timestamp_ns, trace_event);
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
    /// `TraceEvent` and pushed to the `EventBus` in real-time.
    ///
    /// When `track_function_frames=true` (and a [`SymbolResolver`] was loaded from
    /// the spawned binary), the live thread drives the function-capture branch
    /// (`run_function_frame_capture_with_callback`) so that real `FunctionEntry`
    /// events reach both `EventBus` and the attached `SegmentedExecutionLog` v2
    /// through the same `dual_push` seam as syscall/registers events. See
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

        // REC-C1.2a: an ExecutionLog attached by the caller (the session owns it)
        // takes precedence. In that case the backend does NOT invent an
        // identity: the record `session_id` comes from the log itself.
        let caller_owned_log: Option<std::sync::Arc<SegmentedExecutionLog>> = self
            .execution_log
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();

        // Legacy m1-03 path: the backend opens its own log from a configured
        // directory. Kept for compatibility; not the canonical path.
        let accepted_raw_observer_for_thread = self.accepted_raw_observer.clone();
        let legacy_log_id = format!("native-{}", session.session_id);
        let log_for_thread: Option<std::sync::Arc<SegmentedExecutionLog>> = match caller_owned_log {
            Some(arc) => {
                info!(
                    "REC-C1.2a: using caller-owned ExecutionLog for session {}",
                    arc.session_id().as_str()
                );
                Some(arc)
            }
            None => match self
                .execution_log_dir
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
            {
                Some(base_dir) => {
                    let log_session_id = legacy_log_id.clone();
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
                             Continuing without a canonical sink; accept_and_publish \
                             will refuse observations until one is attached.",
                                log_dir, e
                            );
                            None
                        }
                    }
                }
                None => None,
            },
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
        // through `on_event`, which we pipe through the same `dual_push` seam
        // the flat loop uses so frames reach both EventBus and the attached
        // SegmentedExecutionLog v2. On any helper error we kill the tracee,
        // fall through to cleanup, and let the thread exit normally.
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
                            seam.log.as_deref(),
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
                            debug!(
                                "frame capture: rejected observation (no canonical sink)"
                            );
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
                    seam.log.as_deref(),
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
                    seam.log.as_deref(),
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

    /// Stub kept only so the trait still compiles; will be REMOVED from
    /// `ProbeBackend` in C2.3.2 (the trait shrink). The bus is gone, so
    /// this returns a `CursorStale` refusal — the canonical reader is the
    /// session's `ExecutionLog` (`chronos-services::canonical_drain`).
    fn read_since(
        &self,
        _cursor: Option<chronos_domain::EventCursor>,
    ) -> chronos_domain::ReadResult {
        Err(chronos_domain::TraceError::CursorStale {
            expected: 0,
            current: 0,
        })
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
}

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // REC-C2.0 / REC-C2.1 / REC-C2.3 characterizations
    // (measure reality; not aspirational).
    // ------------------------------------------------------------------

    /// REC-C2.3 — the accepted-Raw seam is the only producer.
    ///
    /// Replaces the previous "the bus must be empty when append is refused"
    /// characterization (which only made sense while a `EventBus` mirror was
    /// still part of the canonical flow). The same invariant now reads as:
    /// an accepted observation lands in the log, and a refused observation
    /// does not.
    #[test]
    fn c2_3_persist_first_accepted_lands_in_log_refused_does_not() {
        let dir = std::env::temp_dir().join(format!(
            "chronos-c23-persistfirst-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("mkdir");
        let session = chronos_log::SessionId::new("c23-persist-first");
        let log = SegmentedExecutionLog::open(
            session.clone(),
            chronos_log::SegmentedConfig::with_dir(&dir),
        )
        .expect("open log");

        let event = TraceEvent::signal(1, 100, 1, 11, "SIGSEGV", 0);

        // Accepted: the log assigned a seq, and a read returns the event.
        let accepted = NativeProbeBackend::accept_and_publish(Some(&log), &event, 123, None)
            .expect("accepted");
        assert!(accepted.is_some(), "the log assigned a seq");
        let (events, _tail, _unparseable, _seen) =
            read_log_with_stats(&log, None, 16).expect("read log");
        assert_eq!(events.len(), 1, "accepted ⇒ one record in the log");

        // Refused: seal the log and push again. The append must fail, and a
        // second read must NOT report a new record.
        log.seal().expect("seal");
        let refused = NativeProbeBackend::accept_and_publish(Some(&log), &event, 124, None);
        assert!(refused.is_err(), "the append is refused");
        let (events_after, _tail_after, _unparseable_after, _seen_after) =
            read_log_with_stats(&log, None, 16).expect("read log after refusal");
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

    /// REC-C2.3 — the accepted-Raw seam: persist, then observe, then refuse
    /// on no-sink.
    ///
    /// The observer runs exactly once for an accepted observation, and the
    /// `source_seq` it sees is the record's. A refused append (sealed log)
    /// never notifies. With the bus retired, the only signal is "the observer
    /// was called" and "the seq is the record's".
    #[test]
    fn c2_3_accepted_raw_seam_observer_runs_after_persist_and_refused_does_not_notify() {
        let dir = std::env::temp_dir().join(format!(
            "chronos-c23-seam-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("mkdir");
        let session = chronos_log::SessionId::new("c23-seam");
        let log = SegmentedExecutionLog::open(
            session.clone(),
            chronos_log::SegmentedConfig::with_dir(&dir),
        )
        .expect("open log");

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
        assert_eq!(
            observations[0], 0,
            "the source seq is the durable record's"
        );

        // A refused append must not notify at all.
        let seen_after = seen.clone();
        let observer2: AcceptedRawObserver = std::sync::Arc::new(move |seq, _ev| {
            seen_after.lock().unwrap().push(seq.0);
        });
        log.seal().expect("seal");
        assert!(NativeProbeBackend::accept_and_publish(
            Some(&log),
            &event,
            124,
            Some(&observer2)
        )
        .is_err());
        assert_eq!(
            seen.lock().unwrap().len(),
            1,
            "no notification for an observation that was never accepted"
        );

        // The no-sink branch must also refuse (REC-C2.3 — no bus fallback).
        assert!(matches!(
            NativeProbeBackend::accept_and_publish(None, &event, 125, None),
            Err(chronos_log::LogError::AppendFailed { .. })
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

/// Decode `TraceEvent`s out of a `SegmentedExecutionLog` with the m1-04 decoder
/// counters, given the log handle directly.
///
/// REC-C1.2 relocation: the read path used to live only on `NativeProbeBackend`,
/// which forced readers to go through the backend. The session now owns the log,
/// so the decoding logic is exposed as a free function that any holder of the
/// handle can call. The backend method delegates here, so there is exactly one
/// implementation.
///
/// Returns `(events, max_seq, unparseable_payload_count, total_records_seen)`.
pub fn read_log_with_stats(
    log: &SegmentedExecutionLog,
    since: Option<u64>,
    limit: usize,
) -> Result<(Vec<TraceEvent>, Option<u64>, u64, u64), TraceError> {
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
            max_seq = Some(match max_seq {
                Some(prev) if prev >= r.seq.0 => prev,
                _ => r.seq.0,
            });
            match serde_json::from_slice::<TraceEvent>(&r.payload.bytes) {
                Ok(ev) => out.push(ev),
                Err(_) => {
                    // m1-04: surface the count instead of silently dropping.
                    // The record stays durable on disk; we just don't decode it.
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
