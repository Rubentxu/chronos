//! Counterexample bundle write-path methods on `SessionStore`.
//!
//! Owns the `save_counterexample_bundle` and `save_bundle_record_and_events`
//! methods that persist a bundle record (with its events as chunked side-table
//! rows) in a single atomic write transaction.
//!
//! Submodule of `counterexample_storage`. The parent module re-exports the
//! methods with `pub use` so external callers continue to find them at
//! `chronos_store::counterexample_storage::save_counterexample_bundle`
//! (path unchanged by this split).

use std::mem;

use crate::counterexample_storage::{
    collect_bundle_chunks_legacy, collect_v3_keys_for_bundle, encode_chunk_key,
    encode_chunk_key_legacy, encode_chunk_value, BUNDLE_EVENTS_CHUNK_SIZE, COUNTEREXAMPLE_BUNDLES,
    COUNTEREXAMPLE_BUNDLE_EVENTS, CURRENT_BUNDLE_SCHEMA_VERSION,
};
use crate::error::StoreError;
use crate::storage::SessionStore;
use chronos_domain::TraceEvent;

impl SessionStore {
    /// Save a counterexample bundle record. Returns the (already-existing)
    /// `bundle_id` for caller convenience (the bundle_id lives in the
    /// record itself).
    ///
    /// **m9-02:** This delegates to the private atomic
    /// [`save_bundle_record_and_events`](Self::save_bundle_record_and_events)
    /// which persists the record (with an empty `events` vec) and all events
    /// as chunked rows in `counterexample_bundle_events` in one write
    /// transaction.
    #[allow(clippy::result_large_err)]
    pub fn save_counterexample_bundle(
        &self,
        record: crate::counterexample_storage::CounterexampleBundleRecord,
    ) -> Result<String, StoreError> {
        if record.summary.bundle_id.is_empty() {
            return Err(StoreError::Serialization(
                "bundle_id must be non-empty".to_string(),
            ));
        }
        if record.summary.bundle_id.contains('/') || record.summary.bundle_id.contains('\\') {
            return Err(StoreError::Serialization(format!(
                "bundle_id `{}` contains path separator",
                record.summary.bundle_id
            )));
        }

        // m9-01 D5: always write the current schema version.
        let mut record = record;
        record.schema_version = CURRENT_BUNDLE_SCHEMA_VERSION;
        record.summary.schema_version = CURRENT_BUNDLE_SCHEMA_VERSION;

        // m9-02 D4: take events out of the record; the atomic wrapper
        // will persist them to the side table.
        let events = mem::take(&mut record.events);
        record.summary.events_count = events.len() as u64;

        self.save_bundle_record_and_events(record, events)
    }

    /// Persist a record and its events in one atomic write transaction (m9-02 D4).
    ///
    /// The `record.events` field is expected to be empty (populated by the
    /// caller via `mem::take`). This function writes the record to
    /// `counterexample_bundles` and all `events` as chunked rows in
    /// `counterexample_bundle_events`.
    ///
    /// **m9-04 D6:** Re-save deletes both v3 and v2 prior chunks before
    /// writing new v3 chunks (atomic dual cleanup). This triggers lazy migration:
    /// re-saving a v2 bundle leaves only v3 chunks.
    #[allow(clippy::result_large_err)]
    fn save_bundle_record_and_events(
        &self,
        record: crate::counterexample_storage::CounterexampleBundleRecord,
        events: Vec<TraceEvent>,
    ) -> Result<String, StoreError> {
        let bundle_id = record.summary.bundle_id.clone();

        let bytes =
            bincode::serialize(&record).map_err(|e| StoreError::Serialization(e.to_string()))?;

        let tx = self
            .db()
            .begin_write()
            .map_err(|e| StoreError::Database(e.into()))?;

        // Write the record (events field is empty at this point).
        {
            let mut table = tx
                .open_table(COUNTEREXAMPLE_BUNDLES)
                .map_err(|e| StoreError::Database(e.into()))?;
            table
                .insert(bundle_id.as_bytes(), bytes.as_slice())
                .map_err(|e| StoreError::Database(e.into()))?;
        }

        // m9-04 D6: delete prior chunks for this bundle (both v3 and v2) before
        // writing new v3 chunks. Collect v3 keys via `collect_v3_keys_for_bundle`
        // (m9-05 R2), v2 keys via `collect_bundle_chunks_legacy`.
        let (prior_v3_keys, prior_v2_keys): (Vec<Vec<u8>>, Vec<Vec<u8>>) = {
            let read_tx = self
                .db()
                .begin_read()
                .map_err(|e| StoreError::Database(e.into()))?;

            let v3_keys = collect_v3_keys_for_bundle(&read_tx, &bundle_id)?;
            let v2_keys = collect_bundle_chunks_legacy(&read_tx, &bundle_id)?
                .into_iter()
                .map(|(idx, _)| encode_chunk_key_legacy(&bundle_id, idx))
                .collect();

            (v3_keys, v2_keys)
        };

        {
            let mut events_table = tx
                .open_table(COUNTEREXAMPLE_BUNDLE_EVENTS)
                .map_err(|e| StoreError::Database(e.into()))?;

            // Delete v3 prior chunks.
            for key in &prior_v3_keys {
                events_table
                    .remove(key.as_slice())
                    .map_err(|e| StoreError::Database(e.into()))?;
            }
            // Delete v2 prior chunks.
            for key in &prior_v2_keys {
                events_table
                    .remove(key.as_slice())
                    .map_err(|e| StoreError::Database(e.into()))?;
            }
            // Write new v3 chunks.
            for (chunk_idx, chunk) in events.chunks(BUNDLE_EVENTS_CHUNK_SIZE).enumerate() {
                let key = encode_chunk_key(&bundle_id, chunk_idx as u32);
                // m9-04 D3: value carries bundle_id for per-chunk identity defense.
                let value = encode_chunk_value(&bundle_id, chunk);
                events_table
                    .insert(key.as_slice(), value.as_slice())
                    .map_err(|e| StoreError::Database(e.into()))?;
            }
        }

        tx.commit().map_err(|e| StoreError::Database(e.into()))?;
        Ok(bundle_id)
    }
}
