//! Content-Addressable Store for trace events.
//!
//! Stores serialized, compressed events. Each event is hashed with BLAKE3
//! to produce a content address. Identical events are automatically deduplicated.

use crate::error::StoreError;
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
    fn ensure_table(&self, tx: &mut redb::WriteTransaction) -> Result<(), StoreError> {
        tx.open_table(CAS_TABLE)
            .map_err(|e| StoreError::Database(e.into()))?;
        Ok(())
    }

    /// Hash + compress + store a TraceEvent. Returns the hex BLAKE3 hash.
    ///
    /// If the event was already stored, returns the existing hash (dedup).
    /// Uses BLAKE3 for content addressing and LZ4 for compression.
    pub fn put(&self, event: &TraceEvent) -> Result<ContentHash, StoreError> {
        // Serialize with bincode
        let serialized =
            bincode::serialize(event).map_err(|e| StoreError::Serialization(e.to_string()))?;

        // Compress with lz4
        let compressed = compress_prepend_size(&serialized);

        // Hash with BLAKE3
        let hash_hex = hash(&compressed).to_hex().to_string();
        let hash_bytes = hash_hex.as_bytes();

        // Check if already stored (read transaction)
        let exists = {
            let tx = self
                .db
                .begin_read()
                .map_err(|e| StoreError::Database(e.into()))?;
            match tx.open_table(CAS_TABLE) {
                Ok(table) => table
                    .get(hash_bytes)
                    .map_err(|e| StoreError::Database(e.into()))?
                    .is_some(),
                Err(_) => false, // Table doesn't exist yet
            }
        };

        if !exists {
            let mut tx = self
                .db
                .begin_write()
                .map_err(|e| StoreError::Database(e.into()))?;
            // Ensure table exists (creates if new)
            self.ensure_table(&mut tx)?;
            let mut table = tx
                .open_table(CAS_TABLE)
                .map_err(|e| StoreError::Database(e.into()))?;
            // Check again in write transaction
            if table
                .get(hash_bytes)
                .map_err(|e| StoreError::Database(e.into()))?
                .is_none()
            {
                table
                    .insert(hash_bytes, compressed.as_slice())
                    .map_err(|e| StoreError::Database(e.into()))?;
            }
            drop(table);
            tx.commit().map_err(|e| StoreError::Database(e.into()))?;
        }

        Ok(hash_hex)
    }

    /// Retrieve and decompress a TraceEvent by its content hash.
    ///
    /// Returns `Ok(None)` if the hash is not found in the store.
    pub fn get(&self, hash_hex: &str) -> Result<Option<TraceEvent>, StoreError> {
        let tx = self
            .db
            .begin_read()
            .map_err(|e| StoreError::Database(e.into()))?;
        let table = match tx.open_table(CAS_TABLE) {
            Ok(t) => t,
            Err(_) => return Ok(None), // Table doesn't exist
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
    pub fn contains(&self, hash_hex: &str) -> Result<bool, StoreError> {
        let tx = self
            .db
            .begin_read()
            .map_err(|e| StoreError::Database(e.into()))?;
        let table = match tx.open_table(CAS_TABLE) {
            Ok(t) => t,
            Err(_) => return Ok(false), // Table doesn't exist
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
    use chronos_domain::{EventData, EventType, SourceLocation};

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
}
