//! Counterexample bundle storage — persists shrink results via redb.
//!
//! Bundles have a distinct identity (`bundle_id: String`) from sessions
//! (`session_id: String`) because their lifecycle + GC + persistence
//! policy differs (m8-01 § "Bundles have a distinct identity"). They live
//! in a dedicated redb table.
//!
//! The full record envelope is stored as a single bincode blob per
//! `bundle_id`. Bundles are typically small (a shrunk subset of one
//! session's events), so blob storage is acceptable for m8-03 (R4
//! mitigation: split into CAS hashes in a follow-up if profiling demands).
//!
//! **Circular-dep disclosure**: chronos-services → chronos-store is
//! already the dependency direction. This module is intentionally
//! independent of chronos-services: the `ExistencePredicateWire` enum
//! here is a separate, line-for-line mirror of
//! `chronos_services::output::ExistencePredicate`, and the boundary
//! conversion happens in `services::counterexample`.
//!
//! See `docs/milestones/m8-03-mcp-wrappers-redb-bundle-table-scoping.md`
//! § "2. counterexample_bundles redb table" for the design.
//!
//! **Schema versioning (m9-01 / m9-02):** `CounterexampleBundleRecord` and
//! `CounterexampleBundleSummary` carry a `schema_version: u32` field
//! that lets the loader distinguish legacy bundles (no field on disk,
//! serde defaults to 1) from future-versioned bundles. The current
//! version is [`CURRENT_BUNDLE_SCHEMA_VERSION`]. Bundles written by a
//! newer chronos-store with a higher version are hard-rejected on
//! `load_counterexample_bundle`; the list path best-effort skips them.
//!
//! **m9-02:** Events are moved out of the blob into `counterexample_bundle_events`
//! side table (chunked, 256 events per chunk). `summary.events_count` carries
//! the O(1) count. Legacy bundles continue to load via `bundle_events_or_legacy`.
//!

use std::mem;

use crate::cas::ContentHash;
use crate::error::StoreError;
use chronos_domain::property::PropertyValue;
use chronos_domain::TraceEvent;
use redb::{ReadableTable, TableDefinition, TableError};
use serde::{Deserialize, Serialize};

/// Table for counterexample bundles.
///
/// Key: `bundle_id: String` (as bytes).
/// Value: bincode-serialised [`CounterexampleBundleRecord`].
const COUNTEREXAMPLE_BUNDLES: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("counterexample_bundles");

/// Side table for counterexample bundle events (m9-02).
///
/// Key: binary-encoded `(bundle_id, chunk_index)` pair.
///   - First 4 bytes: length of bundle_id as big-endian u32
///   - Next N bytes: bundle_id UTF-8 bytes
///   - Final 4 bytes: chunk_index as big-endian u32
///   - Value: bincode-serialised `Vec<TraceEvent>` chunk.
///
/// Chunk size is [`BUNDLE_EVENTS_CHUNK_SIZE`]. Each chunk contains up to
/// that many events; the last chunk may be smaller.
const COUNTEREXAMPLE_BUNDLE_EVENTS: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("counterexample_bundle_events");

/// Encode `(bundle_id, chunk_index)` into a byte key for
/// `COUNTEREXAMPLE_BUNDLE_EVENTS`.
///
/// Layout: [len(bundle_id) as u32 BE][bundle_id bytes][chunk_index as u32 BE]
fn encode_chunk_key(bundle_id: &str, chunk_index: u32) -> Vec<u8> {
    let id_bytes = bundle_id.as_bytes();
    let mut key = Vec::with_capacity(4 + id_bytes.len() + 4);
    key.extend_from_slice(&(id_bytes.len() as u32).to_be_bytes());
    key.extend_from_slice(id_bytes);
    key.extend_from_slice(&chunk_index.to_be_bytes());
    key
}

/// Decode a byte key back to `(bundle_id, chunk_index)`.
fn decode_chunk_key(key: &[u8]) -> Option<(String, u32)> {
    if key.len() < 8 {
        return None;
    }
    let id_len = u32::from_be_bytes(key[..4].try_into().ok()?) as usize;
    if key.len() < 8 + id_len {
        return None;
    }
    let id_bytes = &key[4..4 + id_len];
    let chunk_bytes: [u8; 4] = key[4 + id_len..].try_into().ok()?;
    let bundle_id = String::from_utf8(id_bytes.to_vec()).ok()?;
    let chunk_index = u32::from_be_bytes(chunk_bytes);
    Some((bundle_id, chunk_index))
}

/// Chunk size for the `counterexample_bundle_events` side table.
///
/// Each chunk holds up to 256 `TraceEvent` items. 256 events at typical
/// event size (~100–500 bytes) produces rows of ~25–125 KB, well within
/// redb's default page size and giving 4 chunks for a typical 1000-event
/// bundle.
///
/// This lives at module scope next to `CURRENT_BUNDLE_SCHEMA_VERSION`
/// (m9-02 D7) so it is grep-able alongside the version constant.
/// Collect all chunk rows for `bundle_id` in `COUNTEREXAMPLE_BUNDLE_EVENTS`.
///
/// Returns `Ok(vec![])` when the table does not exist.
#[allow(clippy::result_large_err)]
fn collect_bundle_chunks(
    tx: &redb::ReadTransaction,
    bundle_id: &str,
) -> Result<Vec<(u32, Vec<u8>)>, StoreError> {
    let table = match tx.open_table(COUNTEREXAMPLE_BUNDLE_EVENTS) {
        Ok(t) => t,
        Err(redb::TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
        Err(e) => return Err(StoreError::Database(e.into())),
    };
    let mut chunks = Vec::new();
    for entry in table.iter().map_err(|e| StoreError::Database(e.into()))? {
        let (k, v) = entry.map_err(|e| StoreError::Database(e.into()))?;
        if let Some((ref id, idx)) = decode_chunk_key(k.value()) {
            if id == bundle_id {
                chunks.push((idx, v.value().to_vec()));
            }
        }
    }
    Ok(chunks)
}

pub const BUNDLE_EVENTS_CHUNK_SIZE: usize = 256;

/// Canonical "what we write today" value. Bumped when the bundle
/// envelope (record + summary + nested wire types) changes in a way
/// that requires a loader-side decision. See module docs.
///
/// m9-01 initial value: 1. All bundles persisted before m9-01 have no
/// field on disk; serde defaults to 1 on load.
///
/// m9-02 bump to 2: events move to side table; `summary.events_count` added.
pub const CURRENT_BUNDLE_SCHEMA_VERSION: u32 = 2;

/// Versions the loader accepts silently. Future cycles add entries here
/// when they introduce a new envelope shape.
#[allow(dead_code)]
const KNOWN_BUNDLE_SCHEMA_VERSIONS: &[u32] = &[1, 2];

fn default_schema_version() -> u32 {
    CURRENT_BUNDLE_SCHEMA_VERSION
}

/// What causal slice triggered the violation. Plumbed via the bundle so
/// `counterexample_get` can re-emit the relevant trace window without
/// re-running the shrink.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MinimisedPayload {
    /// Minimised `PropertyValue` for an `Invariant` shrink.
    Constant(PropertyValue),
    /// Minimised predicate for an `Existence` shrink. Same wire shape as
    /// `chronos_services::output::ExistencePredicate` but a separate type
    /// to break the circular dep. The dispatcher in
    /// `services::counterexample` does a `match`-based conversion at
    /// the boundary.
    Predicate(ExistencePredicateWire),
    /// Minimised (caller, callee, optional max_depth) for a `CallPath` shrink.
    CallPath {
        caller: String,
        callee: String,
        max_depth: Option<u64>,
    },
}

/// Wire-friendly existence predicate, owned by chronos-store.
///
/// Mirrors `chronos_services::output::ExistencePredicate` line-for-line
/// (same three variants, same field names). Kept independent to break
/// the circular dependency.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExistencePredicateWire {
    EventTypeEquals { event_type: String },
    ThreadEquals { thread_id: u64 },
    PropertyKeyEquals { target: String },
}

/// m8-07: wire mirror of `chronos_services::hypothesis_test::HypothesisInput`.
///
/// Persisted in the bundle record (alongside the `minimised` payload) so
/// `chronos test replay` can reconstruct the EXACT HypothesisInput the user
/// passed to `counterexample_shrink`, instead of synthesising defaults from
/// the minimised payload (the m8-04 R-hypothesis-reconstruction-fidelity gap).
///
/// R1 disclosure (m8-07): this is a hand-maintained mirror of the
/// chronos-services type. Drift is possible if `HypothesisInput` evolves
/// without updating this mirror; mitigated by the roundtrip test in
/// `crates/chronos-services/src/counterexample.rs::tests`.
///
/// All fields are plain string / Option types to avoid cross-crate type
/// sharing. Kind/scope/comparison are stringified to keep chronos-store
/// independent of chronos-services' enum types (same precedent as
/// `CounterexampleBundleSummary.property_kind`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HypothesisInputWire {
    pub session_id: String,
    /// "invariant" | "existence" | "call_path".
    pub kind: String,
    /// "event_count" | "property_value" | "latency_ms" | None.
    pub scope: Option<String>,
    /// "Eq" | "Ne" | "Ge" | "Gt" | "Le" | "Lt" | None.
    pub comparison: Option<String>,
    pub constant: Option<PropertyValue>,
    pub property_target: Option<String>,
    pub predicate: Option<ExistencePredicateWire>,
    pub caller: Option<String>,
    pub callee: Option<String>,
    /// HypothesisInput::max_depth is Option<usize>; we persist as Option<u64>
    /// for forward compatibility (future widening to u128). Cast is
    /// saturating in `hypothesis_input_from_wire` (bounded by usize::MAX).
    pub max_depth: Option<u64>,
}

/// Filter shape for `SessionStore::list_counterexample_bundles`.
///
/// All fields are optional; passing `None` for everything returns the
/// most recent bundles up to `limit`.
///
/// m8-05 (B2): `cursor` carries the `bundle_id` returned by the
/// previous page's `next_cursor`. When `Some`, the list skips rows
/// whose `bundle_id <= cursor`, effectively returning the page that
/// begins AFTER the cursor. The cursor is opaque to callers — we use
/// the bundle_id directly (uuid::v7 is monotonically increasing, so
/// lexicographic `>` gives chronological forward paging).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CounterexampleBundleFilter<'a> {
    pub workspace_id: Option<&'a str>,
    pub property_kind: Option<&'a str>,
    pub since_ms: Option<u64>,
    pub until_ms: Option<u64>,
    pub limit: u32,
    /// m8-05: opaque pagination cursor (= last `bundle_id` from previous
    /// page). When `Some(c)`, the list starts at the first row whose
    /// `bundle_id > c`. `None` means first page.
    pub cursor: Option<String>,
}

/// Summary shape used for list responses (no events).
///
/// Mirrors `chronos_services::counterexample::CounterexampleBundleSummary`
/// but as a free-standing struct so chronos-store stays independent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CounterexampleBundleSummary {
    pub bundle_id: String,
    /// String form ("invariant" | "existence" | "call_path") — avoids
    /// forcing chronos-store to depend on chronos_services::output::HypothesisKind.
    pub property_kind: String,
    pub workspace_id: String,
    pub created_at_ms: u64,
    pub rounds_used: u32,
    pub has_full_bundle: bool,
    /// m9-01: monotonic version of the bundle envelope. Defaults to 1
    /// for bundles persisted before m9-01 (serde `#[serde(default)]`).
    /// Loader rejects bundles with `schema_version > CURRENT_BUNDLE_SCHEMA_VERSION`.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    /// m9-02: total event count at save time. Allows O(1) reads of the
    /// bundle size without touching the side table. Defaults to 0 for
    /// pre-m9-02 bundles (serde `#[serde(default)]`).
    #[serde(default)]
    pub events_count: u64,
}

/// Full record stored as the redb value.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CounterexampleBundleRecord {
    pub summary: CounterexampleBundleSummary,
    pub events: Vec<TraceEvent>,
    pub minimised: Option<MinimisedPayload>,
    /// Snapshot of CAS hashes backing the events. Not strictly required
    /// for read (we read `events` directly), but useful for future dedup.
    #[serde(default)]
    pub event_cas_hashes: Vec<ContentHash>,
    /// m8-07: original `HypothesisInput` the user passed to
    /// `counterexample_shrink`. None for bundles persisted before m8-07
    /// (serde's `#[serde(default)]` makes legacy loads a clean
    /// `target_hypothesis: None`).
    ///
    /// When `Some`, `chronos test replay` uses this verbatim to
    /// reconstruct the user's original hypothesis for replay, instead of
    /// the m8-04 synthetic-default reconstruction (which loses
    /// scope/comparison/property_target for Invariant targets).
    #[serde(default)]
    pub target_hypothesis: Option<HypothesisInputWire>,
    /// m9-01: envelope-level version. Always equals `summary.schema_version`
    /// for records written by this build; the loader reads `record.schema_version`
    /// (the envelope) as authoritative.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
}

impl crate::storage::SessionStore {
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
        record: CounterexampleBundleRecord,
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
    /// Re-save deletes any pre-existing chunks for `record.summary.bundle_id`
    /// before writing new ones (R7).
    #[allow(clippy::result_large_err)]
    fn save_bundle_record_and_events(
        &self,
        record: CounterexampleBundleRecord,
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

        // m9-02 R7: delete prior chunks for this bundle before writing new ones.
        // We collect existing keys first (requires a read), then delete in the
        // same write transaction.
        let prior_keys: Vec<Vec<u8>> = {
            let read_tx = self
                .db()
                .begin_read()
                .map_err(|e| StoreError::Database(e.into()))?;
            collect_bundle_chunks(&read_tx, &bundle_id)?
                .into_iter()
                .map(|(idx, _)| encode_chunk_key(&bundle_id, idx))
                .collect()
        };

        {
            let mut events_table = tx
                .open_table(COUNTEREXAMPLE_BUNDLE_EVENTS)
                .map_err(|e| StoreError::Database(e.into()))?;
            // Delete prior chunks.
            for key in &prior_keys {
                events_table
                    .remove(key.as_slice())
                    .map_err(|e| StoreError::Database(e.into()))?;
            }
            // Write new chunks.
            for (chunk_idx, chunk) in events.chunks(BUNDLE_EVENTS_CHUNK_SIZE).enumerate() {
                let key = encode_chunk_key(&bundle_id, chunk_idx as u32);
                let value = bincode::serialize(chunk)
                    .map_err(|e| StoreError::Serialization(e.to_string()))?;
                events_table
                    .insert(key.as_slice(), value.as_slice())
                    .map_err(|e| StoreError::Database(e.into()))?;
            }
        }

        tx.commit().map_err(|e| StoreError::Database(e.into()))?;
        Ok(bundle_id)
    }

    /// Load all events for a bundle from the side table, sorted by chunk index.
    ///
    /// Returns `Ok(vec![])` when no chunks exist (the table is absent or
    /// the bundle has no side-table events).
    #[allow(clippy::result_large_err)]
    pub fn load_counterexample_bundle_events(
        &self,
        bundle_id: &str,
    ) -> Result<Vec<TraceEvent>, StoreError> {
        let tx = self
            .db()
            .begin_read()
            .map_err(|e| StoreError::Database(e.into()))?;

        let chunks_data = collect_bundle_chunks(&tx, bundle_id)?;
        let mut chunks: Vec<(u32, Vec<TraceEvent>)> = Vec::new();
        for (idx, bytes) in chunks_data {
            #[allow(clippy::needless_borrow)]
            let chunk: Vec<TraceEvent> = bincode::deserialize(&bytes)
                .map_err(|e| StoreError::Serialization(e.to_string()))?;
            chunks.push((idx, chunk));
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
    /// Returns 0 when the side table does not exist or the bundle has no chunks.
    #[allow(clippy::result_large_err)]
    pub fn count_counterexample_bundle_events(&self, bundle_id: &str) -> Result<u64, StoreError> {
        let tx = self
            .db()
            .begin_read()
            .map_err(|e| StoreError::Database(e.into()))?;

        let chunks_data = collect_bundle_chunks(&tx, bundle_id)?;
        let mut total: u64 = 0;
        for (_idx, bytes) in chunks_data {
            #[allow(clippy::needless_borrow)]
            let chunk: Vec<TraceEvent> = bincode::deserialize(&bytes)
                .map_err(|e| StoreError::Serialization(e.to_string()))?;
            total += chunk.len() as u64;
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
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(e) => return Err(StoreError::Database(e.into())),
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
                    return Err(StoreError::Serialization(format!(
                        "bundle schema_version {} is newer than supported {}; \
                         upgrade chronos-store to read this bundle",
                        record.schema_version, CURRENT_BUNDLE_SCHEMA_VERSION,
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
            Err(TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
            Err(e) => return Err(StoreError::Database(e.into())),
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

/// Load events for a bundle, handling both legacy (blob-embedded) and
/// post-m9-02 (side-table) layouts.
///
/// This is the single chokepoint (m9-02 D5) that consumers call:
/// - When `bundle.events` is non-empty (pre-m9-02 blob), returns a clone.
/// - When `bundle.events` is empty (post-m9-02), loads chunks from the
///   side table and concatenates them in `chunk_index` order.
///
/// Consumers should use this instead of accessing `bundle.events` directly.
#[allow(clippy::result_large_err)]
pub fn bundle_events_or_legacy(
    store: &crate::storage::SessionStore,
    bundle: &CounterexampleBundleRecord,
) -> Result<Vec<TraceEvent>, StoreError> {
    if !bundle.events.is_empty() {
        // Legacy: events are embedded in the blob (pre-m9-02).
        return Ok(bundle.events.clone());
    }
    // Post-m9-02: load from the side table.
    store.load_counterexample_bundle_events(&bundle.summary.bundle_id)
}

/// Count events for `bundle` using the D2 policy: prefer `summary.events_count`
/// (O(1)) when populated, fall back to the blob-embedded event count only for
/// pre-m9-02 bundles that have events in the blob.
///
/// This is the count analogue of [`bundle_events_or_legacy`] (D5 loading
/// chokepoint). Both call sites in `chronos-services` previously duplicated
/// the fallback logic independently.
pub fn bundle_events_count_or_legacy(bundle: &CounterexampleBundleRecord) -> u64 {
    if bundle.summary.events_count > 0 {
        bundle.summary.events_count
    } else {
        bundle.events.len() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_store() -> crate::storage::SessionStore {
        crate::storage::SessionStore::in_memory().expect("in_memory store")
    }

    #[test]
    fn save_then_load_roundtrip() {
        let store = make_store();
        let rec = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: "b1".into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 1000,
                rounds_used: 4,
                has_full_bundle: true,
                schema_version: 1,
                events_count: 0,
            },
            events: vec![],
            minimised: Some(MinimisedPayload::Constant(PropertyValue::Number(2.0))),
            event_cas_hashes: vec![],
            target_hypothesis: None,
            schema_version: 1,
        };
        store.save_counterexample_bundle(rec.clone()).unwrap();
        let loaded = store.load_counterexample_bundle("b1").unwrap().unwrap();
        assert_eq!(loaded.summary.bundle_id, "b1");
        assert_eq!(loaded.summary.rounds_used, 4);
        assert!(matches!(
            loaded.minimised,
            Some(MinimisedPayload::Constant(_))
        ));
    }

    #[test]
    fn load_unknown_returns_none() {
        let store = make_store();
        let r = store.load_counterexample_bundle("nonexistent").unwrap();
        assert_eq!(r, None);
    }

    #[test]
    fn list_filters_by_property_kind() {
        let store = make_store();
        for (id, kind) in [
            ("b-inv", "invariant"),
            ("b-ext", "existence"),
            ("b-cp", "call_path"),
        ] {
            store
                .save_counterexample_bundle(CounterexampleBundleRecord {
                    summary: CounterexampleBundleSummary {
                        bundle_id: id.into(),
                        property_kind: kind.into(),
                        workspace_id: "ws".into(),
                        created_at_ms: 100,
                        rounds_used: 1,
                        has_full_bundle: true,
                        schema_version: 1,
                        events_count: 0,
                    },
                    events: vec![],
                    minimised: None,
                    event_cas_hashes: vec![],
                    target_hypothesis: None,
                    schema_version: 1,
                })
                .unwrap();
        }
        let filter = CounterexampleBundleFilter {
            workspace_id: None,
            property_kind: Some("invariant"),
            since_ms: None,
            until_ms: None,
            limit: 100,
            cursor: None,
        };
        let summaries = store.list_counterexample_bundles(filter).unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].bundle_id, "b-inv");
    }

    #[test]
    fn list_with_limit_truncates() {
        let store = make_store();
        for i in 0..10 {
            store
                .save_counterexample_bundle(CounterexampleBundleRecord {
                    summary: CounterexampleBundleSummary {
                        bundle_id: format!("b-{:02}", i),
                        property_kind: "invariant".into(),
                        workspace_id: "ws".into(),
                        created_at_ms: i,
                        rounds_used: 1,
                        has_full_bundle: true,
                        schema_version: 1,
                        events_count: 0,
                    },
                    events: vec![],
                    minimised: None,
                    event_cas_hashes: vec![],
                    target_hypothesis: None,
                    schema_version: 1,
                })
                .unwrap();
        }
        let filter = CounterexampleBundleFilter {
            workspace_id: None,
            property_kind: None,
            since_ms: None,
            until_ms: None,
            limit: 3,
            cursor: None,
        };
        let summaries = store.list_counterexample_bundles(filter).unwrap();
        assert_eq!(summaries.len(), 3);
    }

    // m8-05 (B2): cursor skips rows <= cursor's bundle_id, returning
    // the page that begins AFTER the cursor.
    #[test]
    fn list_cursor_paginates_forward() {
        let store = make_store();
        for i in 0..6 {
            store
                .save_counterexample_bundle(CounterexampleBundleRecord {
                    summary: CounterexampleBundleSummary {
                        bundle_id: format!("b-{:02}", i),
                        property_kind: "invariant".into(),
                        workspace_id: "ws".into(),
                        created_at_ms: i as u64,
                        rounds_used: 1,
                        has_full_bundle: true,
                        schema_version: 1,
                        events_count: 0,
                    },
                    events: vec![],
                    minimised: None,
                    event_cas_hashes: vec![],
                    target_hypothesis: None,
                    schema_version: 1,
                })
                .unwrap();
        }
        // Page 1: limit=2, no cursor -> ["b-00", "b-01"].
        let page1 = store
            .list_counterexample_bundles(CounterexampleBundleFilter {
                workspace_id: None,
                property_kind: None,
                since_ms: None,
                until_ms: None,
                limit: 2,
                cursor: None,
            })
            .unwrap();
        assert_eq!(page1.len(), 2);
        assert_eq!(page1[0].bundle_id, "b-00");
        assert_eq!(page1[1].bundle_id, "b-01");
        // Page 2: cursor = "b-01" -> ["b-02", "b-03"].
        let page2 = store
            .list_counterexample_bundles(CounterexampleBundleFilter {
                workspace_id: None,
                property_kind: None,
                since_ms: None,
                until_ms: None,
                limit: 2,
                cursor: Some("b-01".into()),
            })
            .unwrap();
        assert_eq!(page2.len(), 2);
        assert_eq!(page2[0].bundle_id, "b-02");
        assert_eq!(page2[1].bundle_id, "b-03");
        // Page 3: cursor = "b-03" -> ["b-04", "b-05"].
        let page3 = store
            .list_counterexample_bundles(CounterexampleBundleFilter {
                workspace_id: None,
                property_kind: None,
                since_ms: None,
                until_ms: None,
                limit: 2,
                cursor: Some("b-03".into()),
            })
            .unwrap();
        assert_eq!(page3.len(), 2);
        assert_eq!(page3[0].bundle_id, "b-04");
        assert_eq!(page3[1].bundle_id, "b-05");
        // Page 4: cursor = "b-05" -> [].
        let page4 = store
            .list_counterexample_bundles(CounterexampleBundleFilter {
                workspace_id: None,
                property_kind: None,
                since_ms: None,
                until_ms: None,
                limit: 2,
                cursor: Some("b-05".into()),
            })
            .unwrap();
        assert!(page4.is_empty(), "page past last bundle is empty");
    }

    // m8-05 (B2): cursor is opaque to callers; passing a non-existent
    // bundle_id as cursor yields a deterministic forward page (all
    // matching rows have id > cursor).
    #[test]
    fn list_cursor_unknown_id_returns_all() {
        let store = make_store();
        for i in 0..3 {
            store
                .save_counterexample_bundle(CounterexampleBundleRecord {
                    summary: CounterexampleBundleSummary {
                        bundle_id: format!("z-{:02}", i),
                        property_kind: "invariant".into(),
                        workspace_id: "ws".into(),
                        created_at_ms: i as u64,
                        rounds_used: 1,
                        has_full_bundle: true,
                        schema_version: 1,
                        events_count: 0,
                    },
                    events: vec![],
                    minimised: None,
                    event_cas_hashes: vec![],
                    target_hypothesis: None,
                    schema_version: 1,
                })
                .unwrap();
        }
        let summaries = store
            .list_counterexample_bundles(CounterexampleBundleFilter {
                workspace_id: None,
                property_kind: None,
                since_ms: None,
                until_ms: None,
                limit: 100,
                // Lexicographically less than every "z-..." row, so
                // all rows pass.
                cursor: Some("a-00".into()),
            })
            .unwrap();
        assert_eq!(summaries.len(), 3);
    }

    #[test]
    fn save_rejects_empty_bundle_id() {
        let store = make_store();
        let rec = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: "".into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 0,
                rounds_used: 0,
                has_full_bundle: false,
                schema_version: 1,
                events_count: 0,
            },
            events: vec![],
            minimised: None,
            event_cas_hashes: vec![],
            target_hypothesis: None,
            schema_version: 1,
        };
        let r = store.save_counterexample_bundle(rec);
        assert!(r.is_err(), "empty bundle_id must be rejected");
    }

    // m8-07 §5 (per-crate integration): save a bundle with a non-None
    // target_hypothesis, load it back, and assert the wire mirror survived
    // the bincode round-trip intact. This pins the D2 serde contract:
    // #[serde(default)] on the field means pre-m8-07 bundles (which have
    // no target_hypothesis on disk) deserialize cleanly as None.
    #[test]
    fn m8_07_save_then_load_preserves_target_hypothesis() {
        use crate::counterexample_storage::HypothesisInputWire;
        let store = make_store();
        let wire = HypothesisInputWire {
            session_id: "sess-m8-07-test".into(),
            kind: "invariant".into(),
            scope: Some("event_count".into()),
            comparison: Some("Ge".into()),
            constant: Some(chronos_domain::property::PropertyValue::Number(1000.0)),
            property_target: None,
            predicate: None,
            caller: None,
            callee: None,
            max_depth: None,
        };
        let rec = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: "b-m8-07".into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 0,
                rounds_used: 4,
                has_full_bundle: true,
                schema_version: 1,
                events_count: 0,
            },
            events: vec![],
            minimised: Some(MinimisedPayload::Constant(
                chronos_domain::property::PropertyValue::Number(0.0),
            )),
            event_cas_hashes: vec![],
            target_hypothesis: Some(wire),
            schema_version: 1,
        };
        store.save_counterexample_bundle(rec.clone()).unwrap();
        let loaded = store
            .load_counterexample_bundle("b-m8-07")
            .unwrap()
            .unwrap();

        // The wire mirror survived the bincode round-trip.
        let th = loaded
            .target_hypothesis
            .expect("target_hypothesis must be present");
        assert_eq!(th.session_id, "sess-m8-07-test");
        assert_eq!(th.kind, "invariant");
        assert_eq!(th.scope.as_deref(), Some("event_count"));
        assert_eq!(th.comparison.as_deref(), Some("Ge"));
        assert!(th.constant.is_some());
    }

    // m8-07 §5 (backward compatibility): a bundle record without target_hypothesis
    // on disk must deserialize as target_hypothesis=None (serde #[serde(default)]).
    // We simulate this by constructing a record with target_hypothesis=None
    // and verifying it round-trips correctly.
    #[test]
    fn m8_07_pre_m8_07_bundle_has_no_target_hypothesis() {
        let store = make_store();
        let rec = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: "b-pre-m8-07".into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 0,
                rounds_used: 1,
                has_full_bundle: true,
                schema_version: 1,
                events_count: 0,
            },
            events: vec![],
            minimised: Some(MinimisedPayload::Constant(
                chronos_domain::property::PropertyValue::Number(0.0),
            )),
            event_cas_hashes: vec![],
            target_hypothesis: None,
            schema_version: 1,
        };
        store.save_counterexample_bundle(rec.clone()).unwrap();
        let loaded = store
            .load_counterexample_bundle("b-pre-m8-07")
            .unwrap()
            .unwrap();
        assert!(
            loaded.target_hypothesis.is_none(),
            "pre-m8-07 bundles must have target_hypothesis=None"
        );
    }

    // ========================================================================
    // m9-01 tests: schema_version on CounterexampleBundleRecord / Summary
    // ========================================================================

    // m9-02 §5: save a bundle via in-memory store, load it, assert both
    // summary.schema_version == CURRENT and record.schema_version == CURRENT.
    #[test]
    fn m9_02_save_writes_current_schema_version() {
        let store = make_store();
        let rec = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: "b-schema-test".into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 0,
                rounds_used: 1,
                has_full_bundle: true,
                schema_version: 1,
                events_count: 0,
            },
            events: vec![],
            minimised: None,
            event_cas_hashes: vec![],
            target_hypothesis: None,
            schema_version: 1,
        };
        store.save_counterexample_bundle(rec).unwrap();
        let loaded = store
            .load_counterexample_bundle("b-schema-test")
            .unwrap()
            .unwrap();
        assert_eq!(
            loaded.schema_version, CURRENT_BUNDLE_SCHEMA_VERSION,
            "record.schema_version must equal CURRENT after save"
        );
        assert_eq!(
            loaded.summary.schema_version, CURRENT_BUNDLE_SCHEMA_VERSION,
            "summary.schema_version must equal CURRENT after save"
        );
    }

    // m9-02 §5: a JSON payload without schema_version must deserialize to
    // CURRENT via #[serde(default = "default_schema_version")].
    #[test]
    fn m9_02_legacy_bundle_deserializes_with_default_schema_version() {
        // Verify the default function returns CURRENT.
        assert_eq!(default_schema_version(), CURRENT_BUNDLE_SCHEMA_VERSION);
        // Verify serde_json correctly assigns the default when the field is absent.
        let json_no_version = r#"{
            "bundle_id": "b-legacy",
            "property_kind": "invariant",
            "workspace_id": "ws",
            "created_at_ms": 0,
            "rounds_used": 1,
            "has_full_bundle": true
        }"#;
        let summary: CounterexampleBundleSummary =
            serde_json::from_str(json_no_version).expect("serde_json must accept pre-m9-01 JSON");
        assert_eq!(
            summary.schema_version, CURRENT_BUNDLE_SCHEMA_VERSION,
            "JSON without schema_version must default to CURRENT"
        );
    }

    // m9-02 §5: a bincode blob with schema_version=3 on the record must be
    // rejected by load_counterexample_bundle with an error mentioning "newer
    // than supported".
    #[test]
    fn m9_02_future_version_load_is_rejected() {
        let store = make_store();

        // Inject a future-versioned record directly into the DB (bypassing
        // save_counterexample_bundle so we control the exact bytes).
        let future_record: CounterexampleBundleRecord = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: "b-future".into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 0,
                rounds_used: 1,
                has_full_bundle: true,
                schema_version: 3, // future version (CURRENT is 2)
                events_count: 0,
            },
            events: vec![],
            minimised: None,
            event_cas_hashes: vec![],
            target_hypothesis: None,
            schema_version: 3, // future version (CURRENT is 2)
        };
        let future_bytes = bincode::serialize(&future_record).unwrap();
        let tx = store.db().begin_write().unwrap();
        {
            let mut table = tx.open_table(COUNTEREXAMPLE_BUNDLES).unwrap();
            table
                .insert(&b"b-future"[..], future_bytes.as_slice())
                .unwrap();
        }
        tx.commit().unwrap();

        // Load it — must be rejected.
        let result = store.load_counterexample_bundle("b-future");
        let err = result.expect_err("future-versioned bundle must be rejected");
        let err_msg = format!("{err}");
        assert!(
            err_msg.contains("newer than supported"),
            "error message must mention 'newer than supported', got: {err_msg}"
        );
        assert!(
            err_msg.contains('3'),
            "error message must mention schema_version 3, got: {err_msg}"
        );
    }

    // m9-02 §5 D5: construct a record with schema_version=99, save it,
    // reload it, assert the persisted version is CURRENT (save overwrites caller's value).
    #[test]
    fn m9_02_save_overwrites_callers_schema_version() {
        let store = make_store();
        let rec = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: "b-overwrite".into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 0,
                rounds_used: 1,
                has_full_bundle: true,
                schema_version: 99, // caller sets stale value
                events_count: 0,
            },
            events: vec![],
            minimised: None,
            event_cas_hashes: vec![],
            target_hypothesis: None,
            schema_version: 99, // caller sets stale value
        };
        store.save_counterexample_bundle(rec).unwrap();
        let loaded = store
            .load_counterexample_bundle("b-overwrite")
            .unwrap()
            .unwrap();
        assert_eq!(
            loaded.schema_version, CURRENT_BUNDLE_SCHEMA_VERSION,
            "save must overwrite caller's schema_version with CURRENT"
        );
        assert_eq!(
            loaded.summary.schema_version, CURRENT_BUNDLE_SCHEMA_VERSION,
            "save must overwrite caller's summary.schema_version with CURRENT"
        );
    }

    // m9-02 §5: list best-effort skips rows where bincode deserialization fails.
    // A future-versioned row with schema_version=3 deserializes fine in bincode
    // (bincode doesn't know about our version check), so it IS included in the
    // list. The explicit rejection only happens in load_counterexample_bundle.
    // This test pins that: both bundles appear in list, but load() rejects the
    // future-versioned row individually.
    #[test]
    fn m9_02_list_includes_future_versioned_row_best_effort() {
        let store = make_store();

        // Normal bundle: save via the API (writes CURRENT schema version).
        let normal_rec = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: "b-normal".into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 100,
                rounds_used: 1,
                has_full_bundle: true,
                schema_version: 1,
                events_count: 0,
            },
            events: vec![],
            minimised: None,
            event_cas_hashes: vec![],
            target_hypothesis: None,
            schema_version: 1,
        };
        store.save_counterexample_bundle(normal_rec).unwrap();

        // Future-versioned bundle: inject directly into the DB (schema_version=3).
        let future_record: CounterexampleBundleRecord = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: "b-future".into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 200,
                rounds_used: 1,
                has_full_bundle: true,
                schema_version: 3,
                events_count: 0,
            },
            events: vec![],
            minimised: None,
            event_cas_hashes: vec![],
            target_hypothesis: None,
            schema_version: 3,
        };
        let future_bytes = bincode::serialize(&future_record).unwrap();
        let tx = store.db().begin_write().unwrap();
        {
            let mut table = tx.open_table(COUNTEREXAMPLE_BUNDLES).unwrap();
            table
                .insert(&b"b-future"[..], future_bytes.as_slice())
                .unwrap();
        }
        tx.commit().unwrap();

        // List all bundles: both appear (bincode deserializes schema_version=3 fine).
        let summaries = store
            .list_counterexample_bundles(CounterexampleBundleFilter {
                workspace_id: None,
                property_kind: None,
                since_ms: None,
                until_ms: None,
                limit: 100,
                cursor: None,
            })
            .unwrap();

        assert_eq!(
            summaries.len(),
            2,
            "both normal and future-versioned bundles must appear in list \
             (best-effort; bincode deserializes fine)"
        );

        // load_counterexample_bundle rejects the future-versioned row individually.
        let load_result = store.load_counterexample_bundle("b-future");
        let err = load_result.expect_err("future-versioned bundle must be rejected on load");
        let err_msg = format!("{err}");
        assert!(
            err_msg.contains("newer than supported"),
            "load error must mention 'newer than supported', got: {err_msg}"
        );
    }

    // ========================================================================
    // m9-02 tests: side table (counterexample_bundle_events)
    // ========================================================================

    /// Helper: make a simple TraceEvent for test fixtures.
    fn make_event(id: u64, func: &str) -> chronos_domain::TraceEvent {
        use chronos_domain::{EventData, EventType, SourceLocation};
        chronos_domain::TraceEvent::new(
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

    // 4.1: save writes events to side table, not blob.
    #[test]
    fn m9_02_save_writes_events_to_side_table_not_blob() {
        let store = make_store();
        let events: Vec<_> = (0..10).map(|i| make_event(i, "test")).collect();
        let rec = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: "b-side".into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 0,
                rounds_used: 1,
                has_full_bundle: true,
                schema_version: 1,
                events_count: 0,
            },
            events: events.clone(),
            minimised: None,
            event_cas_hashes: vec![],
            target_hypothesis: None,
            schema_version: 1,
        };
        store.save_counterexample_bundle(rec).unwrap();

        // Blob record has empty events.
        let loaded = store.load_counterexample_bundle("b-side").unwrap().unwrap();
        assert!(
            loaded.events.is_empty(),
            "blob record must have empty events after save"
        );
        assert_eq!(
            loaded.summary.events_count, 10,
            "summary.events_count must be 10"
        );
        assert_eq!(
            loaded.summary.schema_version, CURRENT_BUNDLE_SCHEMA_VERSION,
            "schema_version must be bumped to CURRENT"
        );

        // Side table has the events.
        let side_events = store.load_counterexample_bundle_events("b-side").unwrap();
        assert_eq!(side_events.len(), 10, "side table must have all 10 events");
        assert_eq!(side_events[0].event_id, 0);
        assert_eq!(side_events[9].event_id, 9);
    }

    // 4.1: load concatenates chunks in ascending chunk_index order.
    #[test]
    fn m9_02_load_events_concatenates_chunks_in_order() {
        let store = make_store();

        // Inject 3 chunks directly at indices 0, 2, 1 (out of order).
        // Each chunk has exactly 1 event so we can distinguish them.
        let inject = |chunk_idx: u32, id: u64, func: &str| {
            let key = encode_chunk_key("b-unordered", chunk_idx);
            let chunk = vec![make_event(id, func)];
            let value = bincode::serialize(&chunk).unwrap();
            let tx = store.db().begin_write().unwrap();
            {
                let mut table = tx.open_table(COUNTEREXAMPLE_BUNDLE_EVENTS).unwrap();
                table.insert(key.as_slice(), value.as_slice()).unwrap();
            }
            tx.commit().unwrap();
        };

        inject(0, 100, "first");
        inject(2, 300, "third");
        inject(1, 200, "second");

        // load_counterexample_bundle_events should return them in 0,1,2 order.
        let events = store
            .load_counterexample_bundle_events("b-unordered")
            .unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].event_id, 100, "chunk 0 first");
        assert_eq!(events[1].event_id, 200, "chunk 1 second");
        assert_eq!(events[2].event_id, 300, "chunk 2 third");
    }

    // 4.1: re-save overwrites prior chunks.
    #[test]
    fn m9_02_save_events_idempotent_overwrites_prior_chunks() {
        let store = make_store();
        let rec = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: "b-overwrite".into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 0,
                rounds_used: 1,
                has_full_bundle: true,
                schema_version: 1,
                events_count: 0,
            },
            events: (0..100).map(|i| make_event(i, "original")).collect(),
            minimised: None,
            event_cas_hashes: vec![],
            target_hypothesis: None,
            schema_version: 1,
        };
        store.save_counterexample_bundle(rec).unwrap();
        assert_eq!(
            store
                .load_counterexample_bundle_events("b-overwrite")
                .unwrap()
                .len(),
            100,
            "initial save must write 100 events"
        );

        // Re-save with fewer events.
        let rec2 = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: "b-overwrite".into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 1,
                rounds_used: 2,
                has_full_bundle: true,
                schema_version: 1,
                events_count: 0,
            },
            events: (0..50).map(|i| make_event(i + 200, "replaced")).collect(),
            minimised: None,
            event_cas_hashes: vec![],
            target_hypothesis: None,
            schema_version: 1,
        };
        store.save_counterexample_bundle(rec2).unwrap();
        let events = store
            .load_counterexample_bundle_events("b-overwrite")
            .unwrap();
        assert_eq!(events.len(), 50, "re-save must overwrite with 50 events");
        assert_eq!(
            events[0].event_id, 200,
            "events must be from the re-saved bundle"
        );
        assert_eq!(events[49].event_id, 249, "last event must be id 249");
    }

    // 4.1: partial last chunk count is correct.
    #[test]
    fn m9_02_count_events_handles_partial_last_chunk() {
        let store = make_store();
        // Write 2 full chunks (256 each) + 1 partial chunk (100).
        let events: Vec<_> = (0..612u64).map(|i| make_event(i, "partial")).collect();
        let rec = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: "b-partial".into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 0,
                rounds_used: 1,
                has_full_bundle: true,
                schema_version: 1,
                events_count: 0,
            },
            events,
            minimised: None,
            event_cas_hashes: vec![],
            target_hypothesis: None,
            schema_version: 1,
        };
        store.save_counterexample_bundle(rec).unwrap();

        // Count via summary (O(1) path).
        let loaded = store
            .load_counterexample_bundle("b-partial")
            .unwrap()
            .unwrap();
        assert_eq!(
            loaded.summary.events_count, 612,
            "summary.events_count must be 612"
        );

        // Count via count_counterexample_bundle_events.
        let count = store
            .count_counterexample_bundle_events("b-partial")
            .unwrap();
        assert_eq!(count, 612, "count must return 612 from 3 chunks");

        // Load and verify total.
        let events = store
            .load_counterexample_bundle_events("b-partial")
            .unwrap();
        assert_eq!(events.len(), 612);
        assert_eq!(events[0].event_id, 0);
        assert_eq!(events[611].event_id, 611);
    }

    // 4.2: legacy bundle with events in blob loads normally via chokepoint.
    #[test]
    fn m9_02_legacy_bundle_with_events_in_blob_loads_normally() {
        let store = make_store();
        // Build a "legacy" record: events in blob, schema_version=1.
        let legacy_record = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: "b-legacy".into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 0,
                rounds_used: 1,
                has_full_bundle: true,
                schema_version: 1,
                events_count: 0, // serde default
            },
            events: vec![
                make_event(1, "legacy-e1"),
                make_event(2, "legacy-e2"),
                make_event(3, "legacy-e3"),
            ],
            minimised: None,
            event_cas_hashes: vec![],
            target_hypothesis: None,
            schema_version: 1,
        };
        // Inject directly (simulating pre-m9-02 on-disk format).
        let bytes = bincode::serialize(&legacy_record).unwrap();
        let tx = store.db().begin_write().unwrap();
        {
            let mut table = tx.open_table(COUNTEREXAMPLE_BUNDLES).unwrap();
            table
                .insert(b"b-legacy".as_slice(), bytes.as_slice())
                .unwrap();
        }
        tx.commit().unwrap();

        // Load via chokepoint — must return blob events.
        let loaded = store
            .load_counterexample_bundle("b-legacy")
            .unwrap()
            .unwrap();
        let events = bundle_events_or_legacy(&store, &loaded).unwrap();
        assert_eq!(
            events.len(),
            3,
            "chokepoint must return 3 legacy blob events"
        );
        assert_eq!(events[0].event_id, 1);
        assert_eq!(events[2].event_id, 3);
    }

    // 4.2: post-m9-02 bundle loads events from side table via chokepoint.
    #[test]
    fn m9_02_post_m9_02_bundle_loads_events_from_side_table() {
        let store = make_store();
        let events: Vec<_> = (0..5).map(|i| make_event(i, "post")).collect();
        let rec = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: "b-post".into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 0,
                rounds_used: 1,
                has_full_bundle: true,
                schema_version: 1,
                events_count: 0,
            },
            events,
            minimised: None,
            event_cas_hashes: vec![],
            target_hypothesis: None,
            schema_version: 1,
        };
        store.save_counterexample_bundle(rec).unwrap();

        let loaded = store.load_counterexample_bundle("b-post").unwrap().unwrap();
        assert!(
            loaded.events.is_empty(),
            "blob events must be empty post-m9-02 save"
        );
        let events = bundle_events_or_legacy(&store, &loaded).unwrap();
        assert_eq!(
            events.len(),
            5,
            "chokepoint must return 5 side-table events"
        );
    }

    // 4.2: summary.events_count defaults to 0 for legacy bundles.
    #[test]
    fn m9_02_summary_events_count_default_zero_for_legacy() {
        // Verify serde default.
        let json_legacy = r#"{
            "bundle_id": "b-legacy-json",
            "property_kind": "invariant",
            "workspace_id": "ws",
            "created_at_ms": 0,
            "rounds_used": 1,
            "has_full_bundle": true
        }"#;
        let summary: CounterexampleBundleSummary =
            serde_json::from_str(json_legacy).expect("must deserialize");
        assert_eq!(
            summary.events_count, 0,
            "events_count must default to 0 for pre-m9-02 JSON"
        );
    }

    // 4.2: loading unknown bundle returns empty events vec.
    #[test]
    fn m9_02_load_unknown_bundle_returns_empty_events_vec() {
        let store = make_store();
        let events = store
            .load_counterexample_bundle_events("nonexistent-bundle")
            .unwrap();
        assert!(
            events.is_empty(),
            "unknown bundle must return empty vec, not an error"
        );
    }
}
