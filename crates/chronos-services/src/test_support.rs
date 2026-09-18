//! REC-C3.3.2 — test-only support for building `SessionExecutionLog`
//! fixtures without routing through the composition root.
//!
//! **Production binaries MUST NOT depend on this module.** The
//! module is gated behind `#[cfg(any(test, feature = "test-utils"))]`
//! and the feature is opt-in (never enabled by the release profile).
//!
//! Why this exists: integration tests in downstream crates
//! (`chronos-mcp/tests/*`, `chronos-sandbox/tests/*`) need to seed
//! a fixture on disk before the server starts. They cannot call the
//! composition root (that is what they are *testing*), so they need
//! a helper that builds the capability bundle directly and wraps it
//! in a `SessionExecutionLog`.
//!
//! R6 (no concrete adapter construction in production services)
//! stays intact: every code path here lives behind `cfg(test)` or
//! the opt-in `test-utils` feature flag.

use std::path::Path;
use std::sync::Arc;

use chronos_domain::ports::execution_log_factory::{
    ExecutionLogCapabilities, ExecutionLogFactory as ExecutionLogFactoryTrait,
};
use chronos_domain::ports::execution_log_maintenance::ExecutionLogMaintenance;
use chronos_domain::ports::execution_log_retention::ExecutionLogRetention;
use chronos_log::{
    SegmentedConfig, SegmentedExecutionLog, SegmentedExecutionLogProvider, SegmentedMaintenance,
    SegmentedRetention, SessionId,
};

use crate::error::ServiceError;
use crate::session_log::SessionExecutionLog;

/// Build a `SessionExecutionLog` backed by a fresh segmented log so
/// tests get the full capability bundle (evidence + retention +
/// maintenance).
///
/// Mirrors the legacy `SessionExecutionLog::create` shape: the helper
/// makes the parent dir, opens the segmented log, wraps it with the
/// three port wrappers, and returns a `SessionExecutionLog`.
pub fn create_with_segmented_backend(
    dir: impl AsRef<Path>,
    session_id: SessionId,
) -> Result<SessionExecutionLog, ServiceError> {
    let dir = dir.as_ref().to_path_buf();
    std::fs::create_dir_all(&dir).map_err(|e| {
        ServiceError::ProbeStartFailed(format!(
            "test_support::create_with_segmented_backend mkdir {}: {e}",
            dir.display()
        ))
    })?;
    let log =
        SegmentedExecutionLog::open(session_id.clone(), SegmentedConfig::with_dir(dir.clone()))
            .map_err(|e| {
                ServiceError::ProbeStartFailed(format!(
                    "test_support::create_with_segmented_backend open: {e:?}"
                ))
            })?;
    let bundle = capabilities_for_segmented(session_id, Arc::new(log));
    Ok(SessionExecutionLog::from_capabilities(bundle, Some(dir)))
}

/// Reopen an existing durable log via the segmented backend.
/// Same contract as `create_with_segmented_backend`.
pub fn reopen_existing_with_segmented_backend(
    dir: impl AsRef<Path>,
    session_id: SessionId,
) -> Result<SessionExecutionLog, ServiceError> {
    let dir = dir.as_ref().to_path_buf();
    let log = SegmentedExecutionLog::open_existing(
        session_id.clone(),
        SegmentedConfig::with_dir(dir.clone()),
    )
    .map_err(|e| {
        ServiceError::ProbeStartFailed(format!(
            "test_support::reopen_existing_with_segmented_backend: {e:?}"
        ))
    })?;
    let bundle = capabilities_for_segmented(session_id, Arc::new(log));
    Ok(SessionExecutionLog::from_capabilities(bundle, Some(dir)))
}

/// Build the segmented `ExecutionLogFactory` cheaply so tests
/// can inject it into a registry without writing their own.
pub fn segmented_execution_log_factory() -> Arc<dyn ExecutionLogFactoryTrait> {
    Arc::new(chronos_log::factory::SegmentedExecutionLogFactory::new())
}

/// Build a capability bundle backed by an already-opened
/// `SegmentedExecutionLog`. Used by the helpers above and by
/// integration tests that need to seed the bundle directly.
pub fn capabilities_for_segmented(
    session_id: SessionId,
    inner: Arc<SegmentedExecutionLog>,
) -> ExecutionLogCapabilities {
    let evidence: Arc<dyn chronos_domain::ports::execution_log::ExecutionLogProvider> = Arc::new(
        SegmentedExecutionLogProvider::new(session_id.clone(), inner.clone()),
    );
    let retention: Arc<dyn ExecutionLogRetention> =
        Arc::new(SegmentedRetention::new(session_id, inner.clone()));
    let maintenance: Arc<dyn ExecutionLogMaintenance> = Arc::new(SegmentedMaintenance::new(inner));
    ExecutionLogCapabilities {
        evidence,
        retention,
        maintenance,
    }
}
