//! REC-C3.3.2.4 — browser probe capability port.
//!
//! `chronos_services::browser_probe` cannot import `chronos_browser`
//! directly; it must consume the browser capability through this port.
//! The composition root (`chronos_mcp::composition`) is the only place
//! in the workspace that constructs the concrete factory, and the
//! services crate only sees the trait objects defined here.
//!
//! ## Why a factory instead of a single adapter
//!
//! Browser probes are **session-scoped**: every `browser_probe_start`
//! call spawns a fresh Chrome process and obtains a dedicated
//! `BrowserProbeBackend`. A shared singleton does not fit the lifecycle.
//! The factory pattern mirrors `ExecutionLogFactory` and
//! `UprobeInjector::acquire`:
//!
//! - `BrowserProbeFactory::create` returns `Arc<dyn BrowserProbeBackend>`
//!   after a capability check (Chrome installed on host).
//! - `BrowserProbeBackend` is the trait the session holds; its methods
//!   are exactly the four the probe service calls (start, stop, drain,
//!   raw_events), so no eBPF/CDP-specific method leaks into the port.
//!
//! ## Why `async fn`
//!
//! `start_probe_async` is genuinely async (it awaits CDP readiness with
//! `tokio::sleep`). Native factories don't need async; the
//! `BrowserProbeFactory` does. We do NOT deform `ExecutionLogFactory`
//! or `UprobeInjector` to match; each factory carries the lifecycle its
//! backend requires.

use std::sync::Arc;

use crate::capability::CapabilityUnavailable;
use crate::{CaptureConfig, CaptureSession, SemanticEvent, TraceEvent};

/// Factory for fresh, session-scoped browser probe backends.
///
/// Every successful `create` returns a new `Arc<dyn BrowserProbeBackend>`
/// the caller owns for the lifetime of the probe session. The factory
/// itself is stateless and cheap to share (`Arc<dyn BrowserProbeFactory>`).
pub trait BrowserProbeFactory: Send + Sync {
    /// Build a fresh backend.
    ///
    /// Returns `BrowserCapabilityUnavailable` (already converted to the
    /// broader `CapabilityUnavailable::browser_probe` variant) when the
    /// host lacks the capability — typically when Chrome is not on PATH.
    fn create(&self) -> Result<Arc<dyn BrowserProbeBackend>, CapabilityUnavailable>;
}

/// The four-method surface a browser probe session consumes.
///
/// Deliberately narrow: it mirrors exactly what
/// `BrowserProbeService::start/stop/drain/raw_events` need. Future
/// backends (Playwright, jsdom, WebDriver) only have to implement these
/// four methods.
#[async_trait::async_trait]
pub trait BrowserProbeBackend: Send + Sync {
    /// Spawn Chrome, attach to CDP, return the capture session that
    /// downstream APIs need to drive the probe. Async because the
    /// backend awaits CDP readiness.
    async fn start_probe_async(
        &self,
        config: CaptureConfig,
        headless: bool,
        chrome_path: Option<&str>,
    ) -> Result<CaptureSession, BrowserError>;

    /// Stop the probe. Best-effort; errors are logged by the caller.
    fn stop_probe(&self, session: &CaptureSession) -> Result<(), BrowserError>;

    /// Destructive read: consume the buffered semantic events.
    fn take_semantic_events(&self) -> Result<Vec<SemanticEvent>, BrowserError>;

    /// Non-destructive read: snapshot all buffered events. Used at
    /// `probe_stop` to honour the MS-RACE-FIX (stop-then-drain) contract
    /// without losing evidence for later consumers.
    fn raw_events(&self) -> Vec<TraceEvent>;
}

/// Error type returned from `BrowserProbeBackend` methods. Concrete
/// error variants are kept in `chronos_browser::BrowserError`; the
/// port carries a string detail to avoid leaking the concrete error
/// type into `chronos_domain`.
#[derive(Debug, Clone, thiserror::Error)]
#[error("browser probe backend error: {detail}")]
pub struct BrowserError {
    pub detail: String,
}

impl BrowserError {
    pub fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
        }
    }
}
