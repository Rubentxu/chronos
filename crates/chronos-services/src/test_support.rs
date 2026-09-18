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
//! composition root (that is what they are *testing*), and they need
//! the `SessionExecutionLog` wrapper to carry the concrete
//! `SegmentedExecutionLog` so maintenance calls (`flush`,
//! `compact_up_to`, `retain_up_to`) keep working in unit-level
//! fixtures until C3.3.2.2 retires the native-log bridge.
//!
//! R6 (no concrete adapter construction in production services)
//! stays intact: every code path here lives behind `cfg(test)` or
//! the opt-in `test-utils` feature flag.

use std::path::Path;
use std::sync::Arc;

use chronos_domain::ports::execution_log_factory::ExecutionLogFactory as ExecutionLogFactoryTrait;
use chronos_log::{SegmentedConfig, SegmentedExecutionLog, SessionId};

use crate::error::ServiceError;
use crate::session_log::SessionExecutionLog;

/// Build a `SessionExecutionLog` backed by a freshly-opened
/// `SegmentedExecutionLog` so tests get maintenance access until
/// C3.3.2.2 closes the native-log bridge.
///
/// Mirrors the legacy `SessionExecutionLog::create` shape: the
/// helper makes the parent dir, opens the segmented log, and
/// wraps it via `try_adopt` so the `ProviderKind::Segmented` tag
/// survives.
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
    SessionExecutionLog::try_adopt(Some(dir), session_id, Arc::new(log))
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
    SessionExecutionLog::try_adopt(Some(dir), session_id, Arc::new(log))
}

/// Build the segmented `ExecutionLogFactory` cheaply so tests
/// can inject it into a registry without writing their own.
pub fn segmented_execution_log_factory() -> Arc<dyn ExecutionLogFactoryTrait> {
    Arc::new(chronos_log::factory::SegmentedExecutionLogFactory::new())
}
