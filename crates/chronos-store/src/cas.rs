//! Content-Addressable Store for trace events.
//!
//! Stores serialized, compressed events. Each event is hashed with BLAKE3
//! to produce a content address. Identical events are automatically deduplicated.

use crate::error::StoreError;
use crate::table_error::classify_read_table_error;
use blake3::hash;
use chronos_domain::TraceEvent;
use lz4_flex::compress_prepend_size;
use lz4_flex::decompress_size_prepended;
use redb::{ReadableTable, TableDefinition};
use std::sync::Arc;

/// A 32-byte BLAKE3 hash as hex string (64 characters).
pub type ContentHash = String;

/// Table definition for the CAS store.
/// Key: ContentHash as bytes (the hex string)
/// Value: Compressed event bytes
const CAS_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("cas");

/// Content-Addressable Store for trace events.
pub struct ContentStore {
    /// The underlying redb database.
    db: Arc<redb::Database>,
}

impl ContentStore {
    /// Create a new ContentStore wrapping the given redb database.
    ///
    /// The database must already be initialized (e.g. via `redb::Database::create`).
    pub fn new(db: Arc<redb::Database>) -> Self {
        Self { db }
    }

    /// Ensure the CAS table exists (call from a write transaction).
    #[allow(clippy::result_large_err)]
    fn ensure_table(&self, tx: &mut redb::WriteTransaction) -> Result<(), StoreError> {
        tx.open_table(CAS_TABLE)
            .map_err(|e| StoreError::Database(e.into()))?;
        Ok(())
    }

    /// Serialize, compress and hash one event for storage. Pure CPU work, no I/O:
    /// callers run this before opening a write transaction so an encoding failure
    /// cannot leave a half-written batch behind.
    #[allow(clippy::result_large_err)]
    fn encode(event: &TraceEvent) -> Result<(ContentHash, Vec<u8>), StoreError> {
        // Serialize with bincode
        let serialized =
            bincode::serialize(event).map_err(|e| StoreError::Serialization(e.to_string()))?;

        // Compress with lz4
        let compressed = compress_prepend_size(&serialized);

        // Hash with BLAKE3
        let hash_hex = hash(&compressed).to_hex().to_string();

        Ok((hash_hex, compressed))
    }

    /// Write already-encoded events in one write transaction.
    ///
    /// `encoded` is `(hash_hex, compressed_bytes)` in caller order. Rows already
    /// present are skipped, so duplicates inside the batch and duplicates against
    /// the existing store both collapse to a single row: a write transaction reads
    /// its own inserts. Returns as soon as the single `commit()` returns.
    #[allow(clippy::result_large_err)]
    fn insert_batch(&self, encoded: &[(ContentHash, Vec<u8>)]) -> Result<(), StoreError> {
        let mut tx = self
            .db
            .begin_write()
            .map_err(|e| StoreError::Database(e.into()))?;
        self.ensure_table(&mut tx)?;
        let mut table = tx
            .open_table(CAS_TABLE)
            .map_err(|e| StoreError::Database(e.into()))?;

        for (hash_hex, compressed) in encoded {
            let hash_bytes = hash_hex.as_bytes();
            // Only insert if not already present (deduplication)
            if table
                .get(hash_bytes)
                .map_err(|e| StoreError::Database(e.into()))?
                .is_none()
            {
                table
                    .insert(hash_bytes, compressed.as_slice())
                    .map_err(|e| StoreError::Database(e.into()))?;
            }
        }

        drop(table);
        tx.commit().map_err(|e| StoreError::Database(e.into()))?;
        Ok(())
    }

    /// Hash + compress + store a TraceEvent. Returns the hex BLAKE3 hash.
    ///
    /// If the event was already stored, returns the existing hash (dedup).
    /// Uses BLAKE3 for content addressing and LZ4 for compression.
    ///
    /// # Concurrency
    /// This operation is atomic - uses a single write transaction with internal
    /// deduplication check. Concurrent calls with identical content will serialize
    /// at the redb write lock, ensuring exactly one compression + store per unique event.
    #[allow(clippy::result_large_err)]
    pub fn put(&self, event: &TraceEvent) -> Result<ContentHash, StoreError> {
        let (hash_hex, compressed) = Self::encode(event)?;
        self.insert_batch(&[(hash_hex.clone(), compressed)])?;
        Ok(hash_hex)
    }

    /// Hash + compress + store many events in a single write transaction.
    ///
    /// m9-74 (closes `FIND-M9-73-CAS-PUT-ONE-WRITE-TRANSACTION-PER-EVENT`):
    /// `put` in a loop costs one `commit()` per event, and redb's default
    /// immediate durability makes every `commit()` a durability barrier, so
    /// saving a 35k-event session took minutes. This method does the same
    /// encoding work and the same per-event deduplication check, but commits
    /// once for the whole batch.
    ///
    /// Returns one hash per input event, in input order. An empty batch returns
    /// an empty `Vec` without touching the database. Because the whole batch is
    /// encoded before the transaction opens, an encoding failure stores nothing.
    ///
    /// # Concurrency
    /// The batch holds redb's single write lock for its duration, so concurrent
    /// writers (including concurrent `put` calls) serialize behind it. Readers
    /// are unaffected: they see either the state before or after the batch.
    #[allow(clippy::result_large_err)]
    pub fn put_many(&self, events: &[TraceEvent]) -> Result<Vec<ContentHash>, StoreError> {
        if events.is_empty() {
            return Ok(Vec::new());
        }

        let mut encoded = Vec::with_capacity(events.len());
        for event in events {
            encoded.push(Self::encode(event)?);
        }

        let hashes = encoded.iter().map(|(h, _)| h.clone()).collect::<Vec<_>>();
        self.insert_batch(&encoded)?;
        Ok(hashes)
    }

    /// Retrieve and decompress a TraceEvent by its content hash.
    ///
    /// Returns `Ok(None)` if the hash is not found in the store.
    #[allow(clippy::result_large_err)]
    pub fn get(&self, hash_hex: &str) -> Result<Option<TraceEvent>, StoreError> {
        let tx = self
            .db
            .begin_read()
            .map_err(|e| StoreError::Database(e.into()))?;
        let table = match tx.open_table(CAS_TABLE) {
            Ok(t) => t,
            // m9-72: an absent table is a virgin database, not a fault. Any other
            // table error must propagate — see `table_error`.
            Err(e) => return classify_read_table_error(e).or_not_found(None),
        };

        let Some(stored) = table
            .get(hash_hex.as_bytes())
            .map_err(|e| StoreError::Database(e.into()))?
        else {
            return Ok(None);
        };

        let bytes: &[u8] = stored.value();
        let decompressed =
            decompress_size_prepended(bytes).map_err(|e| StoreError::Compression(e.to_string()))?;

        let event: TraceEvent = bincode::deserialize(&decompressed)
            .map_err(|e| StoreError::Serialization(e.to_string()))?;

        Ok(Some(event))
    }

    /// Check if a hash exists in the store without deserializing the event.
    #[allow(clippy::result_large_err)]
    pub fn contains(&self, hash_hex: &str) -> Result<bool, StoreError> {
        let tx = self
            .db
            .begin_read()
            .map_err(|e| StoreError::Database(e.into()))?;
        let table = match tx.open_table(CAS_TABLE) {
            Ok(t) => t,
            // m9-72: see `get` above.
            Err(e) => return classify_read_table_error(e).or_not_found(false),
        };
        Ok(table
            .get(hash_hex.as_bytes())
            .map_err(|e| StoreError::Database(e.into()))?
            .is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::{EventData, EventType, MonotonicNs, SourceLocation};

    fn in_memory_db() -> Arc<redb::Database> {
        Arc::new(
            redb::Builder::new()
                .create_with_backend(redb::backends::InMemoryBackend::new())
                .unwrap(),
        )
    }

    fn make_event(id: u64, func: &str) -> TraceEvent {
        TraceEvent::new(
            id,
            MonotonicNs::from(id * 100),
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

    /// m9-72 (closes FIND-M9-71-LOAD-SESSION-TABLE-ERROR-COLLAPSE): a real
    /// storage fault must be reported, never answered as "content not found".
    ///
    /// The fault is genuine, not simulated: the `cas` table is created with an
    /// incompatible key/value signature, which makes redb fail `open_table`
    /// with `TableTypeMismatch` (a database written by a different binary would
    /// look the same way). Before m9-72 this arm was `Err(_) => Ok(None)`, and
    /// `SessionStore::load_session` consumes `get` in a loop
    /// (`if let Some(evt) = self.cas.get(h)? { events.push(evt) }`), so the
    /// fault silently *dropped events* from the loaded session.
    #[test]
    fn test_get_propagates_storage_fault_instead_of_reporting_missing_content() {
        let db = in_memory_db();
        let tx = db.begin_write().unwrap();
        {
            // Same table name, incompatible signature.
            let _ = tx
                .open_table(TableDefinition::<u64, u64>::new("cas"))
                .expect("creating the cas table with a foreign signature must succeed");
        }
        tx.commit().unwrap();

        let store = ContentStore::new(db);
        match store.get("00") {
            Err(StoreError::Database(_)) => {}
            other => panic!("a storage fault must propagate from get, got {other:?}"),
        }
        match store.contains("00") {
            Err(StoreError::Database(_)) => {}
            other => panic!("a storage fault must propagate from contains, got {other:?}"),
        }
    }

    #[test]
    fn test_cas_put_returns_hash() {
        let store = ContentStore::new(in_memory_db());
        let event = make_event(1, "main");
        let hash = store.put(&event).unwrap();
        // BLAKE3 hex is 64 chars
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_cas_get_returns_event() {
        let store = ContentStore::new(in_memory_db());
        let event = make_event(1, "main");
        let hash = store.put(&event).unwrap();
        let retrieved = store.get(&hash).unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.event_id, 1);
        assert_eq!(retrieved.location.function.as_deref(), Some("main"));
    }

    #[test]
    fn test_cas_same_event_same_hash() {
        let store = ContentStore::new(in_memory_db());
        let event = make_event(1, "main");
        let hash1 = store.put(&event).unwrap();
        let hash2 = store.put(&event).unwrap();
        assert_eq!(hash1, hash2, "Same event must produce same hash (dedup)");
    }

    #[test]
    fn test_cas_different_events_different_hashes() {
        let store = ContentStore::new(in_memory_db());
        let event1 = make_event(1, "main");
        let event2 = make_event(2, "helper");
        let hash1 = store.put(&event1).unwrap();
        let hash2 = store.put(&event2).unwrap();
        assert_ne!(
            hash1, hash2,
            "Different events must produce different hashes"
        );
    }

    #[test]
    fn test_cas_contains() {
        let store = ContentStore::new(in_memory_db());
        let event = make_event(1, "main");
        let hash = store.put(&event).unwrap();
        assert!(store.contains(&hash).unwrap());
        assert!(!store
            .contains("0000000000000000000000000000000000000000000000000000000000000000")
            .unwrap());
    }

    /// m9-74: `put_many` returns one hash per input event, in input order, and
    /// every hash resolves to the event that produced it.
    #[test]
    fn test_put_many_returns_one_hash_per_event_in_order() {
        let store = ContentStore::new(in_memory_db());
        let reference = in_memory_db_store();
        let events: Vec<_> = (0..50).map(|i| make_event(i, "batch")).collect();

        let hashes = store.put_many(&events).unwrap();

        assert_eq!(hashes.len(), events.len());
        for (event, hash) in events.iter().zip(&hashes) {
            let loaded = store.get(hash).unwrap().expect("every hash must resolve");
            assert_eq!(loaded.event_id, event.event_id);
            // The hash is the content address, so it must equal the one `put`
            // computes for the same event: the batch path is not a second
            // hashing scheme.
            assert_eq!(reference.put(event).unwrap(), *hash);
        }
    }

    /// m9-74: duplicates inside one batch collapse to a single row, and a batch
    /// of events already stored writes nothing new — the same dedup `put`
    /// promises, now inside one transaction for the whole batch.
    #[test]
    fn test_put_many_deduplicates_within_and_across_batches() {
        let db = in_memory_db();
        let store = ContentStore::new(db.clone());
        let event = make_event(7, "dup");

        let hashes = store
            .put_many(&[event.clone(), event.clone(), event.clone()])
            .unwrap();

        assert_eq!(hashes.len(), 3);
        assert_eq!(hashes[0], hashes[1]);
        assert_eq!(hashes[1], hashes[2]);
        assert_eq!(
            cas_row_count(&db),
            1,
            "three identical events must occupy one row"
        );

        // Re-sending the same event in a later batch is also a no-op.
        let again = store
            .put_many(&[event.clone(), make_event(8, "new")])
            .unwrap();
        assert_eq!(again.len(), 2);
        assert_eq!(again[0], hashes[0]);
        assert_eq!(cas_row_count(&db), 2, "the repeat must not add a row");
    }

    /// m9-74: an empty batch is a no-op, not an empty transaction. A fresh
    /// database must still have no `cas` table afterwards, which is how we know
    /// nothing was committed.
    #[test]
    fn test_put_many_empty_batch_does_not_touch_the_database() {
        let db = in_memory_db();
        let store = ContentStore::new(db.clone());

        let hashes = store.put_many(&[]).unwrap();

        assert!(hashes.is_empty());
        let tx = db.begin_read().unwrap();
        assert!(
            tx.open_table(CAS_TABLE).is_err(),
            "an empty batch must not create the cas table"
        );
    }

    /// m9-74 (closes FIND-M9-73-CAS-PUT-ONE-WRITE-TRANSACTION-PER-EVENT): one
    /// batch costs one durability barrier, no matter how many events it holds.
    ///
    /// This is the assertion the fix exists for and it is not a timing
    /// measurement: `test_support::counting_file_db` wraps redb's real
    /// `FileBackend` and counts the `sync_data` calls redb issues at commit
    /// time, which is exactly what redb's default immediate durability makes
    /// expensive. The control loop at the end is what keeps the bound honest —
    /// it proves this backend really does charge one barrier per commit, so
    /// `put_many`'s single barrier is a property of the batching and not of an
    /// environment that never syncs at all.
    #[cfg(unix)]
    #[test]
    fn test_put_many_commits_once_per_batch() {
        use crate::test_support::counting_file_db;
        use std::sync::atomic::Ordering;

        let dir = tempfile::tempdir().unwrap();
        let (db, syncs) = counting_file_db(dir.path(), "cas-barriers.redb");
        let store = ContentStore::new(db);

        let batch: Vec<_> = (0..500).map(|i| make_event(i, "batch")).collect();
        let before = syncs.load(Ordering::SeqCst);
        let hashes = store.put_many(&batch).unwrap();
        let batch_barriers = syncs.load(Ordering::SeqCst) - before;

        assert_eq!(hashes.len(), batch.len());
        assert!(
            batch_barriers <= 3,
            "put_many of 500 events issued {batch_barriers} durability barriers; \
             expected a small constant for the whole batch — the per-event commit \
             loop is back"
        );

        // Control: the same backend charges one barrier per `put` in a loop.
        let singles: Vec<_> = (0..50).map(|i| make_event(10_000 + i, "loop")).collect();
        let before_loop = syncs.load(Ordering::SeqCst);
        for event in &singles {
            store.put(event).unwrap();
        }
        let loop_barriers = syncs.load(Ordering::SeqCst) - before_loop;
        assert!(
            loop_barriers >= singles.len(),
            "expected at least one durability barrier per put, saw {loop_barriers} for \
             {} events — this control must fail loudly if the backend stops syncing",
            singles.len()
        );
    }

    /// Row count of the `cas` table, for dedup assertions. Returns 0 when the
    /// table does not exist (a database nothing has been written to).
    fn cas_row_count(db: &Arc<redb::Database>) -> u64 {
        let tx = db.begin_read().unwrap();
        use redb::ReadableTableMetadata;
        match tx.open_table(CAS_TABLE) {
            Ok(table) => table.len().unwrap(),
            Err(_) => 0,
        }
    }

    fn in_memory_db_store() -> ContentStore {
        ContentStore::new(in_memory_db())
    }
}
