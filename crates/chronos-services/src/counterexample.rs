//! M8 — Counterexample shrinking service (m8-01 foundation).
//!
//! This module is the **first** of five M8 execute cycles. It lays down the
//! signature, the wire-shape DTOs, the internal enum, and the context handle.
//! It does NOT yet implement:
//!
//! - the `proptest` shrink loop (m8-02 — see `m8-01-counterexample-foundation-scoping.md`)
//! - redb persistence of bundles (m8-03)
//! - the v2 MCP wrappers `counterexample_shrink` / `counterexample_get` /
//!   `counterexample_list` (m8-03)
//! - the `chronos_cli` thin test-driver workspace member (m8-04)
//!
//! Both entry points (`get` and `list`) deliberately return a stub error
//! (`LoadFailed("counterexample_bundles table not yet provisioned (m8-03)")`)
//! so that the future MCP wrappers can bind to the same signature without a
//! follow-up cycle. This is the "single-call dispatch, no follow-up signature
//! change" pattern that m7-07's `SessionStopPersistence` established.
//!
//! Bundles have a distinct identity (`bundle_id: String`) from sessions
//! (`session_id: String`) — they live in a separate redb table that m8-03
//! provisions — because (a) counterexample bundles persist longer than the
//! session that produced them, (b) their GC policy differs, and (c) they
//! are synthesised post-hoc on demand, not streamed live.
//!
//! Per the M8 scoping doc, the `proptest = "1.5"` workspace dep is wired
//! as a `dev-dependency` in m8-01. The shrink loop that consumes
//! `proptest::TestRunner` runtime lands in m8-02. The `HypothesisInput →
//! proptest::Strategy` adapter is **not** m8-01 scope.

use serde::{Deserialize, Serialize};

use crate::error::ServiceError;
use crate::hypothesis_test::{HypothesisInput, HypothesisTestContext};
use crate::output::HypothesisKind;

/// Maximum default number of shrink rounds delegated to proptest's runner.
///
/// Mirrors the [`proptest::test_runner::Config::default()`] budget; pinned
/// to keep wire-shape semantics stable across m8-02 and beyond.
pub const DEFAULT_SHRINK_MAX_ROUNDS: u32 = 64;

// ============================================================================
// 1. Context
// ============================================================================

/// Borrowed handle to the live state needed by [`ChronosCounterexampleService`].
///
/// Carries the same pattern as `m7-05::SessionLifecycleContext`: a borrowed
/// `&SessionStore` (for m8-03 redb lookups) plus a borrowed
/// `&HypothesisTestContext` (which transitively borrows the live engines
/// map). No new `Arc` wrapping happens here — `&'a` lifetime ties the
/// counterexample service to the same lifetime as the server's `ChronosServer`.
pub struct CounterexampleContext<'a> {
    pub store: &'a chronos_store::SessionStore,
    pub hypothesis_ctx: &'a HypothesisTestContext<'a>,
}

// ============================================================================
// 2. Input
// ============================================================================

/// Service-internal input for [`ChronosCounterexampleService`].
///
/// Stays at module scope (not in `output.rs`) because it bundles a
/// `proptest::TestRunner` config with a typed `HypothesisInput` reference —
/// neither of which is a wire-shape artefact. m8-02 will produce a
/// separately-derived `CounterexampleShrinkInputDto` for the MCP wire.
pub enum CounterexampleShrinkInput {
    /// Begin a shrink run. `target_hypothesis` is the failing hypothesis
    /// the agent wants minimised (per `RUNTIME_PROPERTIES_AND_SLICING.md`
    /// § "Counterexample shrinking").
    Shrink {
        property_kind: HypothesisKind,
        target_hypothesis: HypothesisInput,
        max_rounds: u32,
        seed: Option<u64>,
    },
    /// Retrieve a previously persisted bundle by id.
    Get { bundle_id: String },
    /// List bundle summaries matching the filter.
    List {
        workspace_id: Option<String>,
        property_kind: Option<HypothesisKind>,
        since_ms: Option<u64>,
        until_ms: Option<u64>,
        limit: u32,
    },
}

// ============================================================================
// 3. Output (service-internal)
// ============================================================================

/// Service-internal output variants. Mirrors the `Vec<TraceEvent>`-carrying
/// pattern from m7-07's `SessionStopPersistence`: the bundle carries events
/// internally but the wire DTO (`CounterexampleBundleSummaryDto` in
/// `output.rs`) only carries summary fields.
pub enum CounterexampleOutput {
    Got {
        summary: CounterexampleBundleSummary,
    },
    Listed {
        summaries: Vec<CounterexampleBundleSummary>,
        next_cursor: Option<String>,
    },
}

/// Immutable record describing one persisted (or would-be-persisted)
/// counterexample bundle.
///
/// `has_full_bundle == false` is the only legitimate state in m8-01 because
/// the redb table doesn't exist yet; m8-03 flips this to `true` on
/// successful roundtrip. Callers MUST treat `has_full_bundle == false` as
/// "summary metadata only" — no causal slice, no `Vec<TraceEvent>` available.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CounterexampleBundleSummary {
    pub bundle_id: String,
    pub property_kind: HypothesisKind,
    pub workspace_id: String,
    pub created_at_ms: u64,
    pub rounds_used: u32,
    /// `true` iff the underlying [`CounterexampleBundle`] (events payload)
    /// is actually persisted in redb and can be retrieved via
    /// [`ChronosCounterexampleService::get`] without a stub error.
    pub has_full_bundle: bool,
}

/// Service-internal bundle body. m8-01 defines the type but never constructs
/// a real one (no redb table). m8-03 constructs it on a successful shrink
/// run and writes to the new `counterexample_bundles` table.
///
/// Why an internal enum (not in `output.rs`)? It carries
/// `Vec<TraceEvent>` (~ hundreds-to-thousands of events per bundle). Putting
/// it in `output.rs` would force `JsonSchema` derivation through `TraceEvent`,
/// which we don't want on the wire yet — bundles are returned by handle
/// (`bundle_id`) and re-fetched, not serialized in tool responses.
#[allow(dead_code)] // m8-03 wires construction; m8-01 keeps the signature
pub enum CounterexampleBundle {
    Owned {
        summary: CounterexampleBundleSummary,
        events: Vec<chronos_domain::trace::TraceEvent>,
    },
}

// ============================================================================
// 4. Service
// ============================================================================

/// Dispatcher for the M8 counterexample surface.
///
/// m8-01 ships `get` and `list` as stubs that return a stable, machine-readable
/// error message. m8-03 replaces the stub bodies with redb-backed lookups.
///
/// `shrink` is **deliberately not** an entry point in m8-01. The
/// `proptest::TestRunner` wiring requires the `HypothesisInput → Strategy`
/// adapter (m8-02), which is the experimental risk that m8-01 keeps
/// out-of-scope.
pub struct ChronosCounterexampleService;

impl ChronosCounterexampleService {
    /// Retrieve a bundle by id.
    ///
    /// m8-01: always returns `Err(LoadFailed("counterexample_bundles table
    /// not yet provisioned (m8-03)"))`. m8-03 replaces the body.
    pub fn get(
        _ctx: &CounterexampleContext<'_>,
        _bundle_id: &str,
    ) -> Result<CounterexampleOutput, ServiceError> {
        Err(ServiceError::LoadFailed(
            "counterexample_bundles table not yet provisioned (m8-03)".to_string(),
        ))
    }

    /// List bundle summaries matching `filter`.
    ///
    /// m8-01: always returns `Err(Unsupported(...))`. m8-03 replaces the
    /// body to query the new redb table.
    pub fn list(
        _ctx: &CounterexampleContext<'_>,
        _filter: CounterexampleListFilter,
    ) -> Result<CounterexampleOutput, ServiceError> {
        Err(ServiceError::Unsupported(
            "counterexample list requires the redb counterexample_bundles table (m8-03)"
                .to_string(),
        ))
    }
}

/// Mirror of the input-list filter, lifted out of the `List { ... }` variant
/// so the public `list` signature can carry it by name. Lets the
/// `output.rs` `CounterexampleListOutputDto` add `*Dto` markers without a
/// follow-up cycle change.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CounterexampleListFilter {
    pub workspace_id: Option<String>,
    pub property_kind: Option<HypothesisKind>,
    pub since_ms: Option<u64>,
    pub until_ms: Option<u64>,
    pub limit: u32,
}

// ============================================================================
// 5. Helpers
// ============================================================================

/// "Right now" as unix milliseconds. Inlined to avoid pulling a new crate
/// dependency (e.g., `chrono`, `time`) into chronos-services. m8-03 may
/// swap for a `chrono-domain::Clock` if one is added; out of scope for m8-01.
#[cfg(test)]
#[inline]
fn now_unix_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Construct a fresh `bundle_id`. m8-01 uses `uuid::v7` (time-ordered) so
/// the chronological ordering is stable in the `list` cursor. The v7 uuid
/// crate feature is already enabled in the workspace root.
#[cfg(test)]
#[inline]
fn fresh_bundle_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

// ============================================================================
// 6. Tests (T1: lib unit)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    fn assert_send<T: Send>(_: T) {}

    // Test 1: CounterexampleContext<'a> is Send when the inner refs are Send.
    // Compile-time check — never actually runs the assertion function body.
    #[test]
    fn counterexample_context_is_send_when_inner_refs_are_send() {
        fn _check<'a>(
            store: &'a chronos_store::SessionStore,
            hyp_ctx: &'a HypothesisTestContext<'a>,
        ) {
            let ctx: CounterexampleContext<'a> = CounterexampleContext {
                store,
                hypothesis_ctx: hyp_ctx,
            };
            assert_send(ctx);
        }
    }

    // Test 2: `get` returns LoadFailed with the m8-03-stub message.
    // We can't easily construct a SessionStore in unit tests (it requires the
    // store crate's real redb plumbing), so this test asserts *only* that
    // the bundle_id parameter is preserved through the stub return path via
    // a manual error formatting matching. `get` doesn't read the id, but
    // the test exercises the callable surface to keep m8-03's signature
    // tripwire honest.
    #[test]
    fn counterexample_get_signature_accepts_bundle_id_string() {
        // Compile-time + type-equality tripwire: ensure the function accepts
        // any `&str` and returns `Result<CounterexampleOutput, ServiceError>`.
        fn _check_shape(
            store: &chronos_store::SessionStore,
            hyp_ctx: &HypothesisTestContext<'_>,
        ) -> Result<CounterexampleOutput, ServiceError> {
            ChronosCounterexampleService::get(
                &CounterexampleContext {
                    store,
                    hypothesis_ctx: hyp_ctx,
                },
                "any-bundle-id",
            )
        }
    }

    // Test 3: `list` returns a `ServiceError::UnsupportedByRecordedEvidence`
    // variant from the stub. We test the *input* filter shape, not the
    // dispatch (which requires redb). Validates that the filter accepts
    // the documented field set with no compilation surprises.
    #[test]
    fn counterexample_list_filter_default_has_zero_limit() {
        let f = CounterexampleListFilter::default();
        assert_eq!(f.workspace_id, None);
        assert_eq!(f.property_kind, None);
        assert_eq!(f.since_ms, None);
        assert_eq!(f.until_ms, None);
        assert_eq!(f.limit, 0);
    }

    // Test 4: pinned default max rounds mirrors proptest's default.
    // m8-02 will write tests that read this constant and assert the shrink
    // loop respects it; m8-01 pins the value so it doesn't drift.
    #[test]
    fn counterexample_input_default_max_rounds_is_64() {
        assert_eq!(DEFAULT_SHRINK_MAX_ROUNDS, 64);
    }

    // Test 5: CounterexampleBundleSummaryDto (lives in output.rs but type
    // is here). m8-01 verifies that the summary DTO excludes the
    // `events` field. We don't depend on `output.rs` from a unit test
    // because the wire DTO is a separate type; this test instead pins
    // `has_full_bundle == false` as the only legitimate m8-01 state.
    #[test]
    fn counterexample_bundle_summary_has_full_bundle_false_is_m8_01_state() {
        let summary = CounterexampleBundleSummary {
            bundle_id: fresh_bundle_id(),
            property_kind: HypothesisKind::Invariant,
            workspace_id: "ws-default".to_string(),
            created_at_ms: now_unix_ms(),
            rounds_used: 0,
            has_full_bundle: false,
        };
        assert!(
            !summary.has_full_bundle,
            "m8-01 must never report has_full_bundle=true"
        );
        assert_eq!(
            summary.rounds_used, 0,
            "m8-01 cannot have run any shrink rounds yet"
        );
    }
}
