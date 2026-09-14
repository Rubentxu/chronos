//! Counterexample bundle chunk key/value encoding + decoding.
//!
//! This submodule owns the wire format for the `counterexample_bundle_events`
//! side-table keys and values. Two layouts coexist:
//!
//! - **v3 (current, m9-04+):** Fixed-width 20-byte keys composed of a 16-byte
//!   blake3 prefix and a 4-byte big-endian `chunk_index`. Values carry
//!   `(bundle_id, Vec<TraceEvent>)` for per-chunk identity defense (D3).
//! - **v2 (legacy, pre-m9-04):** Variable-width keys composed of a 4-byte
//!   length prefix, the UTF-8 bundle_id bytes, and a 4-byte `chunk_index`.
//!   Values carry `Vec<TraceEvent>` only.
//!
//! The encoding helpers here are pure functions with no state and no side
//! effects. They are `pub` at the submodule level and re-exported at the
//! parent `counterexample_storage` path via `pub(super) use`, so internal
//! callers in the parent module invoke them without `crate::` paths and
//! the public surface (`encode_chunk_key_legacy`) remains reachable from
//! downstream crates at
//! `chronos_store::counterexample_storage::encode_chunk_key_legacy`.
//!
//! `BUNDLE_EVENTS_CHUNK_SIZE` deliberately stays in the parent module because
//! it is referenced by `save_bundle_record_and_events` (D7 save-path ladder)
//! and is conceptually a schema parameter rather than an encoding helper.

use blake3::Hasher;
use chronos_domain::TraceEvent;

/// Compute the 16-byte blake3 prefix for a bundle_id.
///
/// m9-04 D1: `blake3(bundle_id)[..16]` gives a fixed-width prefix that
/// enables bounded redb range scans. Collision probability at 10^9 bundle_ids
/// is ~10^-21 (birthday bound at sqrt(2^128) ≈ 1.8×10^19).
pub fn bundle_prefix(bundle_id: &str) -> [u8; 16] {
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
pub fn encode_chunk_key(bundle_id: &str, chunk_index: u32) -> Vec<u8> {
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
pub fn decode_chunk_key(key: &[u8]) -> Option<([u8; 16], u32)> {
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
pub fn encode_chunk_value(bundle_id: &str, chunk: &[TraceEvent]) -> Vec<u8> {
    bincode::serialize(&(bundle_id.to_string(), chunk.to_vec())).unwrap()
}

/// Decode a v3 chunk value back to `(bundle_id, Vec<TraceEvent>)`.
///
/// Returns `None` if bincode deserialization fails.
pub fn decode_chunk_value(b: &[u8]) -> Option<(String, Vec<TraceEvent>)> {
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
pub fn decode_chunk_payload(bytes: &[u8]) -> Option<Vec<TraceEvent>> {
    if let Some((_, events)) = decode_chunk_value(bytes) {
        return Some(events);
    }
    if let Ok(events) = bincode::deserialize::<Vec<TraceEvent>>(bytes) {
        return Some(events);
    }
    None
}

/// Encode a chunk key for v2 layout (legacy).
///
/// Layout: `[len(bundle_id) as u32 BE][bundle_id bytes][chunk_index as u32 BE]`
/// (variable width).
///
/// **Public** because downstream callers reference it through
/// `chronos_store::counterexample_storage::encode_chunk_key_legacy`.
pub fn encode_chunk_key_legacy(bundle_id: &str, chunk_index: u32) -> Vec<u8> {
    let id_bytes = bundle_id.as_bytes();
    let mut key = Vec::with_capacity(4 + id_bytes.len() + 4);
    key.extend_from_slice(&(id_bytes.len() as u32).to_be_bytes());
    key.extend_from_slice(id_bytes);
    key.extend_from_slice(&chunk_index.to_be_bytes());
    key
}

/// Decode a v2 (legacy) chunk key back to `(bundle_id, chunk_index)`.
pub fn decode_chunk_key_legacy(key: &[u8]) -> Option<(String, u32)> {
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
