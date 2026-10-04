//! Behavioral tests for the probe ports (REC-C3.1, B7).
//!
//! Coverage:
//! - `NullProbeFactory::create` returns `CapabilityUnavailable` for
//!   each kind of capability requested (eBPF and ptrace).
//! - `NullProbeRegistry::attach` followed by `list_active` reports
//!   the registered session, and `detach` removes it.
//! - `detach` on an unknown session id surfaces
//!   `TraceError::SessionNotFound`.

use std::sync::Arc;

use chronos_domain::adapter::ProbeBackend;
use chronos_domain::capability::Capability;
use chronos_domain::error::TraceError;
use chronos_domain::ports::{
    NullProbeFactory, NullProbeRegistry, ProbeController, ProbeFactory, ProbeRegistry,
};
use chronos_domain::session_id::SessionId;
use chronos_domain::trace::CaptureConfig;

mod common;
use common::session_id;

// --- A stub ProbeController used only by the registry tests ----------

#[derive(Debug, Clone)]
struct StubController {
    id: SessionId,
}

impl ProbeController for StubController {
    fn session_id(&self) -> &SessionId {
        &self.id
    }

    fn stop(&self) -> Result<(), TraceError> {
        Ok(())
    }

    fn detach(self: Box<Self>) {}

    fn backend(&self) -> &dyn ProbeBackend {
        // No stub backend is needed for these tests; panic since the
        // trait method is never called by the suites here.
        unimplemented!("StubController::backend is never invoked in these tests")
    }
}

// --- Tests -----------------------------------------------------------

fn make_config() -> CaptureConfig {
    CaptureConfig::new("test_target")
}

#[test]
fn null_probe_factory_rejects_ebpf_request() {
    let factory = NullProbeFactory;
    let err = factory
        .create(
            session_id("alpha"),
            &make_config(),
            &[Capability::EbpfUprobe],
        )
        .unwrap_err();
    assert_eq!(err.capability(), Capability::EbpfUprobe);
}

#[test]
fn null_probe_factory_rejects_ptrace_request() {
    let factory = NullProbeFactory;
    let err = factory
        .create(
            session_id("bravo"),
            &make_config(),
            &[Capability::PtraceAttach],
        )
        .unwrap_err();
    assert_eq!(err.capability(), Capability::PtraceAttach);
}

#[test]
fn null_probe_factory_handles_empty_requirements() {
    let factory = NullProbeFactory;
    // Defensive default: empty list falls back to ebpf error.
    let err = factory
        .create(session_id("charlie"), &make_config(), &[])
        .unwrap_err();
    assert_eq!(err.capability(), Capability::EbpfUprobe);
}

#[test]
fn null_probe_factory_works_through_dyn() {
    let factory: Arc<dyn ProbeFactory> = Arc::new(NullProbeFactory);
    let err = factory
        .create(
            session_id("delta"),
            &make_config(),
            &[Capability::PtraceAttach],
        )
        .unwrap_err();
    assert_eq!(err.capability(), Capability::PtraceAttach);
}

#[test]
fn registry_attach_then_detach_round_trip() {
    let registry = NullProbeRegistry::new();
    let id = session_id("echo");
    let controller: Box<dyn ProbeController> = Box::new(StubController { id: id.clone() });

    registry.attach(id.clone(), controller);
    assert!(registry.is_active(&id));
    assert_eq!(registry.list_active(), vec![id.clone()]);

    registry.detach(&id).unwrap();
    assert!(!registry.is_active(&id));
    assert!(registry.list_active().is_empty());
}

#[test]
fn registry_detach_unknown_session_returns_session_not_found() {
    let registry = NullProbeRegistry::new();
    let id = session_id("foxtrot");
    let err = registry.detach(&id).unwrap_err();
    match err {
        TraceError::SessionNotFound { session_id } => {
            assert_eq!(session_id, id.to_string());
        }
        other => panic!("expected SessionNotFound, got {other:?}"),
    }
}

#[test]
fn registry_attach_is_idempotent_via_replace() {
    let registry = NullProbeRegistry::new();
    let id = session_id("golf");
    let first: Box<dyn ProbeController> = Box::new(StubController { id: id.clone() });
    let second: Box<dyn ProbeController> = Box::new(StubController { id: id.clone() });

    registry.attach(id.clone(), first);
    registry.attach(id.clone(), second);
    // Exactly one entry remains; the first was silently replaced.
    assert_eq!(registry.list_active().len(), 1);
}

#[test]
fn registry_works_through_dyn() {
    let registry: Arc<dyn ProbeRegistry> = Arc::new(NullProbeRegistry::new());
    let id = session_id("hotel");
    registry.attach(id.clone(), Box::new(StubController { id: id.clone() }));
    assert!(registry.is_active(&id));
}

// =====================================================================
// LSP coverage for `NativeProbeController` (REC-C3-hexagonal-closure Etapa A.1)
// =====================================================================
//
// These tests exercise the contract as a `dyn NativeProbeController`
// trait object, the way `chronos_services::probe::LiveProbeSession`
// will hold it. A second test exercises a free-standing impl to
// confirm the contract does not depend on any field layout of a
// concrete backend.

use chronos_domain::ports::execution_log::ExecutionLogProvider;
use chronos_domain::ports::{AdvanceOutcome, NativeProbeController, ProbeLiveness, StepOutcome};

#[derive(Debug)]
struct StubNativeController {
    id: SessionId,
    advance_outcome: AdvanceOutcome,
    step_outcome: StepOutcome,
    /// Pre-set liveness so the round-trip test can prove the value crosses
    /// the trait-object boundary instead of being invented downstream.
    liveness: ProbeLiveness,
}

impl NativeProbeController for StubNativeController {
    fn session_id(&self) -> &SessionId {
        &self.id
    }

    fn attach_to_pid(
        &self,
        _pid: u32,
        _config: &CaptureConfig,
    ) -> Result<chronos_domain::CaptureSession, TraceError> {
        // The stub does not spawn a real tracee; tests exercise this
        // branch indirectly via session_id() correlation only.
        Err(TraceError::capture_failed(
            "StubNativeController::attach_to_pid is not exercised in LSP tests",
        ))
    }

    fn start(
        &self,
        _config: &CaptureConfig,
        _track_function_frames: bool,
    ) -> Result<chronos_domain::CaptureSession, TraceError> {
        Err(TraceError::capture_failed(
            "StubNativeController::start is not exercised in LSP tests",
        ))
    }

    fn stop(&self) -> Result<(), TraceError> {
        Ok(())
    }

    fn advance(&self) -> Result<AdvanceOutcome, TraceError> {
        Ok(self.advance_outcome.clone())
    }

    fn step(&self) -> Result<StepOutcome, TraceError> {
        Ok(self.step_outcome.clone())
    }

    fn liveness(&self) -> ProbeLiveness {
        self.liveness
    }

    fn execution_log(&self) -> Option<Arc<dyn ExecutionLogProvider>> {
        None
    }

    fn clone_resolver_pipeline(&self) -> chronos_domain::semantic::ResolverPipeline {
        chronos_domain::semantic::ResolverPipeline::new()
    }

    fn resolve_context(
        &self,
        _binary_path: Option<String>,
    ) -> chronos_domain::semantic::ResolveContext {
        chronos_domain::semantic::ResolveContext {
            pid: 0,
            binary_path: _binary_path,
        }
    }
}

#[test]
fn native_probe_controller_round_trip_via_dyn() {
    // The whole point of the port: services hold a `Box<dyn
    // NativeProbeController>`. This test asserts the trait object
    // surface is the one ProbeService will use.
    let id = session_id("lima");
    let advance_outcome: AdvanceOutcome = (true, Some("single-step".to_string()), false);
    let step_outcome: StepOutcome = (true, Some("SIGTRAP".to_string()));
    let stub = StubNativeController {
        id: id.clone(),
        advance_outcome: advance_outcome.clone(),
        step_outcome: step_outcome.clone(),
        // Not the healthy value on purpose: a test that asserted
        // `Capturing` here would pass even if the port invented the
        // answer, because `Capturing` is the value a consumer would
        // wrongly assume by default.
        liveness: ProbeLiveness::TraceeGone,
    };
    let controller: Box<dyn NativeProbeController> = Box::new(stub);

    // Identity is stable across the trait object boundary.
    assert_eq!(controller.session_id(), &id);

    // Advance / step return the port types exactly.
    assert_eq!(controller.advance().unwrap(), advance_outcome);
    assert_eq!(controller.step().unwrap(), step_outcome);

    // Liveness crosses the boundary verbatim (DEBT-PROBE-LIVENESS-01).
    assert_eq!(controller.liveness(), ProbeLiveness::TraceeGone);
    assert!(!controller.liveness().is_healthy());

    // Stop is observable as a typed outcome.
    assert!(controller.stop().is_ok());

    // Capability surfaces (no backend() / no ProbeBackend exposure).
    assert!(controller.execution_log().is_none());
    // clone_resolver_pipeline returns an empty pipeline; resolver_count() == 0
    assert_eq!(controller.clone_resolver_pipeline().resolver_count(), 0);
}

#[test]
fn native_probe_controller_send_sync() {
    // Compile-time assertion: a `Box<dyn NativeProbeController>` must
    // be usable across tokio task boundaries. This is the property
    // `LiveProbeSession` relies on when probe_stop runs in a
    // different task than probe_start.
    fn assert_send<T: Send + Sync>() {}
    assert_send::<StubNativeController>();
    assert_send::<Box<dyn NativeProbeController>>();
}

#[test]
fn advance_outcome_preserves_pause_reason() {
    // The pause_reason field is the kernel event that triggered the
    // pause. Some events carry None (e.g. plain continue without
    // immediate re-pause); the port contract must preserve that
    // distinction instead of collapsing to empty-string.
    let no_pause: AdvanceOutcome = (true, None, true);
    let with_sigtrap: AdvanceOutcome = (true, Some("SIGTRAP".to_string()), false);

    assert!(no_pause.0);
    assert!(no_pause.1.is_none());
    assert!(no_pause.2);

    assert_eq!(with_sigtrap.1.as_deref(), Some("SIGTRAP"));
}

#[test]
fn step_outcome_preserves_pause_reason() {
    let stepped: StepOutcome = (true, Some("single-step".to_string()));
    let not_stepped: StepOutcome = (false, None);

    assert!(stepped.0);
    assert_eq!(stepped.1.as_deref(), Some("single-step"));
    assert!(!not_stepped.0);
    assert!(not_stepped.1.is_none());
}

/// DEBT-PROBE-LIVENESS-01: only `Capturing` may be called healthy, and
/// every wire spelling must be distinct. Both halves have teeth: an
/// `is_healthy` that accepted a second variant would let a dead session
/// pass for a working one, and a duplicated spelling would collapse two
/// states back into one on the wire — which is the defect in a new coat.
#[test]
fn probe_liveness_healthy_is_only_capturing_and_wire_values_are_distinct() {
    let all = [
        ProbeLiveness::Capturing,
        ProbeLiveness::Starting,
        ProbeLiveness::NotStarted,
        ProbeLiveness::TraceeGone,
        ProbeLiveness::WorkerFinished,
        ProbeLiveness::WorkerStopped,
        ProbeLiveness::WorkerFailed,
    ];

    let mut spellings: Vec<&str> = all.iter().map(|l| l.as_str()).collect();
    let before = spellings.len();
    spellings.sort_unstable();
    spellings.dedup();
    assert_eq!(
        spellings.len(),
        before,
        "every ProbeLiveness variant needs its own wire spelling: {spellings:?}"
    );

    let healthy: Vec<&str> = all
        .iter()
        .filter(|l| l.is_healthy())
        .map(|l| l.as_str())
        .collect();
    assert_eq!(
        healthy,
        vec!["capturing"],
        "only the tracee-confirmed state may be reported healthy"
    );

    // `Display` and `as_str` must not drift apart: both are on the path
    // from a classification to a consumer.
    for liveness in all {
        assert_eq!(liveness.to_string(), liveness.as_str());
    }
}
