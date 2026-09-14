//! Counterexample bundle read-path methods on `SessionStore`.
//!
//! Owns the 5 read-path methods:
//! - `load_counterexample_bundle_events` — load all events for a bundle.
//! - `count_counterexample_bundle_events` — count events without loading.
//! - `get_bundle_events_count` — read `summary.events_count` from the
//!   bundle record (private helper for the two methods above; m9-05 R3).
//! - `load_counterexample_bundle` — load the bundle record by id.
//! - `list_counterexample_bundles` — list summaries with cursor + filters.
//!
//! Submodule of `counterexample_storage`. The parent module re-exports the
//! methods with `pub use` so external callers continue to find them at
//! `chronos_store::counterexample_storage::load_counterexample_bundle`
//! (path unchanged by this split).

use crate::counterexample_storage::{
    collect_bundle_chunks, decode_chunk_payload, CounterexampleBundleFilter,
    CounterexampleBundleRecord, CounterexampleBundleSummary, COUNTEREXAMPLE_BUNDLES,
    CURRENT_BUNDLE_SCHEMA_VERSION,
};
use crate::error::StoreError;
use crate::storage::SessionStore;
use crate::table_error::classify_read_table_error;
use chronos_domain::TraceEvent;
use redb::ReadableTable;

impl SessionStore {
    /// Load all events for a bundle from the side table, sorted by chunk index.
    ///
    /// **m9-04 D7:** Uses `collect_bundle_chunks` which tries v3 range scan first,
    /// falling back to v2 full scan when v3 returns 0 rows.
    ///
    /// Returns `Ok(vec![])` when no chunks exist (the table is absent or
    /// the bundle has no side-table events).
    ///
    /// Internal helper: look up `summary.events_count` from the bundle record (m9-05 R3).
    /// Returns `events_count` when the bundle record exists, or `0` when the table
    /// is absent or no record exists for the queried `bundle_id`. Used by
    /// `load_counterexample_bundle_events` and `count_counterexample_bundle_events`
    /// to pass the D7 `events_count == 0` guard into `collect_bundle_chunks`.
    #[allow(clippy::result_large_err)]
    fn get_bundle_events_count(
        tx: &redb::ReadTransaction,
        bundle_id: &str,
    ) -> Result<u64, StoreError> {
        let table = match tx.open_table(COUNTEREXAMPLE_BUNDLES) {
            Ok(t) => t,
            Err(e) => return classify_read_table_error(e).or_not_found(0_u64),
        };
        let bytes_opt = match table.get(bundle_id.as_bytes()) {
            Ok(o) => o,
            Err(e) => return Err(StoreError::Database(e.into())),
        };
        match bytes_opt {
            None => Ok(0),
            Some(guard) => {
                let record: CounterexampleBundleRecord = bincode::deserialize(guard.value())
                    .map_err(|e| StoreError::Serialization(e.to_string()))?;
                Ok(record.summary.events_count)
            }
        }
    }

    #[allow(clippy::result_large_err)]
    pub fn load_counterexample_bundle_events(
        &self,
        bundle_id: &str,
    ) -> Result<Vec<TraceEvent>, StoreError> {
        let tx = self
            .db()
            .begin_read()
            .map_err(|e| StoreError::Database(e.into()))?;

        let events_count = Self::get_bundle_events_count(&tx, bundle_id)?;
        let chunks_data = collect_bundle_chunks(&tx, bundle_id, events_count)?;
        let mut chunks: Vec<(u32, Vec<TraceEvent>)> = Vec::new();
        for (idx, bytes) in chunks_data {
            // m9-05 R1: v3-first / v2-fallback decode ladder extracted into
            // `decode_chunk_payload`. Drops entries neither layout can decode.
            if let Some(events) = decode_chunk_payload(&bytes) {
                chunks.push((idx, events));
            }
        }

        chunks.sort_by_key(|(idx, _)| *idx);
        let mut result = Vec::new();
        for (_, chunk) in chunks {
            result.extend(chunk);
        }
        Ok(result)
    }

    /// Count total events across all chunks for a bundle.
    ///
    /// **m9-04 D7:** Uses `collect_bundle_chunks` which tries v3 range scan first,
    /// falling back to v2 full scan when v3 returns 0 rows.
    ///
    /// Returns 0 when the side table does not exist or the bundle has no chunks.
    #[allow(clippy::result_large_err)]
    pub fn count_counterexample_bundle_events(&self, bundle_id: &str) -> Result<u64, StoreError> {
        let tx = self
            .db()
            .begin_read()
            .map_err(|e| StoreError::Database(e.into()))?;

        let events_count = Self::get_bundle_events_count(&tx, bundle_id)?;
        let chunks_data = collect_bundle_chunks(&tx, bundle_id, events_count)?;
        let mut total: u64 = 0;
        for (_idx, bytes) in chunks_data {
            // m9-05 R1: shared with `load_counterexample_bundle_events`.
            if let Some(events) = decode_chunk_payload(&bytes) {
                total += events.len() as u64;
            }
        }
        Ok(total)
    }

    /// Load a counterexample bundle by id.
    ///
    /// Returns `Ok(None)` when the bundle does not exist (idempotent —
    /// let the caller decide between LoadFailed vs NotFound). Also
    /// returns `Ok(None)` when the `counterexample_bundles` table has
    /// never been written (a fresh in-memory DB or a live DB on which
    /// no shrink has ever run); redb's read-only `open_table` errors
    /// `TableDoesNotExist` in that case.
    #[allow(clippy::result_large_err)]
    pub fn load_counterexample_bundle(
        &self,
        bundle_id: &str,
    ) -> Result<Option<CounterexampleBundleRecord>, StoreError> {
        let tx = self
            .db()
            .begin_read()
            .map_err(|e| StoreError::Database(e.into()))?;
        let table_result = tx.open_table(COUNTEREXAMPLE_BUNDLES);
        let table = match table_result {
            Ok(t) => t,
            Err(e) => {
                return classify_read_table_error(e)
                    .or_not_found(None::<CounterexampleBundleRecord>)
            }
        };
        let bytes_opt = match table.get(bundle_id.as_bytes()) {
            Ok(o) => o,
            Err(e) => return Err(StoreError::Database(e.into())),
        };
        match bytes_opt {
            None => Ok(None),
            Some(bytes_guard) => {
                let bytes: &[u8] = bytes_guard.value();
                let record: CounterexampleBundleRecord = bincode::deserialize(bytes)
                    .map_err(|e| StoreError::Serialization(e.to_string()))?;
                // m9-01 D3: reject bundles written by a newer chronos-store.
                // This is a hard-reject (not a warning) because silently loading
                // a future-versioned bundle risks panicking on unknown enum
                // variants in nested wire types downstream.
                if record.schema_version > CURRENT_BUNDLE_SCHEMA_VERSION {
                    // m9-08 (closes FIND-M9-01-DV-COUP-02): previously this
                    // overload-returned `StoreError::Serialization`, which
                    // collapsed corrupt-blob failures and forward-compat
                    // rejections into one variant callers could not
                    // distinguish without message parsing. Dedicate the
                    // `SchemaTooNew { found, supported }` variant so callers
                    // can branch on the error kind itself.
                    return Err(StoreError::SchemaTooNew {
                        found: record.schema_version,
                        supported: CURRENT_BUNDLE_SCHEMA_VERSION,
                    });
                }
                // m9-07 (closes FIND-M9-01-DV-COUP-01): the record and its
                // nested summary both carry a `schema_version`. `save()`
                // canonicalizes both to `CURRENT_BUNDLE_SCHEMA_VERSION`, but a
                // hand-constructed record (e.g. a future migration tool) could
                // mismatch. The loader reads `record.schema_version` as the
                // envelope-level authoritative version; assert the summary
                // matches before returning. Mirrors the invariant asserted at
                // save time without a second canonicalization site.
                if record.schema_version != record.summary.schema_version {
                    return Err(StoreError::Serialization(format!(
                        "bundle envelope schema_version {} disagrees with summary \
                         schema_version {}; rejecting as malformed",
                        record.schema_version, record.summary.schema_version,
                    )));
                }
                Ok(Some(record))
            }
        }
    }

    /// List bundle summaries matching `filter`, oldest-first (uuid::v7
    /// lexicographic order matches chronological).
    ///
    /// m8-05 (B2 note): when `filter.cursor` is `Some(c)`, the iteration
    /// skips rows whose `bundle_id <= c` before applying other filters
    /// and `limit`. This gives forward pagination: the first page returns
    /// up to `limit` rows and sets `next_cursor = last.bundle_id`. The
    /// next call passes that bundle_id as `cursor` to fetch the
    /// following page.
    ///
    /// Returns `Ok(vec![])` when the `counterexample_bundles` table has
    /// never been written (redb's read-only `open_table` errors
    /// `TableDoesNotExist` in that case; we collapse to empty per the
    /// `load_counterexample_bundle` precedent).
    #[allow(clippy::result_large_err)]
    pub fn list_counterexample_bundles(
        &self,
        filter: CounterexampleBundleFilter<'_>,
    ) -> Result<Vec<CounterexampleBundleSummary>, StoreError> {
        let tx = self
            .db()
            .begin_read()
            .map_err(|e| StoreError::Database(e.into()))?;
        let table = match tx.open_table(COUNTEREXAMPLE_BUNDLES) {
            Ok(t) => t,
            Err(e) => return classify_read_table_error(e).or_not_found(Vec::new()),
        };

        let mut out: Vec<CounterexampleBundleSummary> = Vec::new();
        let iter = table.iter().map_err(|e| StoreError::Database(e.into()))?;
        let limit_usize: Option<usize> = if filter.limit == 0 {
            None
        } else {
            Some(filter.limit as usize)
        };

        for entry in iter {
            let (_k, v) = entry.map_err(|e| StoreError::Database(e.into()))?;
            let bytes: &[u8] = v.value();
            let record: CounterexampleBundleRecord = match bincode::deserialize(bytes) {
                Ok(r) => r,
                // Skip corrupt rows but don't fail the whole list (best-effort).
                Err(_) => continue,
            };
            let s = &record.summary;
            // m8-05 B2: forward pagination. Skip rows at or before the
            // cursor's bundle_id (uuid::v7 is monotonic, so the row key
            // matches chronological order).
            if let Some(c) = filter.cursor.as_deref() {
                if s.bundle_id.as_str() <= c {
                    continue;
                }
            }
            if let Some(w) = filter.workspace_id {
                if s.workspace_id != w {
                    continue;
                }
            }
            if let Some(k) = filter.property_kind {
                if s.property_kind != k {
                    continue;
                }
            }
            if let Some(since) = filter.since_ms {
                if s.created_at_ms < since {
                    continue;
                }
            }
            if let Some(until) = filter.until_ms {
                if s.created_at_ms > until {
                    continue;
                }
            }
            out.push(s.clone());
            if let Some(l) = limit_usize {
                if out.len() >= l {
                    break;
                }
            }
        }

        // Sort by uuid::v7 ordering: bundle_id is uuid::v7 (m8-02).
        // Lexicographic sort on uuid::v7 ≈ chronological order.
        out.sort_by(|a, b| a.bundle_id.cmp(&b.bundle_id));
        Ok(out)
    }
}
