//! Probe ports — domain-side abstractions over the probe/query adapters.
//!
//! This module declares four related abstractions:
//!
//! - `ProbeController`: a lifetime-focused trait for one running probe
//!   (who owns it, when does it stop, how does it detach?).
//! - `ProbeFactory`: a factory trait that produces a `ProbeController`
//!   for a given session, picking the right backend for the
//!   requested capabilities.
//! - `ProbeRegistry`: a registry that owns the controllers by session id
//!   and exposes attach/detach/list.
//! - `NativeProbeController` (REC-C3.3.4-native / REC-C3-hexagonal-closure
//!   Etapa A): a capability-focused port that `chronos-services`
//!   consumes instead of the concrete `NativeProbeBackend`.
//!
//! See `REC-C3.1` design (AD-2, AD-5, AD-6) for the rationale behind
//! the lifetime/registry/factory split.
//! See the audit §3.2 A2 + §4.5 S4 for the rationale behind the new
//! capability-focused `NativeProbeController`.

use std::fmt::Debug;
use std::sync::Arc;

use crate::adapter::ProbeBackend;
use crate::capability::{Capability, CapabilityUnavailable};
use crate::error::TraceError;
use crate::ports::execution_log::ExecutionLogProvider;
use crate::semantic::SemanticResolver;
use crate::session_id::SessionId;
use crate::trace::{CaptureConfig, CaptureSession};

/// Lifetime-focused trait for one running probe.
///
/// Split from `ProbeBackend` (which is capability-focused) so that the
/// registry can attach/detach without depending on the backend's
/// capability surface.
pub trait ProbeController: Send + Sync + Debug {
    /// Stable session identity for the probe.
    fn session_id(&self) -> &SessionId;

    /// Stop the probe and release all resources.
    ///
    /// Mirrors `ProbeBackend::stop_probe` semantically; the concrete
    /// implementation may delegate.
    fn stop(&self) -> Result<(), TraceError>;

    /// Consume the boxed controller to release resources exactly once.
    ///
    /// The `Box<Self>` receiver guarantees this cannot be called twice
    /// on the same instance. See AD-6 in the design doc.
    fn detach(self: Box<Self>);

    /// Reach the underlying capability-focused backend.
    fn backend(&self) -> &dyn ProbeBackend;
}

/// Factory trait — pick the best backend for `requirements` and produce
/// a `ProbeController`. Implementations live in infrastructure crates
/// (`chronos_native`, `chronos_ebpf`, `chronos_browser`) and are wired
/// at the composition root.
pub trait ProbeFactory: Send + Sync {
    /// Produce a controller for `session_id` given `config` and the
    /// set of required capabilities. Returns `CapabilityUnavailable`
    /// if no backend can satisfy the request.
    fn create(
        &self,
        session_id: SessionId,
        config: &CaptureConfig,
        requirements: &[Capability],
    ) -> Result<Box<dyn ProbeController>, CapabilityUnavailable>;
}

/// Registry trait — owns controllers by session id.
///
/// The composition root instantiates one registry and hands it to
/// `chronos_services`. Services call `attach` when a probe starts and
/// `detach` when one stops; `list_active` exposes the live set to MCP
/// handlers that need to refuse destructive operations on still-live
/// sessions (REC-C1.6 lifecycle-safe delete).
pub trait ProbeRegistry: Send + Sync {
    /// Register `controller` for `session_id`. Idempotent: if a
    /// controller is already registered for the id, the new one
    /// replaces the old (the old is detached and dropped).
    fn attach(&self, session_id: SessionId, controller: Box<dyn ProbeController>);

    /// Detach and drop the controller for `session_id`. Returns
    /// `TraceError::SessionNotFound` if no controller is registered.
    fn detach(&self, session_id: &SessionId) -> Result<(), TraceError>;

    /// Snapshot of active session ids (cloned for the caller).
    fn list_active(&self) -> Vec<SessionId>;

    /// True iff a controller is registered for `session_id`.
    fn is_active(&self, session_id: &SessionId) -> bool;
}

// =====================================================================
// No-op implementations (AD-5)
// =====================================================================

/// Factory that always returns `CapabilityUnavailable`. Useful for
/// domain unit tests and for the composition root's "no probe
/// configured" fallback.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullProbeFactory;

impl ProbeFactory for NullProbeFactory {
    fn create(
        &self,
        _session_id: SessionId,
        _config: &CaptureConfig,
        requirements: &[Capability],
    ) -> Result<Box<dyn ProbeController>, CapabilityUnavailable> {
        // Pick the first requested capability as the "missing" one
        // for the error variant. Falls back to the eBPF slot if the
        // list is empty (defensive default).
        let err = match requirements.first().copied() {
            Some(Capability::EbpfUprobe) | None => CapabilityUnavailable::ebpf_uprobe(
                "NullProbeFactory: no backend wired for capability",
            ),
            Some(Capability::PtraceAttach) => CapabilityUnavailable::ptrace_attach(
                "NullProbeFactory: no backend wired for capability",
            ),
            Some(Capability::BrowserProbe) => CapabilityUnavailable::browser_probe(
                "NullProbeFactory: no backend wired for capability",
            ),
        };
        Err(err)
    }
}

/// Registry that accepts attach but always detaches as a no-op.
/// Useful as a fallback when no real registry is wired.
#[derive(Debug, Default)]
pub struct NullProbeRegistry {
    inner: std::sync::Mutex<Vec<(SessionId, Box<dyn ProbeController>)>>,
}

impl NullProbeRegistry {
    pub fn new() -> Self {
        Self::default()
    }
}

impl ProbeRegistry for NullProbeRegistry {
    fn attach(&self, session_id: SessionId, controller: Box<dyn ProbeController>) {
        let mut guard = self.inner.lock().expect("NullProbeRegistry mutex poisoned");
        guard.retain(|(id, _)| id != &session_id);
        guard.push((session_id, controller));
    }

    fn detach(&self, session_id: &SessionId) -> Result<(), TraceError> {
        let mut guard = self.inner.lock().expect("NullProbeRegistry mutex poisoned");
        let before = guard.len();
        guard.retain(|(id, _)| id != session_id);
        if guard.len() == before {
            Err(TraceError::SessionNotFound {
                session_id: session_id.to_string(),
            })
        } else {
            Ok(())
        }
    }

    fn list_active(&self) -> Vec<SessionId> {
        let guard = self.inner.lock().expect("NullProbeRegistry mutex poisoned");
        guard.iter().map(|(id, _)| id.clone()).collect()
    }

    fn is_active(&self, session_id: &SessionId) -> bool {
        let guard = self.inner.lock().expect("NullProbeRegistry mutex poisoned");
        guard.iter().any(|(id, _)| id == session_id)
    }
}

// =====================================================================
// `NativeProbeController` — capability-focused port for native ptrace probes
// (REC-C3.3.4-native, REC-C3-hexagonal-closure Etapa A)
// =====================================================================
//
// This trait is the inverse dependency that `chronos-services` consumes
// instead of `chronos_native::probe_backend::NativeProbeBackend`. The
// audit §3.2 A2 flagged that `LiveProbeSession` held a concrete
// `NativeProbeBackend`, which coupled the application layer to a
// specific probe backend.
//
// Design rationale (REC-C3-hexagonal-closure):
//
// 1. **No `backend() -> &dyn ProbeBackend` accessor**. The audit §4.5 S4
//    flagged the equivalent accessor on `ProbeController` as a smell
//    that "permite acceder desde un contrato centrado en el ciclo de
//    vida hacia una interfaz de capacidades más amplia". This trait
//    exposes the capabilities `services/*` actually need directly.
//
// 2. **Returns primitive tuples for advance/step**, not the
//    `AdvanceOutput`/`StepOutput` structs (those live in
//    `chronos-services::output`). This keeps the port contract narrow
//    and ISP-compliant (audit §3.2 A1). The conversion from tuple to
//    output struct lives in `ProbeService`, where the JSON shape is
//    canonicalised.
//
// 3. **Implementations live in `chronos-native`** (production) and in
//    tests (`MockNativeProbeController` in `chronos-services`). The
//    composition root wires the production impl via a factory.

/// Outcome of `advance`: `(advanced, paused_reason, running)`.
///
/// `advanced = true` means the tracee was signalled to continue. If the
/// tracee paused again during the same call, `paused_reason` carries a
/// short string from the kernel event (e.g. `"SIGTRAP"`,
/// `"single-step"`, `None` if no event observed). `running` reports
/// whether the tracee is currently executing.
pub type AdvanceOutcome = (bool, Option<String>, bool);

/// Outcome of `step`: `(stepped, paused_reason)`.
///
/// `stepped = true` means the tracee executed one instruction.
pub type StepOutcome = (bool, Option<String>);

/// Capability-focused port for native ptrace probes.
///
/// Implementations wrap a real probe backend (production) or simulate
/// one (tests). The contract describes the operations
/// `chronos_services::probe::ProbeService` performs on a live session.
///
/// Implementations MUST be `Send + Sync` because they are shared
/// across the lifetime of a session and may be touched from multiple
/// tokio tasks (probe_start spawns the capture thread; probe_stop
/// signals it from another task).
pub trait NativeProbeController: Send + Sync + Debug {
    /// Stable session identity for the controller.
    ///
    /// Same value as the `CaptureSession::session_id` returned by
    /// `attach_to_pid`. The duplicate field exists so that
    /// `LiveProbeSession` can correlate the controller with its
    /// `CaptureSession` without exposing the latter through this trait.
    fn session_id(&self) -> &SessionId;

    /// Attach the probe to an existing process by pid.
    ///
    /// The returned `CaptureSession` is the application's handle to
    /// the running capture; subsequent `advance`/`step`/`stop` calls
    /// operate against the same tracee.
    fn attach_to_pid(&self, pid: i32, config: &CaptureConfig)
        -> Result<CaptureSession, TraceError>;

    /// Stop the probe and release all resources (blocking).
    ///
    /// See `ProbeBackend::stop_probe` for the rationale on blocking
    /// semantics (MS-RACE-FIX, ADR-0005): the caller can rely on a
    /// subsequent read of the execution log observing every event the
    /// probe emitted.
    fn stop(&self) -> Result<(), TraceError>;

    /// Signal the tracee to continue execution.
    ///
    /// Returns `(advanced, paused_reason, running)`. Returns
    /// `TraceError::capture_failed` if there is no traced pid.
    fn advance(&self) -> Result<AdvanceOutcome, TraceError>;

    /// Single-step the tracee by one instruction.
    ///
    /// Returns `(stepped, paused_reason)`. Returns
    /// `TraceError::capture_failed` if there is no traced pid.
    fn step(&self) -> Result<StepOutcome, TraceError>;

    /// Reach the session-owned execution log, if any.
    ///
    /// Returns `None` if no log has been attached via
    /// `attach_execution_log` (production wiring) or if the controller
    /// is a mock with no log.
    fn execution_log(&self) -> Option<Arc<dyn ExecutionLogProvider>>;

    /// Reach the resolver pipeline for snapshot generation.
    ///
    /// Returns `None` if no resolver pipeline has been configured
    /// (production defaults it; mocks may return `None`).
    fn resolver_pipeline(&self) -> Option<Arc<dyn SemanticResolver>>;
}

// =====================================================================
// Tests live under `crates/chronos-domain/tests/ports/probe.rs`
// (B7 in tasks.md). Behavioral coverage for NullProbeFactory and
// NullProbeRegistry is concentrated there.
// =====================================================================
