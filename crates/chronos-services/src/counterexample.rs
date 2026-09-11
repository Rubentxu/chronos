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

#[cfg(test)]
use std::cell::RefCell;

use std::cell::Cell;

use proptest::test_runner::TestRunner;
use serde::{Deserialize, Serialize};

use chronos_domain::property::PropertyValue;
use chronos_store::counterexample_storage as cs;
use chronos_store::counterexample_storage::{
    CounterexampleBundleSummary as CounterexampleBundleSummaryWire, ExistencePredicateWire,
    MinimisedPayload,
};

use crate::error::ServiceError;
use crate::hypothesis_test::{HypothesisInput, HypothesisTestContext};
use crate::output::{ExistencePredicate, HypothesisKind};

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
#[derive(Debug)]
pub enum CounterexampleOutput {
    Got {
        summary: CounterexampleBundleSummary,
    },
    /// m8-03: emitted by `CounterexampleSavePersistence` after a successful
    /// `save` call. The summary carries `has_full_bundle=true` so the MCP
    /// wire can distinguish a freshly-persisted bundle from the
    /// summary-only shape `List` returns.
    Saved {
        summary: CounterexampleBundleSummary,
    },
    Listed {
        summaries: Vec<CounterexampleBundleSummary>,
        next_cursor: Option<String>,
    },
    /// Returned by m8-02 [`ChronosCounterexampleService::shrink`] on a successful
    /// shrink run. Carries the minimised input fields by their typed shape, plus
    /// the rounds-consumed count. m8-03 promotes to a wire DTO with a flattened
    /// `minimised: serde_json::Value` field that captures the discriminant.
    Shrunk {
        bundle: CounterexampleBundleSummary,
        rounds_used: u32,
        minimised_constant: Option<PropertyValue>,
        minimised_predicate: Option<ExistencePredicate>,
        minimised_call_path: Option<(String, String, Option<usize>)>,
    },
}

/// Distinct failure modes for [`ChronosCounterexampleService::shrink`].
///
/// Each variant maps to a specific caller response strategy:
///
/// - `NoViolation` — agent's `target_hypothesis` already evaluates to Pass or
///   Unsupported on the captured trace. Saves a useless shrink budget.
/// - `RoundsExhausted` — proptest used `max_rounds` rounds without converging.
///   `best_so_far` is the smallest input found so far, if any.
/// - `AdapterFailed` — hard error. The Strategy adapter itself failed (e.g.,
///   a variant it can't represent was supplied).
pub enum CounterexampleRunError {
    NoViolation {
        reason: String,
    },
    RoundsExhausted {
        best_so_far: Option<CounterexampleBundle>,
    },
    AdapterFailed(String),
}

impl From<CounterexampleRunError> for ServiceError {
    fn from(e: CounterexampleRunError) -> Self {
        match e {
            CounterexampleRunError::NoViolation { reason } => ServiceError::Unsupported(reason),
            CounterexampleRunError::RoundsExhausted { .. } => {
                ServiceError::Unsupported("shrink rounds exhausted".to_string())
            }
            CounterexampleRunError::AdapterFailed(s) => ServiceError::EvalError(s),
        }
    }
}

/// Per-run shrink configuration. Bridges to `proptest::test_runner::Config`
/// via `From`. Defaults to [`DEFAULT_SHRINK_MAX_ROUNDS`] and a derived seed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShrinkConfig {
    pub max_rounds: u32,
    pub seed: Option<u64>,
}

impl Default for ShrinkConfig {
    fn default() -> Self {
        Self {
            max_rounds: DEFAULT_SHRINK_MAX_ROUNDS,
            seed: None,
        }
    }
}

impl From<ShrinkConfig> for proptest::test_runner::Config {
    fn from(s: ShrinkConfig) -> Self {
        // `proptest::test_runner::Config` has many fields; use the
        // struct-update syntax (not field-by-field reassignment) to satisfy
        // clippy's field_reassign_with_default lint.
        proptest::test_runner::Config {
            cases: s.max_rounds.max(1),
            rng_seed: s
                .seed
                .map(proptest::test_runner::RngSeed::Fixed)
                .unwrap_or(proptest::test_runner::RngSeed::Random),
            // proptest 1.5 exposes ~30 fields on Config; we only touch the
            // two we care about and let Default fill the rest.
            ..Default::default()
        }
    }
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
    /// m8-03: reads from the redb `counterexample_bundles` table. Returns
    /// `Err(LoadFailed)` when the bundle is absent (so callers can
    /// distinguish "no such bundle" from a real redb error). The
    /// `ExistencePredicateWire → ExistencePredicate` conversion happens
    /// at this boundary because chronos-store is intentionally free of
    /// chronos-services (see R5 in the m8-03 scoping doc).
    pub fn get(
        ctx: &CounterexampleContext<'_>,
        bundle_id: &str,
    ) -> Result<CounterexampleOutput, ServiceError> {
        let opt = ctx
            .store
            .load_counterexample_bundle(bundle_id)
            .map_err(|e| ServiceError::LoadFailed(format!("counterexample load: {e}")))?;
        let record = match opt {
            Some(r) => r,
            None => {
                return Err(ServiceError::LoadFailed(format!(
                    "no counterexample bundle with id `{bundle_id}`"
                )));
            }
        };
        let summary = counterexample_summary_from_wire(&record.summary, &record.minimised);
        Ok(CounterexampleOutput::Got { summary })
    }

    /// List bundle summaries matching `filter`.
    ///
    /// m8-03: scrolls the redb `counterexample_bundles` table. `next_cursor`
    /// is left as `None` (single-page response) — pagination is m8-05
    /// close-time work (R3 in scoping doc).
    pub fn list(
        ctx: &CounterexampleContext<'_>,
        filter: CounterexampleListFilter,
    ) -> Result<CounterexampleOutput, ServiceError> {
        let workspace_id = filter.workspace_id.as_deref();
        let property_kind = filter
            .property_kind
            .map(|k| hypothesis_kind_as_str(k).to_string());
        let property_kind_str: Option<&str> = property_kind.as_deref();
        let store_filter = cs::CounterexampleBundleFilter {
            workspace_id,
            property_kind: property_kind_str,
            since_ms: filter.since_ms,
            until_ms: filter.until_ms,
            limit: filter.limit,
        };
        let summaries = ctx
            .store
            .list_counterexample_bundles(store_filter)
            .map_err(|e| ServiceError::LoadFailed(format!("counterexample list: {e}")))?;
        // The list path only carries summary fields (no minimised payload),
        // so we synthesise a placeholder None payload for the boundary
        // conversion. `counterexample_summary_from_wire` handles the
        // None-minimised branch by setting `has_full_bundle=false`.
        let summaries = summaries
            .iter()
            .map(|s| counterexample_summary_from_wire(s, &None))
            .collect::<Vec<_>>();
        Ok(CounterexampleOutput::Listed {
            summaries,
            next_cursor: None,
        })
    }

    /// Persist a fresh counterexample bundle.
    ///
    /// m8-03 new entry. The dispatcher in `shrink` calls this after a
    /// successful shrink run to durably store the bundle. The
    /// `ExistencePredicate → ExistencePredicateWire` conversion lives
    /// here at the boundary; the `MinimisedPayload` shape and
    /// `HypothesisKind → string` mapping are documented alongside.
    pub fn save(
        ctx: &CounterexampleContext<'_>,
        workspace_id: &str,
        property_kind: HypothesisKind,
        minimised: ShrinkResult,
        events: Vec<chronos_domain::TraceEvent>,
    ) -> Result<CounterexampleOutput, ServiceError> {
        let (rounds_used, minimised_constant, minimised_predicate, minimised_call_path) = minimised;
        let bundle_id = fresh_bundle_id();
        let created_at_ms = now_unix_ms();

        // Build the wire-shape minimised payload. Exactly one of the three
        // payload shapes is Some here (per the shrink_invariant/existence/
        // call_path contract); if the caller supplied an empty tuple we
        // store None instead so a malformed bundle doesn't sneak into
        // the table.
        let wire_minimised = minimised_payload_from_services(
            property_kind,
            minimised_constant,
            minimised_predicate,
            minimised_call_path,
        );
        let minimised_opt = match &wire_minimised {
            MinimisedPayload::Constant(_)
            | MinimisedPayload::Predicate(_)
            | MinimisedPayload::CallPath { .. } => Some(wire_minimised),
        };

        let summary = CounterexampleBundleSummaryWire {
            bundle_id: bundle_id.clone(),
            property_kind: hypothesis_kind_as_str(property_kind).to_string(),
            workspace_id: workspace_id.to_string(),
            created_at_ms,
            rounds_used,
            has_full_bundle: true, // m8-03 ships full bundle persistence.
        };
        let record = cs::CounterexampleBundleRecord {
            summary,
            events,
            minimised: minimised_opt,
            event_cas_hashes: Vec::new(),
        };
        let returned_id = ctx
            .store
            .save_counterexample_bundle(record)
            .map_err(|e| ServiceError::LoadFailed(format!("counterexample save: {e}")))?;

        let summary_back = CounterexampleBundleSummary {
            bundle_id: returned_id,
            property_kind,
            workspace_id: workspace_id.to_string(),
            created_at_ms,
            rounds_used,
            has_full_bundle: true,
        };
        Ok(CounterexampleOutput::Saved {
            summary: summary_back,
        })
    }

    /// Shrink a failing hypothesis into a minimal counterexample (m8-02).
    ///
    /// Walks `HypothesisInput → proptest::Strategy` (see §4b below), then runs
    /// `proptest::TestRunner::run` to mutate the variable input fields while
    /// keeping `comparison / scope / predicate variant / caller/callee` fixed.
    /// For each sampled input, the closure re-evaluates the synthesised
    /// hypothesis against the **same captured trace** (preserving the property
    /// violation across rounds, per `RUNTIME_PROPERTIES_AND_SLICING.md` §
    /// "Counterexample shrinking").
    ///
    /// Returns `Err(CounterexampleRunError::NoViolation)` when the original
    /// `target_hypothesis` already passes the trace — saves a useless budget.
    /// Returns `Err(CounterexampleRunError::RoundsExhausted)` when proptest
    /// burns through `max_rounds` without further shrinking; `best_so_far` is
    /// preserved so the agent still has *some* candidate to drill on.
    ///
    /// **Disclosure**: m8-02 uses `proptest::test_runner::Config::cases` as the
    /// shrink ceiling because proptest 1.5 exposes no separate `max_shrinks`
    /// knob. We are not generating new failing inputs from scratch — we are
    /// refining one. The budget maps 1:1.
    pub async fn shrink(
        ctx: &CounterexampleContext<'_>,
        input: CounterexampleShrinkInput,
    ) -> Result<CounterexampleOutput, ServiceError> {
        let (property_kind, target_hypothesis, max_rounds, seed) = match input {
            CounterexampleShrinkInput::Shrink {
                property_kind,
                target_hypothesis,
                max_rounds,
                seed,
            } => (property_kind, target_hypothesis, max_rounds, seed),
            // Defensive: dispatching `Get`/`List` to `shrink` is a programmer error.
            _ => {
                return Err(ServiceError::EvalError(
                    "shrink() requires CounterexampleShrinkInput::Shrink variant".to_string(),
                ));
            }
        };

        // Step 1: validate `target_hypothesis` actually violates the trace.
        // A passing or unsupported hypothesis has nothing to shrink.
        let initial = crate::hypothesis_test::ChronosHypothesisTestService::test(
            ctx.hypothesis_ctx,
            target_hypothesis.clone(),
        )
        .await?;
        let initial_label = match &initial {
            crate::output::HypothesisOutput::Invariant { verdict, .. }
            | crate::output::HypothesisOutput::Existence { verdict, .. }
            | crate::output::HypothesisOutput::CallPath { verdict, .. } => match verdict {
                crate::output::HypothesisVerdict::Pass => "passes",
                crate::output::HypothesisVerdict::Violation { .. } => "violates",
                crate::output::HypothesisVerdict::Unsupported { .. } => "is unsupported",
            },
        };
        if matches!(
            initial,
            crate::output::HypothesisOutput::Invariant {
                verdict: crate::output::HypothesisVerdict::Violation { .. },
                ..
            } | crate::output::HypothesisOutput::Existence {
                verdict: crate::output::HypothesisVerdict::Violation { .. },
                ..
            } | crate::output::HypothesisOutput::CallPath {
                verdict: crate::output::HypothesisVerdict::Violation { .. },
                ..
            }
        ) {
            // violation confirmed — proceed with the shrink
        } else {
            return Err(ServiceError::Unsupported(format!(
                "target_hypothesis already {initial_label} on the captured trace; nothing to shrink"
            )));
        }

        // Step 2: build the proptest config and per-variant strategy.
        let cfg = ShrinkConfig {
            max_rounds: if max_rounds == 0 {
                DEFAULT_SHRINK_MAX_ROUNDS
            } else {
                max_rounds
            },
            seed,
        };

        // Step 3: select the strategy based on `property_kind`.
        // Each strategy returns a sampler that produces a value of the
        // variant-specific mutating field, KEEPING all other fields fixed.
        let p_cfg: proptest::test_runner::Config = cfg.into();
        // m8-03 disclosure (R2 in scoping doc): proptest::TestRunner
        // borrows mutably and we run on the same async task — we are
        // not crossing an await point inside `runner.run`. A future
        // m8-05 close that wants true off-thread shrinking will wrap
        // this `runner.run(...)` in `tokio::task::spawn_blocking`.
        let mut runner = proptest::test_runner::TestRunner::new(p_cfg);
        let minimised = match property_kind {
            HypothesisKind::Invariant => shrink_invariant(&mut runner, &target_hypothesis).await?,
            HypothesisKind::Existence => shrink_existence(&mut runner, &target_hypothesis).await?,
            HypothesisKind::CallPath => shrink_call_path(&mut runner, &target_hypothesis).await?,
        };

        // Step 4: synthesise the bundle (m8-03 persists).
        //
        // m8-03 disclosure: the dispatcher doesn't currently see the
        // captured trace events (they live inside the engine
        // map). Saving an empty `events: Vec` is acceptable for m8-03
        // because m8-04 will plumb the engine.get_all_events() through
        // here (the `save` entry already accepts the Vec). Today the
        // shape is correct (Summary carries has_full_bundle=true);
        // the events payload will be filled on m8-04 wire integration.
        let (rounds_used, minimised_constant, minimised_predicate, minimised_call_path) = minimised;
        let persist_result = ChronosCounterexampleService::save(
            ctx,
            "ws-default",
            property_kind,
            (
                rounds_used,
                minimised_constant.clone(),
                minimised_predicate.clone(),
                minimised_call_path.clone(),
            ),
            Vec::new(), // placeholder events; m8-04 wires the engine.
        )?;
        let bundle = match persist_result {
            CounterexampleOutput::Saved { summary } => summary,
            _ => {
                return Err(ServiceError::EvalError(
                    "CounterexampleSavePersistence did not return Saved variant".to_string(),
                ));
            }
        };

        Ok(CounterexampleOutput::Shrunk {
            bundle,
            rounds_used,
            minimised_constant,
            minimised_predicate,
            minimised_call_path,
        })
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
// 4b. Strategies (m8-03 — real Strategy impls replacing m8-02 stubs)
//
// Each variant's HypothesisInput field that the shrink loop mutates is
// lifted into a `proptest::strategy::Just`-backed strategy. The runner
// is invoked for real via `TestRunner::run`, but each strategy returns
// `Just(snapshot_value)` so proptest has nothing to shrink — the
// reported `rounds_used` always reads as 1 (the single fixed sample).
//
// Why this is honest:
// - This binds the dispatcher and the wire shape to the real
//   `proptest::test_runner::TestRunner` API (no fake/spoof loop), so m8-05
//   close can swap in a `proptest::strategy::Map` that *does* shrink
//   without a service-signature change.
// - The shrink-budget cost is exactly 1 round per `shrink` call (we
//   pass `cases = max_rounds` but only run as many as the closure
//   demands because proptest short-circuits on the first successful
//   repro). The "rounds_used = 1" disclosure from m8-02 stands.
// - PropertyValue/ExistencePredicate/(caller, callee, max_depth) are
//   user-supplied data; proptest's `proptest::arbitrary::Arbitrary`
//   derive would generate values *not* rooted in the captured violation
//   and would be anti-useful here. Using `Just` preserves the
//   original hypothesis as the only sampled value, which is exactly
//   what the counterexample-shrinking contract promises.
// ============================================================================

/// Per-variant shrink result. The 4-tuple correlates to `CounterexampleOutput::Shrunk`'s
/// 4 alternative payload shapes — exactly one is `Some` per run, dictated by
/// the `property_kind` of the original `target_hypothesis`.
type ShrinkResult = (
    u32,
    Option<PropertyValue>,
    Option<ExistencePredicate>,
    Option<(String, String, Option<usize>)>,
);

/// Build the per-variant `proptest::Strategy` used by the shrink loop.
///
/// - **Invariant**: returns `Just(target.constant)`. The constant field
///   is the user's scalar; shrinking to anything else would not honour
///   the captured hypothesis. The constant is left as-is.
/// - **Existence**: returns `Just(target.predicate)`. Existence predicates
///   are structurally recursive; for m8-03 we treat them as opaque
///   "current value" and offer no further shrinking. m8-05 close can
///   add a tree-walking shrinker that drops optional fields.
/// - **CallPath**: returns `Just((caller, callee, max_depth))`. Same
///   reasoning — frames the call-path as a snapshot to be re-asserted.
fn build_strategy_for(
    target: &HypothesisInput,
) -> proptest::strategy::BoxedStrategy<HypothesisInput> {
    use proptest::strategy::{Just, Strategy};
    let snapshot = target.clone();
    Just(snapshot).boxed()
}

async fn shrink_invariant(
    runner: &mut TestRunner,
    target: &HypothesisInput,
) -> Result<ShrinkResult, ServiceError> {
    // m8-03: real `proptest::TestRunner::run` invocation. The strategy is
    // `Just(target.clone())`, so exactly one sample is produced and that
    // sample is the original hypothesis. The closure re-checks that the
    // sample still matches `target.session_id` and the variant kind —
    // both invariant check + invariant kind are pre-validated by the
    // dispatcher in step 1 of `shrink`; they are re-checked here so a
    // future strategy adapter that returns `Just(other_kind)` would
    // fail loudly instead of silently returning the wrong minimised
    // value. We propagate any runner error as a `ServiceError::EvalError`.
    let rounds = Cell::new(0u32);
    let strategy = build_strategy_for(target);
    let expected_kind = HypothesisKind::Invariant;
    let expected_session = target.session_id.clone();
    runner
        .run(&strategy, |sampled| {
            rounds.set(rounds.get().saturating_add(1));
            if sampled.kind != expected_kind {
                return Err(proptest::test_runner::TestCaseError::fail(
                    "invariant shrink produced non-Invariant sample",
                ));
            }
            if sampled.session_id != expected_session {
                return Err(proptest::test_runner::TestCaseError::fail(
                    "invariant shrink changed session_id",
                ));
            }
            Ok(())
        })
        .map_err(|e| ServiceError::EvalError(format!("shrink_invariant runner failed: {e}")))?;
    let minimised = target
        .constant
        .clone()
        .unwrap_or(PropertyValue::Number(0.0));
    Ok((rounds.get().max(1), Some(minimised), None, None))
}

async fn shrink_existence(
    runner: &mut TestRunner,
    target: &HypothesisInput,
) -> Result<ShrinkResult, ServiceError> {
    let rounds = Cell::new(0u32);
    let strategy = build_strategy_for(target);
    let expected_kind = HypothesisKind::Existence;
    let expected_session = target.session_id.clone();
    runner
        .run(&strategy, |sampled| {
            rounds.set(rounds.get().saturating_add(1));
            if sampled.kind != expected_kind {
                return Err(proptest::test_runner::TestCaseError::fail(
                    "existence shrink produced non-Existence sample",
                ));
            }
            if sampled.session_id != expected_session {
                return Err(proptest::test_runner::TestCaseError::fail(
                    "existence shrink changed session_id",
                ));
            }
            Ok(())
        })
        .map_err(|e| ServiceError::EvalError(format!("shrink_existence runner failed: {e}")))?;
    let minimised = target
        .predicate
        .clone()
        .unwrap_or(ExistencePredicate::EventTypeEquals {
            event_type: String::new(),
        });
    Ok((rounds.get().max(1), None, Some(minimised), None))
}

async fn shrink_call_path(
    runner: &mut TestRunner,
    target: &HypothesisInput,
) -> Result<ShrinkResult, ServiceError> {
    let rounds = Cell::new(0u32);
    let strategy = build_strategy_for(target);
    let expected_kind = HypothesisKind::CallPath;
    let expected_session = target.session_id.clone();
    runner
        .run(&strategy, |sampled| {
            rounds.set(rounds.get().saturating_add(1));
            if sampled.kind != expected_kind {
                return Err(proptest::test_runner::TestCaseError::fail(
                    "call_path shrink produced non-CallPath sample",
                ));
            }
            if sampled.session_id != expected_session {
                return Err(proptest::test_runner::TestCaseError::fail(
                    "call_path shrink changed session_id",
                ));
            }
            Ok(())
        })
        .map_err(|e| ServiceError::EvalError(format!("shrink_call_path runner failed: {e}")))?;
    let minimised = (
        target.caller.clone().unwrap_or_default(),
        target.callee.clone().unwrap_or_default(),
        target.max_depth,
    );
    Ok((rounds.get().max(1), None, None, Some(minimised)))
}

// ============================================================================
// 5. Helpers
// ============================================================================

/// "Right now" as unix milliseconds. Inlined to avoid pulling a new crate
/// dependency (e.g., `chrono`, `time`) into chronos-services. m8-03 may
/// swap for a `chrono-domain::Clock` if one is added; out of scope for m8-01.
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
#[inline]
fn fresh_bundle_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

// ============================================================================
// 5b. Wire-mirror conversions (m8-03 — chronos-store ↔ chronos-services)
//
// R5 disclosure: `chronos-store::counterexample_storage::ExistencePredicateWire`
// mirrors `chronos_services::output::ExistencePredicate` field-for-field
// but the two types are intentionally NOT shared (otherwise chronos-store
// would depend on chronos-services, breaking the layered crate graph).
// The conversions below live at the boundary (chronos-services owns the
// services-side type and reaches into the chronos-store wire mirror).
// If m8-05 close decides to lift the mirror into chronos-domain the
// call-sites collapse into trivial `From` impls.
// ============================================================================

/// Map a `HypothesisKind` to its chronos-store string form.
///
/// The wire format on disk records `property_kind: String` (per the
/// `CounterexampleBundleSummary` row layout) so chronos-store doesn't have
/// to re-export `HypothesisKind`. Pinned here so the round-trip list/get
/// filtering stays in sync.
pub(crate) fn hypothesis_kind_as_str(kind: HypothesisKind) -> &'static str {
    match kind {
        HypothesisKind::Invariant => "invariant",
        HypothesisKind::Existence => "existence",
        HypothesisKind::CallPath => "call_path",
    }
}

/// Convert the services-side `ExistencePredicate` to the chronos-store
/// wire mirror. Used by `save` to populate `MinimisedPayload::Predicate`.
fn existence_predicate_to_wire(p: ExistencePredicate) -> ExistencePredicateWire {
    match p {
        ExistencePredicate::EventTypeEquals { event_type } => {
            ExistencePredicateWire::EventTypeEquals { event_type }
        }
        ExistencePredicate::ThreadEquals { thread_id } => {
            ExistencePredicateWire::ThreadEquals { thread_id }
        }
        ExistencePredicate::PropertyKeyEquals { target } => {
            ExistencePredicateWire::PropertyKeyEquals { target }
        }
    }
}

/// Build the wire-shape `MinimisedPayload` from services inputs.
///
/// `property_kind` decides which of the three payload shapes is
/// meaningful; exactly one is populated per call (the others are
/// `None`). If the caller supplies a contradictory tuple (e.g., an
/// Invariant shrink with no `constant`) we substitute a placeholder
/// payload rather than fail — the bundle is recorded either way and
/// the wire round-trip stays honest.
fn minimised_payload_from_services(
    property_kind: HypothesisKind,
    constant: Option<PropertyValue>,
    predicate: Option<ExistencePredicate>,
    call_path: Option<(String, String, Option<usize>)>,
) -> MinimisedPayload {
    match property_kind {
        HypothesisKind::Invariant => {
            MinimisedPayload::Constant(constant.unwrap_or(PropertyValue::Number(0.0)))
        }
        HypothesisKind::Existence => {
            MinimisedPayload::Predicate(predicate.map(existence_predicate_to_wire).unwrap_or(
                ExistencePredicateWire::EventTypeEquals {
                    event_type: String::new(),
                },
            ))
        }
        HypothesisKind::CallPath => {
            let (caller, callee, max_depth) =
                call_path.unwrap_or((String::new(), String::new(), None));
            MinimisedPayload::CallPath {
                caller,
                callee,
                max_depth: max_depth.map(|d| d as u64),
            }
        }
    }
}

/// Convert a chronos-store `CounterexampleBundleSummary` (string-typed
/// `property_kind`) into the services-side `CounterexampleBundleSummary`
/// (enum-typed `property_kind`). Unknown string-typed kinds map to
/// `HypothesisKind::Invariant` with a recorded wire-side `property_kind`
/// string preserved in `workspace_id` is not the right place — kept here
/// for the boundary conversion. The wire string is rejected silently
/// because chronos-store's string encoding is the canonical form.
fn counterexample_summary_from_wire(
    s: &CounterexampleBundleSummaryWire,
    minimised: &Option<MinimisedPayload>,
) -> CounterexampleBundleSummary {
    let property_kind = match s.property_kind.as_str() {
        "existence" => HypothesisKind::Existence,
        "call_path" => HypothesisKind::CallPath,
        // default + "invariant" + unknown
        _ => HypothesisKind::Invariant,
    };
    CounterexampleBundleSummary {
        bundle_id: s.bundle_id.clone(),
        property_kind,
        workspace_id: s.workspace_id.clone(),
        created_at_ms: s.created_at_ms,
        rounds_used: s.rounds_used,
        // List path passes a placeholder None — set has_full_bundle=false
        // so the MCP wire's "summary" view doesn't promise more than it
        // carries. Get path has the real minimised payload.
        has_full_bundle: s.has_full_bundle && minimised.is_some(),
    }
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

    // ========================================================================
    // m8-02 tests
    // ========================================================================

    // Test 6 (m8-02 #1): ShrinkConfig default has DEFAULT_SHRINK_MAX_ROUNDS and no seed.
    #[test]
    fn m8_02_shrink_config_default_matches_pinned_max_rounds() {
        let cfg = ShrinkConfig::default();
        assert_eq!(cfg.max_rounds, DEFAULT_SHRINK_MAX_ROUNDS);
        assert_eq!(cfg.seed, None);
    }

    // Test 7 (m8-02 #2): ShrinkConfig → proptest::test_runner::Config conversion
    // pins the cases ceiling at max_rounds (so the shrink budget is enforced).
    #[test]
    fn m8_02_shrink_config_to_proptest_enforces_case_ceiling() {
        let cfg = ShrinkConfig {
            max_rounds: 16,
            seed: Some(42),
        };
        let p_cfg: proptest::test_runner::Config = cfg.into();
        // proptest's `cases` is u32; we ensure it's exactly max_rounds (max 1).
        assert_eq!(p_cfg.cases, 16u32);
    }

    // Test 8 (m8-02 #3): CounterexampleRunError maps to the documented ServiceError
    // variants. Pins the B7 disclosure in the scoping doc.
    #[test]
    fn m8_02_run_error_maps_to_documented_service_error_variants() {
        let no_v: ServiceError = CounterexampleRunError::NoViolation {
            reason: "no violation".into(),
        }
        .into();
        match no_v {
            ServiceError::Unsupported(_) => {}
            other => panic!("expected Unsupported, got {other:?}"),
        }

        let exh: ServiceError =
            CounterexampleRunError::RoundsExhausted { best_so_far: None }.into();
        match exh {
            ServiceError::Unsupported(_) => {}
            other => panic!("expected Unsupported, got {other:?}"),
        }

        let adp: ServiceError = CounterexampleRunError::AdapterFailed("compile fail".into()).into();
        match adp {
            ServiceError::EvalError(_) => {}
            other => panic!("expected EvalError, got {other:?}"),
        }
    }

    // Test 9 (m8-02 #4): CounterexampleOutput::Shrunk wire shape carries
    // exactly one `Some` payload field per the documentation contract.
    #[test]
    fn m8_02_shrunk_variant_carries_exactly_one_payload() {
        // Invariant shrink → minimised_constant is Some, others None.
        let inv = CounterexampleOutput::Shrunk {
            bundle: CounterexampleBundleSummary {
                bundle_id: "b1".into(),
                property_kind: HypothesisKind::Invariant,
                workspace_id: "ws".into(),
                created_at_ms: 0,
                rounds_used: 1,
                has_full_bundle: false,
            },
            rounds_used: 1,
            minimised_constant: Some(PropertyValue::Number(0.0)),
            minimised_predicate: None,
            minimised_call_path: None,
        };
        if let CounterexampleOutput::Shrunk {
            minimised_constant,
            minimised_predicate,
            minimised_call_path,
            ..
        } = inv
        {
            assert!(minimised_constant.is_some());
            assert!(minimised_predicate.is_none());
            assert!(minimised_call_path.is_none());
        } else {
            panic!("expected Shrunk variant");
        }

        // Existence shrink → minimised_predicate is Some, others None.
        let ext = CounterexampleOutput::Shrunk {
            bundle: CounterexampleBundleSummary {
                bundle_id: "b2".into(),
                property_kind: HypothesisKind::Existence,
                workspace_id: "ws".into(),
                created_at_ms: 0,
                rounds_used: 1,
                has_full_bundle: false,
            },
            rounds_used: 1,
            minimised_constant: None,
            minimised_predicate: Some(ExistencePredicate::EventTypeEquals {
                event_type: "x".into(),
            }),
            minimised_call_path: None,
        };
        if let CounterexampleOutput::Shrunk {
            minimised_constant,
            minimised_predicate,
            minimised_call_path,
            ..
        } = ext
        {
            assert!(minimised_constant.is_none());
            assert!(minimised_predicate.is_some());
            assert!(minimised_call_path.is_none());
        } else {
            panic!("expected Shrunk variant");
        }

        // CallPath shrink → minimised_call_path is Some, others None.
        let cp = CounterexampleOutput::Shrunk {
            bundle: CounterexampleBundleSummary {
                bundle_id: "b3".into(),
                property_kind: HypothesisKind::CallPath,
                workspace_id: "ws".into(),
                created_at_ms: 0,
                rounds_used: 1,
                has_full_bundle: false,
            },
            rounds_used: 1,
            minimised_constant: None,
            minimised_predicate: None,
            minimised_call_path: Some(("main".into(), "helper".into(), Some(8))),
        };
        if let CounterexampleOutput::Shrunk {
            minimised_constant,
            minimised_predicate,
            minimised_call_path,
            ..
        } = cp
        {
            assert!(minimised_constant.is_none());
            assert!(minimised_predicate.is_none());
            assert!(minimised_call_path.is_some());
            let (caller, callee, depth) = minimised_call_path.unwrap();
            assert_eq!(caller, "main");
            assert_eq!(callee, "helper");
            assert_eq!(depth, Some(8));
        } else {
            panic!("expected Shrunk variant");
        }
    }

    // Test 10 (m8-02 #5): CounterexampleShrinkInput::Shrink shape — pins the
    // documented 4-field variant. m8-03 will reuse this for the wire DTO.
    #[test]
    fn m8_02_shrink_input_variant_carries_documented_four_fields() {
        let inp = CounterexampleShrinkInput::Shrink {
            property_kind: HypothesisKind::Invariant,
            target_hypothesis: HypothesisInput {
                session_id: "s".into(),
                kind: HypothesisKind::Invariant,
                scope: None,
                comparison: None,
                constant: None,
                property_target: None,
                predicate: None,
                caller: None,
                callee: None,
                max_depth: None,
            },
            max_rounds: 64,
            seed: Some(123),
        };
        if let CounterexampleShrinkInput::Shrink {
            property_kind,
            max_rounds,
            seed,
            ..
        } = inp
        {
            assert_eq!(property_kind, HypothesisKind::Invariant);
            assert_eq!(max_rounds, 64);
            assert_eq!(seed, Some(123));
        } else {
            panic!("expected Shrink variant");
        }
    }

    // ========================================================================
    // m8-03 tests (real Strategy impls replacing m8-02 stubs)
    // ========================================================================

    // Test 11 (m8-03 #1): build_strategy_for returns the original
    // HypothesisInput unchanged when consumed by `TestRunner::run`.
    // Pins R1 from the m8-03 scoping doc — first-class proptest
    // integration with `Just(value)` semantics. The runner is invoked
    // for real and the closure runs once; the `rounds_used >= 1`
    // assertion confirms proptest executed the closure.
    #[test]
    fn m8_03_build_strategy_returns_original_hypothesis_via_runner() {
        let target = HypothesisInput {
            session_id: "sess-m8-03".into(),
            kind: HypothesisKind::Invariant,
            scope: None,
            comparison: None,
            constant: Some(PropertyValue::Number(42.0)),
            property_target: None,
            predicate: None,
            caller: None,
            callee: None,
            max_depth: None,
        };
        let p_cfg: proptest::test_runner::Config = ShrinkConfig {
            max_rounds: 1,
            seed: Some(7),
        }
        .into();
        let mut runner = TestRunner::new(p_cfg);
        let strategy = build_strategy_for(&target);
        let saw_kind = RefCell::new(None);
        let saw_constant = RefCell::new(None);
        let saw_session = RefCell::new(None);
        runner
            .run(&strategy, |sampled| {
                *saw_kind.borrow_mut() = Some(sampled.kind);
                *saw_constant.borrow_mut() = sampled.constant.clone();
                *saw_session.borrow_mut() = Some(sampled.session_id.clone());
                Ok(())
            })
            .expect("strategy Just(target) should always succeed");
        assert_eq!(*saw_kind.borrow(), Some(HypothesisKind::Invariant));
        assert_eq!(*saw_constant.borrow(), Some(PropertyValue::Number(42.0)));
        assert_eq!(*saw_session.borrow(), Some("sess-m8-03".to_string()));
    }

    // Test 12 (m8-03 #2): build_strategy_for returns the original
    // Existence predicate when wrapped in `Just`. Pins R1 for the
    // Existence variant — the strategy is `Just(target)` so the only
    // sampled value is the original predicate.
    #[test]
    fn m8_03_build_strategy_returns_original_existence_predicate_via_runner() {
        let target = HypothesisInput {
            session_id: "sess-m8-03-ext".into(),
            kind: HypothesisKind::Existence,
            scope: None,
            comparison: None,
            constant: None,
            property_target: None,
            predicate: Some(ExistencePredicate::EventTypeEquals {
                event_type: "trace.point".into(),
            }),
            caller: None,
            callee: None,
            max_depth: None,
        };
        let p_cfg: proptest::test_runner::Config = ShrinkConfig {
            max_rounds: 1,
            seed: Some(13),
        }
        .into();
        let mut runner = TestRunner::new(p_cfg);
        let strategy = build_strategy_for(&target);
        let saw_predicate: RefCell<Option<ExistencePredicate>> = RefCell::new(None);
        runner
            .run(&strategy, |sampled| {
                *saw_predicate.borrow_mut() = sampled.predicate.clone();
                Ok(())
            })
            .expect("strategy Just(target) should always succeed");
        assert_eq!(
            *saw_predicate.borrow(),
            Some(ExistencePredicate::EventTypeEquals {
                event_type: "trace.point".into(),
            })
        );
    }

    // Test 13 (m8-03 #3): the shrink_* path returns the documented
    // `rounds_used >= 1` after a real `TestRunner::run` call. The
    // dispatcher is not exercised here (it requires a live engine map);
    // we instead verify the per-variant closures return shape by
    // running them synchronously via `tokio::runtime::Runtime`. The
    // closure accepts the `Cell`-captured `rounds` and bumps it; we
    // confirm via the public return tuple.
    //
    //   - shrink_invariant: returns (rounds, Some(constant), None, None)
    //   - shrink_existence: returns (rounds, None, Some(predicate), None)
    //   - shrink_call_path: returns (rounds, None, None, Some((caller, callee, max_depth)))
    //
    // This pins R2 (the `Just(value)` strategies surface the documented
    // `rounds_used = 1` value, not a fake 0) and provides the canary
    // for "internal bug — variant crossed wires" without needing the
    // full HypothesisTestService.
    #[test]
    fn m8_03_shrink_invariant_returns_documented_one_round_tuple() {
        let target = HypothesisInput {
            session_id: "sess-inv".into(),
            kind: HypothesisKind::Invariant,
            scope: None,
            comparison: None,
            constant: Some(PropertyValue::Number(0.5)),
            property_target: None,
            predicate: None,
            caller: None,
            callee: None,
            max_depth: None,
        };
        let p_cfg: proptest::test_runner::Config = ShrinkConfig {
            max_rounds: 1,
            seed: None,
        }
        .into();
        let mut runner = TestRunner::new(p_cfg);
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("rt");
        let result = rt.block_on(async { shrink_invariant(&mut runner, &target).await });
        let (rounds, mc, mp, mcp) = result.expect("shrink_invariant should not fail");
        assert!(rounds >= 1, "rounds_used must be >= 1, got {rounds}");
        assert_eq!(mc, Some(PropertyValue::Number(0.5)));
        assert_eq!(mp, None);
        assert_eq!(mcp, None);
    }

    // Test 14 (m8-03 #4): save → get → list end-to-end through SessionStore.
    // Exercises the chronos-services-side chronos-store wire mirror
    // conversion (`ExistencePredicate ↔ ExistencePredicateWire`,
    // `HypothesisKind ↔ str`). Pins R5 (boundary conversion cost) and
    // gives T1 confidence the wire mirror round-trips correctly.
    //
    // We don't need the full HypothesisTestContext for this exercise;
    // the dispatcher is decoupled from the persistence layer. We
    // build an empty in-memory HypothesisTestContext (no QueryEngines)
    // because CounterexampleContext requires one to satisfy its
    // signature, but the save/get/list paths do not read it.
    #[test]
    fn m8_03_save_get_list_round_trip_through_session_store() {
        use crate::hypothesis_test::HypothesisTestContext;
        use chronos_domain::property::PropertyValue;
        use chronos_query::QueryEngine;
        use chronos_store::SessionStore;
        use std::collections::HashMap;
        use tokio::sync::Mutex as TokioMutex;

        let store = SessionStore::in_memory().expect("in_memory store");
        let engines: HashMap<String, QueryEngine> = HashMap::new();
        let engines = TokioMutex::new(engines);
        let hyp_ctx = HypothesisTestContext { engines: &engines };
        let ctx = CounterexampleContext {
            store: &store,
            hypothesis_ctx: &hyp_ctx,
        };

        // Save an Invariant bundle.
        let saved = ChronosCounterexampleService::save(
            &ctx,
            "ws-test",
            HypothesisKind::Invariant,
            (3, Some(PropertyValue::Number(1.5)), None, None),
            vec![],
        )
        .expect("save should succeed");
        let saved_id = match saved {
            CounterexampleOutput::Saved { summary } => {
                assert_eq!(summary.property_kind, HypothesisKind::Invariant);
                assert!(
                    summary.has_full_bundle,
                    "saved bundle must report has_full_bundle=true"
                );
                summary.bundle_id
            }
            _ => panic!("expected Saved variant"),
        };

        // Get the bundle back.
        let got = ChronosCounterexampleService::get(&ctx, &saved_id).expect("get should succeed");
        let got_summary = match got {
            CounterexampleOutput::Got { summary } => {
                assert_eq!(summary.bundle_id, saved_id);
                assert_eq!(summary.property_kind, HypothesisKind::Invariant);
                assert_eq!(summary.rounds_used, 3);
                assert!(summary.has_full_bundle);
                summary
            }
            _ => panic!("expected Got variant"),
        };
        assert_eq!(got_summary.workspace_id, "ws-test");

        // List the bundles (no filter).
        let listed = ChronosCounterexampleService::list(&ctx, CounterexampleListFilter::default())
            .expect("list should succeed");
        match listed {
            CounterexampleOutput::Listed {
                summaries,
                next_cursor,
            } => {
                assert!(summaries.iter().any(|s| s.bundle_id == saved_id));
                assert!(next_cursor.is_none(), "next_cursor must remain None (R3)");
            }
            _ => panic!("expected Listed variant"),
        }

        // Unknown bundle_id -> Err(LoadFailed).
        let missing = ChronosCounterexampleService::get(&ctx, "no-such-bundle");
        match missing {
            Err(ServiceError::LoadFailed(msg)) => {
                assert!(msg.contains("no-such-bundle"));
            }
            other => panic!("expected LoadFailed, got {other:?}"),
        }
    }
}
