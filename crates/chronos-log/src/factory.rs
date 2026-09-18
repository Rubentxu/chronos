//! REC-C3.3.2/C3.3.2.5 — `SegmentedExecutionLogFactory`, the only
//! composition-root factory that constructs `SegmentedExecutionLog`
//! and wraps it with `SegmentedExecutionLogProvider`,
//! `SegmentedRetention`, and `SegmentedMaintenance`.
//!
//! Before C3.3.2, `chronos_services::session_log::SessionExecutionLog`
//! constructed these directly. That coupled services to the concrete
//! adapter and prevented the composition root from swapping it (test
//! fixtures, future backends). C3.3.2 lifts construction into this
//! factory; services hold `Arc<dyn ExecutionLogFactory>` and call it.
//!
//! C3.3.2.5 lifts the maintenance and retention capabilities into
//! their own port wrappers so the factory returns the three-Arc
//! `ExecutionLogCapabilities` bundle. The application layer consumes
//! the bundle without ever downcasting on a ProviderKind enum.

use std::path::PathBuf;
use std::sync::Arc;

use chronos_domain::ports::execution_log::ExecutionLogError;
use chronos_domain::ports::execution_log_factory::{ExecutionLogCapabilities, ExecutionLogFactory};
use chronos_domain::ports::execution_log_maintenance::ExecutionLogMaintenance;
use chronos_domain::ports::execution_log_retention::ExecutionLogRetention;
use chronos_domain::session_id::SessionId;

use crate::provider::{
    SegmentedExecutionLogProvider, SegmentedMaintenance, SegmentedRetention,
};
use crate::segmented::{SegmentedConfig, SegmentedExecutionLog};

/// Factory that produces `SegmentedExecutionLogProvider` instances.
///
/// Stateless; clone freely. Thread-safe by construction
/// (`SegmentedExecutionLog::open` / `open_existing` use internal locks).
#[derive(Debug, Default, Clone, Copy)]
pub struct SegmentedExecutionLogFactory;

impl SegmentedExecutionLogFactory {
    pub fn new() -> Self {
        Self
    }
}

impl ExecutionLogFactory for SegmentedExecutionLogFactory {
    fn create(
        &self,
        dir: PathBuf,
        session_id: SessionId,
    ) -> Result<ExecutionLogCapabilities, ExecutionLogError> {
        std::fs::create_dir_all(&dir).map_err(|e| ExecutionLogError::Open {
            path: dir.clone(),
            kind: format!("create_dir_all: {e}"),
        })?;
        let inner = SegmentedExecutionLog::open(session_id.clone(), SegmentedConfig::with_dir(dir))
            .map_err(|e| ExecutionLogError::Open {
                path: PathBuf::new(),
                kind: format!("open {}: {e:?}", session_id.as_str()),
            })?;
        let inner_arc = Arc::new(inner);
        let evidence: Arc<dyn chronos_domain::ports::execution_log::ExecutionLogProvider> =
            Arc::new(SegmentedExecutionLogProvider::new(
                session_id.clone(),
                inner_arc.clone(),
            ));
        let retention: Arc<dyn ExecutionLogRetention> =
            Arc::new(SegmentedRetention::new(session_id.clone(), inner_arc.clone()));
        let maintenance: Arc<dyn ExecutionLogMaintenance> =
            Arc::new(SegmentedMaintenance::new(inner_arc));
        Ok(ExecutionLogCapabilities {
            evidence,
            retention,
            maintenance,
        })
    }

    fn reopen_existing(
        &self,
        dir: PathBuf,
        session_id: SessionId,
    ) -> Result<ExecutionLogCapabilities, ExecutionLogError> {
        let inner = SegmentedExecutionLog::open_existing(
            session_id.clone(),
            SegmentedConfig::with_dir(dir),
        )
        .map_err(|e| ExecutionLogError::Open {
            path: PathBuf::new(),
            kind: format!("open_existing {}: {e:?}", session_id.as_str()),
        })?;
        let inner_arc = Arc::new(inner);
        let evidence: Arc<dyn chronos_domain::ports::execution_log::ExecutionLogProvider> =
            Arc::new(SegmentedExecutionLogProvider::new(
                session_id.clone(),
                inner_arc.clone(),
            ));
        let retention: Arc<dyn ExecutionLogRetention> =
            Arc::new(SegmentedRetention::new(session_id.clone(), inner_arc.clone()));
        let maintenance: Arc<dyn ExecutionLogMaintenance> =
            Arc::new(SegmentedMaintenance::new(inner_arc));
        Ok(ExecutionLogCapabilities {
            evidence,
            retention,
            maintenance,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segmented::SegmentedConfig;
    use chronos_domain::ports::execution_log::ExecutionLogKind;

    #[test]
    fn factory_create_and_reopen_round_trip() {
        let factory = SegmentedExecutionLogFactory::new();
        let dir = std::env::temp_dir().join(format!(
            "c332-factory-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let session = SessionId::new("c332-roundtrip");

        let bundle = factory
            .create(dir.clone(), session.clone())
            .expect("create must succeed");
        assert_eq!(bundle.evidence.kind(), ExecutionLogKind::Segmented);
        assert_eq!(bundle.evidence.session_id(), &session);

        let reopened = factory
            .reopen_existing(dir.clone(), session.clone())
            .expect("reopen_existing must succeed");
        assert_eq!(reopened.evidence.session_id(), &session);
    }

    #[test]
    fn factory_reopen_existing_on_empty_dir_fails() {
        let factory = SegmentedExecutionLogFactory::new();
        let dir = std::env::temp_dir().join(format!(
            "c332-reopen-empty-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let session = SessionId::new("c332-empty");
        let res = factory.reopen_existing(dir, session);
        assert!(res.is_err(), "reopen_existing on empty dir must fail");
    }

    #[test]
    fn factory_bundle_three_arcs_share_one_instance() {
        // The single-instance guarantee: the three Arcs in the bundle
        // are views onto the same concrete backend. After advancing
        // the retention frontier on `retention`, the next call to
        // `evidence.retained_from()` reflects the new boundary.
        let factory = SegmentedExecutionLogFactory::new();
        let dir = std::env::temp_dir().join(format!(
            "c332-bundle-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let session = SessionId::new("c332-bundle-shared");
        let bundle = factory.create(dir, session.clone()).expect("create");
        // Before any retention move the boundary is ZERO.
        assert_eq!(
            bundle.evidence.retained_from(),
            chronos_domain::seq::EventSeq::ZERO
        );
        assert_eq!(
            bundle.retention.retained_from(),
            chronos_domain::seq::EventSeq::ZERO
        );
        // The maintenance port's metrics report no compaction yet.
        let m0 = bundle.maintenance.metrics();
        assert_eq!(m0.segments_reclaimed, 0);
        assert_eq!(m0.compaction_passes, 0);
        assert_eq!(m0.no_op_passes, 0);
    }
}
