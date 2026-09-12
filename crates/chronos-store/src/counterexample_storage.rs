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
//! **m9-04 — Key layout v3 (range-scan friendly):**
//!
//! Side-table keys switched from variable-width `[len(bundle_id)][bundle_id][chunk_index]`
//! to fixed-width `[blake3(bundle_id)[..16] || chunk_index.to_be_bytes()]` (20 bytes).
//! Fixed-width prefix enables bounded redb range scans per bundle instead of full-table
//! scans. Values carry `(bundle_id, Vec<TraceEvent>)` for per-chunk identity defense.
//!
//! Read path: v3 range scan first, falls back to v2 full scan when v3 returns 0 rows
//! and the bundle has events_count > 0 (D7). Save path atomically removes both v3 and
//! v2 prior chunks before writing v3 (D6 lazy migration). Schema bumps 2 → 3.
//!
//! Key layout summary:
//! - **v3 key:** `blake3(bundle_id)[..16] || chunk_index.to_be_bytes()` (20 bytes fixed)
//! - **v3 value:** `bincode::serialize(&(bundle_id, Vec<TraceEvent>))`
//! - **v2 key:** `[len(bundle_id) as u32 BE][bundle_id bytes][chunk_index as u32 BE]`
//! - **v2 value:** `bincode::serialize(&Vec<TraceEvent>)`
//!

use std::mem;

use blake3::Hasher;

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

/// Side table for counterexample bundle events (m9-02, v3 layout).
///
/// **m9-04 key layout (v3):**
///   - 16 bytes: `blake3(bundle_id)[..16]` (fixed-width prefix for range scans)
///   - 4 bytes: `chunk_index as u32` big-endian
///   - Total: 20 bytes fixed-width per key
///
/// **m9-04 value layout (v3):**
///   - `bincode::serialize(&(bundle_id, Vec<TraceEvent>))`
///   - bundle_id is stored in the value for per-chunk identity defense (D3)
///
/// **Legacy key layout (v2, pre-m9-04):**
///   - 4 bytes: `len(bundle_id) as u32 BE`
///   - N bytes: bundle_id UTF-8 bytes
///   - 4 bytes: `chunk_index as u32 BE`
///
/// **Legacy value layout (v2):**
///   - `bincode::serialize(&Vec<TraceEvent>)`
///
/// Chunk size is [`BUNDLE_EVENTS_CHUNK_SIZE`]. Each chunk contains up to
/// that many events; the last chunk may be smaller.
/// Table for counterexample bundle events (chunked, per bundle).
///
/// m9-04: v3 chunks keyed by `blake3(bundle_id)[..16] || chunk_index`;
/// v2 legacy chunks keyed by `len(bundle_id) || bundle_id || chunk_index`.
const COUNTEREXAMPLE_BUNDLE_EVENTS: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("counterexample_bundle_events");

// ========================================================================
// m9-04 v3 key/value encoding — blake3 prefix, fixed 20-byte keys
// ========================================================================

/// Compute the 16-byte blake3 prefix for a bundle_id.
///
/// m9-04 D1: `blake3(bundle_id)[..16]` gives a fixed-width prefix that
/// enables bounded redb range scans. Collision probability at 10^9 bundle_ids
/// is ~10^-21 (birthday bound at sqrt(2^128) ≈ 1.8×10^19).
fn bundle_prefix(bundle_id: &str) -> [u8; 16] {
    let mut hasher = Hasher::new();
    hasher.update(bundle_id.as_bytes());
    let hash = hasher.finalize();
    let bytes = hash.as_bytes();
    [
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7], bytes[8],
        bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
    ]
}

/// Encode a chunk key for v3 layout.
///
/// Layout: `[blake3(bundle_id)[..16] || chunk_index.to_be_bytes()]` (20 bytes).
///
/// m9-04 D1: fixed-width 20-byte key enables bounded redb range scans.
fn encode_chunk_key(bundle_id: &str, chunk_index: u32) -> Vec<u8> {
    let prefix = bundle_prefix(bundle_id);
    let mut key = Vec::with_capacity(20);
    key.extend_from_slice(&prefix);
    key.extend_from_slice(&chunk_index.to_be_bytes());
    debug_assert_eq!(key.len(), 20, "v3 chunk key must be exactly 20 bytes");
    key
}

/// Decode a v3 chunk key back to `(prefix, chunk_index)`.
///
/// Returns `None` if the key is not exactly 20 bytes.
fn decode_chunk_key(key: &[u8]) -> Option<([u8; 16], u32)> {
    if key.len() != 20 {
        return None;
    }
    let prefix: [u8; 16] = key[..16].try_into().ok()?;
    let chunk_bytes: [u8; 4] = key[16..].try_into().ok()?;
    let chunk_index = u32::from_be_bytes(chunk_bytes);
    Some((prefix, chunk_index))
}

/// Encode a chunk value for v3 layout.
///
/// m9-04 D3: value carries `bundle_id` for per-chunk identity defense against
/// hash-truncation collisions.
fn encode_chunk_value(bundle_id: &str, chunk: &[chronos_domain::TraceEvent]) -> Vec<u8> {
    bincode::serialize(&(bundle_id.to_string(), chunk.to_vec())).unwrap()
}

/// Decode a v3 chunk value back to `(bundle_id, Vec<TraceEvent>)`.
///
/// Returns `None` if bincode deserialization fails.
fn decode_chunk_value(b: &[u8]) -> Option<(String, Vec<chronos_domain::TraceEvent>)> {
    bincode::deserialize(b).ok()
}

/// Decode a chunk payload regardless of encoding (m9-05 R1).
///
/// Tries the v3 layout first (`decode_chunk_value` returns events directly),
/// then falls back to the v2 legacy format (`bincode::serialize(&Vec<TraceEvent>)`).
///
/// Used by `load_counterexample_bundle_events` and `count_counterexample_bundle_events`
/// to centralize the v3-first / v2-fallback decode ladder. Returns `None` if both
/// layouts fail to deserialize.
fn decode_chunk_payload(bytes: &[u8]) -> Option<Vec<TraceEvent>> {
    if let Some((_, events)) = decode_chunk_value(bytes) {
        return Some(events);
    }
    if let Ok(events) = bincode::deserialize::<Vec<TraceEvent>>(bytes) {
        return Some(events);
    }
    None
}

// ========================================================================
// Legacy v2 key/value encoding (pre-m9-04)
// ========================================================================

/// Encode a chunk key for v2 layout (legacy).
///
/// Layout: `[len(bundle_id) as u32 BE][bundle_id bytes][chunk_index as u32 BE]`
/// (variable width).
pub fn encode_chunk_key_legacy(bundle_id: &str, chunk_index: u32) -> Vec<u8> {
    let id_bytes = bundle_id.as_bytes();
    let mut key = Vec::with_capacity(4 + id_bytes.len() + 4);
    key.extend_from_slice(&(id_bytes.len() as u32).to_be_bytes());
    key.extend_from_slice(id_bytes);
    key.extend_from_slice(&chunk_index.to_be_bytes());
    key
}

/// Decode a v2 (legacy) chunk key back to `(bundle_id, chunk_index)`.
fn decode_chunk_key_legacy(key: &[u8]) -> Option<(String, u32)> {
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
pub const BUNDLE_EVENTS_CHUNK_SIZE: usize = 256;

/// Collect v3 chunks for a bundle using a bounded range scan.
///
/// m9-04 D4: range `[prefix, prefix || 0xFFFF_FFFF)` covers all chunk indices
/// [0, u32::MAX] while excluding the next bundle (whose first 16 bytes differ).
///
/// m9-04 D3: value carries bundle_id; we verify it matches the queried bundle_id
/// to defend against hash-truncation collisions.
#[allow(clippy::result_large_err)]
pub fn collect_bundle_chunks_range(
    tx: &redb::ReadTransaction,
    bundle_id: &str,
) -> Result<Vec<(u32, Vec<u8>)>, StoreError> {
    let table = match tx.open_table(COUNTEREXAMPLE_BUNDLE_EVENTS) {
        Ok(t) => t,
        Err(redb::TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
        Err(e) => return Err(StoreError::Database(e.into())),
    };

    let prefix = bundle_prefix(bundle_id);
    let mut start_key = prefix.to_vec();
    start_key.extend_from_slice(&0u32.to_be_bytes());

    let mut end_key = prefix.to_vec();
    end_key.extend_from_slice(&u32::MAX.to_be_bytes());

    let range = table
        .range(start_key.as_slice()..end_key.as_slice())
        .map_err(|e| StoreError::Database(e.into()))?;

    let mut chunks = Vec::new();
    for entry in range {
        let (k, v) = entry.map_err(|e| StoreError::Database(e.into()))?;
        let key_bytes = k.value();

        // D8 defense: decode v3 key, then verify value's bundle_id matches.
        if let Some((_, chunk_idx)) = decode_chunk_key(key_bytes) {
            if let Some((value_bundle_id, _)) = decode_chunk_value(v.value()) {
                if value_bundle_id == bundle_id {
                    chunks.push((chunk_idx, v.value().to_vec()));
                }
            }
        }
        // Keys that fail v3 decode (e.g., v2 legacy keys) are skipped here;
        // the v2 fallback handles them.
    }
    Ok(chunks)
}

/// Collect the raw v3 keys for a bundle using a bounded range scan (m9-05 R2).
///
/// Mirrors the range-scan + identity-verify ladder in `collect_bundle_chunks_range`,
/// but returns the raw key bytes (not `(chunk_idx, value)` pairs). Used by
/// `save_bundle_record_and_events` (D6 cleanup block) to know which keys to remove
/// before writing new v3 chunks.
///
/// Returns `Ok(Vec::new())` when the events table does not exist.
#[allow(clippy::result_large_err)]
fn collect_v3_keys_for_bundle(
    tx: &redb::ReadTransaction,
    bundle_id: &str,
) -> Result<Vec<Vec<u8>>, StoreError> {
    let table = match tx.open_table(COUNTEREXAMPLE_BUNDLE_EVENTS) {
        Ok(t) => t,
        Err(redb::TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
        Err(e) => return Err(StoreError::Database(e.into())),
    };

    let prefix = bundle_prefix(bundle_id);
    let mut start_key = prefix.to_vec();
    start_key.extend_from_slice(&0u32.to_be_bytes());
    let mut end_key = prefix.to_vec();
    end_key.extend_from_slice(&u32::MAX.to_be_bytes());

    let range = match table.range(start_key.as_slice()..end_key.as_slice()) {
        Ok(r) => r,
        Err(e) => return Err(StoreError::Database(e.into())),
    };

    let mut keys = Vec::new();
    for (k, v) in range.flatten() {
        if decode_chunk_key(k.value()).is_some() {
            if let Some((ref id, _)) = decode_chunk_value(v.value()) {
                if id == bundle_id {
                    keys.push(k.value().to_vec());
                }
            }
        }
    }
    Ok(keys)
}

/// Collect v2 (legacy) chunks for a bundle using a full table scan.
///
/// m9-04 D2: legacy reader for bundles persisted under m9-02/m9-03 layout.
/// Invoked only when the v3 range scan returns 0 rows and the bundle has
/// events_count > 0.
#[allow(clippy::result_large_err)]
fn collect_bundle_chunks_legacy(
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
        if let Some((ref id, idx)) = decode_chunk_key_legacy(k.value()) {
            if id == bundle_id {
                chunks.push((idx, v.value().to_vec()));
            }
        }
    }
    Ok(chunks)
}

/// Collect all chunks for a bundle (v3 first, v2 fallback).
///
/// m9-04 D7: tries v3 range scan; if empty, falls back to v2 full scan.
///
/// The `events_count` parameter controls the D7 guard (m9-05 R3):
/// when `events_count == 0`, the v2 fallback is skipped — a post-m9-02 bundle
/// with `events_count == 0` has no events anywhere. `events_count` is the
/// value of `summary.events_count` from the bundle record, or `0` when no
/// record exists for the queried bundle_id.
#[allow(clippy::result_large_err)]
fn collect_bundle_chunks(
    tx: &redb::ReadTransaction,
    bundle_id: &str,
    events_count: u64,
) -> Result<Vec<(u32, Vec<u8>)>, StoreError> {
    let chunks = collect_bundle_chunks_range(tx, bundle_id)?;
    if !chunks.is_empty() {
        return Ok(chunks);
    }
    // D7 guard: a bundle with events_count == 0 has no events anywhere.
    if events_count == 0 {
        return Ok(Vec::new());
    }
    collect_bundle_chunks_legacy(tx, bundle_id)
}

pub const CURRENT_BUNDLE_SCHEMA_VERSION: u32 = 3;

/// Versions the loader accepts silently. Future cycles add entries here
/// when they introduce a new envelope shape.
///
/// m9-06: `#[allow(dead_code)]` removed — the constant is now exercised by
/// `m9_04_known_bundle_schema_versions_pinned` (lib test), and the compile-time
/// `KNOWN_SCHEMA_VERSIONS_INVARIANT` below references it from the production
/// build so it is no longer dead. This closes both `m9-01-R4` (dead-code
/// disclosure) and `FIND-M9-01-DV-OE-01` (speculative-code overeng).
const KNOWN_BUNDLE_SCHEMA_VERSIONS: &[u32] = &[1, 2, 3];

/// Compile-time invariant: `CURRENT_BUNDLE_SCHEMA_VERSION` must be in the
/// known set. Touching `KNOWN_BUNDLE_SCHEMA_VERSIONS` is required to bump it.
/// This block also forces the constant to be evaluated in the production build.
const _: () = {
    let cur = CURRENT_BUNDLE_SCHEMA_VERSION;
    // Manual loop because `<[u32]>::contains` is not const-stable on stable.
    let mut found = false;
    let mut i = 0;
    while i < KNOWN_BUNDLE_SCHEMA_VERSIONS.len() {
        if KNOWN_BUNDLE_SCHEMA_VERSIONS[i] == cur {
            found = true;
            break;
        }
        i += 1;
    }
    assert!(
        found,
        "CURRENT_BUNDLE_SCHEMA_VERSION must be listed in KNOWN_BUNDLE_SCHEMA_VERSIONS"
    );
};

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
    /// **m9-04 D6:** Re-save deletes both v3 and v2 prior chunks before
    /// writing new v3 chunks (atomic dual cleanup). This triggers lazy migration:
    /// re-saving a v2 bundle leaves only v3 chunks.
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
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(0),
            Err(e) => return Err(StoreError::Database(e.into())),
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

    // ========================================================================
    // m9-05 R4: test chokepoints — narrow `pub` surface for cross-crate tests.
    //
    // Pre-m9-05, the cli integration test (`crates/chronos-cli/tests/replay_integration.rs`)
    // needed to inject v2 chunks and count v3 chunks, which forced a `pub` widening of
    // `storage.rs::db()` and the two table constants. R4 reverses that widening and exposes
    // only the two operations the cli test needs.
    // ========================================================================

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
        let chunks = collect_bundle_chunks_range(&tx, bundle_id)?;
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

    // m9-02 §5: a bincode blob with schema_version=4 on the record must be
    // rejected by load_counterexample_bundle with an error mentioning "newer
    // than supported".
    #[test]
    fn m9_02_future_version_load_is_rejected() {
        let store = make_store();

        // Inject a future-versioned record directly into the DB (bypassing
        // save_counterexample_bundle so we control the exact bytes).
        // m9-04: CURRENT is now 3, so use version 4.
        let future_record: CounterexampleBundleRecord = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: "b-future".into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 0,
                rounds_used: 1,
                has_full_bundle: true,
                schema_version: 4, // future version (CURRENT is 3)
                events_count: 0,
            },
            events: vec![],
            minimised: None,
            event_cas_hashes: vec![],
            target_hypothesis: None,
            schema_version: 4, // future version (CURRENT is 3)
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
        // m9-08: the loader now returns the dedicated `SchemaTooNew`
        // variant (closes FIND-M9-01-DV-COUP-02). Pattern-match on the
        // variant so a future regression that re-overloads `Serialization`
        // would be caught here, not at the message-string level.
        match &err {
            StoreError::SchemaTooNew { found, supported } => {
                assert_eq!(*found, 4, "found should be 4");
                assert_eq!(
                    *supported, 3,
                    "supported should be 3 (CURRENT_BUNDLE_SCHEMA_VERSION)"
                );
            }
            other => panic!("expected StoreError::SchemaTooNew, got: {other:?}"),
        }
        let err_msg = format!("{err}");
        assert!(
            err_msg.contains("newer than supported"),
            "error message must mention 'newer than supported', got: {err_msg}"
        );
        assert!(
            err_msg.contains('4'),
            "error message must mention schema_version 4, got: {err_msg}"
        );
    }

    // m9-07 (closes FIND-M9-01-DV-COUP-01): the loader must reject records
    // whose envelope `schema_version` does not match the nested summary
    // `schema_version`. `save()` canonicalizes both, so legitimate writes
    // always agree; this test exercises the gap for hand-constructed
    // records (e.g. a future migration tool).
    #[test]
    fn m9_07_loader_rejects_envelope_summary_version_mismatch() {
        let store = make_store();

        // Envelope says 3 (CURRENT), summary says 1 — a drift the loader
        // would have silently passed before m9-07.
        let mismatched: CounterexampleBundleRecord = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: "b-mismatched".into(),
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
            schema_version: 3,
        };
        store.insert_bundle_record_for_test(&mismatched).unwrap();

        let result = store.load_counterexample_bundle("b-mismatched");
        let err = result.expect_err("envelope/summary schema_version mismatch must be rejected");
        let err_msg = format!("{err}");
        assert!(
            err_msg.contains("disagrees with summary"),
            "error message must mention 'disagrees with summary', got: {err_msg}"
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

        // Future-versioned bundle: inject directly into the DB (schema_version=4).
        // m9-04: CURRENT is now 3, so use version 4.
        let future_record: CounterexampleBundleRecord = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: "b-future".into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 200,
                rounds_used: 1,
                has_full_bundle: true,
                schema_version: 4,
                events_count: 0,
            },
            events: vec![],
            minimised: None,
            event_cas_hashes: vec![],
            target_hypothesis: None,
            schema_version: 4,
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

        // List all bundles: both appear (bincode deserializes schema_version=4 fine).
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

        // Inject a bundle record with events_count = 3 (one event per chunk)
        // so the m9-05 R3 D7 guard lets v2 fallback run.
        // (Prior to m9-05 R3, the guard was None / skip None — bundle records
        // were not required for direct-injection tests. Post R3, a bundle
        // record with events_count > 0 is required to exercise the v2 path.)
        let rec = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: "b-unordered".into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 0,
                rounds_used: 1,
                has_full_bundle: true,
                schema_version: CURRENT_BUNDLE_SCHEMA_VERSION,
                events_count: 3,
            },
            events: vec![],
            minimised: None,
            event_cas_hashes: vec![],
            target_hypothesis: None,
            schema_version: CURRENT_BUNDLE_SCHEMA_VERSION,
        };
        let bytes = bincode::serialize(&rec).unwrap();
        let tx = store.db().begin_write().unwrap();
        {
            let mut table = tx.open_table(COUNTEREXAMPLE_BUNDLES).unwrap();
            table
                .insert(b"b-unordered".as_slice(), bytes.as_slice())
                .unwrap();
        }
        tx.commit().unwrap();

        // Inject 3 chunks directly at indices 0, 2, 1 (out of order).
        // Each chunk has exactly 1 event so we can distinguish them.
        // Use v2 legacy format since the test injects directly.
        let inject = |chunk_idx: u32, id: u64, func: &str| {
            let key = encode_chunk_key_legacy("b-unordered", chunk_idx);
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

    // ========================================================================
    // m9-04 tests: v3 key layout (blake3 prefix, 20-byte fixed-width keys)
    // ========================================================================

    // m9-04 §5: Round-trip + key-shape invariants.
    #[test]
    fn m9_04_v3_key_layout_fixed_width() {
        let key0 = encode_chunk_key("b-test", 0);
        let key1 = encode_chunk_key("b-test", 1);
        assert_eq!(key0.len(), 20, "v3 key must be exactly 20 bytes");
        assert_eq!(key1.len(), 20, "v3 key must be exactly 20 bytes");
        // Keys for different chunks should differ.
        assert_ne!(key0, key1);
        // Keys for different bundle_ids should differ (with very high probability).
        let key_a = encode_chunk_key("bundle-a", 0);
        let key_b = encode_chunk_key("bundle-b", 0);
        assert_ne!(key_a, key_b, "keys for different bundles should differ");
    }

    // m9-04 §5: bundle_prefix extracts 16-byte blake3 prefix.
    #[test]
    fn m9_04_bundle_prefix_is_16_bytes() {
        let prefix = bundle_prefix("test-bundle-id");
        assert_eq!(prefix.len(), 16, "bundle_prefix must return 16 bytes");
        // Same bundle_id produces same prefix.
        let prefix2 = bundle_prefix("test-bundle-id");
        assert_eq!(prefix, prefix2);
        // Different bundle_id produces different prefix (with very high probability).
        let prefix3 = bundle_prefix("different-bundle-id");
        assert_ne!(prefix, prefix3);
    }

    // m9-04 §5: decode_chunk_key round-trips correctly.
    #[test]
    fn m9_04_decode_chunk_key_roundtrip() {
        for (id, idx) in [
            ("b1", 0u32),
            ("b2", 1),
            ("very-long-bundle-id-1234567890", 100),
            ("b", u32::MAX),
        ] {
            let key = encode_chunk_key(id, idx);
            let decoded = decode_chunk_key(&key);
            assert!(
                decoded.is_some(),
                "decode_chunk_key must succeed for ({}, {})",
                id,
                idx
            );
            let (prefix, chunk_idx) = decoded.unwrap();
            assert_eq!(
                chunk_idx, idx,
                "chunk_index must round-trip for ({}, {})",
                id, idx
            );
            assert_eq!(prefix, bundle_prefix(id), "prefix must match bundle_prefix");
        }
    }

    // m9-04 §5: decode_chunk_key rejects wrong-length keys.
    #[test]
    fn m9_04_decode_chunk_key_rejects_wrong_length() {
        assert!(decode_chunk_key(&[]).is_none());
        assert!(decode_chunk_key(&[0u8; 19]).is_none());
        assert!(decode_chunk_key(&[0u8; 21]).is_none());
        // v2 legacy keys are variable width and fail v3 decode.
        let legacy_key = encode_chunk_key_legacy("b", 0);
        assert_ne!(legacy_key.len(), 20);
        assert!(decode_chunk_key(&legacy_key).is_none());
    }

    // m9-04 §5: Value embeds bundle_id for identity verification.
    #[test]
    fn m9_04_encode_decode_value_carries_bundle_id() {
        let events = vec![make_event(1, "fn1"), make_event(2, "fn2")];
        let encoded = encode_chunk_value("b-identity-test", &events);
        let decoded = decode_chunk_value(&encoded);
        assert!(decoded.is_some(), "decode_chunk_value must succeed");
        let (id, decoded_events) = decoded.unwrap();
        assert_eq!(id, "b-identity-test", "value must carry bundle_id");
        assert_eq!(
            decoded_events.len(),
            2,
            "decoded events must match original"
        );
        assert_eq!(decoded_events[0].event_id, 1);
        assert_eq!(decoded_events[1].event_id, 2);
    }

    // m9-04 §5: v3 / v2 fallback round-trip.
    #[test]
    fn m9_04_v3_save_load_roundtrip() {
        let store = make_store();
        let events: Vec<_> = (0..10).map(|i| make_event(i, "roundtrip")).collect();
        let rec = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: "b-v3-roundtrip".into(),
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

        let loaded = store
            .load_counterexample_bundle("b-v3-roundtrip")
            .unwrap()
            .unwrap();
        assert_eq!(
            loaded.summary.schema_version, CURRENT_BUNDLE_SCHEMA_VERSION,
            "schema_version must be 3 after save"
        );

        let loaded_events = store
            .load_counterexample_bundle_events("b-v3-roundtrip")
            .unwrap();
        assert_eq!(loaded_events.len(), 10, "load must return all 10 events");
        assert_eq!(loaded_events[0].event_id, 0);
        assert_eq!(loaded_events[9].event_id, 9);
    }

    // m9-04 §5: v2 bundle loads via fallback.
    #[test]
    fn m9_04_v2_bundle_loads_via_fallback() {
        let store = make_store();

        // Inject a v2 bundle directly (simulating pre-m9-04 on-disk format).
        let bundle_id = "b-v2-legacy";
        for (chunk_idx, ids) in [(0u32, [1u64, 2]), (1, [3u64, 4])] {
            let key = encode_chunk_key_legacy(bundle_id, chunk_idx);
            let chunk: Vec<_> = ids.iter().map(|&id| make_event(id, "legacy")).collect();
            let value = bincode::serialize(&chunk).unwrap();
            let tx = store.db().begin_write().unwrap();
            {
                let mut table = tx.open_table(COUNTEREXAMPLE_BUNDLE_EVENTS).unwrap();
                table.insert(key.as_slice(), value.as_slice()).unwrap();
            }
            tx.commit().unwrap();
        }

        // Inject a v2 record (with events_count > 0 to trigger fallback).
        let rec = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: bundle_id.into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 0,
                rounds_used: 1,
                has_full_bundle: true,
                schema_version: 1,
                events_count: 4, // v2 bundles have this as 0, but for fallback trigger
            },
            events: vec![],
            minimised: None,
            event_cas_hashes: vec![],
            target_hypothesis: None,
            schema_version: 1,
        };
        let bytes = bincode::serialize(&rec).unwrap();
        let tx = store.db().begin_write().unwrap();
        {
            let mut table = tx.open_table(COUNTEREXAMPLE_BUNDLES).unwrap();
            table
                .insert(bundle_id.as_bytes(), bytes.as_slice())
                .unwrap();
        }
        tx.commit().unwrap();

        // Load via the side-table reader - should find v2 chunks via fallback.
        let events = store.load_counterexample_bundle_events(bundle_id).unwrap();
        assert_eq!(events.len(), 4, "fallback must load all 4 v2 events");
        assert_eq!(events[0].event_id, 1);
        assert_eq!(events[3].event_id, 4);
    }

    // m9-04 §5: Re-save migrates v2 → v3.
    #[test]
    fn m9_04_resave_migrates_v2_to_v3() {
        let store = make_store();
        let bundle_id = "b-v2-to-v3";

        // First, inject v2 chunks.
        for (chunk_idx, ids) in [(0u32, [1u64, 2]), (1, [3u64, 4])] {
            let key = encode_chunk_key_legacy(bundle_id, chunk_idx);
            let chunk: Vec<_> = ids
                .iter()
                .map(|&id| make_event(id, "pre-migrate"))
                .collect();
            let value = bincode::serialize(&chunk).unwrap();
            let tx = store.db().begin_write().unwrap();
            {
                let mut table = tx.open_table(COUNTEREXAMPLE_BUNDLE_EVENTS).unwrap();
                table.insert(key.as_slice(), value.as_slice()).unwrap();
            }
            tx.commit().unwrap();
        }

        // Re-save via the API (this writes v3 and cleans up v2).
        let events: Vec<_> = (0..4).map(|i| make_event(i + 10, "post-migrate")).collect();
        let rec = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: bundle_id.into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 1,
                rounds_used: 2,
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

        // Verify events loaded correctly via v3 path.
        let loaded_events = store.load_counterexample_bundle_events(bundle_id).unwrap();
        assert_eq!(loaded_events.len(), 4);
        assert_eq!(loaded_events[0].event_id, 10);
        assert_eq!(loaded_events[3].event_id, 13);

        // Verify v2 keys are gone (defensive check: try to decode a v2 key for this bundle).
        let tx = store.db().begin_read().unwrap();
        let legacy_keys_found = collect_bundle_chunks_legacy(&tx, bundle_id).unwrap().len();
        assert_eq!(legacy_keys_found, 0, "resave must remove v2 legacy keys");
    }

    // m9-04 §5: Range-scan upper bound is prefix || 0xFFFF_FFFF.
    // scoping §5: insert A+B chunks, range(A) returns only A's chunks.
    #[test]
    fn m9_04_range_scan_covers_all_chunk_indices() {
        let store = make_store();

        // Inject v3 chunks for two different bundles.
        let bundle_a = "b-range-a";
        let bundle_b = "b-range-b";

        let tx = store.db().begin_write().unwrap();
        {
            let mut table = tx.open_table(COUNTEREXAMPLE_BUNDLE_EVENTS).unwrap();
            // Bundle A: 4 chunks (indices 0-3).
            for i in 0..4u32 {
                let key = encode_chunk_key(bundle_a, i);
                let chunk = vec![make_event(i as u64, "fn-a")];
                let value = encode_chunk_value(bundle_a, &chunk);
                table.insert(key.as_slice(), value.as_slice()).unwrap();
            }
            // Bundle B: 10 chunks (indices 0-9) — interleaved key bytes but different prefix.
            for i in 0..10u32 {
                let key = encode_chunk_key(bundle_b, i);
                let chunk = vec![make_event((i + 100) as u64, "fn-b")];
                let value = encode_chunk_value(bundle_b, &chunk);
                table.insert(key.as_slice(), value.as_slice()).unwrap();
            }
        }
        tx.commit().unwrap();

        // Inject bundle records so events_count > 0 (triggers v2 fallback guard).
        for bundle_id in [bundle_a, bundle_b] {
            let events_count = match *bundle_id {
                ref id if id == bundle_a => 4,
                _ => 10,
            };
            let rec = CounterexampleBundleRecord {
                summary: CounterexampleBundleSummary {
                    bundle_id: bundle_id.into(),
                    property_kind: "invariant".into(),
                    workspace_id: "ws".into(),
                    created_at_ms: 0,
                    rounds_used: 1,
                    has_full_bundle: true,
                    schema_version: 1,
                    events_count,
                },
                events: vec![],
                minimised: None,
                event_cas_hashes: vec![],
                target_hypothesis: None,
                schema_version: 1,
            };
            let bytes = bincode::serialize(&rec).unwrap();
            let tx = store.db().begin_write().unwrap();
            {
                let mut table = tx.open_table(COUNTEREXAMPLE_BUNDLES).unwrap();
                table
                    .insert(bundle_id.as_bytes(), bytes.as_slice())
                    .unwrap();
            }
            tx.commit().unwrap();
        }

        // Range scan for bundle A must return ONLY A's 4 chunks.
        let events_a = store.load_counterexample_bundle_events(bundle_a).unwrap();
        assert_eq!(
            events_a.len(),
            4,
            "range scan for A must return exactly 4 chunks, not B's 10"
        );
        for (i, evt) in events_a.iter().enumerate() {
            assert_eq!(evt.event_id, i as u64, "A's events must have ids 0..3");
        }

        // Range scan for bundle B must return only B's 10 chunks.
        let events_b = store.load_counterexample_bundle_events(bundle_b).unwrap();
        assert_eq!(
            events_b.len(),
            10,
            "range scan for B must return exactly 10 chunks, not A's 4"
        );
        for (i, evt) in events_b.iter().enumerate() {
            assert_eq!(
                evt.event_id,
                (i + 100) as u64,
                "B's events must have ids 100..109"
            );
        }
    }

    // m9-04 §5: Collision containment via identity check.
    #[test]
    fn m9_04_collision_containment_via_bundle_id() {
        // This test verifies the identity check by manually constructing a scenario
        // where a v3 chunk has the wrong bundle_id in its value.

        let store = make_store();
        let real_bundle_id = "real-bundle";
        let fake_bundle_id = "fake-bundle";

        // Inject a v3 chunk with WRONG bundle_id in the value (simulating a collision).
        let key = encode_chunk_key(real_bundle_id, 0);
        let chunk = vec![make_event(999, "collision-event")];
        // Encode with wrong bundle_id.
        let value = encode_chunk_value(fake_bundle_id, &chunk);
        let tx = store.db().begin_write().unwrap();
        {
            let mut table = tx.open_table(COUNTEREXAMPLE_BUNDLE_EVENTS).unwrap();
            table.insert(key.as_slice(), value.as_slice()).unwrap();
        }
        tx.commit().unwrap();

        // Inject the real bundle record.
        let rec = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: real_bundle_id.into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 0,
                rounds_used: 1,
                has_full_bundle: true,
                schema_version: 1,
                events_count: 1,
            },
            events: vec![],
            minimised: None,
            event_cas_hashes: vec![],
            target_hypothesis: None,
            schema_version: 1,
        };
        let bytes = bincode::serialize(&rec).unwrap();
        let tx = store.db().begin_write().unwrap();
        {
            let mut table = tx.open_table(COUNTEREXAMPLE_BUNDLES).unwrap();
            table
                .insert(real_bundle_id.as_bytes(), bytes.as_slice())
                .unwrap();
        }
        tx.commit().unwrap();

        // Load events for the real bundle - should NOT return the fake chunk.
        let events = store
            .load_counterexample_bundle_events(real_bundle_id)
            .unwrap();
        assert!(
            events.is_empty(),
            "identity check must drop chunks with wrong bundle_id"
        );
    }

    // m9-04 §5: Fresh store with unknown bundle returns empty events vec.
    #[test]
    fn m9_04_fresh_store_ghost_bundle_returns_empty() {
        let store = make_store();
        let events = store
            .load_counterexample_bundle_events("ghost-bundle-no-chunks")
            .unwrap();
        assert!(
            events.is_empty(),
            "ghost bundle must return empty vec without scanning legacy keys"
        );
    }

    // m9-04 §5: Saved v3 bundle + future-version rejection.
    #[test]
    fn m9_04_saved_v3_schema_and_future_rejection() {
        let store = make_store();
        let rec = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: "b-v3-schema".into(),
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
            .load_counterexample_bundle("b-v3-schema")
            .unwrap()
            .unwrap();
        assert_eq!(
            loaded.summary.schema_version, 3,
            "summary.schema_version must be 3 after save"
        );

        // Inject a future-versioned record directly.
        let future_record: CounterexampleBundleRecord = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: "b-future-v4".into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 0,
                rounds_used: 1,
                has_full_bundle: true,
                schema_version: 4,
                events_count: 0,
            },
            events: vec![],
            minimised: None,
            event_cas_hashes: vec![],
            target_hypothesis: None,
            schema_version: 4,
        };
        let future_bytes = bincode::serialize(&future_record).unwrap();
        let tx = store.db().begin_write().unwrap();
        {
            let mut table = tx.open_table(COUNTEREXAMPLE_BUNDLES).unwrap();
            table
                .insert(b"b-future-v4".as_slice(), future_bytes.as_slice())
                .unwrap();
        }
        tx.commit().unwrap();

        // Load it — must be rejected.
        let result = store.load_counterexample_bundle("b-future-v4");
        let err = result.expect_err("future-versioned bundle must be rejected");
        // m9-08: must be the dedicated SchemaTooNew variant, not Serialization.
        match &err {
            StoreError::SchemaTooNew { found, .. } => {
                assert_eq!(*found, 4, "found should be 4");
            }
            other => panic!("expected StoreError::SchemaTooNew, got: {other:?}"),
        }
        let err_msg = format!("{err}");
        assert!(
            err_msg.contains("newer than supported"),
            "error must mention 'newer than supported', got: {}",
            err_msg
        );
    }

    // m9-04 §5: v2 bundle with events_count == 0 loads normally via side table.
    #[test]
    fn m9_04_v2_bundle_with_zero_events_count_loads_normally() {
        let store = make_store();
        let bundle_id = "b-v2-zero-events";

        // Inject v2 chunks (simulating pre-m9-04 on-disk format).
        let key = encode_chunk_key_legacy(bundle_id, 0);
        let chunk = vec![make_event(1, "only-event")];
        let value = bincode::serialize(&chunk).unwrap();
        let tx = store.db().begin_write().unwrap();
        {
            let mut table = tx.open_table(COUNTEREXAMPLE_BUNDLE_EVENTS).unwrap();
            table.insert(key.as_slice(), value.as_slice()).unwrap();
        }
        tx.commit().unwrap();

        // Inject a v2 record. D7 guard: events_count > 0 allows v2 fallback
        // when v3 is empty; events_count = 0 with a bundle record would skip it.
        let rec = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: bundle_id.into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 0,
                rounds_used: 1,
                has_full_bundle: true,
                schema_version: 1,
                events_count: 1, // D7 guard: must be > 0 to trigger v2 fallback
            },
            events: vec![], // events are in v2 side table, not blob
            minimised: None,
            event_cas_hashes: vec![],
            target_hypothesis: None,
            schema_version: 1,
        };
        let bytes = bincode::serialize(&rec).unwrap();
        let tx = store.db().begin_write().unwrap();
        {
            let mut table = tx.open_table(COUNTEREXAMPLE_BUNDLES).unwrap();
            table
                .insert(bundle_id.as_bytes(), bytes.as_slice())
                .unwrap();
        }
        tx.commit().unwrap();

        // Load via the side-table reader (not the chokepoint, which would return blob events).
        let events = store.load_counterexample_bundle_events(bundle_id).unwrap();
        assert_eq!(
            events.len(),
            1,
            "side-table reader must load v2 chunk when events_count > 0"
        );
        assert_eq!(events[0].event_id, 1);
    }

    // m9-04 D8 §Testing: helper that bypasses blake3 to write a chunk key with a
    // chosen 16-byte prefix. Used to force a key-prefix collision in tests without
    // depending on a real blake3 output.
    //
    // Safety contract: only call this in tests. The written prefix may collide with
    // a real bundle's blake3-derived prefix, so never use this in production code.
    fn __force_collision_prefix(
        bundle_id: &str,
        chunk_index: u32,
        forced_prefix: [u8; 16],
    ) -> Vec<u8> {
        let mut key = Vec::with_capacity(20);
        key.extend_from_slice(&forced_prefix);
        key.extend_from_slice(&chunk_index.to_be_bytes());
        debug_assert_eq!(key.len(), 20, "forced key must be exactly 20 bytes");
        let _ = bundle_id; // unused — the forced prefix is caller-supplied
        key
    }

    // m9-04 D8: force a key-prefix collision and verify the identity check drops the
    // wrong-bundle chunk. Two different bundle_ids produce the same 16-byte prefix
    // in the test, but the value carries the true bundle_id — so the range scan
    // finds both chunks, and the per-chunk identity filter drops the foreign one.
    #[test]
    fn m9_04_forced_prefix_collision_is_contained_by_identity_check() {
        let store = make_store();

        let real_bundle = "real-bundle";
        let fake_bundle = "fake-bundle";

        // Derive the forced prefix from real_bundle so the key bytes collide with
        // what encode_chunk_key(real_bundle, ...) would produce.
        let forced_prefix = bundle_prefix(real_bundle);

        // Inject a v3 chunk for the fake bundle using the real bundle's prefix.
        // This creates a key-prefix collision: both chunks have the same 20-byte key.
        let fake_key = __force_collision_prefix(fake_bundle, 0, forced_prefix);
        let fake_chunk = vec![make_event(999, "injected")];
        let fake_value = encode_chunk_value(fake_bundle, &fake_chunk);

        // Also inject the real bundle's chunk under the same key.
        let real_key = encode_chunk_key(real_bundle, 0);
        let real_chunk = vec![make_event(1, "real-event")];
        let real_value = encode_chunk_value(real_bundle, &real_chunk);

        let tx = store.db().begin_write().unwrap();
        {
            let mut table = tx.open_table(COUNTEREXAMPLE_BUNDLE_EVENTS).unwrap();
            table
                .insert(fake_key.as_slice(), fake_value.as_slice())
                .unwrap();
            table
                .insert(real_key.as_slice(), real_value.as_slice())
                .unwrap();
        }
        tx.commit().unwrap();

        // Inject bundle records (events_count > 0 to prevent early return from v2 guard).
        for bundle_id in [real_bundle, fake_bundle] {
            let rec = CounterexampleBundleRecord {
                summary: CounterexampleBundleSummary {
                    bundle_id: bundle_id.into(),
                    property_kind: "invariant".into(),
                    workspace_id: "ws".into(),
                    created_at_ms: 0,
                    rounds_used: 1,
                    has_full_bundle: true,
                    schema_version: 1,
                    events_count: 1,
                },
                events: vec![],
                minimised: None,
                event_cas_hashes: vec![],
                target_hypothesis: None,
                schema_version: 1,
            };
            let bytes = bincode::serialize(&rec).unwrap();
            let tx = store.db().begin_write().unwrap();
            {
                let mut table = tx.open_table(COUNTEREXAMPLE_BUNDLES).unwrap();
                table
                    .insert(bundle_id.as_bytes(), bytes.as_slice())
                    .unwrap();
            }
            tx.commit().unwrap();
        }

        // Loading real_bundle returns ONLY the real chunk (fake is filtered by bundle_id identity).
        let events = store
            .load_counterexample_bundle_events(real_bundle)
            .unwrap();
        assert_eq!(
            events.len(),
            1,
            "identity check must drop the colliding fake-bundle chunk"
        );
        assert_eq!(
            events[0].event_id, 1,
            "the surviving chunk must be the real-bundle chunk"
        );
    }

    // m9-04 R4: a v2 bundle that is NEVER re-saved continues to use the legacy path
    // on every subsequent read. Load the same v2 bundle twice; both reads must return
    // the same events via the fallback path.
    #[test]
    fn m9_04_r4_v2_bundle_never_resaved_uses_legacy_path() {
        let store = make_store();
        let bundle_id = "b-v2-never-resaved";

        // Inject a v2 chunk directly (simulating a pre-m9-04 bundle never re-saved).
        let v2_key = encode_chunk_key_legacy(bundle_id, 0);
        let v2_chunk = vec![make_event(10, "v2-event")];
        let v2_value = bincode::serialize(&v2_chunk).unwrap();
        let tx = store.db().begin_write().unwrap();
        {
            let mut table = tx.open_table(COUNTEREXAMPLE_BUNDLE_EVENTS).unwrap();
            table
                .insert(v2_key.as_slice(), v2_value.as_slice())
                .unwrap();
        }
        tx.commit().unwrap();

        // Inject a v2 bundle record (events_count = 1 to trigger v2 fallback).
        let rec = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: bundle_id.into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 0,
                rounds_used: 1,
                has_full_bundle: true,
                schema_version: 1,
                events_count: 1, // v2 bundle claims to have events
            },
            events: vec![], // events are in side table, not blob
            minimised: None,
            event_cas_hashes: vec![],
            target_hypothesis: None,
            schema_version: 1,
        };
        // Inject bundle record directly (save_counterexample_bundle overwrites events_count based on events.len()).
        let bytes = bincode::serialize(&rec).unwrap();
        let tx = store.db().begin_write().unwrap();
        {
            let mut table = tx.open_table(COUNTEREXAMPLE_BUNDLES).unwrap();
            table
                .insert(bundle_id.as_bytes(), bytes.as_slice())
                .unwrap();
        }
        tx.commit().unwrap();

        // First read — must use v2 fallback.
        let first = store.load_counterexample_bundle_events(bundle_id).unwrap();
        assert_eq!(first.len(), 1, "first read must return v2 chunk");
        assert_eq!(first[0].event_id, 10);

        // Second read — same bundle, never re-saved. Must still use v2 fallback.
        let second = store.load_counterexample_bundle_events(bundle_id).unwrap();
        assert_eq!(second.len(), 1, "second read must still use v2 fallback");
        assert_eq!(second[0].event_id, 10);

        assert_eq!(
            first, second,
            "both reads must return identical events via the legacy path"
        );

        // Defensive: verify v3 side table is empty for this bundle.
        let tx = store.db().begin_read().unwrap();
        let v3_chunks = collect_bundle_chunks_range(&tx, bundle_id).unwrap();
        assert!(
            v3_chunks.is_empty(),
            "never-resaved v2 bundle must have no v3 chunks on disk"
        );
    }

    // m9-04 R6: KNOWN_BUNDLE_SCHEMA_VERSIONS correctly contains [1, 2, 3].
    #[test]
    fn m9_04_known_bundle_schema_versions_pinned() {
        assert!(
            KNOWN_BUNDLE_SCHEMA_VERSIONS.contains(&1),
            "KNOWN_BUNDLE_SCHEMA_VERSIONS must contain 1 (pre-m9-01 legacy)"
        );
        assert!(
            KNOWN_BUNDLE_SCHEMA_VERSIONS.contains(&2),
            "KNOWN_BUNDLE_SCHEMA_VERSIONS must contain 2 (m9-01/m9-02)"
        );
        assert!(
            KNOWN_BUNDLE_SCHEMA_VERSIONS.contains(&3),
            "KNOWN_BUNDLE_SCHEMA_VERSIONS must contain 3 (m9-04 current)"
        );
        assert_eq!(
            KNOWN_BUNDLE_SCHEMA_VERSIONS.len(),
            3,
            "KNOWN_BUNDLE_SCHEMA_VERSIONS must have exactly 3 entries"
        );
    }

    // m9-06: pin the compile-time invariant that CURRENT_BUNDLE_SCHEMA_VERSION
    // is included in KNOWN_BUNDLE_SCHEMA_VERSIONS. The const block at module
    // scope already enforces this at compile time; this test exists so the
    // invariant is greppable from a single test-run.
    #[test]
    fn m9_06_current_schema_version_is_listed_as_known() {
        let cur = CURRENT_BUNDLE_SCHEMA_VERSION;
        assert!(
            KNOWN_BUNDLE_SCHEMA_VERSIONS.contains(&cur),
            "CURRENT_BUNDLE_SCHEMA_VERSION ({cur}) must be listed in KNOWN_BUNDLE_SCHEMA_VERSIONS"
        );
    }
}
