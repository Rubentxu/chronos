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
