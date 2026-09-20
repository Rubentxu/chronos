//! `LifecycleStore` — domain-side port for read+write session lifecycle access.
//!
//! Services that need to **save** a session (e.g. `mark_sealed`,
//! `session_start{action=spawn}`) in addition to the load operations
//! covered by [`SessionReader`] depend on this trait, not on
//! `chronos_store::SessionStore` directly.
//!
//! Why a separate port from `SessionReader`:
//! - **Smaller surface area for read-only consumers.** `diff`,
//!   `session_compare`, and `session_explain` need only `load_session`;
//!   forcing them to depend on a trait that also exposes `save_session`
//!   widens the API and creates a write surface where none is needed.
//! - **Single-direction ownership at composition time.** The
//!   `Arc<dyn LifecycleStore>` adapter is owned by the composition root
//!   and handed to the lifecycle service; the read-only services get
//!   `Arc<dyn SessionReader>`. Different casts, different coupling
//!   levels.
//!
//! The port extends [`SessionReader`] so any consumer that wants the
//! combined surface can take `Arc<dyn LifecycleStore>` and call
//! `load_session` through the inherited method.
//!
//! See `REC-C3.1` (exploration) and `REC-C3.5-B.4` for the broader
//! rationale; see `REC-C3-hexagonal-closure/proposal.md` for the
//! single-port-multiple-consumers convention used in this codebase.

use std::fmt;

use crate::ports::session_reader::{SessionReader, SessionReaderError};
use crate::session::SessionMetadata;
use crate::trace::TraceEvent;

/// Errors surfaced by `LifecycleStore` write operations.
///
/// Read-side errors come from the inherited [`SessionReader`] via
/// `SessionReaderError`. The composition root or the service maps
/// them to its own error type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleStoreError {
    /// The session id failed validation (path-separator injection,
    /// empty id, etc.).
    InvalidId(String),
    /// Underlying I/O or storage failure (corruption, redb panic, etc.).
    SaveFailed(String),
    /// Underlying I/O or storage failure during delete.
    DeleteFailed(String),
}

impl fmt::Display for LifecycleStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidId(id) => write!(f, "invalid session id: '{}'", id),
            Self::SaveFailed(msg) => write!(f, "save failed: {}", msg),
            Self::DeleteFailed(msg) => write!(f, "delete failed: {}", msg),
        }
    }
}

impl std::error::Error for LifecycleStoreError {}

/// Port — read + write access to saved sessions.
///
/// Read operations come from the inherited [`SessionReader`] bound.
/// Write operations are the lifecycle surface: `save_session` for
/// sealing / persisting, `delete_session` for teardown.
///
/// Implementations are expected to be thread-safe. The composition
/// root picks an adapter based on configuration (production: SQLite
/// wrapper around the same `chronos_store::SessionStore`; tests: in-memory
/// fake or the same SQLite adapter in test mode).
pub trait LifecycleStore: SessionReader {
    /// Persist (or overwrite) the session identified by
    /// `metadata.session_id` with the given `events`.
    ///
    /// On success the session is durable and loadable via
    /// `load_session`. The store may rewrite internal indices as
    /// needed.
    fn save_session(
        &self,
        metadata: &SessionMetadata,
        events: &[TraceEvent],
    ) -> Result<(), LifecycleStoreError>;

    /// Remove the session identified by `session_id`. Returns `Ok`
    /// if the session was deleted or did not exist; returns `Err`
    /// only on underlying storage failures or invalid id.
    fn delete_session(&self, session_id: &str) -> Result<(), LifecycleStoreError>;
}

// =====================================================================
// In-memory implementation (AD-5: test composition)
// =====================================================================

/// `LifecycleStore` backed by an in-process map.
///
/// Used by unit tests; production wires the SQLite-backed adapter
/// (`LifecycleStoreAdapter` in `chronos-store`).
#[derive(Default, Debug, Clone)]
pub struct InMemoryLifecycleStore {
    inner: crate::ports::session_reader::InMemorySessionReader,
}

impl InMemoryLifecycleStore {
    /// New empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of sessions currently held.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// `len() == 0`.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl SessionReader for InMemoryLifecycleStore {
    fn load_session(
        &self,
        session_id: &str,
    ) -> Result<(SessionMetadata, Vec<TraceEvent>), crate::ports::session_reader::SessionReaderError>
    {
        self.inner.load_session(session_id)
    }
}

impl LifecycleStore for InMemoryLifecycleStore {
    fn save_session(
        &self,
        metadata: &SessionMetadata,
        events: &[TraceEvent],
    ) -> Result<(), LifecycleStoreError> {
        // Mirror the production validator.
        if metadata.session_id.contains('/') || metadata.session_id.contains('\\') {
            return Err(LifecycleStoreError::InvalidId(metadata.session_id.clone()));
        }
        self.inner.put(metadata.clone(), events.to_vec());
        Ok(())
    }

    fn delete_session(&self, session_id: &str) -> Result<(), LifecycleStoreError> {
        if session_id.contains('/') || session_id.contains('\\') {
            return Err(LifecycleStoreError::InvalidId(session_id.to_string()));
        }
        // No-op: the underlying InMemorySessionReader does not expose
        // a remove operation; the tests that exercise delete_session
        // must assert against a production adapter (or we extend
        // InMemorySessionReader in a follow-up cycle). For now, the
        // delete path is validated by the adapter unit tests.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::session_reader::SessionReader;

    fn meta(id: &str) -> SessionMetadata {
        SessionMetadata {
            session_id: id.to_string(),
            created_at: 0,
            language: "rust".to_string(),
            target: "noop".to_string(),
            event_count: 0,
            duration_ms: 0,
            tail_sealed: false,
            sealed_at: None,
        }
    }

    #[test]
    fn save_then_load_via_reader_bound() {
        let store = InMemoryLifecycleStore::new();
        let m = meta("a");
        store.save_session(&m, &[]).unwrap();
        // Load via the inherited SessionReader method.
        let (loaded, events) = SessionReader::load_session(&store, "a").unwrap();
        assert_eq!(loaded.session_id, "a");
        assert!(events.is_empty());
    }

    #[test]
    fn invalid_id_save_rejected() {
        let store = InMemoryLifecycleStore::new();
        let m = meta("bad/id");
        let err = store.save_session(&m, &[]).unwrap_err();
        assert!(matches!(err, LifecycleStoreError::InvalidId(_)));
    }

    #[test]
    fn invalid_id_delete_rejected() {
        let store = InMemoryLifecycleStore::new();
        let err = store.delete_session("bad/id").unwrap_err();
        assert!(matches!(err, LifecycleStoreError::InvalidId(_)));
    }
}
