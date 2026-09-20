//! Wire types for counterexample bundle persistence (REC-C3.5-R.4).
//!
//! These 4 types were originally declared in
//! `chronos_store::counterexample_storage` (pre-REC-C3.5). They are pure
//! serde wire shapes with no storage machinery, so owning them in the
//! domain lets `chronos-services` and `chronos-cli` consume them without
//! reaching across the hexagonal boundary into chronos-store internals
//! (audit §3.2 A2: wire types leaking storage crate into application
//! layer). `chronos_store::counterexample_storage` re-exports them so
//! the redb encoding path and existing external paths are unchanged.
//!
//! Field-for-field identical to the pre-R.4 chronos-store declarations;
//! bincode/serde encoding is unchanged (same derives, same field order,
//! same `#[serde(default)]` semantics), so on-disk bundles written
//! before and after R.4 are byte-compatible.
//!
//! **R1 disclosure (m8-07, preserved):** `HypothesisInputWire` is a
//! hand-maintained mirror of `chronos_services::hypothesis_test::
//! HypothesisInput`. Drift is possible if `HypothesisInput` evolves
//! without updating this mirror; mitigated by the roundtrip test in
//! `crates/chronos-services/src/counterexample.rs::tests`.

use serde::{Deserialize, Serialize};

use crate::property::PropertyValue;

/// What causal slice triggered the violation. Plumbed via the bundle so
/// `counterexample_get` can re-emit the relevant trace window without
/// re-running the shrink.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MinimisedPayload {
    /// Minimised `PropertyValue` for an `Invariant` shrink.
    Constant(PropertyValue),
    /// Minimised predicate for an `Existence` shrink. Same wire shape as
    /// `chronos_services::output::ExistencePredicate` but a separate type
    /// to keep the domain independent of the application layer. The
    /// dispatcher in `services::counterexample` does a `match`-based
    /// conversion at the boundary.
    Predicate(ExistencePredicateWire),
    /// Minimised (caller, callee, optional max_depth) for a `CallPath` shrink.
    CallPath {
        caller: String,
        callee: String,
        max_depth: Option<u64>,
    },
}

/// Wire-friendly existence predicate (domain-owned since REC-C3.5-R.4).
///
/// Mirrors `chronos_services::output::ExistencePredicate` line-for-line
/// (same three variants, same field names). Kept independent so the
/// domain does not depend on the application layer.
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
/// All fields are plain string / Option types to avoid cross-crate type
/// sharing. Kind/scope/comparison are stringified to keep the domain
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

/// Summary wire shape used at the persistence boundary (no events).
///
/// Same field-for-field shape as
/// `chronos_store::counterexample_storage::CounterexampleBundleSummary`
/// (minus the `schema_version` + `events_count` storage bookkeeping) so
/// `chronos-cli` can summarise a bundle without pulling in chronos-store.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CounterexampleBundleSummaryWire {
    pub bundle_id: String,
    /// String form ("invariant" | "existence" | "call_path").
    pub property_kind: String,
    pub workspace_id: String,
    pub created_at_ms: u64,
    pub rounds_used: u32,
    pub has_full_bundle: bool,
}
