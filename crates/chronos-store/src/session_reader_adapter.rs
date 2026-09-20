//! `SessionStoreBackedSessionReader` — driven adapter that forwards
//! `chronos_domain::ports::session_reader::SessionReader` calls to a
//! `chronos_store::SessionStore`.
//!
//! Part of REC-C3-hexagonal-closure Etapa B.1: introduces the
//! `SessionReader` port (REC-C3.5 application contract) and a thin
//! adapter that lets services consume the existing persistent store
//! through the port, without changing `SessionStore` itself. The
//! adapter does no caching or buffering; every call is a direct
//! forward to the underlying store, with `StoreError` mapped to the
//! canonical `SessionReaderError` so service code stays decoupled from
//! `chronos_store` types.
//!
//! One adapter, three services: `ChronosDiffService`,
//! `ChronosSessionExplainService`, and `ChronosSessionCompareService`
//! share this single port because they all have identical load
//! semantics — fetch `(SessionMetadata, Vec<TraceEvent>)` by id. The
//! write side (seal, append, delete) stays on `SessionStore` direct
//! for now (REC-C3.5-B.4 introduces `LifecycleStore` separately).

use std::sync::Arc;

use chronos_domain::ports::session_reader::{SessionReader, SessionReaderError};
use chronos_domain::{SessionMetadata, TraceEvent};

use crate::error::StoreError;
use crate::storage::SessionStore;

/// `SessionReader` driven by a `SessionStore`. Construct once at the
/// composition root and hand the `Arc<dyn SessionReader>` to services.
pub struct SessionStoreBackedSessionReader {
    store: Arc<SessionStore>,
}

impl SessionStoreBackedSessionReader {
    /// Build a new adapter over `store`.
    pub fn new(store: Arc<SessionStore>) -> Self {
        Self { store }
    }

    /// Borrow the inner store. Useful for tests that still need
    /// direct `SessionStore` access (e.g. seeding fixtures). Production
    /// services do NOT call this — they consume only the trait.
    pub fn store(&self) -> &SessionStore {
        &self.store
    }
}

impl SessionReader for SessionStoreBackedSessionReader {
    fn load_session(
        &self,
        session_id: &str,
    ) -> Result<(SessionMetadata, Vec<TraceEvent>), SessionReaderError> {
        self.store.load_session(session_id).map_err(map_store_error)
    }
}

/// Map `StoreError` to the canonical `SessionReaderError`.
///
/// Services do pattern matching on `SessionReaderError` to keep
/// `chronos_store` types out of their public API. The mapping
/// preserves the three canonical variants every service needs:
/// - `SessionNotFound` → `SessionReaderError::NotFound`
/// - `InvalidSessionId` → `SessionReaderError::InvalidId`
/// - everything else → `SessionReaderError::LoadFailed(msg)`
fn map_store_error(e: StoreError) -> SessionReaderError {
    match e {
        StoreError::SessionNotFound(id) => SessionReaderError::NotFound(id),
        StoreError::InvalidSessionId(id) => SessionReaderError::InvalidId(id),
        other => SessionReaderError::LoadFailed(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Loading from an empty in-memory store should surface NotFound.
    #[test]
    fn empty_store_returns_not_found() {
        let store = Arc::new(SessionStore::in_memory().unwrap());
        let adapter = SessionStoreBackedSessionReader::new(store);
        let err = adapter.load_session("ghost").unwrap_err();
        assert!(matches!(err, SessionReaderError::NotFound(_)));
    }

    /// Path-separator ids must be rejected as InvalidId, mirroring
    /// the production validator in `SessionStore::load_session`.
    #[test]
    fn invalid_id_rejected() {
        let store = Arc::new(SessionStore::in_memory().unwrap());
        let adapter = SessionStoreBackedSessionReader::new(store);
        let err = adapter.load_session("bad/id").unwrap_err();
        assert!(matches!(err, SessionReaderError::InvalidId(_)));
    }

    /// Round-trip: saving a session through the underlying store
    /// must be visible through the adapter.
    #[test]
    fn round_trip_via_store() {
        let store = Arc::new(SessionStore::in_memory().unwrap());
        // Seed: create a session with one event via the direct store API.
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
        store.save_session(meta, &[]).unwrap();

        let adapter = SessionStoreBackedSessionReader::new(store);
        let (loaded_meta, _events) = adapter.load_session("round-trip").unwrap();
        assert_eq!(loaded_meta.session_id, "round-trip");
    }
}
