//! Counterexample bundle type definitions — re-exports the domain-owned
//! wire types plus the storage-owned record/filter/summary types
//! (REC-C3.5-R.4).
//!
//! Submodule of `counterexample_storage`. The parent module re-exports
//! the types with `pub use` so external callers continue to find them
//! at `chronos_store::counterexample_storage::*` (path unchanged).
//!
//! **R.4 note:** `MinimisedPayload`, `ExistencePredicateWire`,
//! `HypothesisInputWire` and the new `CounterexampleBundleSummaryWire`
//! live in `chronos_domain::ports::counterexample::wire` since
//! REC-C3.5-R.4 (audit §3.2 A2: application layer consuming storage
//! wire types). The aliases here keep the redb encoding path and
//! external imports stable; bincode/serde encoding is unchanged.

use crate::cas::ContentHash;
use chronos_domain::ports::counterexample::wire::CURRENT_BUNDLE_SCHEMA_VERSION;
use chronos_domain::TraceEvent;
use serde::{Deserialize, Serialize};

// Re-export the domain-owned wire types so `ce_write`/`ce_read` and
// external consumers keep the `chronos_store::counterexample_storage::*`
// path without churn (R.4 alias requirement).
pub use chronos_domain::ports::counterexample::wire::{
    ExistencePredicateWire, HypothesisInputWire, MinimisedPayload,
};

pub(crate) fn default_schema_version() -> u32 {
    CURRENT_BUNDLE_SCHEMA_VERSION
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
