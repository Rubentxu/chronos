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
    collect_legacy_keys_for_table, collect_v3_keys_for_table, encode_chunk_key, encode_chunk_value,
    BUNDLE_EVENTS_CHUNK_SIZE, COUNTEREXAMPLE_BUNDLES, COUNTEREXAMPLE_BUNDLE_EVENTS,
    CURRENT_BUNDLE_SCHEMA_VERSION,
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

    /// Persist a record and its events in one atomic write transaction (m9-02 D4,
    /// **m9-97 atomic**).
    ///
    /// The `record.events` field is expected to be empty (populated by the
    /// caller via `mem::take`). This function writes the record to
    /// `counterexample_bundles` and all `events` as chunked rows in
    /// `counterexample_bundle_events`.
    ///
    /// **m9-04 D6:** Re-save deletes both v3 and v2 prior chunks before
    /// writing new v3 chunks (atomic dual cleanup). This triggers lazy migration:
    /// re-saving a v2 bundle leaves only v3 chunks.
    ///
    /// **m9-97:** Discovery (`collect_v3_keys_for_table` /
    /// `collect_legacy_keys_for_table`), deletion, and new-chunk insertion
    /// all run on the **same** write-tx-opened `Table` handle. There is no
    /// separate `ReadTransaction`, which closes `cc-004-implicit-io-toctou`
    /// (a TOCTOU window where a concurrent writer could commit between the
    /// read and the write, leaving the local record's `events_count`
    /// inconsistent with the surviving chunks).
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

        // m9-97 atomic: open the events table once from the same write tx
        // and run discovery + delete + insert against the same handle.
        // No separate read tx is opened, so there is no TOCTOU window
        // between key discovery and chunk deletion (cc-004-implicit-io-toctou).
        let mut events_table = tx
            .open_table(COUNTEREXAMPLE_BUNDLE_EVENTS)
            .map_err(|e| StoreError::Database(e.into()))?;

        // m9-04 D6: delete prior chunks for this bundle (both v3 and v2)
        // before writing new v3 chunks. m9-97: discovery is on the same
        // write tx as the deletion.
        let prior_v3_keys = collect_v3_keys_for_table(&events_table, &bundle_id)?;
        let prior_v2_keys = collect_legacy_keys_for_table(&events_table, &bundle_id)?;

        for key in &prior_v3_keys {
            events_table
                .remove(key.as_slice())
                .map_err(|e| StoreError::Database(e.into()))?;
        }
        for key in &prior_v2_keys {
            events_table
                .remove(key.as_slice())
                .map_err(|e| StoreError::Database(e.into()))?;
        }
        for (chunk_idx, chunk) in events.chunks(BUNDLE_EVENTS_CHUNK_SIZE).enumerate() {
            let key = encode_chunk_key(&bundle_id, chunk_idx as u32);
            // m9-04 D3: value carries bundle_id for per-chunk identity defense.
            let value = encode_chunk_value(&bundle_id, chunk);
            events_table
                .insert(key.as_slice(), value.as_slice())
                .map_err(|e| StoreError::Database(e.into()))?;
        }
        // Drop the table handle so the subsequent `tx.commit()` does not
        // have to move out from under a still-borrowed `events_table`.
        drop(events_table);

        tx.commit().map_err(|e| StoreError::Database(e.into()))?;
        Ok(bundle_id)
    }
}
