//! `NativeProbeControllerFactory` — production-side adapter (REC-C3.5-R.3).
//!
//! Before this module the application layer (`chronos-services`) reached
//! into `chronos_native::probe_backend::NativeProbeBackend` directly to
//! build a probe session. That kept a production edge
//! `chronos-services -> chronos-native` and violated the hexagonal
//! boundary the audit §4.5 S4 flagged.
//!
//! After this module the application layer only consumes the port
//! `chronos_domain::ports::probe::NativeProbeControllerFactory`; the
//! production wiring lives here, in the same crate as the concrete
//! backend, so the composition root can inject it via
//! `chronos_mcp::composition::default_native_probe_controller_factory`.
//!
//! ## Why the seam stops here
//!
//! The factory takes the application hook (the tripwire observer) and
//! the session-scoped `ExecutionLogProvider` as ports, and wires them
//! onto a freshly-minted `NativeProbeBackend`. Internally it still
//! calls `NativeProbeBackend::new().with_language(...).with_accepted_raw_observer(...).start_probe(...)`,
//! but those calls never leak across the crate boundary.

use std::sync::Arc;

use chronos_domain::ports::execution_log::ExecutionLogProvider;
use chronos_domain::ports::{
    NativeProbeBuildError, NativeProbeController, NativeProbeControllerFactory, RawAcceptedObserver,
};
use chronos_domain::session_id::SessionId;
use chronos_domain::trace::{CaptureConfig, CaptureSession, Language};

use crate::native_probe_controller::NativeProbeControllerImpl;
use crate::probe_backend::NativeProbeBackend;
use chronos_domain::TraceError;

/// Production-side implementation of [`NativeProbeControllerFactory`].
///
/// Holds no per-session state; instances are cheap to share
/// (`Arc<ChronosNativeProbeControllerFactory>`).
///
/// The factory is the seam through which the application layer requests
/// a fresh, wired port controller without naming the concrete backend.
/// All the configuration that is intrinsic to the production wiring
/// (default capture-config knobs, kernel capability preflight, etc.)
/// lives inside `build_for_spawn`; nothing crosses the boundary except
/// the port-shaped inputs and outputs.
#[derive(Debug, Default, Clone)]
pub struct ChronosNativeProbeControllerFactory;

impl ChronosNativeProbeControllerFactory {
    pub fn new() -> Self {
        Self
    }
}

impl NativeProbeControllerFactory for ChronosNativeProbeControllerFactory {
    fn build_for_spawn(
        &self,
        config: CaptureConfig,
        session_id: SessionId,
        language: Language,
        log_provider: Arc<dyn ExecutionLogProvider>,
        accepted_raw_observer: Option<RawAcceptedObserver>,
        track_function_frames: bool,
    ) -> Result<(Box<dyn NativeProbeController>, CaptureSession), NativeProbeBuildError> {
        let mut backend = NativeProbeBackend::new().with_language(language);
        if let Some(observer) = accepted_raw_observer {
            // REC-C3.3.2: the observer is the same closure type the
            // native backend uses internally (`AcceptedRawObserver` is
            // structurally identical to `RawAcceptedObserver`); the
            // safe coercion through the same `Arc<dyn Fn>` shape
            // preserves the lifetime annotations.
            backend = backend.with_accepted_raw_observer(observer);
        }
        let backend = Arc::new(backend);
        backend.attach_execution_log(log_provider);

        let session =
            backend
                .start_probe(config, track_function_frames)
                .map_err(|e: TraceError| {
                    NativeProbeBuildError::new(format!("native probe start failed: {e}"))
                })?;

        let controller: Box<dyn NativeProbeController> = Box::new(NativeProbeControllerImpl::new(
            backend,
            session_id,
            session.clone(),
        ));

        Ok((controller, session))
    }

    fn build_for_attach(
        &self,
        config: CaptureConfig,
        pid: u32,
        session_id: SessionId,
        language: Language,
        log_provider: Arc<dyn ExecutionLogProvider>,
        accepted_raw_observer: Option<RawAcceptedObserver>,
    ) -> Result<(Box<dyn NativeProbeController>, CaptureSession), NativeProbeBuildError> {
        let mut backend = NativeProbeBackend::new().with_language(language);
        if let Some(observer) = accepted_raw_observer {
            backend = backend.with_accepted_raw_observer(observer);
        }
        let backend = Arc::new(backend);
        backend.attach_execution_log(log_provider);

        let session = backend.attach_probe(pid, config).map_err(|e: TraceError| {
            NativeProbeBuildError::new(format!("native probe attach failed: {e}"))
        })?;

        let controller: Box<dyn NativeProbeController> = Box::new(NativeProbeControllerImpl::new(
            backend,
            session_id,
            session.clone(),
        ));

        Ok((controller, session))
    }
}
