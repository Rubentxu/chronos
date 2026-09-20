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

use crate::error::StoreError;
use chronos_domain::TraceEvent;

#[cfg(test)]
use chronos_domain::property::PropertyValue;

/// Submodule owning the chunk key/value wire format (v3 + v2 legacy).
///
/// All helpers are `pub` in the submodule; this parent module re-exports
/// them with `pub(super) use` so internal callers in this module can invoke
/// them without `crate::` paths. `encode_chunk_key_legacy` is additionally
/// accessible to downstream crates via
/// `chronos_store::counterexample_storage::encode_chunk_key_legacy`.
#[path = "ce_chunk_keys.rs"]
pub(crate) mod ce_chunk_keys;
pub(super) use ce_chunk_keys::{
    decode_chunk_payload, encode_chunk_key, encode_chunk_key_legacy, encode_chunk_value,
};

// Test-only re-exports: the integration tests in this file (and the
// m9-04 layout tests) use `bundle_prefix`, `decode_chunk_key`,
// `decode_chunk_value`. In production these items are reached via the
// sibling submodules' own `use crate::counterexample_storage::ce_chunk_keys::*`
// path. `#[allow(unused_imports)]` silences the warning without dropping
// the test reachability.
#[cfg(test)]
#[allow(unused_imports)]
pub(super) use ce_chunk_keys::{bundle_prefix, decode_chunk_key, decode_chunk_value};

/// Submodule owning the write-path `impl SessionStore` methods
/// (`save_counterexample_bundle`, `save_bundle_record_and_events`).
///
/// Methods are defined in `impl SessionStore` blocks inside the submodule.
/// Rust inherent methods are reachable via `store.method_name()` regardless of
/// which file the `impl` lives in, so the public path is unchanged: callers
/// still write `store.save_counterexample_bundle(record)`. There is no
/// `pub use` here because methods are not module-level items.
#[path = "ce_write.rs"]
pub mod ce_write;

/// Submodule owning the read-path `impl SessionStore` methods
/// (`load_counterexample_bundle_events`, `count_counterexample_bundle_events`,
/// `get_bundle_events_count`, `load_counterexample_bundle`,
/// `list_counterexample_bundles`).
///
/// Methods are defined in `impl SessionStore` inside the submodule and are
/// reachable via `<SessionStore>::method_name` or `store.method_name()`.
#[path = "ce_read.rs"]
pub mod ce_read;

/// Submodule owning the m9-05 R4 test chokepoint methods
/// (`insert_v2_chunk_for_test`, `count_v3_chunks_for_test`,
/// `insert_bundle_record_for_test`).
///
/// Methods are defined in `impl SessionStore` inside the submodule and are
/// reachable via `<SessionStore>::method_name` or `store.method_name()`. They
/// are `#[doc(hidden)] pub` inside the submodule to keep the public docs
/// clean for `SessionStore`'s user-facing API.
#[path = "ce_test_hooks.rs"]
pub mod ce_test_hooks;

/// Submodule owning the 6 `pub` types used by counterexample bundle
/// persistence (`MinimisedPayload`, `ExistencePredicateWire`,
/// `HypothesisInputWire`, `CounterexampleBundleFilter`,
/// `CounterexampleBundleSummary`, `CounterexampleBundleRecord`) plus the
/// `default_schema_version` serde helper.
///
/// Types are `pub` in the submodule; this parent module re-exports
/// them with `pub use` so internal callers and external crates
/// continue to find them at
/// `chronos_store::counterexample_storage::*` (path unchanged by this
/// split).
#[path = "ce_types.rs"]
pub(crate) mod ce_types;
#[cfg(test)]
pub(crate) use ce_types::default_schema_version;
pub use ce_types::{
    CounterexampleBundleFilter, CounterexampleBundleRecord, CounterexampleBundleSummary,
    ExistencePredicateWire, HypothesisInputWire, MinimisedPayload,
};

/// Submodule owning the schema/table definitions, the chunk-size
/// constant, the 4 collect helpers, and the compile-time invariant
/// block binding `CURRENT_BUNDLE_SCHEMA_VERSION` to
/// `KNOWN_BUNDLE_SCHEMA_VERSIONS`.
///
/// All items are re-exported at the parent path so sibling submodules
/// (ce_read, ce_write, ce_test_hooks) and external callers continue to
/// find them at `chronos_store::counterexample_storage::*` (path
/// unchanged by this split).
#[path = "ce_schema.rs"]
pub(crate) mod ce_schema;
pub use ce_schema::{
    collect_bundle_chunks, collect_bundle_chunks_legacy, collect_bundle_chunks_range,
    collect_v3_keys_for_bundle, BUNDLE_EVENTS_CHUNK_SIZE, COUNTEREXAMPLE_BUNDLES,
    COUNTEREXAMPLE_BUNDLE_EVENTS, KNOWN_BUNDLE_SCHEMA_VERSIONS,
};
// REC-C3.5-R.5: the schema-version constant is part of the wire
// contract and is domain-owned. The external path
// `chronos_store::counterexample_storage::CURRENT_BUNDLE_SCHEMA_VERSION`
// is preserved via this re-export; the known-versions invariant stays
// in ce_schema.
pub use chronos_domain::ports::counterexample::wire::CURRENT_BUNDLE_SCHEMA_VERSION;

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
#[path = "ce_storage_tests.rs"]
mod tests;
