//! Session storage — persists session metadata and events via the CAS.

use crate::cas::ContentStore;
use crate::error::StoreError;
use crate::table_error::classify_read_table_error;
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

pub use chronos_domain::SessionMetadata;

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
    /// Whether this store is backed by a file on disk (`Persistent`) or
    /// an in-memory redb backend (`InMemory`). Exposed via
    /// [`SessionStore::is_persistent`] so that higher layers can tell
    /// the operator that the in-memory fallback (m9-75) is in effect.
    kind: StoreKind,
}

/// Whether the underlying redb backend is file-backed or in-memory.
///
/// m9-82: distinguishes persistent `SessionStore`s from in-memory ones
/// at runtime so the MCP layer can disclose the degraded mode in tool
/// responses (closes FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum StoreKind {
    Persistent,
    InMemory,
}

impl SessionStore {
    /// Whether this store is backed by a file on disk.
    ///
    /// `true` for a store opened via [`SessionStore::open`] or
    /// [`SessionStore::try_open`]; `false` for a store opened via
    /// [`SessionStore::in_memory`].
    pub fn is_persistent(&self) -> bool {
        self.kind == StoreKind::Persistent
    }

    /// Accessor to the underlying redb database (m9-05 R4: narrowed to `pub(crate)`).
    ///
    /// Cross-crate test access is provided through narrow typed chokepoints on
    /// `SessionStore` itself (e.g., `insert_v2_chunk_for_test`, `count_v3_chunks_for_test`),
    /// not through the raw database handle.
    pub(crate) fn db(&self) -> &Arc<redb::Database> {
        &self.db
    }

    /// Open a session store at the given path, creating it if necessary.
    #[allow(clippy::result_large_err)]
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let db =
            Arc::new(redb::Database::create(path).map_err(|e| StoreError::Database(e.into()))?);
        let cas = ContentStore::new(db.clone());
        Ok(Self {
            db,
            cas,
            kind: StoreKind::Persistent,
        })
    }

    /// Try to open an existing session store, with graceful handling of lock conflicts.
    /// If the database is locked by another process, returns a special error.
    /// If the database appears corrupted, attempts recovery.
    #[allow(clippy::result_large_err)]
    pub fn try_open(path: &Path) -> Result<Self, StoreError> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Try to open existing database
        match redb::Database::open(path) {
            Ok(db) => {
                let db = Arc::new(db);
                let cas = ContentStore::new(db.clone());
                Ok(Self {
                    db,
                    cas,
                    kind: StoreKind::Persistent,
                })
            }
            Err(e) => {
                // Check if it's a lock error
                let error_str = e.to_string();
                if error_str.contains("DatabaseAlreadyOpen") || error_str.contains("lock") {
                    tracing::warn!(
                        "Database at {:?} is locked by another process, trying to recover...",
                        path
                    );

                    // Try to remove stale lock files and retry once
                    Self::cleanup_stale_locks(path);

                    // Retry opening
                    if let Ok(db) = redb::Database::open(path) {
                        let db = Arc::new(db);
                        let cas = ContentStore::new(db.clone());
                        return Ok(Self {
                            db,
                            cas,
                            kind: StoreKind::Persistent,
                        });
                    }
                }

                // If still failing, try creating a fresh database
                tracing::warn!(
                    "Could not open existing database at {:?}: {}, creating fresh database",
                    path,
                    e
                );
                Self::open(path)
            }
        }
    }

    /// Clean up stale lock files that may be left by crashed processes.
    fn cleanup_stale_locks(path: &Path) {
        if let Some(parent) = path.parent() {
            // Look for common lock file patterns and remove them
            let lock_patterns = ["sessions.redb.lock", ".sessions.redb.lock", "sessions.lock"];
            for pattern in lock_patterns {
                let lock_path = parent.join(pattern);
                if lock_path.exists() {
                    tracing::info!("Removing stale lock file: {:?}", lock_path);
                    let _ = std::fs::remove_file(&lock_path);
                }
            }
        }
    }

    /// Create an in-memory session store (for testing).
    #[allow(clippy::result_large_err)]
    pub fn in_memory() -> Result<Self, StoreError> {
        let db = Arc::new(
            redb::Builder::new()
                .create_with_backend(redb::backends::InMemoryBackend::new())
                .map_err(|e| StoreError::Database(e.into()))?,
        );
        let cas = ContentStore::new(db.clone());
        Ok(Self {
            db,
            cas,
            kind: StoreKind::InMemory,
        })
    }

    /// Save all events for a session. Stores events in CAS and records metadata.
    /// Returns the list of content hashes.
    ///
    /// m9-74 (closes `FIND-M9-73-CAS-PUT-ONE-WRITE-TRANSACTION-PER-EVENT`): the
    /// CAS side is written with `ContentStore::put_many`, so a session costs one
    /// write transaction for its events plus one for its metadata, no matter how
    /// many events it holds. The previous loop called `put` per event and paid a
    /// durability barrier each time (redb's default immediate durability), which
    /// put a 35k-event session at minutes.
    ///
    /// The two transactions are ordered CAS-then-metadata, and a failure between
    /// them leaves CAS content that no session references. That is harmless:
    /// CAS rows are content-addressed and deduplicated, so a later save of the
    /// same events reuses them, and no session can observe half its events.
    #[allow(clippy::result_large_err)]
    pub fn save_session(
        &self,
        metadata: SessionMetadata,
        events: &[TraceEvent],
    ) -> Result<Vec<String>, StoreError> {
        // Validate session_id contains no path separators
        if metadata.session_id.contains('/') || metadata.session_id.contains('\\') {
            return Err(StoreError::InvalidSessionId(metadata.session_id.clone()));
        }

        // Store all events in one CAS write transaction.
        let hashes = self.cas.put_many(events)?;

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
    #[allow(clippy::result_large_err)]
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
                // m9-72: absent table -> SessionNotFound; anything else propagates.
                Err(e) => return Err(classify_read_table_error(e).session_not_found(session_id)),
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
                // m9-72: see the SESSION_META read above.
                Err(e) => return Err(classify_read_table_error(e).session_not_found(session_id)),
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
    ///
    /// Best-effort: a single record whose bytes cannot be deserialized into a
    /// [`SessionMetadata`] (for example a record written by an older binary
    /// whose `SessionMetadata` had a different field layout — `bincode` is not
    /// self-describing) is **skipped** with a warning rather than failing the
    /// entire call. One unreadable record must not brick session listing.
    #[allow(clippy::result_large_err)]
    pub fn list_sessions(&self) -> Result<Vec<SessionMetadata>, StoreError> {
        let tx = self
            .db
            .begin_read()
            .map_err(|e| StoreError::Database(e.into()))?;
        let table = match tx.open_table(SESSION_META) {
            Ok(t) => t,
            // A virgin database has no tables until something is written to it.
            // `redb` reports that as `TableDoesNotExist`; for a read path it means
            // "no sessions yet", not a failure. Without this, `session_list` on a
            // freshly created store (new install, or the hermetic in-memory store
            // used by `chronos-mcp` tests) returned an error instead of `[]`.
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
            Err(e) => return Err(StoreError::Database(e.into())),
        };
        let mut results = Vec::new();

        for entry in table.iter().map_err(|e| StoreError::Database(e.into()))? {
            let (key, value) = entry.map_err(|e| StoreError::Database(e.into()))?;
            match bincode::deserialize::<SessionMetadata>(value.value()) {
                Ok(meta) => results.push(meta),
                Err(e) => {
                    tracing::warn!(
                        "list_sessions: skipping unreadable session metadata for key {:?}: {}",
                        String::from_utf8_lossy(key.value()),
                        e
                    );
                }
            }
        }

        Ok(results)
    }

    /// Delete a session and its event references (not CAS entries — they may be shared).
    #[allow(clippy::result_large_err)]
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
    #[allow(clippy::result_large_err)]
    pub fn session_exists(&self, session_id: &str) -> Result<bool, StoreError> {
        let tx = self
            .db
            .begin_read()
            .map_err(|e| StoreError::Database(e.into()))?;
        let table = match tx.open_table(SESSION_META) {
            Ok(t) => t,
            // See `list_sessions`: an absent table means "no sessions", not an error.
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(false),
            Err(e) => return Err(StoreError::Database(e.into())),
        };
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
                symbol_id: None,
                invocation_id: None,
                parent_invocation_id: None,
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
            tail_sealed: false,
            sealed_at: None,
        }
    }

    /// m9-72: `load_session` must report a storage fault instead of disguising
    /// it as a missing session. The fault is genuine: the `sessions` table is
    /// created with an incompatible signature, so redb fails `open_table` with
    /// `TableTypeMismatch`. Before m9-72 the read paths matched `Err(_)` and
    /// returned `SessionNotFound`, which reads as "you never saved that session"
    /// and hides a broken database.
    #[test]
    fn test_load_session_propagates_storage_fault_instead_of_session_not_found() {
        let store = SessionStore::in_memory().unwrap();
        let tx = store.db().begin_write().unwrap();
        {
            let _ = tx
                .open_table(redb::TableDefinition::<u64, u64>::new("sessions"))
                .expect("creating the sessions table with a foreign signature must succeed");
        }
        tx.commit().unwrap();

        let err = store
            .load_session("s1")
            .expect_err("a storage fault must not read as a missing session");
        assert!(
            matches!(err, StoreError::Database(_)),
            "expected StoreError::Database, got {err:?}"
        );
    }

    /// m9-72: the second `open_table` in `load_session` (the event-hash table)
    /// gets the same treatment. Reaching it needs a database where `sessions`
    /// opens and holds a record while `session_events` is broken, so the
    /// metadata row is written directly with the correct signature and only the
    /// event table is injected with a foreign one.
    #[test]
    fn test_load_session_propagates_event_table_storage_fault() {
        let store = SessionStore::in_memory().unwrap();
        let db = store.db().clone();
        let meta = session_meta("s1");
        let bytes = bincode::serialize(&meta).expect("metadata must serialize");

        let tx = db.begin_write().unwrap();
        {
            let mut table = tx.open_table(SESSION_META).unwrap();
            table.insert("s1".as_bytes(), bytes.as_slice()).unwrap();
            drop(table);
            let _ = tx
                .open_table(redb::TableDefinition::<u64, u64>::new("session_events"))
                .expect("creating session_events with a foreign signature must succeed");
        }
        tx.commit().unwrap();

        let err = store
            .load_session("s1")
            .expect_err("a fault on the event table must not read as a missing session");
        assert!(
            matches!(err, StoreError::Database(_)),
            "expected StoreError::Database, got {err:?}"
        );
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

    /// REQ-ListSkipsUnreadableRecord: one record whose value cannot be
    /// deserialized into `SessionMetadata` (e.g. written by an older binary with
    /// a different field layout — `bincode` is not self-describing) must not
    /// brick the whole listing. The readable sessions are still returned.
    #[test]
    fn test_session_store_list_sessions_skips_unreadable_record() {
        let store = SessionStore::in_memory().unwrap();
        let events = vec![make_event(1, "main")];
        store.save_session(session_meta("good"), &events).unwrap();

        // Inject a record whose value is not valid bincode(SessionMetadata).
        {
            let tx = store.db().begin_write().unwrap();
            let mut table = tx.open_table(SESSION_META).unwrap();
            table
                .insert(
                    b"corrupt".as_slice(),
                    b"\xff\xff\xff\xff\xff\xff".as_slice(),
                )
                .unwrap();
            drop(table);
            tx.commit().unwrap();
        }

        let sessions = store.list_sessions().unwrap();
        assert_eq!(
            sessions.len(),
            1,
            "unreadable record must be skipped, readable one kept"
        );
        assert_eq!(sessions[0].session_id, "good");
    }

    /// A database with no tables yet (fresh on-disk install, or a fresh in-memory
    /// store) has no `sessions` table. Reading must report "no sessions", not
    /// `TableDoesNotExist`. Before this, `session_list` errored on a brand-new
    /// install, and the hermetic in-memory store used by `chronos-mcp` tests could
    /// not be listed at all until a session was saved.
    #[test]
    fn test_list_sessions_on_virgin_store_is_empty() {
        let store = SessionStore::in_memory().unwrap();
        assert!(store.list_sessions().unwrap().is_empty());
    }

    #[test]
    fn test_session_exists_on_virgin_store_is_false() {
        let store = SessionStore::in_memory().unwrap();
        assert!(!store.session_exists("anything").unwrap());
    }

    #[test]
    fn test_session_store_is_persistent_after_in_memory() {
        // m9-82: in-memory stores must self-report as not persistent so
        // higher layers can disclose the degraded mode in tool responses
        // (closes FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE).
        let store = SessionStore::in_memory().unwrap();
        assert!(!store.is_persistent());
    }

    #[test]
    fn test_session_store_is_persistent_after_open() {
        // m9-82: file-backed stores opened via `open(path)` are persistent.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sessions.redb");
        let store = SessionStore::open(&path).expect("open must succeed");
        assert!(store.is_persistent());
    }

    #[test]
    fn test_session_store_is_persistent_after_try_open() {
        // m9-82: file-backed stores opened via `try_open(path)` are persistent.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sessions.redb");
        let store = SessionStore::try_open(&path).expect("try_open must succeed");
        assert!(store.is_persistent());
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

    /// m9-74 (closes FIND-M9-73-CAS-PUT-ONE-WRITE-TRANSACTION-PER-EVENT):
    /// `save_session` must cost a constant number of durability barriers, not
    /// one per event.
    ///
    /// Before this cycle the CAS side looped over `ContentStore::put`, and redb's
    /// default immediate durability makes every `commit()` a `sync_data` barrier,
    /// so this session took 1,000 barriers instead of two. The counting backend
    /// is file-backed for exactly that reason: on an in-memory backend redb never
    /// syncs and the assertion would pass for the wrong reason. The bound leaves
    /// room for redb's own bookkeeping around the two transactions this method
    /// performs (one CAS batch plus one for the metadata and hash list).
    #[cfg(unix)]
    #[test]
    fn test_save_session_costs_a_constant_number_of_durability_barriers() {
        use crate::test_support::counting_file_db;
        use std::sync::atomic::Ordering;

        let dir = tempfile::tempdir().unwrap();
        let (db, syncs) = counting_file_db(dir.path(), "save-session-barriers.redb");
        let store = SessionStore {
            db: db.clone(),
            cas: ContentStore::new(db.clone()),
            kind: StoreKind::Persistent,
        };

        let events: Vec<_> = (0..1_000).map(|i| make_event(i, "batch")).collect();
        let before = syncs.load(Ordering::SeqCst);
        let hashes = store.save_session(session_meta("s1"), &events).unwrap();
        let barriers = syncs.load(Ordering::SeqCst) - before;

        assert_eq!(hashes.len(), events.len());
        assert!(
            barriers <= 4,
            "save_session of {} events issued {barriers} durability barriers; expected \
             one transaction for the CAS batch plus one for the metadata — the \
             per-event commit loop is back",
            events.len()
        );

        // The session must still round-trip: the batching is not allowed to
        // change what is stored, only how it is written.
        let (meta, loaded) = store.load_session("s1").unwrap();
        assert_eq!(meta.session_id, "s1");
        assert_eq!(loaded.len(), events.len());
        for (event, hash) in loaded.iter().zip(&hashes) {
            assert_eq!(
                store.cas.get(hash).unwrap().unwrap().event_id,
                event.event_id
            );
        }
    }
}
