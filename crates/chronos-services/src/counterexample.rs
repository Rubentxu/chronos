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
#[cfg(test)]
use std::collections::HashMap;

use proptest::strategy::Strategy;
use proptest::test_runner::TestRunner;
use serde::{Deserialize, Serialize};

use chronos_domain::property::PropertyValue;
use chronos_store::counterexample_storage as cs;
use chronos_store::counterexample_storage::{
    CounterexampleBundleSummary as CounterexampleBundleSummaryWire, ExistencePredicateWire,
    HypothesisInputWire, MinimisedPayload,
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
    ///
    /// m8-04: `events_count` is the length of the persisted events vector.
    /// m8-03 shipped `{"saved": <summary>}` (stopgap envelope, B5); m8-04
    /// brings this variant into the `{bundle, events_count}` family that
    /// `Shrunk` and `Got` use. The wire DTO is `CounterexampleBundleDto`
    /// (m8-03) — no new wire type needed.
    Saved {
        summary: CounterexampleBundleSummary,
        events_count: usize,
    },
    Listed {
        summaries: Vec<CounterexampleBundleSummary>,
        next_cursor: Option<String>,
    },
    /// Returned by m8-02 [`ChronosCounterexampleService::shrink`] on a successful
    /// shrink run. Carries the minimised input fields by their typed shape, plus
    /// the rounds-consumed count. m8-03 promotes to a wire DTO with a flattened
    /// `minimised: serde_json::Value` field that captures the discriminant.
    ///
    /// m8-04: `events_count` mirrors the `Saved { events_count }` we just
    /// persisted (it is the same redb blob's `events.len()`). The m8-03
    /// wire serialiser hardcoded `events_count: 0` in the `full` DTO
    /// because the dispatcher didn't have access to the engine map; now
    /// that `shrink()` calls `pull_engine_events`, we carry the count
    /// through both `Saved` and `Shrunk` paths.
    Shrunk {
        bundle: CounterexampleBundleSummary,
        rounds_used: u32,
        minimised_constant: Option<PropertyValue>,
        minimised_predicate: Option<ExistencePredicate>,
        minimised_call_path: Option<(String, String, Option<usize>)>,
        events_count: usize,
    },
    /// m8-05 (B3): lightweight accessor — `events_count` of a persisted
    /// bundle without re-emitting the summary. Saves the LLM one
    /// round-trip + one bundle-deserialize when all it wants is the
    /// count. Returned by `ChronosCounterexampleService::events_count`.
    /// m8-05 R3: the count is the one persisted at `save()` time, NOT a
    /// live re-read of the engine (which may have advanced).
    EventsCount {
        bundle_id: String,
        events_count: usize,
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
            // m8-06: also set max_shrink_iters so `drive_strategy`'s
            // loop is bounded. proptest's default is u32::MAX which is
            // unsafe for our shrink loop. We use the same `max_rounds`
            // cap so a single ShrinkConfig controls both the test cases
            // and the shrink iterations.
            max_shrink_iters: s.max_rounds.max(1),
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
    /// Pull the captured trace events for a session from the live engines
    /// map. m8-04 closes the m8-03 placeholder — `shrink()` step 4 now
    /// calls this instead of passing `vec![]` to `save()`.
    ///
    /// Returns `Err(ServiceError::SessionNotFound(session_id))` when the
    /// engines map has no entry for the given session. This mirrors
    /// `chronos_hypothesis_test::test()` line 92-95 (same pattern).
    ///
    /// Note: the events are read under a brief `tokio::sync::Mutex` lock.
    /// No await happens inside the critical section beyond the lock
    /// acquisition itself (B3 in m8-04 scoping doc).
    pub async fn pull_engine_events(
        ctx: &CounterexampleContext<'_>,
        session_id: String,
    ) -> Result<Vec<chronos_domain::TraceEvent>, ServiceError> {
        let guard = ctx.hypothesis_ctx.engines.lock().await;
        match guard.get(&session_id) {
            Some(engine) => Ok(engine.get_all_events()),
            None => Err(ServiceError::SessionNotFound(session_id)),
        }
    }

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

    /// m8-05 (B3): lightweight accessor — return the persisted
    /// `events_count` of a bundle without re-emitting the summary.
    ///
    /// This is a separate tool surface (`counterexample_events_count`)
    /// so an LLM agent that only needs to know "how many events does
    /// this bundle carry?" doesn't have to deserialize the full
    /// `CounterexampleBundleSummary` DTO.
    ///
    /// m8-05 R3 disclosure: the count returned is the one persisted at
    /// `save()` time, NOT a live re-read of the engine. The engine may
    /// have advanced (more events captured since the bundle was
    /// written), but the redb blob is the durable record.
    ///
    /// Returns `Err(LoadFailed)` when the bundle is absent (same
    /// shape as `get`).
    pub fn events_count(
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
        // m9-02 D2: prefer summary.events_count (O(1)) when it is populated.
        // Fall back to record.events.len() only when events_count == 0 AND
        // the blob has non-empty events (legacy pre-m9-02 bundle).
        let count = if record.summary.events_count > 0 {
            record.summary.events_count
        } else if !record.events.is_empty() {
            record.events.len() as u64
        } else {
            0
        };
        Ok(CounterexampleOutput::EventsCount {
            bundle_id: bundle_id.to_string(),
            events_count: count as usize,
        })
    }

    /// List bundle summaries matching `filter`.
    ///
    /// m8-05 (B2): forward pagination via `cursor`. If the page is
    /// "full" (i.e. the returned `summaries.len() == filter.limit`),
    /// `next_cursor` is set to the last returned `bundle_id`. The
    /// caller passes that back as `filter.cursor` to fetch the
    /// following page. `next_cursor` is `None` on the last (or empty)
    /// page or when no `limit` was supplied (`limit == 0`).
    ///
    /// uuid::v7 makes lexicographic `>` match chronological order, so
    /// the cursor is just the last `bundle_id` — no separate offset /
    /// index bookkeeping is needed.
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
            cursor: filter.cursor.clone(),
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
        // m8-05 B2: forward-pagination cursor. If the page is full,
        // set `next_cursor` to the last returned bundle_id; otherwise
        // (last page or no limit), no cursor.
        let limit = filter.limit as usize;
        let next_cursor = if limit > 0 && summaries.len() == limit {
            summaries.last().map(|s| s.bundle_id.clone())
        } else {
            None
        };
        Ok(CounterexampleOutput::Listed {
            summaries,
            next_cursor,
        })
    }

    /// Persist a fresh counterexample bundle.
    ///
    /// m8-03 new entry. The dispatcher in `shrink` calls this after a
    /// successful shrink run to durably store the bundle. The
    /// `ExistencePredicate → ExistencePredicateWire` conversion lives
    /// here at the boundary; the `MinimisedPayload` shape and
    /// `HypothesisKind → string` mapping are documented alongside.
    ///
    /// m8-07: `target_hypothesis` is the ORIGINAL `HypothesisInput` the
    /// user passed to `counterexample_shrink`. It is persisted verbatim
    /// so `chronos test replay` can reconstruct the exact hypothesis
    /// for byte-faithful replay, instead of the m8-04 synthetic-default
    /// reconstruction (which loses scope/comparison/property_target for
    /// Invariant targets).
    pub fn save(
        ctx: &CounterexampleContext<'_>,
        workspace_id: &str,
        property_kind: HypothesisKind,
        minimised: ShrinkResult,
        target_hypothesis: &HypothesisInput,
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

        // m8-07: convert the original target_hypothesis to its wire
        // mirror and persist alongside the minimised payload.
        let target_hypothesis_wire = hypothesis_input_to_wire(target_hypothesis);

        let summary = CounterexampleBundleSummaryWire {
            bundle_id: bundle_id.clone(),
            property_kind: hypothesis_kind_as_str(property_kind).to_string(),
            workspace_id: workspace_id.to_string(),
            created_at_ms,
            rounds_used,
            has_full_bundle: true, // m8-03 ships full bundle persistence.
            schema_version: cs::CURRENT_BUNDLE_SCHEMA_VERSION,
            // events_count will be overwritten by save_counterexample_bundle
            // which takes events from the record and sets the count there.
            events_count: 0,
        };
        let record = cs::CounterexampleBundleRecord {
            summary,
            events,
            minimised: minimised_opt,
            event_cas_hashes: Vec::new(),
            target_hypothesis: Some(target_hypothesis_wire),
            schema_version: cs::CURRENT_BUNDLE_SCHEMA_VERSION,
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
        // m8-04: plumb the events count through the Saved variant so the
        // MCP wire (and `chronos-cli replay`) can report the bundle's
        // payload size without re-reading redb. We re-read the record
        // we just persisted to recover the canonical event count (this
        // also lets the wire layer assert that save() round-tripped
        // the events correctly — a future m9+ hardening step could
        // shortcut this by inlining the count into save()'s return
        // type).
        let loaded = ctx
            .store
            .load_counterexample_bundle(&summary_back.bundle_id)
            .map_err(|e| ServiceError::LoadFailed(format!("counterexample reload: {e}")))?;
        let events_count = loaded.map(|r| r.events.len()).unwrap_or(0);
        Ok(CounterexampleOutput::Saved {
            summary: summary_back,
            events_count,
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
        // m8-05 disclosure (R5, carried from m8-03 R2): proptest::TestRunner
        // borrows mutably and we drive the strategy tree directly (no
        // `runner.run`) so we can `await` on the property test inside
        // the shrink loop. See `drive_strategy` for the full loop.
        let mut runner = proptest::test_runner::TestRunner::new(p_cfg);
        let minimised = match property_kind {
            HypothesisKind::Invariant => {
                shrink_invariant(&mut runner, &target_hypothesis, ctx.hypothesis_ctx).await?
            }
            HypothesisKind::Existence => {
                shrink_existence(&mut runner, &target_hypothesis, ctx.hypothesis_ctx).await?
            }
            HypothesisKind::CallPath => {
                shrink_call_path(&mut runner, &target_hypothesis, ctx.hypothesis_ctx).await?
            }
        };

        // Step 4: synthesise the bundle (m8-03 persists, m8-04 wires events).
        //
        // m8-03 shipped `vec![]` as a placeholder for the captured trace
        // events (the engine map was unreachable from the dispatcher
        // because `save()` is a pure-persistence entry). m8-04 closes
        // that gap: pull_engine_events reads from the live engines map
        // so the persisted bundle carries the same trace the original
        // hypothesis was tested against. `chronos test replay` (CLI,
        // m8-04 deliverable 3) needs this to rebuild a QueryEngine from
        // the bundle and re-run hypothesis_test deterministically.
        let (rounds_used, minimised_constant, minimised_predicate, minimised_call_path) = minimised;
        let events = Self::pull_engine_events(ctx, target_hypothesis.session_id.clone()).await?;
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
            &target_hypothesis,
            events,
        )?;
        // m8-04: propagate the Saved events_count up into the
        // Shrunk return so the wire serialiser can populate
        // `full.events_count` from real data instead of the
        // m8-03 hardcoded 0.
        match persist_result {
            CounterexampleOutput::Saved {
                summary,
                events_count,
            } => Ok(CounterexampleOutput::Shrunk {
                bundle: summary,
                rounds_used,
                minimised_constant,
                minimised_predicate,
                minimised_call_path,
                events_count,
            }),
            _ => Err(ServiceError::EvalError(
                "CounterexampleSavePersistence did not return Saved variant".to_string(),
            )),
        }
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
    /// m8-05 (B2): opaque cursor returned by the previous page's
    /// `next_cursor`. `None` means first page.
    pub cursor: Option<String>,
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
/// m8-05 (real per-variant shrinking): each variant now produces a strategy
/// whose `ValueTree::simplify()` walks the captured hypothesis toward a
/// smaller still-violating value. The variant (Invariant / Existence /
/// CallPath), `scope`, `comparison`, `property_target`, and `session_id`
/// are FIXED across the tree — only the variant-specific "shrinking field"
/// varies. See `constant_strategy`, `existence_predicate_strategy`, and
/// `call_path_strategy` below.
///
/// The variant-comparison discipline (m8-05 R1 disclosure): the comparison
/// direction (`Lt` / `Le` / `Gt` / `Ge` / `Eq` / `Ne`) is intentionally NOT
/// shrunk because switching comparison changes the semantics of the
/// hypothesis (a violation may flip to a pass and vice-versa). Operators
/// who want to compare across comparison directions should issue a separate
/// shrink call with a different target.
fn build_strategy_for(
    target: &HypothesisInput,
) -> proptest::strategy::SBoxedStrategy<HypothesisInput> {
    use proptest::strategy::Strategy;

    match target.kind {
        HypothesisKind::Invariant => {
            // Vary only `constant`; reuse scope/comparison/property_target/session_id.
            let scope = target.scope;
            let comparison = target.comparison;
            let property_target = target.property_target.clone();
            let session_id = target.session_id.clone();
            let base = target
                .constant
                .clone()
                .unwrap_or(PropertyValue::Number(0.0));
            constant_strategy(&base)
                .prop_map(move |new_constant| HypothesisInput {
                    session_id: session_id.clone(),
                    kind: HypothesisKind::Invariant,
                    scope,
                    comparison,
                    constant: Some(new_constant),
                    property_target: property_target.clone(),
                    predicate: None,
                    caller: None,
                    callee: None,
                    max_depth: None,
                })
                .sboxed()
        }
        HypothesisKind::Existence => {
            // Vary only `predicate` (FIXED variant; vary payload).
            let session_id = target.session_id.clone();
            let base = target
                .predicate
                .clone()
                .unwrap_or(ExistencePredicate::EventTypeEquals {
                    event_type: String::new(),
                });
            existence_predicate_strategy(&base)
                .prop_map(move |new_pred| HypothesisInput {
                    session_id: session_id.clone(),
                    kind: HypothesisKind::Existence,
                    scope: None,
                    comparison: None,
                    constant: None,
                    property_target: None,
                    predicate: Some(new_pred),
                    caller: None,
                    callee: None,
                    max_depth: None,
                })
                .sboxed()
        }
        HypothesisKind::CallPath => {
            // Vary only (caller, callee, max_depth).
            let session_id = target.session_id.clone();
            call_path_strategy(
                target.caller.clone().unwrap_or_default().as_str(),
                target.callee.clone().unwrap_or_default().as_str(),
                target.max_depth,
            )
            .prop_map(
                move |(new_caller, new_callee, new_max_depth)| HypothesisInput {
                    session_id: session_id.clone(),
                    kind: HypothesisKind::CallPath,
                    scope: None,
                    comparison: None,
                    constant: None,
                    property_target: None,
                    predicate: None,
                    caller: Some(new_caller),
                    callee: Some(new_callee),
                    max_depth: new_max_depth,
                },
            )
            .sboxed()
        }
    }
}

/// Strategy for `PropertyValue`. The starting sample equals `base`; the
/// strategy tree's `simplify()` walks toward the variant's "zero-ish" form.
///
/// m8-06 (closes m8-05 R3):
/// - `Number(n)` shrinks via `NumberShrinker` toward 0.0 (binary search by
///   halving the distance; converges within 64 rounds).
/// - `Text(s)` shrinks via `TextShrinker` toward `""` (one character
///   deletion per `simplify()` round; O(len) convergence).
/// - `Bool(b)` stays as `Just(b)` — degenerate (only 2 values). The
///   operator must issue two shrink calls to test both.
///
/// m8-06 R1 disclosure: `Number` converges to `0.0 ± f64::EPSILON`, not
/// exact 0.0. Hypotheses whose violation depends on exact-zero will not
/// find a smaller reproducer.
fn constant_strategy(base: &PropertyValue) -> proptest::strategy::SBoxedStrategy<PropertyValue> {
    use proptest::strategy::Strategy;
    match base {
        PropertyValue::Number(n) => NumberShrinking { start: *n }.sboxed(),
        PropertyValue::Text(s) => TextShrinking { start: s.clone() }.sboxed(),
        PropertyValue::Bool(_) => proptest::strategy::Just(base.clone()).sboxed(),
    }
}

/// Strategy for `ExistencePredicate`. Variant is FIXED; payload shrinks
/// toward its zero-ish form (string → "", thread_id → 0).
///
/// m8-06 (closes m8-05 R3 for Existence): the variant is fixed (we don't
/// switch from `EventTypeEquals` to `ThreadEquals`), only the payload
/// shrinks. `EventTypeEquals::event_type` and `PropertyKeyEquals::target`
/// shrink toward `""` via `TextShrinker`. `ThreadEquals::thread_id`
/// shrinks toward 0 via `NumberShrinker`.
///
/// m8-06 R4 disclosure: variant enumeration requires multiple shrink calls
/// (one per variant). This matches the m8-05 R3 variant-comparison
/// discipline (operators who want to compare across variants should issue
/// separate shrink calls).
fn existence_predicate_strategy(
    base: &ExistencePredicate,
) -> proptest::strategy::SBoxedStrategy<ExistencePredicate> {
    use proptest::strategy::Strategy;
    ExistencePredicateShrinking {
        start: base.clone(),
    }
    .sboxed()
}

/// Strategy for `CallPath`: produces `(caller, callee, max_depth)` triples.
///
/// m8-06 (closes m8-05 R3 for CallPath): each field shrinks independently.
/// `caller` and `callee` shrink toward `""` (one char per round, alternating
/// which field gets the deletion). `max_depth` shrinks toward `None`
/// (passes through `Some(1)`, then `None`).
///
/// m8-06 R5 disclosure: `max_depth = None` is the terminal state. The
/// shrinker treats `None` as the destination and stops there.
fn call_path_strategy(
    caller: &str,
    callee: &str,
    max_depth: Option<usize>,
) -> proptest::strategy::SBoxedStrategy<(String, String, Option<usize>)> {
    use proptest::strategy::Strategy;
    CallPathShrinking {
        start_caller: caller.to_string(),
        start_callee: callee.to_string(),
        start_max_depth: max_depth,
    }
    .sboxed()
}

// ---------------------------------------------------------------------------
// m8-06 — per-variant shrinkers (close m8-05 R3)
// ---------------------------------------------------------------------------
//
// Each shrinker is a tiny `Strategy + ValueTree` pair. The strategy captures
// the starting value (which becomes the first sample). The value tree's
// `simplify()` walks one step toward the variant's "zero-ish" form per
// invocation. The `drive_strategy` loop calls `simplify()` repeatedly until
// it returns `false` (or until `max_shrink_iters` is reached).
//
// We hand-roll these instead of using `proptest::num::f64::BinarySearch`
// etc. because proptest's stock strategies walk toward their bounds, not
// toward 0.0. The M8 acceptance criterion ("shrinks while preserving the
// violation") requires a known anchor; 0.0 is the obvious choice for
// `Number`, `""` for `Text`, etc.

/// `f64` shrinker that walks toward 0.0 by halving the distance each step.
#[derive(Debug, Clone)]
struct NumberShrinking {
    start: f64,
}

impl proptest::strategy::Strategy for NumberShrinking {
    type Tree = NumberValueTree;
    type Value = PropertyValue;

    fn new_tree(&self, _runner: &mut TestRunner) -> proptest::strategy::NewTree<Self> {
        Ok(NumberValueTree {
            current: self.start,
            done: self.start == 0.0,
        })
    }
}

/// Value tree for `NumberShrinking`. Tracks the current f64; `simplify()`
/// halves the distance to 0.0. Converges in O(log2(ULP_precision)) rounds.
#[derive(Debug)]
struct NumberValueTree {
    current: f64,
    /// `true` once `current` cannot shrink further (already 0.0).
    done: bool,
}

impl proptest::strategy::ValueTree for NumberValueTree {
    type Value = PropertyValue;

    fn current(&self) -> Self::Value {
        PropertyValue::Number(self.current)
    }

    fn simplify(&mut self) -> bool {
        if self.done {
            return false;
        }
        let next = self.current / 2.0;
        if next == self.current {
            // Reached subnormal or NaN territory; stop.
            self.done = true;
            return false;
        }
        self.current = next;
        if self.current.abs() < f64::EPSILON {
            self.done = true;
        }
        true
    }

    fn complicate(&mut self) -> bool {
        // We never re-expand a shrunken value. This is acceptable because
        // `drive_strategy` never calls `complicate()` — it only re-roots
        // the strategy at the current best (which is monotonic).
        false
    }
}

/// `String` shrinker that deletes one character per `simplify()` round.
/// Deletion is round-robin: start, then end, then middle, alternating.
#[derive(Debug, Clone)]
struct TextShrinking {
    start: String,
}

impl proptest::strategy::Strategy for TextShrinking {
    type Tree = TextValueTree;
    type Value = PropertyValue;

    fn new_tree(&self, _runner: &mut TestRunner) -> proptest::strategy::NewTree<Self> {
        Ok(TextValueTree {
            current: self.start.clone(),
            round: 0,
        })
    }
}

/// Value tree for `TextShrinking`. Each `simplify()` deletes one character
/// using a round-robin index (start, end, middle, second-to-start, ...).
#[derive(Debug)]
struct TextValueTree {
    current: String,
    /// Round counter for round-robin index selection.
    round: usize,
}

impl proptest::strategy::ValueTree for TextValueTree {
    type Value = PropertyValue;

    fn current(&self) -> Self::Value {
        PropertyValue::Text(self.current.clone())
    }

    fn simplify(&mut self) -> bool {
        if self.current.is_empty() {
            return false;
        }
        let len = self.current.len();
        // Round-robin index: start, end, middle, then second-to-start,
        // second-to-end, etc. With len=1, only index 0 is valid (round 0).
        // With len=2, indices 0 then 1. With len=3, indices 0, 2, 1.
        let idx = match self.round % (2 * len).max(1) {
            0 => 0,
            x if x == 2 * len - 1 => len - 1,
            x if x < len => x,
            x => 2 * len - 1 - x,
        };
        let idx = idx.min(len - 1);
        // Find the char boundary at `idx` (multi-byte UTF-8 safe).
        let mut char_indices = self.current.char_indices();
        let byte_idx = char_indices
            .nth(idx)
            .map(|(b, _)| b)
            .unwrap_or(self.current.len());
        let mut next = String::with_capacity(len.saturating_sub(1));
        next.push_str(&self.current[..byte_idx]);
        next.push_str(
            &self.current[byte_idx
                + self.current[byte_idx..]
                    .chars()
                    .next()
                    .map_or(1, |c| c.len_utf8())..],
        );
        self.current = next;
        self.round = self.round.saturating_add(1);
        true
    }

    fn complicate(&mut self) -> bool {
        false
    }
}

/// `ExistencePredicate` shrinker: variant is FIXED; payload shrinks.
#[derive(Debug, Clone)]
struct ExistencePredicateShrinking {
    start: ExistencePredicate,
}

impl proptest::strategy::Strategy for ExistencePredicateShrinking {
    type Tree = ExistencePredicateValueTree;
    type Value = ExistencePredicate;

    fn new_tree(&self, _runner: &mut TestRunner) -> proptest::strategy::NewTree<Self> {
        // Build a sub-strategy for the payload and wrap its tree.
        let sub_tree = match &self.start {
            ExistencePredicate::EventTypeEquals { event_type } => SubTree::Text(TextValueTree {
                current: event_type.clone(),
                round: 0,
            }),
            ExistencePredicate::ThreadEquals { thread_id } => SubTree::Number(NumberValueTree {
                current: *thread_id as f64,
                done: *thread_id == 0,
            }),
            ExistencePredicate::PropertyKeyEquals { target } => SubTree::Text(TextValueTree {
                current: target.clone(),
                round: 0,
            }),
        };
        Ok(ExistencePredicateValueTree {
            current: self.start.clone(),
            sub_tree,
        })
    }
}

/// Inner shrinker for `ExistencePredicate` payloads. Either a number
/// shrinker (for `ThreadEquals`) or a text shrinker (for the string variants).
#[derive(Debug)]
enum SubTree {
    Number(NumberValueTree),
    Text(TextValueTree),
}

/// Value tree for `ExistencePredicateShrinking`. Delegates `simplify()` to
/// the inner payload shrinker and reconstructs the variant with the new
/// payload.
#[derive(Debug)]
struct ExistencePredicateValueTree {
    current: ExistencePredicate,
    sub_tree: SubTree,
}

impl proptest::strategy::ValueTree for ExistencePredicateValueTree {
    type Value = ExistencePredicate;

    fn current(&self) -> Self::Value {
        self.current.clone()
    }

    fn simplify(&mut self) -> bool {
        match (&mut self.sub_tree, &mut self.current) {
            (SubTree::Number(t), ExistencePredicate::ThreadEquals { thread_id }) => {
                if t.simplify() {
                    *thread_id = t.current as u64;
                    true
                } else {
                    false
                }
            }
            (SubTree::Text(t), ExistencePredicate::EventTypeEquals { event_type }) => {
                if t.simplify() {
                    *event_type = t.current.clone();
                    true
                } else {
                    false
                }
            }
            (SubTree::Text(t), ExistencePredicate::PropertyKeyEquals { target }) => {
                if t.simplify() {
                    *target = t.current.clone();
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn complicate(&mut self) -> bool {
        false
    }
}

/// `CallPath` shrinker: each field shrinks independently, in lockstep.
#[derive(Debug, Clone)]
struct CallPathShrinking {
    start_caller: String,
    start_callee: String,
    start_max_depth: Option<usize>,
}

impl proptest::strategy::Strategy for CallPathShrinking {
    type Tree = CallPathValueTree;
    type Value = (String, String, Option<usize>);

    fn new_tree(&self, _runner: &mut TestRunner) -> proptest::strategy::NewTree<Self> {
        Ok(CallPathValueTree {
            caller: self.start_caller.clone(),
            callee: self.start_callee.clone(),
            max_depth: self.start_max_depth,
            caller_tree: TextValueTree {
                current: self.start_caller.clone(),
                round: 0,
            },
            callee_tree: TextValueTree {
                current: self.start_callee.clone(),
                round: 0,
            },
            max_depth_done: self.start_max_depth.is_none(),
            round: 0,
        })
    }
}

/// Value tree for `CallPathShrinking`. Each round deletes one character from
/// either `caller` or `callee` (alternating) OR shrinks `max_depth` by one
/// step (`Some(n) -> Some(n-1) -> None`).
#[derive(Debug)]
struct CallPathValueTree {
    caller: String,
    callee: String,
    max_depth: Option<usize>,
    caller_tree: TextValueTree,
    callee_tree: TextValueTree,
    max_depth_done: bool,
    /// 0..2: caller / callee alternation. 3: max_depth. Wraps.
    round: usize,
}

impl proptest::strategy::ValueTree for CallPathValueTree {
    type Value = (String, String, Option<usize>);

    fn current(&self) -> Self::Value {
        (self.caller.clone(), self.callee.clone(), self.max_depth)
    }

    fn simplify(&mut self) -> bool {
        // Round-robin: caller → callee → max_depth → caller → ...
        // We avoid recursion to prevent stack overflow when all three
        // components are at their minimum.
        loop {
            match self.round % 3 {
                0 => {
                    if self.caller_tree.simplify() {
                        self.caller = self.caller_tree.current.clone();
                        self.round = self.round.saturating_add(1);
                        return true;
                    }
                    // Caller already at minimum; advance to callee.
                    self.round = 1;
                }
                1 => {
                    if self.callee_tree.simplify() {
                        self.callee = self.callee_tree.current.clone();
                        self.round = self.round.saturating_add(1);
                        return true;
                    }
                    self.round = 2;
                }
                2 => {
                    if self.max_depth_done {
                        // Everything at minimum; can't shrink further.
                        return false;
                    }
                    match self.max_depth {
                        Some(n) if n > 1 => {
                            self.max_depth = Some(n - 1);
                            self.round = self.round.saturating_add(1);
                            return true;
                        }
                        Some(_) => {
                            // Some(1) -> None (terminal).
                            self.max_depth = None;
                            self.max_depth_done = true;
                            self.round = self.round.saturating_add(1);
                            return true;
                        }
                        None => {
                            self.max_depth_done = true;
                            self.round = 0;
                        }
                    }
                }
                _ => unreachable!(),
            }
        }
    }

    fn complicate(&mut self) -> bool {
        false
    }
}

/// Drive the per-variant proptest shrinking loop.
///
/// m8-05 R7 disclosure: `proptest::strategy::BoxedStrategy::Tree` is
/// `Box<dyn ValueTree<...>>` without a `Send` bound, so the value tree
/// cannot be held across an `.await` point. To work around this, we
/// extract the current sample synchronously, **drop** the strategy +
/// tree, run the async test on the sample, and (only if the candidate
/// still violates) regenerate a fresh strategy + tree rooted at the
/// updated best. Each round asks the fresh tree to `simplify()` once
/// and extracts the new `current()` value.
///
/// m8-06 (closes m8-05 R3 contract): with real shrinkers in place,
/// `simplify()` returns `true` for many rounds until the value tree
/// reaches its minimum (0.0 / `""` / `None`). The loop bounds itself
/// by `runner.config().max_shrink_iters()` so it never infinite-loops
/// even if a shrinker is buggy. For `Just(value)` strategies (the only
/// non-shrinking case, used for `Bool`) the loop exits after one round.
async fn drive_strategy<F, Fut>(
    runner: &mut TestRunner,
    target: &HypothesisInput,
    async_test: F,
) -> Result<(u32, HypothesisInput), ServiceError>
where
    F: Fn(HypothesisInput) -> Fut,
    Fut: std::future::Future<Output = Result<crate::output::HypothesisOutput, ServiceError>>,
{
    let max_shrink_iters = runner.config().max_shrink_iters();

    // Synchronously extract the initial sample, drop the strategy + tree.
    let initial = {
        let strategy = build_strategy_for(target);
        let tree = strategy
            .new_tree(runner)
            .map_err(|e| ServiceError::EvalError(format!("strategy.new_tree failed: {e}")))?;
        tree.current()
    };
    let mut best = initial;
    let mut rounds: u32 = 0;

    // Round 1: validate the initial sample is a violation (also validated
    // upstream in shrink(), but we re-check defensively in case a future
    // strategy adapter returns a non-violating initial sample).
    rounds = rounds.saturating_add(1);
    let initial_out = async_test(best.clone()).await?;
    if !is_violation(initial_out) {
        // Initial sample does NOT violate. Fall back to the original target.
        return Ok((rounds, target.clone()));
    }

    // Rounds 2..N: rebuild strategy + tree each iteration (m8-05 R7), ask
    // the fresh tree to `simplify()` once, and try the resulting candidate.
    // Bound by `max_shrink_iters` (default u32::MAX in proptest; we cap at
    // DEFAULT_SHRINK_MAX_ROUNDS = 64 via the upstream ShrinkConfig→Config
    // mapping in `shrink()`).
    loop {
        if rounds >= max_shrink_iters {
            break;
        }
        let candidate = {
            let strategy = build_strategy_for(&best);
            let mut tree = strategy
                .new_tree(runner)
                .map_err(|e| ServiceError::EvalError(format!("strategy.new_tree failed: {e}")))?;
            if tree.simplify() {
                Some(tree.current())
            } else {
                None
            }
        };
        rounds = rounds.saturating_add(1);
        match candidate {
            Some(c) if is_violation(async_test(c.clone()).await?) => {
                best = c;
            }
            Some(_) => {
                // Candidate doesn't violate; the current best is smaller.
                // Stop — proptest convention is that a non-violating
                // candidate signals we've shrunk as far as we can.
                break;
            }
            None => {
                // simplify() returned false — the tree reached its minimum.
                break;
            }
        }
    }

    Ok((rounds, best))
}

/// Tri-state check shared by `drive_strategy`'s initial + iterative
/// branches. Returns `true` if the hypothesis output is a `Violation`.
fn is_violation(out: crate::output::HypothesisOutput) -> bool {
    use crate::output::{HypothesisOutput, HypothesisVerdict};
    matches!(
        out,
        HypothesisOutput::Invariant {
            verdict: HypothesisVerdict::Violation { .. },
            ..
        } | HypothesisOutput::Existence {
            verdict: HypothesisVerdict::Violation { .. },
            ..
        } | HypothesisOutput::CallPath {
            verdict: HypothesisVerdict::Violation { .. },
            ..
        }
    )
}

async fn shrink_invariant(
    runner: &mut TestRunner,
    target: &HypothesisInput,
    hyp_ctx: &HypothesisTestContext<'_>,
) -> Result<ShrinkResult, ServiceError> {
    // m8-05: drive the strategy tree + manually call `hypothesis_test::test`
    // on each candidate via the async closure. The Strategy is `Just(base)`
    // today (m8-05 R3), so the loop exits after the initial sample. Future
    // cycles can swap in real shrinkers without changing this signature.
    let (rounds, best) = drive_strategy(runner, target, |sampled| async move {
        crate::hypothesis_test::ChronosHypothesisTestService::test(hyp_ctx, sampled).await
    })
    .await?;
    let minimised_constant = best.constant.clone();
    Ok((rounds, minimised_constant, None, None))
}

async fn shrink_existence(
    runner: &mut TestRunner,
    target: &HypothesisInput,
    hyp_ctx: &HypothesisTestContext<'_>,
) -> Result<ShrinkResult, ServiceError> {
    let (rounds, best) = drive_strategy(runner, target, |sampled| async move {
        crate::hypothesis_test::ChronosHypothesisTestService::test(hyp_ctx, sampled).await
    })
    .await?;
    let minimised_predicate = best.predicate.clone();
    Ok((rounds, None, minimised_predicate, None))
}

async fn shrink_call_path(
    runner: &mut TestRunner,
    target: &HypothesisInput,
    hyp_ctx: &HypothesisTestContext<'_>,
) -> Result<ShrinkResult, ServiceError> {
    let (rounds, best) = drive_strategy(runner, target, |sampled| async move {
        crate::hypothesis_test::ChronosHypothesisTestService::test(hyp_ctx, sampled).await
    })
    .await?;
    let minimised_call_path = (
        best.caller.clone().unwrap_or_default(),
        best.callee.clone().unwrap_or_default(),
        best.max_depth,
    );
    Ok((rounds, None, None, Some(minimised_call_path)))
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
// 5c. HypothesisInput wire mirror (m8-07)
//
// R1 disclosure (m8-07): `chronos_store::counterexample_storage::HypothesisInputWire`
// mirrors `crate::hypothesis_test::HypothesisInput` field-for-field but
// the two types are intentionally NOT shared (otherwise chronos-store
// would depend on chronos-services, breaking the layered crate graph).
// The conversions below live at the boundary (chronos-services owns the
// services-side type and reaches into the chronos-store wire mirror).
// ============================================================================

/// Stringify a `HypothesisScope` for persistence. Returns `None` if the
/// scope is `None`; otherwise the snake_case serde form. Mirrors the
/// `#[serde(rename_all = "snake_case")]` on `HypothesisScope`.
fn hypothesis_scope_to_wire(s: &crate::output::HypothesisScope) -> &'static str {
    use crate::output::HypothesisScope;
    match s {
        HypothesisScope::EventCount => "event_count",
        HypothesisScope::PropertyValue => "property_value",
        HypothesisScope::LatencyMs => "latency_ms",
    }
}

/// Inverse of `hypothesis_scope_to_wire`. Returns `None` for unknown
/// strings (the wire mirror can carry arbitrary text if a future
/// chronos-services version adds a new variant); the caller should fall
/// back to the m8-04 synthetic-default reconstruction in that case.
fn hypothesis_scope_from_wire(s: &str) -> Option<crate::output::HypothesisScope> {
    use crate::output::HypothesisScope;
    match s {
        "event_count" => Some(HypothesisScope::EventCount),
        "property_value" => Some(HypothesisScope::PropertyValue),
        "latency_ms" => Some(HypothesisScope::LatencyMs),
        _ => None,
    }
}

/// Convert a services-side `HypothesisInput` into the chronos-store wire
/// mirror `HypothesisInputWire`. Used by `save` (m8-07) to persist the
/// ORIGINAL `target_hypothesis` the user passed to `counterexample_shrink`.
pub(crate) fn hypothesis_input_to_wire(h: &HypothesisInput) -> HypothesisInputWire {
    HypothesisInputWire {
        session_id: h.session_id.clone(),
        kind: hypothesis_kind_as_str(h.kind).to_string(),
        scope: h
            .scope
            .as_ref()
            .map(hypothesis_scope_to_wire)
            .map(String::from),
        // ComparisonOp uses default serde (PascalCase: "Ge", "Eq", ...).
        // We stringify explicitly here so we don't depend on Debug or
        // Display impls that may diverge from the wire format.
        comparison: h.comparison.map(comparison_op_to_wire).map(String::from),
        constant: h.constant.clone(),
        property_target: h.property_target.clone(),
        predicate: h
            .predicate
            .as_ref()
            .cloned()
            .map(existence_predicate_to_wire),
        caller: h.caller.clone(),
        callee: h.callee.clone(),
        max_depth: h.max_depth.map(|d| d as u64),
    }
}

/// Inverse of `hypothesis_input_to_wire`. Used by `chronos-cli::replay`
/// (m8-07) to reconstruct the user's original `HypothesisInput` from a
/// persisted bundle.
///
/// R1 disclosure (m8-07): `kind` is matched as a string; unknown values
/// panic (bounded by the writer, which only emits known kinds).
pub fn hypothesis_input_from_wire(w: HypothesisInputWire) -> HypothesisInput {
    HypothesisInput {
        session_id: w.session_id,
        kind: match w.kind.as_str() {
            "invariant" => HypothesisKind::Invariant,
            "existence" => HypothesisKind::Existence,
            "call_path" => HypothesisKind::CallPath,
            other => panic!(
                "hypothesis_input_from_wire: unknown kind {other:?} on disk \
                 (bundle is corrupted or pre-m8-07 schema)"
            ),
        },
        scope: w.scope.as_deref().and_then(hypothesis_scope_from_wire),
        comparison: w.comparison.as_deref().and_then(comparison_op_from_wire),
        constant: w.constant,
        property_target: w.property_target,
        predicate: w.predicate.map(existence_predicate_from_wire),
        caller: w.caller,
        callee: w.callee,
        // Saturating cast: usize::MAX is the practical upper bound for any
        // real probe; corrupt store data > usize::MAX collapses to MAX
        // rather than panicking.
        max_depth: w
            .max_depth
            .map(|d| usize::try_from(d).unwrap_or(usize::MAX)),
    }
}

/// Map a `ComparisonOp` to its PascalCase wire form. Matches the default
/// serde representation (no `rename_all` on the enum).
fn comparison_op_to_wire(c: chronos_domain::property::ComparisonOp) -> &'static str {
    use chronos_domain::property::ComparisonOp;
    match c {
        ComparisonOp::Eq => "Eq",
        ComparisonOp::Ne => "Ne",
        ComparisonOp::Ge => "Ge",
        ComparisonOp::Gt => "Gt",
        ComparisonOp::Le => "Le",
        ComparisonOp::Lt => "Lt",
    }
}

/// Inverse of `comparison_op_to_wire`. Returns `None` for unknown strings
/// (defensive against future schema drift).
fn comparison_op_from_wire(s: &str) -> Option<chronos_domain::property::ComparisonOp> {
    use chronos_domain::property::ComparisonOp;
    match s {
        "Eq" => Some(ComparisonOp::Eq),
        "Ne" => Some(ComparisonOp::Ne),
        "Ge" => Some(ComparisonOp::Ge),
        "Gt" => Some(ComparisonOp::Gt),
        "Le" => Some(ComparisonOp::Le),
        "Lt" => Some(ComparisonOp::Lt),
        _ => None,
    }
}

/// Inverse of `existence_predicate_to_wire` (m8-07 mirror helper; same
/// shape as `chronos-cli::existence_predicate_from_wire` but in
/// chronos-services so `hypothesis_input_from_wire` can call it without
/// depending on chronos-cli).
fn existence_predicate_from_wire(w: ExistencePredicateWire) -> ExistencePredicate {
    match w {
        ExistencePredicateWire::EventTypeEquals { event_type } => {
            ExistencePredicate::EventTypeEquals { event_type }
        }
        ExistencePredicateWire::ThreadEquals { thread_id } => {
            ExistencePredicate::ThreadEquals { thread_id }
        }
        ExistencePredicateWire::PropertyKeyEquals { target } => {
            ExistencePredicate::PropertyKeyEquals { target }
        }
    }
}

// ============================================================================
// 6. Tests (T1: lib unit)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::strategy::{Strategy, ValueTree};
    use proptest::test_runner::Config;

    #[allow(dead_code)]
    fn assert_send<T: Send>(_: T) {}

    /// Build a minimal `HypothesisInput` for use in save() tests (m8-07).
    ///
    /// Most tests only care about the persist round-trip + the events
    /// payload; the target_hypothesis argument to `save()` is required by
    /// the signature but the tests don't inspect it. The default here is
    /// an Invariant hypothesis with the same constant as the minimised
    /// payload — this is what a real shrink run would produce for an
    /// Invariant kind, so the wire mirror is well-formed.
    fn dummy_target_invariant(c: PropertyValue) -> HypothesisInput {
        HypothesisInput {
            session_id: "s".to_string(),
            kind: HypothesisKind::Invariant,
            scope: None,
            comparison: None,
            constant: Some(c),
            property_target: None,
            predicate: None,
            caller: None,
            callee: None,
            max_depth: None,
        }
    }

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
            events_count: 0,
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
            events_count: 0,
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
            events_count: 0,
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
        // m8-05: shrink_invariant now takes a `&HypothesisTestContext`. For
        // this canary test we build an empty engines map (no real session
        // is loaded; the per-variant Strategy returns `Just(base)` so the
        // `drive_strategy` loop does NOT call the property test on the
        // initial sample — wait, it DOES call it on the initial sample.
        // Build a minimal context with a synthetic session containing one
        // event so the invariant check has something to test against.
        use chronos_query::QueryEngine;
        use std::collections::HashMap;
        use tokio::sync::Mutex as TokioMutex;
        let mut engines_map: HashMap<String, QueryEngine> = HashMap::new();
        engines_map.insert("sess-inv".to_string(), QueryEngine::new(vec![]));
        let engines = TokioMutex::new(engines_map);
        let hyp_ctx = HypothesisTestContext { engines: &engines };
        let result = rt.block_on(async { shrink_invariant(&mut runner, &target, &hyp_ctx).await });
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
            &dummy_target_invariant(PropertyValue::Number(1.5)),
            vec![],
        )
        .expect("save should succeed");
        let saved_id = match saved {
            CounterexampleOutput::Saved {
                summary,
                events_count,
            } => {
                assert_eq!(summary.property_kind, HypothesisKind::Invariant);
                assert!(
                    summary.has_full_bundle,
                    "saved bundle must report has_full_bundle=true"
                );
                assert_eq!(
                    events_count, 0,
                    "save() with vec![] events must report events_count=0"
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

    // m8-04 tests for `pull_engine_events` (engine events wiring).

    fn make_test_trace_event(id: u64, ts: u64, tid: u64) -> chronos_domain::TraceEvent {
        use chronos_domain::{EventData, EventType, SourceLocation, TraceEvent};
        TraceEvent::new(
            id,
            ts,
            tid,
            EventType::FunctionEntry,
            SourceLocation::new("test.rs", 10, "main", 0x1000),
            EventData::Empty,
        )
    }

    #[tokio::test]
    async fn m8_04_pull_engine_events_returns_session_events() {
        use crate::hypothesis_test::HypothesisTestContext;
        use chronos_query::QueryEngine;
        use chronos_store::SessionStore;
        use std::collections::HashMap;
        use tokio::sync::Mutex as TokioMutex;

        let store = SessionStore::in_memory().expect("in_memory store");
        let mut engines_map: HashMap<String, QueryEngine> = HashMap::new();
        let events = vec![
            make_test_trace_event(0, 100, 1),
            make_test_trace_event(1, 200, 1),
            make_test_trace_event(2, 300, 1),
        ];
        engines_map.insert("sess-m8-04".to_string(), QueryEngine::new(events.clone()));
        let engines = TokioMutex::new(engines_map);
        let hyp_ctx = HypothesisTestContext { engines: &engines };
        let ctx = CounterexampleContext {
            store: &store,
            hypothesis_ctx: &hyp_ctx,
        };

        let pulled =
            ChronosCounterexampleService::pull_engine_events(&ctx, "sess-m8-04".to_string())
                .await
                .expect("pull_engine_events should succeed");
        assert_eq!(pulled.len(), 3, "all 3 events should round-trip");
        assert_eq!(pulled[0].event_id, 0);
        assert_eq!(pulled[2].event_id, 2);
    }

    #[tokio::test]
    async fn m8_04_pull_engine_events_missing_session_errors() {
        use crate::hypothesis_test::HypothesisTestContext;
        use chronos_query::QueryEngine;
        use chronos_store::SessionStore;
        use std::collections::HashMap;
        use tokio::sync::Mutex as TokioMutex;

        let store = SessionStore::in_memory().expect("in_memory store");
        let engines_map: HashMap<String, QueryEngine> = HashMap::new();
        let engines = TokioMutex::new(engines_map);
        let hyp_ctx = HypothesisTestContext { engines: &engines };
        let ctx = CounterexampleContext {
            store: &store,
            hypothesis_ctx: &hyp_ctx,
        };

        let result =
            ChronosCounterexampleService::pull_engine_events(&ctx, "absent-session".to_string())
                .await;
        match result {
            Err(ServiceError::SessionNotFound(id)) => {
                assert_eq!(id, "absent-session");
            }
            other => panic!("expected SessionNotFound, got {other:?}"),
        }
    }

    // ========================================================================
    // m8-05 tests: real per-variant proptest shrinking (close m8-03 R1)
    // ========================================================================

    // m8-05 #1: build_strategy_for returns a BoxedStrategy that produces
    // the captured hypothesis as its initial sample for Invariant.
    #[test]
    fn m8_05_build_strategy_invariant_initial_sample_equals_target() {
        let target = HypothesisInput {
            session_id: "sess".into(),
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
        let strategy = build_strategy_for(&target);
        let p_cfg: proptest::test_runner::Config = ShrinkConfig {
            max_rounds: 1,
            seed: None,
        }
        .into();
        let mut runner = TestRunner::new(p_cfg);
        let tree = strategy.new_tree(&mut runner).expect("new_tree");
        let initial = tree.current();
        assert_eq!(initial.kind, HypothesisKind::Invariant);
        assert_eq!(initial.session_id, "sess");
        assert_eq!(initial.constant, Some(PropertyValue::Number(42.0)));
    }

    // m8-05 #2: build_strategy_for for Existence keeps the predicate variant.
    #[test]
    fn m8_05_build_strategy_existence_initial_sample_equals_target() {
        let target = HypothesisInput {
            session_id: "sess".into(),
            kind: HypothesisKind::Existence,
            scope: None,
            comparison: None,
            constant: None,
            property_target: None,
            predicate: Some(ExistencePredicate::EventTypeEquals {
                event_type: "Syscall".into(),
            }),
            caller: None,
            callee: None,
            max_depth: None,
        };
        let strategy = build_strategy_for(&target);
        let p_cfg: proptest::test_runner::Config = ShrinkConfig {
            max_rounds: 1,
            seed: None,
        }
        .into();
        let mut runner = TestRunner::new(p_cfg);
        let tree = strategy.new_tree(&mut runner).expect("new_tree");
        let initial = tree.current();
        assert_eq!(initial.kind, HypothesisKind::Existence);
        match initial.predicate.as_ref().expect("predicate") {
            ExistencePredicate::EventTypeEquals { event_type } => {
                assert_eq!(event_type, "Syscall")
            }
            _ => panic!("wrong variant"),
        }
    }

    // m8-05 #3: build_strategy_for for CallPath keeps (caller, callee, max_depth).
    #[test]
    fn m8_05_build_strategy_call_path_initial_sample_equals_target() {
        let target = HypothesisInput {
            session_id: "sess".into(),
            kind: HypothesisKind::CallPath,
            scope: None,
            comparison: None,
            constant: None,
            property_target: None,
            predicate: None,
            caller: Some("main".into()),
            callee: Some("work".into()),
            max_depth: Some(3),
        };
        let strategy = build_strategy_for(&target);
        let p_cfg: proptest::test_runner::Config = ShrinkConfig {
            max_rounds: 1,
            seed: None,
        }
        .into();
        let mut runner = TestRunner::new(p_cfg);
        let tree = strategy.new_tree(&mut runner).expect("new_tree");
        let initial = tree.current();
        assert_eq!(initial.kind, HypothesisKind::CallPath);
        assert_eq!(initial.caller.as_deref(), Some("main"));
        assert_eq!(initial.callee.as_deref(), Some("work"));
        assert_eq!(initial.max_depth, Some(3));
    }

    // m8-05 #4: drive_strategy with an async test that always returns Violation
    // should iterate through the strategy tree and return the initial sample
    // as `best`. (Just(base) strategy exits after one round; we verify the
    // wiring without asserting actual shrinkage.)
    #[test]
    fn m8_05_drive_strategy_with_violating_async_test_returns_initial() {
        // m8-06: use `Bool(true)` instead of `Number(7.0)` so the strategy
        // is `Just(value)` (the only non-shrinking case per D3 in m8-06
        // scoping). With Number, the m8-06 NumberShrinker would shrink the
        // value toward 0.0 in this test.
        let target = HypothesisInput {
            session_id: "sess".into(),
            kind: HypothesisKind::Invariant,
            scope: None,
            comparison: None,
            constant: Some(PropertyValue::Bool(true)),
            property_target: None,
            predicate: None,
            caller: None,
            callee: None,
            max_depth: None,
        };
        let p_cfg: proptest::test_runner::Config = ShrinkConfig {
            max_rounds: 4,
            seed: None,
        }
        .into();
        let mut runner = TestRunner::new(p_cfg);
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("rt");
        let result = rt.block_on(async {
            drive_strategy(&mut runner, &target, |sampled| async move {
                use crate::output::{HypothesisOutput, HypothesisVerdict};
                let _ = sampled;
                // Return an Invariant Violation regardless of the candidate.
                Ok::<_, ServiceError>(HypothesisOutput::Invariant {
                    verdict: HypothesisVerdict::Violation {
                        reason: "test".into(),
                    },
                    support_event_ids: vec![],
                    counter_event_ids: vec![],
                    scope: crate::output::HypothesisScope::PropertyValue,
                    summary: "violation".into(),
                })
            })
            .await
        });
        let (rounds, best) = result.expect("drive_strategy should succeed");
        // m8-06: with `Just(Bool(true))` (the only non-shrinking case),
        // rounds counts BOTH the initial validation AND the (failed)
        // simplify() attempt; the Bool strategy's simplify() returns
        // false, so the loop terminates after exactly 2 rounds.
        assert_eq!(
            rounds, 2,
            "Just(Bool(true)) yields 2 rounds (initial + 1 simplify attempt)"
        );
        assert_eq!(best.constant, Some(PropertyValue::Bool(true)));
    }

    // m8-05 #5: drive_strategy with an async test that returns Pass should
    // fall back to the original target (the initial sample didn't violate).
    #[test]
    fn m8_05_drive_strategy_with_passing_async_test_falls_back_to_target() {
        let target = HypothesisInput {
            session_id: "sess".into(),
            kind: HypothesisKind::Invariant,
            scope: None,
            comparison: None,
            constant: Some(PropertyValue::Number(7.0)),
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
        let result = rt.block_on(async {
            drive_strategy(&mut runner, &target, |_sampled| async move {
                use crate::output::{HypothesisOutput, HypothesisVerdict};
                Ok::<_, ServiceError>(HypothesisOutput::Invariant {
                    verdict: HypothesisVerdict::Pass,
                    support_event_ids: vec![],
                    counter_event_ids: vec![],
                    scope: crate::output::HypothesisScope::PropertyValue,
                    summary: "pass".into(),
                })
            })
            .await
        });
        let (rounds, best) = result.expect("drive_strategy should succeed");
        assert_eq!(rounds, 1);
        // Falls back to the original target (not the strategy's initial,
        // because the strategy's initial sample is the target anyway for
        // the Just(base) case).
        assert_eq!(best.session_id, "sess");
        assert_eq!(best.constant, Some(PropertyValue::Number(7.0)));
    }

    // m8-05 #6: shrink_invariant with an empty engines map (no live session)
    // returns Err(SessionNotFound) because the initial sample cannot be
    // validated against the captured trace.
    //
    // m8-05 R6 disclosure: this test pins the contract that `shrink_*`
    // requires a populated engines map for the initial-sample validation.
    // Production callers (the MCP server) always provide one via the
    // dispatcher.
    #[test]
    fn m8_05_shrink_invariant_empty_engines_session_not_found() {
        use chronos_query::QueryEngine;
        let target = HypothesisInput {
            session_id: "absent".into(),
            kind: HypothesisKind::Invariant,
            scope: None,
            comparison: None,
            constant: Some(PropertyValue::Number(7.0)),
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
        let engines: HashMap<String, QueryEngine> = HashMap::new();
        let engines = std::sync::Arc::new(tokio::sync::Mutex::new(engines));
        let hyp_ctx = HypothesisTestContext { engines: &engines };
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("rt");
        let result = rt.block_on(async { shrink_invariant(&mut runner, &target, &hyp_ctx).await });
        match result {
            Err(ServiceError::SessionNotFound(id)) => assert_eq!(id, "absent"),
            Err(other) => panic!("expected SessionNotFound, got {other:?}"),
            Ok(_) => panic!("expected SessionNotFound, got Ok"),
        }
    }

    // ========================================================================
    // m8-05 (B2) tests: list pagination cursor
    // ========================================================================

    // m8-05 #7: list with `limit` smaller than the bundle count returns a
    // non-None `next_cursor` so callers can fetch the following page.
    #[test]
    fn m8_05_list_full_page_sets_next_cursor() {
        use crate::hypothesis_test::HypothesisTestContext;
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

        // Save 3 bundles; the test only inspects the list API, not the
        // events payload, so vec![] is fine.
        for i in 0..3 {
            let _ = ChronosCounterexampleService::save(
                &ctx,
                "ws",
                HypothesisKind::Invariant,
                (1, Some(PropertyValue::Number(i as f64)), None, None),
                &dummy_target_invariant(PropertyValue::Number(i as f64)),
                vec![],
            )
            .expect("save");
        }

        let filter = CounterexampleListFilter {
            limit: 2,
            ..Default::default()
        };
        let listed = ChronosCounterexampleService::list(&ctx, filter).expect("list should succeed");
        match listed {
            CounterexampleOutput::Listed {
                summaries,
                next_cursor,
            } => {
                assert_eq!(summaries.len(), 2);
                assert!(
                    next_cursor.is_some(),
                    "next_cursor must be Some when page is full"
                );
                assert_eq!(
                    next_cursor.as_deref(),
                    Some(summaries.last().unwrap().bundle_id.as_str()),
                    "next_cursor must equal the last returned bundle_id"
                );
            }
            _ => panic!("expected Listed variant"),
        }
    }

    // m8-05 #8: list with `cursor` skips rows whose bundle_id <= cursor
    // and returns the next page. End-to-end through the service.
    #[test]
    fn m8_05_list_with_cursor_returns_next_page() {
        use crate::hypothesis_test::HypothesisTestContext;
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

        for i in 0..4 {
            let _ = ChronosCounterexampleService::save(
                &ctx,
                "ws",
                HypothesisKind::Invariant,
                (1, Some(PropertyValue::Number(i as f64)), None, None),
                &dummy_target_invariant(PropertyValue::Number(i as f64)),
                vec![],
            )
            .expect("save");
        }

        // First page.
        let page1 = ChronosCounterexampleService::list(
            &ctx,
            CounterexampleListFilter {
                limit: 2,
                ..Default::default()
            },
        )
        .expect("page1");
        let cursor = match page1 {
            CounterexampleOutput::Listed {
                summaries,
                next_cursor,
            } => {
                assert_eq!(summaries.len(), 2);
                next_cursor.expect("page1 must have next_cursor")
            }
            _ => panic!("expected Listed"),
        };

        // Second page: cursor = page1.next_cursor. With limit=2 and
        // 4 total bundles, page2 is also full — by m8-05 B2 semantics
        // ("cursor when page is full"), page2 ALSO returns a
        // next_cursor. The caller fetches page3 to discover the actual
        // end of the data (page3 returns 0 rows, no cursor).
        let page2 = ChronosCounterexampleService::list(
            &ctx,
            CounterexampleListFilter {
                limit: 2,
                cursor: Some(cursor.clone()),
                ..Default::default()
            },
        )
        .expect("page2");
        let cursor2 = match page2 {
            CounterexampleOutput::Listed {
                summaries,
                next_cursor,
            } => {
                assert_eq!(summaries.len(), 2);
                // page2's bundle_ids must all be > cursor.
                for s in &summaries {
                    assert!(
                        s.bundle_id.as_str() > cursor.as_str(),
                        "{} must be > cursor {}",
                        s.bundle_id,
                        cursor
                    );
                }
                // page2 is also full -> next_cursor is Some.
                next_cursor.expect("page2 must also have next_cursor (page full)")
            }
            _ => panic!("expected Listed"),
        };

        // Third page: cursor = page2.next_cursor. Now only 0 rows
        // remain, so next_cursor is None.
        let page3 = ChronosCounterexampleService::list(
            &ctx,
            CounterexampleListFilter {
                limit: 2,
                cursor: Some(cursor2),
                ..Default::default()
            },
        )
        .expect("page3");
        match page3 {
            CounterexampleOutput::Listed {
                summaries,
                next_cursor,
            } => {
                assert!(summaries.is_empty(), "no rows past the last cursor");
                assert!(next_cursor.is_none());
            }
            _ => panic!("expected Listed"),
        }
    }

    // m8-05 #9: list with limit > total rows returns no `next_cursor`.
    #[test]
    fn m8_05_list_last_page_has_no_cursor() {
        use crate::hypothesis_test::HypothesisTestContext;
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

        for i in 0..3 {
            let _ = ChronosCounterexampleService::save(
                &ctx,
                "ws",
                HypothesisKind::Invariant,
                (1, Some(PropertyValue::Number(i as f64)), None, None),
                &dummy_target_invariant(PropertyValue::Number(i as f64)),
                vec![],
            )
            .expect("save");
        }
        let listed = ChronosCounterexampleService::list(
            &ctx,
            CounterexampleListFilter {
                limit: 10,
                ..Default::default()
            },
        )
        .expect("list");
        match listed {
            CounterexampleOutput::Listed {
                summaries,
                next_cursor,
            } => {
                assert_eq!(summaries.len(), 3);
                assert!(next_cursor.is_none(), "last page must not have next_cursor");
            }
            _ => panic!("expected Listed"),
        }
    }

    // ========================================================================
    // m8-05 (B3) tests: events_count accessor
    // ========================================================================

    // m8-05 #10: events_count returns the persisted events vec length
    // via the new variant.
    #[test]
    fn m8_05_events_count_returns_persisted_length() {
        use crate::hypothesis_test::HypothesisTestContext;
        use chronos_domain::TraceEvent;
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

        // Save with 5 fake trace events.
        let events: Vec<TraceEvent> = (0..5)
            .map(|i| make_test_trace_event(i, 100 + i, 1))
            .collect();
        let saved = ChronosCounterexampleService::save(
            &ctx,
            "ws",
            HypothesisKind::Invariant,
            (1, Some(PropertyValue::Number(3.0)), None, None),
            &HypothesisInput {
                session_id: "s".to_string(),
                kind: HypothesisKind::Invariant,
                scope: None,
                comparison: None,
                constant: Some(PropertyValue::Number(3.0)),
                property_target: None,
                predicate: None,
                caller: None,
                callee: None,
                max_depth: None,
            },
            events,
        )
        .expect("save");
        let bundle_id = match saved {
            CounterexampleOutput::Saved { summary, .. } => summary.bundle_id,
            _ => panic!("expected Saved variant"),
        };

        let count = ChronosCounterexampleService::events_count(&ctx, &bundle_id)
            .expect("events_count should succeed");
        match count {
            CounterexampleOutput::EventsCount {
                bundle_id: bid,
                events_count,
            } => {
                assert_eq!(bid, bundle_id);
                assert_eq!(events_count, 5, "persisted count must match input");
            }
            _ => panic!("expected EventsCount variant"),
        }
    }

    // m8-05 #11: events_count for an unknown bundle_id returns
    // LoadFailed (same shape as `get`).
    #[test]
    fn m8_05_events_count_unknown_bundle_errors() {
        use crate::hypothesis_test::HypothesisTestContext;
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

        let r = ChronosCounterexampleService::events_count(&ctx, "no-such-bundle");
        match r {
            Err(ServiceError::LoadFailed(msg)) => {
                assert!(msg.contains("no-such-bundle"));
            }
            other => panic!("expected LoadFailed, got {other:?}"),
        }
    }

    // ========================================================================
    // m8-06 — real per-variant proptest shrinking (closes m8-05 R3)
    // ========================================================================

    /// `NumberShrinking` strategy: starting from `Number(100.0)`, after 8
    /// rounds the value is less than 1.0. Binary search halving distance.
    #[test]
    fn m8_06_number_strategy_shrinks_toward_zero() {
        let strategy = NumberShrinking { start: 100.0 };
        let mut runner = TestRunner::new(Config::default());
        let mut tree = strategy.new_tree(&mut runner).expect("new_tree");
        assert_eq!(tree.current(), PropertyValue::Number(100.0));
        // 8 rounds of halving: 100 -> 50 -> 25 -> 12.5 -> 6.25 -> 3.125 -> 1.5625 -> 0.78125 -> 0.390625.
        for _ in 0..8 {
            assert!(tree.simplify(), "expected simplify() to make progress");
        }
        let v = if let PropertyValue::Number(n) = tree.current() {
            n
        } else {
            panic!("expected Number, got {:?}", tree.current())
        };
        assert!(
            v < 1.0,
            "after 8 rounds, Number(100.0) should be < 1.0, got {v}"
        );
    }

    /// `NumberShrinking` strategy: converges to ~0.0 within 64 rounds.
    #[test]
    fn m8_06_number_strategy_converges_within_max_rounds() {
        let strategy = NumberShrinking { start: 1_000_000.0 };
        let mut runner = TestRunner::new(Config::default());
        let mut tree = strategy.new_tree(&mut runner).expect("new_tree");
        let mut rounds = 0;
        while tree.simplify() {
            rounds += 1;
            if rounds > 100 {
                panic!("not converging within 100 rounds");
            }
        }
        let v = if let PropertyValue::Number(n) = tree.current() {
            n
        } else {
            panic!("expected Number, got {:?}", tree.current())
        };
        assert!(v.abs() < f64::EPSILON, "should converge to ~0.0, got {v}");
    }

    /// `TextShrinking` strategy: deletes one character per simplify round.
    #[test]
    fn m8_06_text_strategy_shrinks_toward_empty() {
        let strategy = TextShrinking {
            start: "hello".to_string(),
        };
        let mut runner = TestRunner::new(Config::default());
        let mut tree = strategy.new_tree(&mut runner).expect("new_tree");
        assert_eq!(tree.current(), PropertyValue::Text("hello".to_string()));
        assert!(tree.simplify());
        let v = if let PropertyValue::Text(s) = tree.current() {
            s
        } else {
            panic!("expected Text, got {:?}", tree.current())
        };
        assert_eq!(v.len(), 4, "after 1 round, one char should be deleted");
    }

    /// `TextShrinking` strategy: converges to empty string.
    #[test]
    fn m8_06_text_strategy_converges_to_empty() {
        let strategy = TextShrinking {
            start: "hi".to_string(),
        };
        let mut runner = TestRunner::new(Config::default());
        let mut tree = strategy.new_tree(&mut runner).expect("new_tree");
        let mut rounds = 0;
        while tree.simplify() {
            rounds += 1;
            if rounds > 10 {
                panic!("not converging within 10 rounds");
            }
        }
        assert_eq!(
            tree.current(),
            PropertyValue::Text(String::new()),
            "should converge to empty string"
        );
        assert_eq!(rounds, 2, "should converge in exactly 2 rounds for 'hi'");
    }

    /// `Bool` strategy stays as `Just(b)` (D3 disclosure): no shrinkage.
    #[test]
    fn m8_06_bool_strategy_returns_base_no_shrink() {
        let strategy = proptest::strategy::Just(PropertyValue::Bool(true));
        let mut runner = TestRunner::new(Config::default());
        let mut tree = strategy.new_tree(&mut runner).expect("new_tree");
        assert_eq!(tree.current(), PropertyValue::Bool(true));
        assert!(!tree.simplify(), "Bool should not simplify");
        assert_eq!(tree.current(), PropertyValue::Bool(true));
    }

    /// `ExistencePredicate::EventTypeEquals` shrinks the event_type string.
    #[test]
    fn m8_06_existence_predicate_strategy_event_type_shrinks_string() {
        let strategy = ExistencePredicateShrinking {
            start: ExistencePredicate::EventTypeEquals {
                event_type: "OPEN_HTTP".to_string(),
            },
        };
        let mut runner = TestRunner::new(Config::default());
        let mut tree = strategy.new_tree(&mut runner).expect("new_tree");
        assert_eq!(
            tree.current(),
            ExistencePredicate::EventTypeEquals {
                event_type: "OPEN_HTTP".to_string(),
            }
        );
        assert!(tree.simplify());
        if let ExistencePredicate::EventTypeEquals { event_type } = tree.current() {
            assert_eq!(event_type.len(), 8, "one char should be deleted");
        } else {
            panic!("variant should be preserved");
        }
    }

    /// `ExistencePredicate::ThreadEquals` shrinks the thread_id toward 0.
    #[test]
    fn m8_06_existence_predicate_strategy_thread_shrinks_u64() {
        let strategy = ExistencePredicateShrinking {
            start: ExistencePredicate::ThreadEquals { thread_id: 1000 },
        };
        let mut runner = TestRunner::new(Config::default());
        let mut tree = strategy.new_tree(&mut runner).expect("new_tree");
        assert!(tree.simplify());
        if let ExistencePredicate::ThreadEquals { thread_id } = tree.current() {
            assert_eq!(thread_id, 500, "thread_id should halve toward 0");
        } else {
            panic!("variant should be preserved");
        }
    }

    /// `CallPath` strategy: each field shrinks independently.
    #[test]
    fn m8_06_call_path_strategy_shrinks_caller_and_callee() {
        let strategy = CallPathShrinking {
            start_caller: "foo".to_string(),
            start_callee: "bar".to_string(),
            start_max_depth: Some(3),
        };
        let mut runner = TestRunner::new(Config::default());
        let mut tree = strategy.new_tree(&mut runner).expect("new_tree");
        let (caller, callee, max_depth) = tree.current();
        assert_eq!(caller, "foo");
        assert_eq!(callee, "bar");
        assert_eq!(max_depth, Some(3));
        // Round 0: caller shrinks first.
        assert!(tree.simplify());
        let (caller, callee, _) = tree.current();
        assert_eq!(caller.len(), 2, "caller lost one char");
        assert_eq!(callee, "bar");
        // Round 1: callee shrinks next.
        assert!(tree.simplify());
        let (caller, callee, _) = tree.current();
        assert_eq!(caller.len(), 2);
        assert_eq!(callee.len(), 2, "callee lost one char");
    }

    /// `CallPath` strategy: `max_depth` shrinks toward `None`.
    #[test]
    fn m8_06_call_path_strategy_max_depth_shrinks_to_none() {
        let strategy = CallPathShrinking {
            start_caller: String::new(),
            start_callee: String::new(),
            start_max_depth: Some(3),
        };
        let mut runner = TestRunner::new(Config::default());
        let mut tree = strategy.new_tree(&mut runner).expect("new_tree");
        // Call simplify() in a loop and observe max_depth's progress.
        // Caller/callee are already empty so they don't shrink; max_depth
        // shrinks Some(3) → Some(2) → Some(1) → None → terminal.
        let mut seen_max_depths = Vec::new();
        let mut shrink_iters = 0;
        while tree.simplify() {
            seen_max_depths.push(tree.current().2);
            shrink_iters += 1;
            if shrink_iters > 20 {
                panic!("not converging within 20 rounds");
            }
        }
        // First three simplifies shrink max_depth (3 → 2 → 1 → None);
        // the 4th simplify hits max_depth_done=true and returns false.
        assert_eq!(
            seen_max_depths,
            vec![Some(2), Some(1), None],
            "max_depth should shrink Some(3) → Some(2) → Some(1) → None"
        );
        assert_eq!(tree.current().2, None, "final max_depth should be None");
    }

    /// `drive_strategy` integration: Number target shrinks through multiple
    /// rounds, ending at a value near 0.0.
    #[test]
    fn m8_06_drive_strategy_with_number_shrinker_terminates_at_zero() {
        let target = HypothesisInput {
            session_id: "sess".into(),
            kind: HypothesisKind::Invariant,
            scope: None,
            comparison: None,
            constant: Some(PropertyValue::Number(1000.0)),
            property_target: None,
            predicate: None,
            caller: None,
            callee: None,
            max_depth: None,
        };
        let p_cfg: proptest::test_runner::Config = ShrinkConfig {
            max_rounds: 64,
            seed: None,
        }
        .into();
        let mut runner = TestRunner::new(p_cfg);
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("rt");
        let result = rt.block_on(async {
            drive_strategy(&mut runner, &target, |sampled| async move {
                use crate::output::{HypothesisOutput, HypothesisVerdict};
                let _ = sampled;
                Ok::<_, ServiceError>(HypothesisOutput::Invariant {
                    verdict: HypothesisVerdict::Violation {
                        reason: "test".into(),
                    },
                    support_event_ids: vec![],
                    counter_event_ids: vec![],
                    scope: crate::output::HypothesisScope::PropertyValue,
                    summary: "violation".into(),
                })
            })
            .await
        });
        let (rounds, best) = result.expect("drive_strategy should succeed");
        // m8-06: with real Number shrinker, rounds >= 8 (initial + 7 halvings
        // to get under 8.0, then continued halving until < EPSILON).
        assert!(
            rounds >= 8,
            "Number(1000.0) should shrink through many rounds, got {rounds}"
        );
        let v = if let Some(PropertyValue::Number(n)) = best.constant {
            n
        } else {
            panic!("expected Number constant");
        };
        assert!(v.abs() < f64::EPSILON, "should converge to ~0.0, got {v}");
    }

    /// `drive_strategy` integration: Text target shrinks to empty string.
    #[test]
    fn m8_06_drive_strategy_with_text_shrinker_terminates_at_empty() {
        let target = HypothesisInput {
            session_id: "sess".into(),
            kind: HypothesisKind::Invariant,
            scope: None,
            comparison: None,
            constant: Some(PropertyValue::Text("hello".to_string())),
            property_target: None,
            predicate: None,
            caller: None,
            callee: None,
            max_depth: None,
        };
        let p_cfg: proptest::test_runner::Config = ShrinkConfig {
            max_rounds: 32,
            seed: None,
        }
        .into();
        let mut runner = TestRunner::new(p_cfg);
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("rt");
        let result = rt.block_on(async {
            drive_strategy(&mut runner, &target, |sampled| async move {
                use crate::output::{HypothesisOutput, HypothesisVerdict};
                let _ = sampled;
                Ok::<_, ServiceError>(HypothesisOutput::Invariant {
                    verdict: HypothesisVerdict::Violation {
                        reason: "test".into(),
                    },
                    support_event_ids: vec![],
                    counter_event_ids: vec![],
                    scope: crate::output::HypothesisScope::PropertyValue,
                    summary: "violation".into(),
                })
            })
            .await
        });
        let (rounds, best) = result.expect("drive_strategy should succeed");
        // m8-06: rounds counts initial + each successful simplify.
        assert!(
            rounds >= 6,
            "Text(\"hello\") should shrink through many rounds, got {rounds}"
        );
        assert_eq!(best.constant, Some(PropertyValue::Text(String::new())));
    }

    // ========================================================================
    // m8-07 tests: HypothesisInput wire mirror (hypothesis_input_to_wire /
    // hypothesis_input_from_wire round-trip). R1 disclosure in §5 of the
    // m8-07 scoping doc: drift is possible if HypothesisInput evolves
    // without updating the mirror; these tests are the mitigation.
    // ========================================================================

    /// m8-07 §5: wire mirror roundtrip. hypothesis_input_to_wire + hypothesis_input_from_wire
    /// should be a no-op for a fully-populated Invariant input (the m8-07 acceptance
    /// scenario: scope=EventCount, comparison=Ge, constant=Number(1000.0) must survive
    /// the persist → load round-trip intact).
    #[test]
    fn m8_07_hypothesis_input_roundtrip_invariant_preserves_non_defaults() {
        let input = HypothesisInput {
            session_id: "sess-ce12".into(),
            kind: HypothesisKind::Invariant,
            scope: Some(crate::output::HypothesisScope::EventCount),
            comparison: Some(chronos_domain::property::ComparisonOp::Ge),
            constant: Some(PropertyValue::Number(1000.0)),
            property_target: None,
            predicate: None,
            caller: None,
            callee: None,
            max_depth: None,
        };
        let wire = hypothesis_input_to_wire(&input);
        let roundtripped = hypothesis_input_from_wire(wire);
        assert_eq!(roundtripped.kind, input.kind);
        assert_eq!(roundtripped.scope, input.scope);
        assert_eq!(roundtripped.comparison, input.comparison);
        assert_eq!(roundtripped.constant, input.constant);
        assert_eq!(roundtripped.property_target, input.property_target);
    }

    /// m8-07 §5: hypothesis_input_to_wire roundtrip for Existence kind.
    #[test]
    fn m8_07_hypothesis_input_roundtrip_existence() {
        let input = HypothesisInput {
            session_id: "sess-ext".into(),
            kind: HypothesisKind::Existence,
            scope: None,
            comparison: None,
            constant: None,
            property_target: None,
            predicate: Some(ExistencePredicate::EventTypeEquals {
                event_type: "Syscall".into(),
            }),
            caller: None,
            callee: None,
            max_depth: None,
        };
        let wire = hypothesis_input_to_wire(&input);
        let roundtripped = hypothesis_input_from_wire(wire);
        assert_eq!(roundtripped.kind, HypothesisKind::Existence);
        match roundtripped.predicate {
            Some(ExistencePredicate::EventTypeEquals { event_type }) => {
                assert_eq!(event_type, "Syscall");
            }
            _ => panic!("expected EventTypeEquals"),
        }
    }

    /// m8-07 §5: hypothesis_input_to_wire roundtrip for CallPath kind.
    #[test]
    fn m8_07_hypothesis_input_roundtrip_call_path() {
        let input = HypothesisInput {
            session_id: "sess-cp".into(),
            kind: HypothesisKind::CallPath,
            scope: None,
            comparison: None,
            constant: None,
            property_target: None,
            predicate: None,
            caller: Some("main".into()),
            callee: Some("work".into()),
            max_depth: Some(4),
        };
        let wire = hypothesis_input_to_wire(&input);
        let roundtripped = hypothesis_input_from_wire(wire);
        assert_eq!(roundtripped.kind, HypothesisKind::CallPath);
        assert_eq!(roundtripped.caller.as_deref(), Some("main"));
        assert_eq!(roundtripped.callee.as_deref(), Some("work"));
        assert_eq!(roundtripped.max_depth, Some(4));
    }

    /// m8-07 §5: hypothesis_input_to_wire roundtrip for Invariant with
    /// property_target set (the non-default case the m8-04 fallback loses).
    #[test]
    fn m8_07_hypothesis_input_roundtrip_invariant_with_property_target() {
        let input = HypothesisInput {
            session_id: "sess-pt".into(),
            kind: HypothesisKind::Invariant,
            scope: Some(crate::output::HypothesisScope::PropertyValue),
            comparison: Some(chronos_domain::property::ComparisonOp::Gt),
            constant: Some(PropertyValue::Number(0.0)),
            property_target: Some("thread_count".into()),
            predicate: None,
            caller: None,
            callee: None,
            max_depth: None,
        };
        let wire = hypothesis_input_to_wire(&input);
        let roundtripped = hypothesis_input_from_wire(wire);
        assert_eq!(
            roundtripped.property_target.as_deref(),
            Some("thread_count")
        );
        assert_eq!(roundtripped.comparison, input.comparison);
    }

    /// m8-07 §5: roundtrip for Existence with ThreadEquals and PropertyKeyEquals
    /// predicate variants.
    #[test]
    fn m8_07_hypothesis_input_roundtrip_existence_all_predicate_variants() {
        use crate::output::HypothesisScope;
        for predicate in [
            ExistencePredicate::ThreadEquals { thread_id: 42 },
            ExistencePredicate::PropertyKeyEquals {
                target: "cpu_time_ms".into(),
            },
        ] {
            let input = HypothesisInput {
                session_id: "sess-pred".into(),
                kind: HypothesisKind::Existence,
                scope: Some(HypothesisScope::EventCount),
                comparison: Some(chronos_domain::property::ComparisonOp::Eq),
                constant: None,
                property_target: Some("syscalls".into()),
                predicate: Some(predicate.clone()),
                caller: None,
                callee: None,
                max_depth: None,
            };
            let wire = hypothesis_input_to_wire(&input);
            let rt = hypothesis_input_from_wire(wire);
            assert_eq!(rt.predicate, Some(predicate));
        }
    }

    // ========================================================================
    // m9-01 tests: schema_version on wire summary via save()
    // ========================================================================

    // m9-01 §5 (per-crate integration): call ChronosCounterexampleService::save
    // end-to-end (with a real in-memory SessionStore), then load_counterexample_bundle,
    // assert schema_version: 1 on both record and summary. Confirms the
    // chronos-services save path passes the version into the wire summary it builds.
    #[test]
    fn m9_01_save_persists_schema_version_1() {
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

        // Save a bundle via ChronosCounterexampleService::save.
        let saved = ChronosCounterexampleService::save(
            &ctx,
            "ws-integration-test",
            HypothesisKind::Invariant,
            (2, Some(PropertyValue::Number(1.0)), None, None),
            &HypothesisInput {
                session_id: "s".to_string(),
                kind: HypothesisKind::Invariant,
                scope: None,
                comparison: None,
                constant: Some(PropertyValue::Number(1.0)),
                property_target: None,
                predicate: None,
                caller: None,
                callee: None,
                max_depth: None,
            },
            vec![],
        )
        .expect("save should succeed");
        let saved_id = match saved {
            CounterexampleOutput::Saved { summary, .. } => summary.bundle_id,
            _ => panic!("expected Saved variant"),
        };

        // Load it back via SessionStore directly (bypasses services-layer
        // conversion so we can inspect the wire-level schema_version).
        let loaded = store
            .load_counterexample_bundle(&saved_id)
            .expect("load should succeed")
            .expect("bundle should exist");

        assert_eq!(loaded.schema_version, 1, "record.schema_version must be 1");
        assert_eq!(
            loaded.summary.schema_version, 1,
            "summary.schema_version must be 1"
        );
    }
}
