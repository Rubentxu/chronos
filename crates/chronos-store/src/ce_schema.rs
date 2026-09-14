//! Counterexample bundle schema — table definitions, schema-version
//! constants, and the 4 collect helpers that drive v3 range scans +
//! v2 legacy fallback.
//!
//! Submodule of `counterexample_storage`. The parent module re-exports
//! all items with `pub use` so they are reachable at
//! `chronos_store::counterexample_storage::*` for sibling submodules
//! (ce_read, ce_write, ce_test_hooks) and any external callers that
//! depend on them.

use crate::counterexample_storage::ce_chunk_keys::{
    bundle_prefix, decode_chunk_key, decode_chunk_key_legacy, decode_chunk_value,
};
use crate::error::StoreError;
use crate::table_error::classify_read_table_error;
use redb::{ReadableTable, TableDefinition};

/// Table for counterexample bundles.
///
/// Key: `bundle_id: String` (as bytes).
/// Value: bincode-serialised [`crate::counterexample_storage::CounterexampleBundleRecord`].
pub const COUNTEREXAMPLE_BUNDLES: TableDefinition<&[u8], &[u8]> =
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
pub const COUNTEREXAMPLE_BUNDLE_EVENTS: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("counterexample_bundle_events");

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
        Err(e) => return classify_read_table_error(e).or_not_found(Vec::new()),
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
pub fn collect_v3_keys_for_bundle(
    tx: &redb::ReadTransaction,
    bundle_id: &str,
) -> Result<Vec<Vec<u8>>, StoreError> {
    let table = match tx.open_table(COUNTEREXAMPLE_BUNDLE_EVENTS) {
        Ok(t) => t,
        Err(e) => return classify_read_table_error(e).or_not_found(Vec::new()),
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
pub fn collect_bundle_chunks_legacy(
    tx: &redb::ReadTransaction,
    bundle_id: &str,
) -> Result<Vec<(u32, Vec<u8>)>, StoreError> {
    let table = match tx.open_table(COUNTEREXAMPLE_BUNDLE_EVENTS) {
        Ok(t) => t,
        Err(e) => return classify_read_table_error(e).or_not_found(Vec::new()),
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
pub fn collect_bundle_chunks(
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
///
/// m9-87: now `pub` (was `const`) so the integration tests in
/// `counterexample_storage` can reach it via
/// `crate::counterexample_storage::KNOWN_BUNDLE_SCHEMA_VERSIONS`.
pub const KNOWN_BUNDLE_SCHEMA_VERSIONS: &[u32] = &[1, 2, 3];

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
