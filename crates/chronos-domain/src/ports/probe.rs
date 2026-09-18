//! Probe ports — domain-side abstractions over the probe/query adapters.
//!
//! This module declares three related abstractions:
//!
//! - `ProbeController`: a lifetime-focused trait for one running probe
//!   (who owns it, when does it stop, how does it detach?).
//! - `ProbeFactory`: a factory trait that produces a `ProbeController`
//!   for a given session, picking the right backend for the
//!   requested capabilities.
//! - `ProbeRegistry`: a registry that owns the controllers by session id
//!   and exposes attach/detach/list.
//!
//! See `REC-C3.1` design (AD-2, AD-5, AD-6) for the rationale behind
//! this trait split.

use std::fmt::Debug;

use crate::adapter::ProbeBackend;
use crate::capability::{Capability, CapabilityUnavailable};
use crate::error::TraceError;
use crate::session_id::SessionId;
use crate::trace::CaptureConfig;

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
// Tests live under `crates/chronos-domain/tests/ports/probe.rs`
// (B7 in tasks.md). Behavioral coverage for NullProbeFactory and
// NullProbeRegistry is concentrated there.
// =====================================================================
