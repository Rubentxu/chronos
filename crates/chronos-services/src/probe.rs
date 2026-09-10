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

use chronos_domain::bus::EventBus;
use chronos_domain::{CaptureConfig, CaptureSession, Language};
use chronos_native::probe_backend::NativeProbeBackend;
use chronos_domain::adapter::ProbeBackend;
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

/// Input for `ProbeService::drain`.
#[derive(Debug, Clone)]
pub struct ProbeDrainInput {
    pub session_id: String,
    /// Pre-parsed cursor (already converted from `CursorDto`).
    pub cursor: Option<chronos_domain::EventCursor>,
    pub offset: usize,
    pub limit: usize,
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
            hint: "Use probe_drain to read events in real-time, probe_stop to finalize.".to_string(),
        })
    }

    /// Stop a live native probe session.
    ///
    /// Returns the drained raw `TraceEvent`s and metadata so the server-side
    /// wrapper can call `build_and_store_engine` (which still lives on the
    /// server because it touches `engines` and `session_languages`).
    pub fn stop(
        ctx: &ProbeContext<'_>,
        session_id: &str,
    ) -> Result<ProbeStopResult, ServiceError> {
        // Remove the live probe session
        let live_probe = ctx
            .live_probes
            .lock()
            .map_err(|_| ServiceError::LockPoisoned)?
            .remove(session_id);

        let live_probe = live_probe.ok_or_else(|| ServiceError::ProbeNotFound(session_id.to_string()))?;

        // Drain final raw events from the bus (for QueryEngine).
        // drain_raw_events() returns TraceEvent directly, which is what
        // build_and_store_engine needs.
        let events: Vec<TraceEvent> = live_probe.backend.drain_raw_events();

        // Detach any eBPF uprobes this session owned before tearing down the
        // ptrace thread. Best-effort; we still proceed to stop the backend.
        if let Some(adapter) = &live_probe.ebpf_adapter {
            if let Err(e) = adapter.detach_all() {
                tracing::warn!("eBPF detach error for session {}: {}", session_id, e);
            }
        }

        // Stop the probe thread
        if let Err(e) = live_probe.backend.stop_probe(&live_probe.session) {
            tracing::warn!("Probe stop error for session {}: {}", session_id, e);
        }

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
}

/// Language inferred from a file path. Mirrors `Language::from_path` but lives
/// in the service so the server doesn't need to import the domain type just
/// for this.
fn ctx_infer_language(program: &str) -> Language {
    Language::from_path(program)
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
