//! SessionStoreBackedSessionArchive — driven adapter that forwards
//! `SessionArchive` calls to a `chronos_store::SessionStore`.
//!
//! Part of REC-C3.3.3 (Tren B) slice C: introduces the
//! `chronos_domain::ports::session::SessionArchive` port and a thin
//! adapter that lets services consume the existing persistent store
//! through the port, without changing `SessionStore` itself. The
//! adapter does no caching or buffering; every call is a direct
//! forward to the underlying store, preserving the wire shape that
//! `SessionsService` already depends on.

use std::sync::Arc;

use chronos_domain::ports::session::{SessionArchive, SessionArchiveError};
use chronos_domain::{SessionMetadata, TraceEvent};

use crate::error::StoreError;
use crate::storage::SessionStore;

/// `SessionArchive` driven by a `SessionStore`. Construct once at the
/// composition root and hand the `Arc<dyn SessionArchive>` to services.
pub struct SessionStoreBackedSessionArchive {
    store: Arc<SessionStore>,
}

impl SessionStoreBackedSessionArchive {
    pub fn new(store: Arc<SessionStore>) -> Self {
        Self { store }
    }

    /// Wrap in `Arc<dyn SessionArchive>` for composition-root injection.
    pub fn into_arc(self) -> Arc<dyn SessionArchive> {
        Arc::new(self)
    }
}

fn convert(e: StoreError) -> SessionArchiveError {
    match e {
        StoreError::SessionNotFound(id) => SessionArchiveError::SessionNotFound(id),
        other => SessionArchiveError::from_store(other),
    }
}

impl SessionArchive for SessionStoreBackedSessionArchive {
    fn save(
        &self,
        metadata: SessionMetadata,
        events: &[TraceEvent],
    ) -> Result<Vec<String>, SessionArchiveError> {
        self.store.save_session(metadata, events).map_err(convert)
    }

    fn load(
        &self,
        session_id: &str,
    ) -> Result<(SessionMetadata, Vec<TraceEvent>), SessionArchiveError> {
        self.store.load_session(session_id).map_err(convert)
    }

    fn list(&self) -> Result<Vec<SessionMetadata>, SessionArchiveError> {
        self.store.list_sessions().map_err(convert)
    }

    fn delete(&self, session_id: &str) -> Result<(), SessionArchiveError> {
        self.store.delete_session(session_id).map_err(convert)
    }

    fn count_events(&self, session_id: &str) -> Result<usize, SessionArchiveError> {
        // `SessionStore` does not expose a dedicated count method; the
        // count equals the length of the event-hash list which equals
        // the length of the loaded events vector. We re-load the events
        // here rather than touching the database internals from outside
        // the store crate (B3: services→store direction stays the only
        // boundary).
        self.store
            .load_session(session_id)
            .map(|(_, events)| events.len())
            .map_err(convert)
    }

    fn is_persistent(&self) -> bool {
        self.store.is_persistent()
    }
}
