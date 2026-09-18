//! REC-C3.3.2 — `SegmentedExecutionLogFactory`, the only composition-root
//! factory that constructs `SegmentedExecutionLog` and wraps it with
//! `SegmentedExecutionLogProvider`.
//!
//! Before C3.3.2, `chronos_services::session_log::SessionExecutionLog`
//! constructed these directly. That coupled services to the concrete
//! adapter and prevented the composition root from swapping it (test
//! fixtures, future backends). C3.3.2 lifts construction into this
//! factory; services hold `Arc<dyn ExecutionLogFactory>` and call it.

use std::path::PathBuf;
use std::sync::Arc;

use chronos_domain::ports::execution_log::{ExecutionLogError, ExecutionLogProvider};
use chronos_domain::ports::execution_log_factory::ExecutionLogFactory;
use chronos_domain::session_id::SessionId;

use crate::provider::SegmentedExecutionLogProvider;
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
    ) -> Result<Arc<dyn ExecutionLogProvider>, ExecutionLogError> {
        std::fs::create_dir_all(&dir).map_err(|e| ExecutionLogError::Open {
            path: dir.clone(),
            kind: format!("create_dir_all: {e}"),
        })?;
        let inner = SegmentedExecutionLog::open(session_id.clone(), SegmentedConfig::with_dir(dir))
            .map_err(|e| ExecutionLogError::Open {
                path: PathBuf::new(),
                kind: format!("open {}: {e:?}", session_id.as_str()),
            })?;
        let provider = SegmentedExecutionLogProvider::new(session_id, Arc::new(inner));
        Ok(Arc::new(provider))
    }

    fn reopen_existing(
        &self,
        dir: PathBuf,
        session_id: SessionId,
    ) -> Result<Arc<dyn ExecutionLogProvider>, ExecutionLogError> {
        let inner = SegmentedExecutionLog::open_existing(
            session_id.clone(),
            SegmentedConfig::with_dir(dir),
        )
        .map_err(|e| ExecutionLogError::Open {
            path: PathBuf::new(),
            kind: format!("open_existing {}: {e:?}", session_id.as_str()),
        })?;
        let provider = SegmentedExecutionLogProvider::new(session_id, Arc::new(inner));
        Ok(Arc::new(provider))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

        let provider = factory
            .create(dir.clone(), session.clone())
            .expect("create must succeed");
        assert_eq!(provider.kind(), ExecutionLogKind::Segmented);
        assert_eq!(provider.session_id(), &session);

        let reopened = factory
            .reopen_existing(dir.clone(), session.clone())
            .expect("reopen_existing must succeed");
        assert_eq!(reopened.session_id(), &session);
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
}
