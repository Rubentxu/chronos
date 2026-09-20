//! `NativeProbeControllerImpl` — production impl of the
//! `chronos_domain::ports::NativeProbeController` port.
//!
//! REC-C3-hexagonal-closure / Etapa A.2.
//!
//! This is a thin wrapper around [`NativeProbeBackend`] that adapts
//! the concrete backend's capability methods to the
//! capability-focused port the application layer consumes.
//!
//! Design notes:
//!
//! - The wrapper holds an `Arc<NativeProbeBackend>` plus the
//!   `CaptureSession` returned by `attach_to_pid`. The
//!   `CaptureSession` is needed because the backend's `stop_probe`,
//!   `advance` and `step` methods take `&CaptureSession` (they were
//!   designed before the port existed).
//! - `attach_to_pid` forwards directly to
//!   `NativeProbeBackend::attach_probe` and stores the returned
//!   session.
//! - `stop`, `advance`, `step` forward to the backend with the
//!   stored session.
//! - `execution_log` returns the backend's
//!   `Option<Arc<dyn ExecutionLogProvider>>`.
//! - `resolver_pipeline` returns `None` for now: the backend owns a
//!   `ResolverPipeline` struct (not a single `Box<dyn
//!   SemanticResolver>`), and exposing it through the port would
//!   require widening the contract. The composition root is
//!   expected to attach a real resolver via a follow-up wiring slice
//!   (audit §3.3 — composition root owns the wiring).

use std::fmt::Debug;
use std::sync::Arc;

use chronos_domain::error::TraceError;
use chronos_domain::ports::execution_log::ExecutionLogProvider;
use chronos_domain::ports::{AdvanceOutcome, NativeProbeController, StepOutcome};
use chronos_domain::session_id::SessionId;
use chronos_domain::trace::{CaptureConfig, CaptureSession};

use crate::probe_backend::NativeProbeBackend;

/// Production implementation of [`NativeProbeController`].
pub struct NativeProbeControllerImpl {
    backend: Arc<NativeProbeBackend>,
    session_id: SessionId,
    session: CaptureSession,
}

impl Debug for NativeProbeControllerImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeProbeControllerImpl")
            .field("session_id", &self.session_id)
            .field("session", &self.session.session_id)
            .finish_non_exhaustive()
    }
}

impl NativeProbeControllerImpl {
    /// Build a wrapper around `backend` for an already-attached
    /// session.
    ///
    /// `session_id` is the canonical session id (a typed
    /// `SessionId`). `session` is the concrete `CaptureSession`
    /// returned by `NativeProbeBackend::attach_probe`; it carries
    /// the metadata (`pid`, `language`, `config`) the backend needs
    /// to forward `stop`/`advance`/`step`.
    pub fn new(
        backend: Arc<NativeProbeBackend>,
        session_id: SessionId,
        session: CaptureSession,
    ) -> Self {
        Self {
            backend,
            session_id,
            session,
        }
    }
}

impl NativeProbeController for NativeProbeControllerImpl {
    fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    fn attach_to_pid(
        &self,
        pid: u32,
        config: &CaptureConfig,
    ) -> Result<CaptureSession, TraceError> {
        // The backend takes ownership of the config; clone for the
        // forwarding call so the caller can keep using `config`.
        self.backend.attach_probe(pid, config.clone())
    }

    fn start(
        &self,
        config: &CaptureConfig,
        track_function_frames: bool,
    ) -> Result<CaptureSession, TraceError> {
        self.backend
            .start_probe(config.clone(), track_function_frames)
    }

    fn stop(&self) -> Result<(), TraceError> {
        self.backend.stop_probe(&self.session)
    }

    fn advance(&self) -> Result<AdvanceOutcome, TraceError> {
        self.backend.advance(&self.session)?;
        // The Tren B slice E partial returns `()`. The port
        // discriminates `(advanced, paused_reason, running)`. We
        // surface `advanced = true` and a placeholder pause reason
        // because the underlying `PtraceTracer` does not currently
        // surface kernel event strings. `running = true` because a
        // successful advance means the tracee was signalled to
        // continue.
        //
        // TODO(REC-C3.4 or later): thread the kernel event through
        // `PtraceTracer` so `paused_reason` carries the actual event
        // ("SIGTRAP", "single-step", etc.).
        Ok((true, Some("advanced".to_string()), true))
    }

    fn step(&self) -> Result<StepOutcome, TraceError> {
        self.backend.step(&self.session)?;
        Ok((true, Some("single-step".to_string())))
    }

    fn execution_log(&self) -> Option<Arc<dyn ExecutionLogProvider>> {
        self.backend.execution_log()
    }

    fn clone_resolver_pipeline(&self) -> chronos_domain::semantic::ResolverPipeline {
        self.backend.clone_resolver_pipeline()
    }

    fn resolve_context(
        &self,
        binary_path: Option<String>,
    ) -> chronos_domain::semantic::ResolveContext {
        self.backend.resolve_context(binary_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_format_does_not_panic() {
        // Smoke test: the impl is `Debug` and the formatter does not
        // touch the backend (which may hold a lock).
        let backend = Arc::new(NativeProbeBackend::new());
        let session = CaptureSession {
            session_id: "stub".to_string(),
            pid: 1,
            language: chronos_domain::Language::C,
            started_at: std::time::Instant::now(),
            started_at_wallclock: std::time::SystemTime::now(),
            config: CaptureConfig::new("stub"),
            state: chronos_domain::trace::SessionState::Active,
        };
        let session_id = SessionId::from("stub");
        let ctrl = NativeProbeControllerImpl::new(backend, session_id, session);
        let formatted = format!("{:?}", ctrl);
        assert!(formatted.contains("NativeProbeControllerImpl"));
    }
}
