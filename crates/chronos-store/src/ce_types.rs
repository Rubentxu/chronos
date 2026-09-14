//! Counterexample bundle type definitions — owns the 6 `pub` types used
//! by counterexample bundle persistence, plus the
//! `default_schema_version` serde helper.
//!
//! Submodule of `counterexample_storage`. The parent module re-exports
//! the types with `pub use` so external callers continue to find them
//! at `chronos_store::counterexample_storage::*` (path unchanged by
//! this split).

use crate::cas::ContentHash;
use crate::counterexample_storage::CURRENT_BUNDLE_SCHEMA_VERSION;
use chronos_domain::property::PropertyValue;
use chronos_domain::TraceEvent;
use serde::{Deserialize, Serialize};

pub(crate) fn default_schema_version() -> u32 {
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
