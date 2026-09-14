//! Counterexample bundle test chokepoints (m9-05 R4).
//!
//! These `#[doc(hidden)] pub fn` methods exist to give the cli integration
//! test (`crates/chronos-cli/tests/replay_integration.rs`) a narrow,
//! versioned surface for direct table manipulation without needing to
//! widen `storage.rs::db()` or expose the table constants.
//!
//! Submodule of `counterexample_storage`. The parent module re-exports the
//! methods with `pub use` so the cli test continues to find them at
//! `chronos_store::counterexample_storage::insert_v2_chunk_for_test`
//! (path unchanged by this split).

use crate::counterexample_storage::{
    encode_chunk_key_legacy, CounterexampleBundleRecord, COUNTEREXAMPLE_BUNDLES,
    COUNTEREXAMPLE_BUNDLE_EVENTS,
};
use crate::error::StoreError;
use crate::storage::SessionStore;
use chronos_domain::TraceEvent;

impl SessionStore {
    /// Insert a v2-format chunk directly into the side table (m9-05 R4 test chokepoint).
    ///
    /// Replaces the cli integration test's direct `db().begin_write()` +
    /// `open_table(COUNTEREXAMPLE_BUNDLE_EVENTS)` + `insert` dance. Used by
    /// `m9_04_replay_v2_bundle_uses_legacy_path` to simulate a pre-m9-04 bundle
    /// that has not yet been re-saved.
    #[doc(hidden)]
    #[allow(clippy::result_large_err)]
    pub fn insert_v2_chunk_for_test(
        &self,
        bundle_id: &str,
        chunk_idx: u32,
        events: &[TraceEvent],
    ) -> Result<(), StoreError> {
        let key = encode_chunk_key_legacy(bundle_id, chunk_idx);
        let value =
            bincode::serialize(events).map_err(|e| StoreError::Serialization(e.to_string()))?;
        let tx = self
            .db()
            .begin_write()
            .map_err(|e| StoreError::Database(e.into()))?;
        {
            let mut table = tx
                .open_table(COUNTEREXAMPLE_BUNDLE_EVENTS)
                .map_err(|e| StoreError::Database(e.into()))?;
            table
                .insert(key.as_slice(), value.as_slice())
                .map_err(|e| StoreError::Database(e.into()))?;
        }
        tx.commit().map_err(|e| StoreError::Database(e.into()))?;
        Ok(())
    }

    /// Count v3 chunks for a bundle (m9-05 R4 test chokepoint).
    ///
    /// Replaces the cli integration test's direct `db().begin_read()` +
    /// `collect_bundle_chunks_range` call. Returns 0 when the bundle has no v3 chunks.
    #[doc(hidden)]
    #[allow(clippy::result_large_err)]
    pub fn count_v3_chunks_for_test(&self, bundle_id: &str) -> Result<u64, StoreError> {
        let tx = self
            .db()
            .begin_read()
            .map_err(|e| StoreError::Database(e.into()))?;
        let chunks = crate::counterexample_storage::collect_bundle_chunks_range(&tx, bundle_id)?;
        Ok(chunks.len() as u64)
    }

    /// Insert a bundle record directly into `counterexample_bundles` (m9-05 R4 test chokepoint).
    ///
    /// Used by `m9_04_replay_v2_bundle_uses_legacy_path` to inject a bundle record
    /// with `events_count > 0` so the D7 guard lets the v2 fallback run, without
    /// going through `save_counterexample_bundle` (which would overwrite `events_count`
    /// based on `events.len()`).
    #[doc(hidden)]
    #[allow(clippy::result_large_err)]
    pub fn insert_bundle_record_for_test(
        &self,
        record: &CounterexampleBundleRecord,
    ) -> Result<(), StoreError> {
        let bytes =
            bincode::serialize(record).map_err(|e| StoreError::Serialization(e.to_string()))?;
        let tx = self
            .db()
            .begin_write()
            .map_err(|e| StoreError::Database(e.into()))?;
        {
            let mut table = tx
                .open_table(COUNTEREXAMPLE_BUNDLES)
                .map_err(|e| StoreError::Database(e.into()))?;
            table
                .insert(record.summary.bundle_id.as_bytes(), bytes.as_slice())
                .map_err(|e| StoreError::Database(e.into()))?;
        }
        tx.commit().map_err(|e| StoreError::Database(e.into()))?;
        Ok(())
    }
}
