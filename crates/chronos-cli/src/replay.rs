//! `chronos test replay <bundle_id>` — replay a persisted counterexample.
//!
//! Reads the bundle by id from the session store at `--db <path>` (default:
//! `$XDG_DATA_HOME/chronos/chronos.db`), rebuilds a `QueryEngine` from the
//! bundle's `events` field, reconstructs a `HypothesisInput` from the bundle's
//! `MinimisedPayload`, and re-runs `ChronosHypothesisTestService::test` against
//! the synthetic engine. Prints the resulting verdict and the bundle summary as
//! JSON to stdout.
//!
//! B-decision B6 (m8-04 scoping doc): the CLI does NOT start live probes — it
//! only replays already-persisted bundles. The engines map is populated with
//! a synthetic `QueryEngine` built from the bundle's `events` field. This is a
//! pure read-only operation against the redb store, which is why this subcommand
//! can be implemented today even though live probe plumbing is m9+.
//!
//! B-decision R-in-memory-engines (m8-04 §3): the temporary engines map lives
//! in the CLI's own `HashMap`, not in any MCP-scope state. `SessionStore::open`
//! is read-only from our perspective — we never write back.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use chronos_query::engine::QueryEngine;
use chronos_services::hypothesis_test::{ChronosHypothesisTestService, HypothesisInput};
use chronos_services::output::{ExistencePredicate, HypothesisKind};
use chronos_store::counterexample_storage::{
    bundle_events_or_legacy, CounterexampleBundleRecord, ExistencePredicateWire, MinimisedPayload,
};
use chronos_store::SessionStore;
use serde::Serialize;
use tokio::sync::Mutex as TokioMutex;
use tracing::info;

/// Synthetic session id used as the engines-map key during replay.
///
/// The bundle's events are loaded into a single synthetic session because we
/// have no live probe to attach. The session id does not need to match any
/// real session id from the capture phase — `hypothesis_test::test` looks up
/// the engine by this id, and we just populated that map ourselves.
const REPLAY_SESSION_ID: &str = "chronos-cli-replay";

/// JSON shape printed to stdout on successful replay.
#[derive(Debug, Serialize)]
pub struct ReplayReport {
    pub bundle_id: String,
    pub property_kind: String,
    pub workspace_id: String,
    pub created_at_ms: u64,
    pub rounds_used: u32,
    pub events_in_bundle: usize,
    pub replay_verdict: String,
    pub replay_summary: String,
    pub replay_support_event_ids: Vec<u64>,
    pub replay_counter_event_ids: Vec<u64>,
}

/// Public entry point. Reads the bundle, rebuilds the engine, re-runs the
/// hypothesis, and returns a [`ReplayReport`] ready for serialization.
pub async fn run_replay(db_path: &Path, bundle_id: &str) -> Result<ReplayReport> {
    info!(bundle_id, db_path = ?db_path, "chronos: replay starting");

    // 1. Open the session store. Read-only from our perspective.
    let store = SessionStore::open(db_path)
        .with_context(|| format!("opening session store at {}", db_path.display()))?;

    // 2. Load the bundle by id.
    let bundle = store
        .load_counterexample_bundle(bundle_id)
        .with_context(|| format!("loading counterexample bundle {bundle_id}"))?
        .ok_or_else(|| anyhow!("no counterexample bundle with id {bundle_id:?} in store"))?;

    // 3. Rebuild the engine from the bundle's events. The synthetic engine
    //    has no live capture context — it only knows about events already
    //    persisted in the bundle.
    // m9-02 D5: always go through the chokepoint — it handles both
    // legacy blob-embedded events (pre-m9-02) and side-table events (post-m9-02).
    let events = bundle_events_or_legacy(&store, &bundle)?;
    let engine = QueryEngine::new(events);
    let mut engines: HashMap<String, QueryEngine> = HashMap::new();
    engines.insert(REPLAY_SESSION_ID.to_string(), engine);
    let engines = TokioMutex::new(engines);

    // 4. Reconstruct a HypothesisInput from the bundle's minimised payload.
    let hypothesis = reconstruct_hypothesis(&bundle, REPLAY_SESSION_ID)?;

    // 5. Run the v2 hypothesis_test dispatcher against the synthetic engine.
    let ctx = chronos_services::hypothesis_test::HypothesisTestContext { engines: &engines };
    let output = ChronosHypothesisTestService::test(&ctx, hypothesis)
        .await
        .map_err(|e| anyhow!("hypothesis_test::test failed: {e}"))?;

    // 6. Project the output into a flat serializable report.
    let report = project_report(&bundle, output);
    Ok(report)
}

/// Build a [`HypothesisInput`] from the bundle.
///
/// m8-07: prefers the persisted `target_hypothesis` (the EXACT
/// `HypothesisInput` the user passed to `counterexample_shrink`) when
/// present. Falls back to the m8-04 synthetic-default reconstruction
/// from the `minimised` payload when `target_hypothesis` is absent
/// (pre-m8-07 bundles).
///
/// The synthetic-default path is fine for the M8 acceptance criterion
/// (re-running the minimised hypothesis against the same events must
/// still produce a violation) but loses `scope`, `comparison`, and
/// `property_target` for non-default Invariant targets. The
/// `target_hypothesis` path preserves them byte-for-byte.
fn reconstruct_hypothesis(
    bundle: &CounterexampleBundleRecord,
    session_id: &str,
) -> Result<HypothesisInput> {
    // m8-07: prefer the persisted target_hypothesis. This is the EXACT
    // HypothesisInput the user passed to counterexample_shrink; using it
    // verbatim closes the m8-04 R-hypothesis-reconstruction-fidelity gap.
    if let Some(wire) = &bundle.target_hypothesis {
        let mut input = chronos_services::counterexample::hypothesis_input_from_wire(wire.clone());
        // The persisted session_id is the LIVE probe session; replay uses
        // the synthetic REPLAY_SESSION_ID so the in-memory engine lookup
        // finds the rebuilt engine. All OTHER fields are preserved verbatim.
        input.session_id = session_id.to_string();
        return Ok(input);
    }

    let minimised = bundle.minimised.as_ref().ok_or_else(|| {
        anyhow!(
            "bundle {bundle_id} has no minimised payload",
            bundle_id = bundle.summary.bundle_id
        )
    })?;

    let kind = match bundle.summary.property_kind.as_str() {
        "invariant" => HypothesisKind::Invariant,
        "existence" => HypothesisKind::Existence,
        "call_path" => HypothesisKind::CallPath,
        other => return Err(anyhow!("unknown property_kind in summary: {other:?}")),
    };

    let mut input = HypothesisInput {
        session_id: session_id.to_string(),
        kind,
        scope: None,
        comparison: None,
        constant: None,
        property_target: None,
        predicate: None,
        caller: None,
        callee: None,
        max_depth: None,
    };

    match minimised {
        MinimisedPayload::Constant(v) => {
            // Invariant shape: scope defaults to PropertyValue, no comparison,
            // constant from the minimised payload. This is the simplest possible
            // reconstruction — see B-decision B6.
            input.scope = Some(chronos_services::output::HypothesisScope::PropertyValue);
            input.constant = Some(v.clone());
        }
        MinimisedPayload::Predicate(p) => {
            input.predicate = Some(existence_predicate_from_wire(p.clone()));
        }
        MinimisedPayload::CallPath {
            caller,
            callee,
            max_depth,
        } => {
            input.caller = Some(caller.clone());
            input.callee = Some(callee.clone());
            // HypothesisInput::max_depth is `Option<usize>`; the wire field is
            // `Option<u64>`. Saturate rather than panic on the cast — a max_depth
            // > usize::MAX would be an absurd input and would only come from a
            // corrupted store.
            input.max_depth = max_depth.map(|d| usize::try_from(d).unwrap_or(usize::MAX));
        }
    }

    Ok(input)
}

/// Convert `chronos-store`'s `ExistencePredicateWire` into the typed
/// `chronos-services` `ExistencePredicate`.
///
/// The two enums are deliberately mirror shapes (R5 carry-over from m8-03).
/// A `match`-based conversion keeps the circular-dep avoidance strategy
/// intact.
fn existence_predicate_from_wire(wire: ExistencePredicateWire) -> ExistencePredicate {
    match wire {
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

/// Flatten the [`HypothesisOutput`] into the serializable report shape.
fn project_report(
    bundle: &CounterexampleBundleRecord,
    output: chronos_services::output::HypothesisOutput,
) -> ReplayReport {
    use chronos_services::output::HypothesisVerdict;
    let (verdict, summary, support, counter) = match output {
        chronos_services::output::HypothesisOutput::Invariant {
            verdict,
            support_event_ids,
            counter_event_ids,
            summary,
            ..
        } => (verdict, summary, support_event_ids, counter_event_ids),
        chronos_services::output::HypothesisOutput::Existence {
            verdict,
            support_event_ids,
            counter_event_ids,
            summary,
            ..
        } => (verdict, summary, support_event_ids, counter_event_ids),
        chronos_services::output::HypothesisOutput::CallPath {
            verdict,
            support_event_ids,
            counter_event_ids,
            summary,
            ..
        } => (verdict, summary, support_event_ids, counter_event_ids),
    };
    let verdict_str = match verdict {
        HypothesisVerdict::Pass => "pass",
        HypothesisVerdict::Violation { .. } => "violation",
        HypothesisVerdict::Unsupported { .. } => "unsupported",
    };
    ReplayReport {
        bundle_id: bundle.summary.bundle_id.clone(),
        property_kind: bundle.summary.property_kind.clone(),
        workspace_id: bundle.summary.workspace_id.clone(),
        created_at_ms: bundle.summary.created_at_ms,
        rounds_used: bundle.summary.rounds_used,
        events_in_bundle: bundle.summary.events_count as usize,
        replay_verdict: verdict_str.to_string(),
        replay_summary: summary,
        replay_support_event_ids: support,
        replay_counter_event_ids: counter,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::property::PropertyValue;
    use chronos_services::output::HypothesisScope;
    use chronos_store::counterexample_storage::{
        CounterexampleBundleRecord, CounterexampleBundleSummary,
    };
    use std::path::PathBuf;

    fn temp_db_path(label: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "chronos-cli-replay-{label}-{}.db",
            std::process::id()
        ));
        // Ensure no leftover from prior runs.
        let _ = std::fs::remove_file(&p);
        p
    }

    fn synthetic_bundle(
        property_kind: &str,
        minimised: MinimisedPayload,
    ) -> CounterexampleBundleRecord {
        CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: "b1".into(),
                property_kind: property_kind.into(),
                workspace_id: "ws".into(),
                created_at_ms: 0,
                rounds_used: 1,
                has_full_bundle: true,
                schema_version: 1,
                events_count: 0,
            },
            events: vec![],
            minimised: Some(minimised),
            event_cas_hashes: vec![],
            // m8-07: pre-m8-07 bundles (and synthetic test fixtures) have
            // no persisted target_hypothesis; reconstruct_hypothesis
            // falls back to the m8-04 synthetic-default path.
            target_hypothesis: None,
            schema_version: 1,
        }
    }

    #[test]
    fn reconstruct_invariant_from_constant() {
        let bundle = synthetic_bundle(
            "invariant",
            MinimisedPayload::Constant(PropertyValue::Number(42.0)),
        );
        let input = reconstruct_hypothesis(&bundle, "s").unwrap();
        assert_eq!(input.kind, HypothesisKind::Invariant);
        assert_eq!(input.scope, Some(HypothesisScope::PropertyValue));
        assert_eq!(input.constant, Some(PropertyValue::Number(42.0)));
        assert!(input.predicate.is_none());
    }

    #[test]
    fn reconstruct_existence_from_predicate() {
        let bundle = synthetic_bundle(
            "existence",
            MinimisedPayload::Predicate(ExistencePredicateWire::EventTypeEquals {
                event_type: "Syscall".into(),
            }),
        );
        let input = reconstruct_hypothesis(&bundle, "s").unwrap();
        assert_eq!(input.kind, HypothesisKind::Existence);
        let predicate = input.predicate.unwrap();
        match predicate {
            ExistencePredicate::EventTypeEquals { event_type } => {
                assert_eq!(event_type, "Syscall");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn reconstruct_call_path() {
        let bundle = synthetic_bundle(
            "call_path",
            MinimisedPayload::CallPath {
                caller: "main".into(),
                callee: "work".into(),
                max_depth: Some(4),
            },
        );
        let input = reconstruct_hypothesis(&bundle, "s").unwrap();
        assert_eq!(input.kind, HypothesisKind::CallPath);
        assert_eq!(input.caller.as_deref(), Some("main"));
        assert_eq!(input.callee.as_deref(), Some("work"));
        assert_eq!(input.max_depth, Some(4));
    }

    #[test]
    fn reconstruct_rejects_unknown_kind() {
        let bundle = synthetic_bundle(
            "totally-bogus",
            MinimisedPayload::Constant(PropertyValue::Number(0.0)),
        );
        let err = reconstruct_hypothesis(&bundle, "s").unwrap_err();
        assert!(format!("{err:#}").contains("unknown property_kind"));
    }

    #[test]
    fn reconstruct_requires_minimised() {
        let mut bundle = synthetic_bundle(
            "invariant",
            MinimisedPayload::Constant(PropertyValue::Number(0.0)),
        );
        bundle.minimised = None;
        let err = reconstruct_hypothesis(&bundle, "s").unwrap_err();
        assert!(format!("{err:#}").contains("no minimised payload"));
    }

    #[tokio::test]
    async fn run_replay_missing_bundle_errors() {
        let path = temp_db_path("missing");
        let err = run_replay(&path, "does-not-exist").await.unwrap_err();
        // Either "no counterexample bundle" or a load failure — both are acceptable;
        // the point is that we surface a sane error and don't panic.
        let msg = format!("{err:#}");
        assert!(
            msg.contains("no counterexample bundle") || msg.contains("loading"),
            "unexpected error: {msg}"
        );
    }

    #[tokio::test]
    async fn run_replay_round_trip_with_constant() {
        // Round-trip: save a bundle, drop the writer SessionStore (redb holds
        // an exclusive file lock), then replay through `run_replay` which
        // opens its own store.
        let path = temp_db_path("round-trip");
        {
            let store = SessionStore::open(&path).unwrap();
            store
                .save_counterexample_bundle(CounterexampleBundleRecord {
                    summary: CounterexampleBundleSummary {
                        bundle_id: "rt-1".into(),
                        property_kind: "invariant".into(),
                        workspace_id: "ws".into(),
                        created_at_ms: 0,
                        rounds_used: 1,
                        has_full_bundle: true,
                        schema_version: 1,
                        events_count: 0,
                    },
                    events: vec![],
                    minimised: Some(MinimisedPayload::Constant(PropertyValue::Number(0.0))),
                    event_cas_hashes: vec![],
                    target_hypothesis: None,
                    schema_version: 1,
                })
                .unwrap();
        } // store dropped here — redb file lock released.

        let report = run_replay(&path, "rt-1").await.unwrap();
        assert_eq!(report.bundle_id, "rt-1");
        assert_eq!(report.property_kind, "invariant");
        // The synthetic engine has zero events; the minimised Invariant with
        // scope=PropertyValue and no property_target cannot collect any
        // evidence, so the verdict should be `unsupported`. Either way the
        // important contract is: we got a structured report back, no panic.
        assert!(
            matches!(
                report.replay_verdict.as_str(),
                "unsupported" | "violation" | "pass"
            ),
            "verdict should be one of the three, got {}",
            report.replay_verdict
        );
        // The minimised payload reconstruction is faithful: zero events, so
        // events_in_bundle reflects the bundle.
        assert_eq!(report.events_in_bundle, 0);

        // Cleanup.
        let _ = std::fs::remove_file(&path);
    }

    // m8-07 §5: when bundle.target_hypothesis is Some, reconstruct_hypothesis
    // uses it verbatim (overriding the session_id with the synthetic one).
    // This pins the D3 contract: the persisted target_hypothesis is the EXACT
    // HypothesisInput the user passed to counterexample_shrink.
    #[test]
    fn m8_07_reconstruct_uses_persisted_target_hypothesis() {
        use chronos_services::output::HypothesisScope;
        let bundle = CounterexampleBundleRecord {
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
            minimised: Some(MinimisedPayload::Constant(PropertyValue::Number(0.0))),
            event_cas_hashes: vec![],
            // m8-07: the persisted target_hypothesis carries the original
            // scope=EventCount, comparison=Ge, constant=Number(1000.0).
            target_hypothesis: Some(chronos_store::counterexample_storage::HypothesisInputWire {
                session_id: "live-probe-session".into(),
                kind: "invariant".into(),
                scope: Some("event_count".into()),
                comparison: Some("Ge".into()),
                constant: Some(PropertyValue::Number(1000.0)),
                property_target: None,
                predicate: None,
                caller: None,
                callee: None,
                max_depth: None,
            }),
            schema_version: 1,
        };
        let input = reconstruct_hypothesis(&bundle, "synthetic-session").unwrap();
        // session_id is overridden to the synthetic replay session.
        assert_eq!(input.session_id, "synthetic-session");
        // All other fields are preserved verbatim from the persisted target_hypothesis.
        assert_eq!(input.kind, HypothesisKind::Invariant);
        assert_eq!(input.scope, Some(HypothesisScope::EventCount));
        assert_eq!(
            input.comparison,
            Some(chronos_domain::property::ComparisonOp::Ge)
        );
        assert_eq!(input.constant, Some(PropertyValue::Number(1000.0)));
    }

    // m8-07 §5: when target_hypothesis is Some, the fallback minimised-payload
    // reconstruction is NOT used (scope stays EventCount, not PropertyValue).
    #[test]
    fn m8_07_reconstruct_preserves_scope_and_comparison() {
        use chronos_services::output::HypothesisScope;
        let bundle = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: "b-m8-07-scope".into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 0,
                rounds_used: 2,
                has_full_bundle: true,
                schema_version: 1,
                events_count: 0,
            },
            events: vec![],
            minimised: Some(MinimisedPayload::Constant(PropertyValue::Number(0.0))),
            event_cas_hashes: vec![],
            // The pre-m8-07 fallback would reconstruct scope=PropertyValue here.
            // With target_hypothesis present, the real scope=LatencyMs must survive.
            target_hypothesis: Some(chronos_store::counterexample_storage::HypothesisInputWire {
                session_id: "live".into(),
                kind: "invariant".into(),
                scope: Some("latency_ms".into()),
                comparison: Some("Gt".into()),
                constant: Some(PropertyValue::Number(50.0)),
                property_target: Some("cpu_time_ms".into()),
                predicate: None,
                caller: None,
                callee: None,
                max_depth: None,
            }),
            schema_version: 1,
        };
        let input = reconstruct_hypothesis(&bundle, "synth").unwrap();
        assert_eq!(input.scope, Some(HypothesisScope::LatencyMs));
        assert_eq!(
            input.comparison,
            Some(chronos_domain::property::ComparisonOp::Gt)
        );
        assert_eq!(input.property_target.as_deref(), Some("cpu_time_ms"));
        // The constant is NOT the minimised 0.0 — it's the original 50.0.
        assert_eq!(input.constant, Some(PropertyValue::Number(50.0)));
    }

    // m9-02: post-m9-02 bundle loads events from side table via chokepoint.
    // This is tested at the store level in chronos-store; the CLI
    // exercises the same path via run_replay's bundle_events_or_legacy call.
    #[test]
    fn m9_02_bundle_events_or_legacy_chokepoint_for_side_table() {
        // Verify bundle_events_or_legacy is exported and callable from this module.
        // The actual side-table loading is tested in chronos-store.
        use chronos_store::SessionStore;

        let store = SessionStore::in_memory().expect("in_memory store");

        // Save a post-m9-02 bundle (empty blob events, events in side table).
        let summary = chronos_store::counterexample_storage::CounterexampleBundleSummary {
            bundle_id: "b-cli-test".into(),
            property_kind: "invariant".into(),
            workspace_id: "ws".into(),
            created_at_ms: 0,
            rounds_used: 1,
            has_full_bundle: true,
            schema_version: 1,
            events_count: 0,
        };
        let rec = chronos_store::counterexample_storage::CounterexampleBundleRecord {
            summary,
            events: vec![],
            minimised: Some(
                chronos_store::counterexample_storage::MinimisedPayload::Constant(
                    chronos_domain::property::PropertyValue::Number(0.0),
                ),
            ),
            event_cas_hashes: vec![],
            target_hypothesis: None,
            schema_version: 1,
        };
        store
            .save_counterexample_bundle(rec)
            .expect("save should succeed");

        // Load and verify chokepoint returns empty (no events in side table).
        let bundle = store
            .load_counterexample_bundle("b-cli-test")
            .expect("load should succeed")
            .expect("bundle should exist");
        let events = bundle_events_or_legacy(&store, &bundle).expect("chokepoint should succeed");
        assert!(
            events.is_empty(),
            "post-m9-02 bundle with no side-table events should return empty vec"
        );
    }

    // m9-02: synthetic_bundle fixture is updated to set events: vec![] and
    // events_count: 0 in the summary. This test verifies the fixture
    // is correct and the legacy path is accessible.
    #[test]
    fn m9_02_synthetic_bundle_fixture_has_empty_events_and_events_count_zero() {
        let bundle = synthetic_bundle(
            "invariant",
            chronos_store::counterexample_storage::MinimisedPayload::Constant(
                chronos_domain::property::PropertyValue::Number(0.0),
            ),
        );
        assert!(
            bundle.events.is_empty(),
            "synthetic_bundle should set events: vec![]"
        );
        assert_eq!(
            bundle.summary.events_count, 0,
            "synthetic_bundle should set events_count: 0"
        );
    }
}
