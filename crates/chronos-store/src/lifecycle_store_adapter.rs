//! `SessionStoreBackedLifecycleStore` — driven adapter that forwards
//! `chronos_domain::ports::lifecycle_store::LifecycleStore` calls to a
//! `chronos_store::SessionStore`.
//!
//! Part of REC-C3-hexagonal-closure Etapa B.4: introduces the
//! `LifecycleStore` port (REC-C3.5 application contract) and a thin
//! adapter that lets `session_lifecycle.rs` consume the existing
//! persistent store through the port, without changing `SessionStore`
//! itself. The adapter does no caching or buffering; every call is a
//! direct forward to the underlying store, with `StoreError` mapped
//! to the canonical `SessionReaderError` / `LifecycleStoreError` so
//! service code stays decoupled from `chronos_store` types.

use std::sync::Arc;

use crate::error::StoreError;
use crate::storage::SessionStore;
use chronos_domain::ports::lifecycle_store::{LifecycleStore, LifecycleStoreError};
use chronos_domain::ports::session_reader::{SessionReader, SessionReaderError};
use chronos_domain::{SessionMetadata, TraceEvent};

/// `LifecycleStore` driven by a `SessionStore`. Construct once at the
/// composition root and hand the `Arc<dyn LifecycleStore>` to the
/// lifecycle service.
pub struct SessionStoreBackedLifecycleStore {
    store: Arc<SessionStore>,
}

impl SessionStoreBackedLifecycleStore {
    /// Build a new adapter over `store`.
    pub fn new(store: Arc<SessionStore>) -> Self {
        Self { store }
    }

    /// Borrow the inner store (production services do NOT call this;
    /// tests may use it for fixtures that still want to construct
    /// sessions directly).
    pub fn store(&self) -> &SessionStore {
        &self.store
    }
}

impl SessionReader for SessionStoreBackedLifecycleStore {
    fn load_session(
        &self,
        session_id: &str,
    ) -> Result<(SessionMetadata, Vec<TraceEvent>), SessionReaderError> {
        self.store.load_session(session_id).map_err(map_store_error)
    }
}

impl LifecycleStore for SessionStoreBackedLifecycleStore {
    fn save_session(
        &self,
        metadata: &SessionMetadata,
        events: &[TraceEvent],
    ) -> Result<(), LifecycleStoreError> {
        // The production `SessionStore::save_session` takes ownership
        // of the metadata and a slice of events; it returns the
        // content hashes that were inserted (CAS bookkeeping).
        // The port consumer doesn't need those, so we discard them.
        self.store
            .save_session(metadata.clone(), events)
            .map(|_hashes| ())
            .map_err(map_save_error)
    }

    fn delete_session(&self, session_id: &str) -> Result<(), LifecycleStoreError> {
        self.store
            .delete_session(session_id)
            .map_err(map_delete_error)
    }
}

fn map_store_error(e: StoreError) -> SessionReaderError {
    match e {
        StoreError::SessionNotFound(id) => SessionReaderError::NotFound(id),
        StoreError::InvalidSessionId(id) => SessionReaderError::InvalidId(id),
        other => SessionReaderError::LoadFailed(other.to_string()),
    }
}

fn map_save_error(e: StoreError) -> LifecycleStoreError {
    match e {
        StoreError::InvalidSessionId(id) => LifecycleStoreError::InvalidId(id),
        other => LifecycleStoreError::SaveFailed(other.to_string()),
    }
}

fn map_delete_error(e: StoreError) -> LifecycleStoreError {
    match e {
        StoreError::InvalidSessionId(id) => LifecycleStoreError::InvalidId(id),
        other => LifecycleStoreError::DeleteFailed(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::ports::session_reader::SessionReader;

    #[test]
    fn empty_store_load_returns_not_found() {
        let store = Arc::new(SessionStore::in_memory().unwrap());
        let adapter = SessionStoreBackedLifecycleStore::new(store);
        let err = adapter.load_session("ghost").unwrap_err();
        assert!(matches!(err, SessionReaderError::NotFound(_)));
    }

    #[test]
    fn save_then_load_via_reader_bound() {
        let store = Arc::new(SessionStore::in_memory().unwrap());
        let adapter = SessionStoreBackedLifecycleStore::new(Arc::clone(&store));
        let meta = chronos_domain::SessionMetadata {
            session_id: "round-trip".to_string(),
            created_at: 0,
            language: "rust".to_string(),
            target: "noop".to_string(),
            event_count: 0,
            duration_ms: 0,
            tail_sealed: false,
            sealed_at: None,
        };
        adapter.save_session(&meta, &[]).unwrap();
        let (loaded, events) = adapter.load_session("round-trip").unwrap();
        assert_eq!(loaded.session_id, "round-trip");
        assert!(events.is_empty());
    }

    #[test]
    fn invalid_id_save_rejected() {
        let store = Arc::new(SessionStore::in_memory().unwrap());
        let adapter = SessionStoreBackedLifecycleStore::new(store);
        let meta = chronos_domain::SessionMetadata {
            session_id: "bad/id".to_string(),
            created_at: 0,
            language: "rust".to_string(),
            target: "noop".to_string(),
            event_count: 0,
            duration_ms: 0,
            tail_sealed: false,
            sealed_at: None,
        };
        let err = adapter.save_session(&meta, &[]).unwrap_err();
        assert!(matches!(err, LifecycleStoreError::InvalidId(_)));
    }

    #[test]
    fn delete_then_load_returns_not_found() {
        let store = Arc::new(SessionStore::in_memory().unwrap());
        let adapter = SessionStoreBackedLifecycleStore::new(Arc::clone(&store));
        let meta = chronos_domain::SessionMetadata {
            session_id: "delete-me".to_string(),
            created_at: 0,
            language: "rust".to_string(),
            target: "noop".to_string(),
            event_count: 0,
            duration_ms: 0,
            tail_sealed: false,
            sealed_at: None,
        };
        adapter.save_session(&meta, &[]).unwrap();
        adapter.delete_session("delete-me").unwrap();
        let err = adapter.load_session("delete-me").unwrap_err();
        assert!(matches!(err, SessionReaderError::NotFound(_)));
    }
}
