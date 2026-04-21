//! Session storage — persists session metadata and events via the CAS.

use crate::cas::ContentStore;
use crate::error::StoreError;
use chronos_domain::TraceEvent;
use redb::{ReadableTable, TableDefinition};
use std::path::Path;
use std::sync::Arc;

/// Table definition for session metadata.
/// Key: session_id bytes
/// Value: bincode(SessionMetadata)
const SESSION_META: TableDefinition<&[u8], &[u8]> = TableDefinition::new("sessions");
/// Table definition for session event hashes.
/// Key: session_id bytes
/// Value: bincode(Vec<ContentHash>)
const SESSION_EVENTS: TableDefinition<&[u8], &[u8]> = TableDefinition::new("session_events");

/// Metadata for a saved session.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionMetadata {
    /// Unique session identifier.
    pub session_id: String,
    /// Unix timestamp ms when the session was created.
    pub created_at: u64,
    /// Language/runtime: "python", "java", "go", "native".
    pub language: String,
    /// Target program path or name.
    pub target: String,
    /// Total number of events stored.
    pub event_count: usize,
    /// Total duration in milliseconds.
    pub duration_ms: u64,
}

/// Session store — manages persistent session data.
///
/// Provides session-level storage on top of the CAS: saves metadata and
/// event hashes, then loads and reconstructs sessions by fetching events
/// from the CAS. Sessions can be listed, loaded, deleted, and compared.
pub struct SessionStore {
    /// The underlying redb database.
    db: Arc<redb::Database>,
    /// The CAS used for event storage.
    cas: ContentStore,
}

impl SessionStore {
    /// Open a session store at the given path, creating it if necessary.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let db =
            Arc::new(redb::Database::create(path).map_err(|e| StoreError::Database(e.into()))?);
        let cas = ContentStore::new(db.clone());
        Ok(Self { db, cas })
    }

    /// Create an in-memory session store (for testing).
    pub fn in_memory() -> Result<Self, StoreError> {
        let db = Arc::new(
            redb::Builder::new()
                .create_with_backend(redb::backends::InMemoryBackend::new())
                .map_err(|e| StoreError::Database(e.into()))?,
        );
        let cas = ContentStore::new(db.clone());
        Ok(Self { db, cas })
    }

    /// Save all events for a session. Stores events in CAS and records metadata.
    /// Returns the list of content hashes.
    pub fn save_session(
        &self,
        metadata: SessionMetadata,
        events: &[TraceEvent],
    ) -> Result<Vec<String>, StoreError> {
        // Validate session_id contains no path separators
        if metadata.session_id.contains('/') || metadata.session_id.contains('\\') {
            return Err(StoreError::InvalidSessionId(metadata.session_id.clone()));
        }

        let mut hashes = Vec::with_capacity(events.len());

        // Store each event in CAS
        for event in events {
            let h = self.cas.put(event)?;
            hashes.push(h);
        }

        // Serialize metadata
        let meta_bytes =
            bincode::serialize(&metadata).map_err(|e| StoreError::Serialization(e.to_string()))?;

        // Write session metadata
        let tx = self
            .db
            .begin_write()
            .map_err(|e| StoreError::Database(e.into()))?;

        let mut meta_table = tx
            .open_table(SESSION_META)
            .map_err(|e| StoreError::Database(e.into()))?;
        meta_table
            .insert(metadata.session_id.as_bytes(), meta_bytes.as_slice())
            .map_err(|e| StoreError::Database(e.into()))?;
        drop(meta_table);

        // Write event hash list
        let hashes_bytes =
            bincode::serialize(&hashes).map_err(|e| StoreError::Serialization(e.to_string()))?;
        let mut evt_table = tx
            .open_table(SESSION_EVENTS)
            .map_err(|e| StoreError::Database(e.into()))?;
        evt_table
            .insert(metadata.session_id.as_bytes(), hashes_bytes.as_slice())
            .map_err(|e| StoreError::Database(e.into()))?;
        drop(evt_table);

        tx.commit().map_err(|e| StoreError::Database(e.into()))?;

        Ok(hashes)
    }

    /// Load all events for a session (via CAS lookup).
    pub fn load_session(
        &self,
        session_id: &str,
    ) -> Result<(SessionMetadata, Vec<TraceEvent>), StoreError> {
        // Validate session_id contains no path separators
        if session_id.contains('/') || session_id.contains('\\') {
            return Err(StoreError::InvalidSessionId(session_id.to_string()));
        }

        // Read metadata
        let meta_bytes = {
            let tx = self
                .db
                .begin_read()
                .map_err(|e| StoreError::Database(e.into()))?;
            let table = match tx.open_table(SESSION_META) {
                Ok(t) => t,
                Err(_) => return Err(StoreError::SessionNotFound(session_id.to_string())),
            };
            let entry = table
                .get(session_id.as_bytes())
                .map_err(|e| StoreError::Database(e.into()))?
                .ok_or_else(|| StoreError::SessionNotFound(session_id.to_string()))?;
            entry.value().to_vec()
        };

        let metadata: SessionMetadata = bincode::deserialize(&meta_bytes)
            .map_err(|e| StoreError::Serialization(e.to_string()))?;

        // Read event hashes
        let hashes: Vec<String> = {
            let tx = self
                .db
                .begin_read()
                .map_err(|e| StoreError::Database(e.into()))?;
            let table = match tx.open_table(SESSION_EVENTS) {
                Ok(t) => t,
                Err(_) => return Err(StoreError::SessionNotFound(session_id.to_string())),
            };
            let entry = table
                .get(session_id.as_bytes())
                .map_err(|e| StoreError::Database(e.into()))?
                .ok_or_else(|| StoreError::SessionNotFound(session_id.to_string()))?;
            let bytes: &[u8] = entry.value();
            bincode::deserialize(bytes).map_err(|e| StoreError::Serialization(e.to_string()))?
        };

        // Reconstruct events from CAS
        let mut events = Vec::with_capacity(hashes.len());
        for h in &hashes {
            if let Some(evt) = self.cas.get(h)? {
                events.push(evt);
            }
        }

        Ok((metadata, events))
    }

    /// List all saved sessions (metadata only).
    pub fn list_sessions(&self) -> Result<Vec<SessionMetadata>, StoreError> {
        let tx = self
            .db
            .begin_read()
            .map_err(|e| StoreError::Database(e.into()))?;
        let table = tx
            .open_table(SESSION_META)
            .map_err(|e| StoreError::Database(e.into()))?;
        let mut results = Vec::new();

        for entry in table.iter().map_err(|e| StoreError::Database(e.into()))? {
            let (_, value) = entry.map_err(|e| StoreError::Database(e.into()))?;
            let meta: SessionMetadata = bincode::deserialize(value.value())
                .map_err(|e| StoreError::Serialization(e.to_string()))?;
            results.push(meta);
        }

        Ok(results)
    }

    /// Delete a session and its event references (not CAS entries — they may be shared).
    pub fn delete_session(&self, session_id: &str) -> Result<(), StoreError> {
        // Validate session_id contains no path separators
        if session_id.contains('/') || session_id.contains('\\') {
            return Err(StoreError::InvalidSessionId(session_id.to_string()));
        }

        let tx = self
            .db
            .begin_write()
            .map_err(|e| StoreError::Database(e.into()))?;

        let mut meta_table = tx
            .open_table(SESSION_META)
            .map_err(|e| StoreError::Database(e.into()))?;
        let mut evt_table = tx
            .open_table(SESSION_EVENTS)
            .map_err(|e| StoreError::Database(e.into()))?;

        if meta_table
            .get(session_id.as_bytes())
            .map_err(|e| StoreError::Database(e.into()))?
            .is_none()
        {
            return Err(StoreError::SessionNotFound(session_id.to_string()));
        }

        meta_table
            .remove(session_id.as_bytes())
            .map_err(|e| StoreError::Database(e.into()))?;
        evt_table
            .remove(session_id.as_bytes())
            .map_err(|e| StoreError::Database(e.into()))?;

        drop(meta_table);
        drop(evt_table);
        tx.commit().map_err(|e| StoreError::Database(e.into()))?;
        Ok(())
    }

    /// Check if a session exists.
    pub fn session_exists(&self, session_id: &str) -> Result<bool, StoreError> {
        let tx = self
            .db
            .begin_read()
            .map_err(|e| StoreError::Database(e.into()))?;
        let table = tx
            .open_table(SESSION_META)
            .map_err(|e| StoreError::Database(e.into()))?;
        Ok(table
            .get(session_id.as_bytes())
            .map_err(|e| StoreError::Database(e.into()))?
            .is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::{EventData, EventType, SourceLocation};

    fn make_event(id: u64, func: &str) -> TraceEvent {
        TraceEvent::new(
            id,
            id * 100,
            1,
            EventType::FunctionEntry,
            SourceLocation::new("test.rs", 10, func, 0x1000 + id),
            EventData::Function {
                name: func.to_string(),
                signature: None,
            },
        )
    }

    fn session_meta(id: &str) -> SessionMetadata {
        SessionMetadata {
            session_id: id.to_string(),
            created_at: 1000,
            language: "native".to_string(),
            target: "/bin/test".to_string(),
            event_count: 2,
            duration_ms: 500,
        }
    }

    #[test]
    fn test_session_store_save_and_load() {
        let store = SessionStore::in_memory().unwrap();
        let meta = session_meta("session-1");
        let events = vec![make_event(1, "main"), make_event(2, "helper")];

        let hashes = store.save_session(meta.clone(), &events).unwrap();
        assert_eq!(hashes.len(), 2);

        let (loaded_meta, loaded_events) = store.load_session("session-1").unwrap();
        assert_eq!(loaded_meta.session_id, "session-1");
        assert_eq!(loaded_events.len(), 2);
    }

    #[test]
    fn test_session_store_list_sessions() {
        let store = SessionStore::in_memory().unwrap();
        let events = vec![make_event(1, "main")];

        store.save_session(session_meta("s1"), &events).unwrap();
        store.save_session(session_meta("s2"), &events).unwrap();

        let sessions = store.list_sessions().unwrap();
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn test_session_store_delete_session() {
        let store = SessionStore::in_memory().unwrap();
        let events = vec![make_event(1, "main")];

        store.save_session(session_meta("s1"), &events).unwrap();
        assert!(store.session_exists("s1").unwrap());

        store.delete_session("s1").unwrap();
        assert!(!store.session_exists("s1").unwrap());
    }

    #[test]
    fn test_session_store_session_not_found() {
        let store = SessionStore::in_memory().unwrap();
        let result = store.load_session("nonexistent");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StoreError::SessionNotFound(_)
        ));
    }

    #[test]
    fn test_session_store_dedup_events() {
        let store = SessionStore::in_memory().unwrap();
        let events = vec![make_event(1, "main")];

        // Two sessions with identical events — CAS dedup means same hashes
        let hashes1 = store.save_session(session_meta("s1"), &events).unwrap();
        let hashes2 = store.save_session(session_meta("s2"), &events).unwrap();

        assert_eq!(
            hashes1, hashes2,
            "Identical events should produce same hashes (dedup)"
        );
    }

    #[test]
    fn test_session_id_rejects_path_separator() {
        let store = SessionStore::in_memory().unwrap();
        let events = vec![make_event(1, "main")];

        // Test save with path separator in session_id
        let result = store.save_session(session_meta("../../evil"), &events);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StoreError::InvalidSessionId(_)
        ));

        // Test load with path separator
        let result = store.load_session("../../evil");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StoreError::InvalidSessionId(_)
        ));

        // Test delete with path separator
        let result = store.delete_session("../../evil");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StoreError::InvalidSessionId(_)
        ));
    }

    #[test]
    fn test_session_id_accepts_uuid() {
        let store = SessionStore::in_memory().unwrap();
        let events = vec![make_event(1, "main")];
        let uuid = "550e8400-e29b-41d4-a716-446655440000";

        let result = store.save_session(session_meta(uuid), &events);
        assert!(result.is_ok());

        let result = store.load_session(uuid);
        assert!(result.is_ok());

        let result = store.delete_session(uuid);
        assert!(result.is_ok());
    }
}
