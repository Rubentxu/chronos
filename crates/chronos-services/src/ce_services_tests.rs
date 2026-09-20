//! Tests for chronos-services counterexample module.
//!
//! Sibling file extracted in m9-95 to keep `counterexample.rs` focused
//! on production code (~1785 LoC). The inline `mod tests { ... }`
//! block (2169 lines, 55% of the file) was the remaining
//! contributor to file length.
//!
//! Tests use `use super::*;` so the parent's private items
//! (`ChronosCounterexampleService`, `CounterexampleContext`,
//! `CounterexampleShrinkInput`, etc.) remain accessible. The
//! `#[path = "ce_services_tests.rs"]` declaration in the parent file
//! makes this module a submodule of `counterexample`, preserving the
//! same module graph as before the extraction.

use chronos_domain::MonotonicNs;

use super::*;
use proptest::strategy::{Strategy, ValueTree};
use proptest::test_runner::Config;
use std::sync::Arc;

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

// Test 1: CounterexampleContext<'a> is Send when the inner fields are Send.
// Compile-time check — never actually runs the assertion function body.
#[test]
fn counterexample_context_is_send_when_inner_fields_are_send() {
    fn _check<'a>(
        repository: Arc<dyn chronos_domain::ports::counterexample::CounterexampleRepository>,
        hyp_ctx: &'a HypothesisTestContext<'a>,
    ) {
        let ctx: CounterexampleContext<'a> = CounterexampleContext {
            repository,
            hypothesis_ctx: hyp_ctx,
        };
        assert_send(ctx);
    }
}

// Test 2: `get` accepts the (repository, hypothesis_ctx) shape — pins
// the m8-03-era tripwire that the constructor accepts a real repository
// port. Uses InMemoryCounterexampleRepository from the port module
// (no redb plumbing required) so the test runs without a SessionStore.
#[test]
fn counterexample_get_signature_accepts_bundle_id_string() {
    // Compile-time + type-equality tripwire: ensure the function accepts
    // any `&str` and returns `Result<CounterexampleOutput, ServiceError>`.
    fn _check_shape(
        repository: Arc<dyn chronos_domain::ports::counterexample::CounterexampleRepository>,
        hyp_ctx: &HypothesisTestContext<'_>,
    ) -> Result<CounterexampleOutput, ServiceError> {
        ChronosCounterexampleService::get(
            &CounterexampleContext {
                repository,
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

    let exh: ServiceError = CounterexampleRunError::RoundsExhausted { best_so_far: None }.into();
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

    let store = std::sync::Arc::new(SessionStore::in_memory().expect("in_memory store"));
    let engines: HashMap<String, QueryEngine> = HashMap::new();
    let engines = TokioMutex::new(engines);
    let hyp_ctx = HypothesisTestContext { engines: &engines };
    let ctx = CounterexampleContext {
        repository: chronos_store::counterexample_repository::SessionStoreBackedCounterexampleRepository::new(std::sync::Arc::clone(&store)).into_arc(),
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
        MonotonicNs::from(ts),
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

    let store = std::sync::Arc::new(SessionStore::in_memory().expect("in_memory store"));
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
        repository: chronos_store::counterexample_repository::SessionStoreBackedCounterexampleRepository::new(std::sync::Arc::clone(&store)).into_arc(),
        hypothesis_ctx: &hyp_ctx,
    };

    let pulled = ChronosCounterexampleService::pull_engine_events(&ctx, "sess-m8-04".to_string())
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

    let store = std::sync::Arc::new(SessionStore::in_memory().expect("in_memory store"));
    let engines_map: HashMap<String, QueryEngine> = HashMap::new();
    let engines = TokioMutex::new(engines_map);
    let hyp_ctx = HypothesisTestContext { engines: &engines };
    let ctx = CounterexampleContext {
        repository: chronos_store::counterexample_repository::SessionStoreBackedCounterexampleRepository::new(std::sync::Arc::clone(&store)).into_arc(),
        hypothesis_ctx: &hyp_ctx,
    };

    let result =
        ChronosCounterexampleService::pull_engine_events(&ctx, "absent-session".to_string()).await;
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

    let store = std::sync::Arc::new(SessionStore::in_memory().expect("in_memory store"));
    let engines: HashMap<String, QueryEngine> = HashMap::new();
    let engines = TokioMutex::new(engines);
    let hyp_ctx = HypothesisTestContext { engines: &engines };
    let ctx = CounterexampleContext {
        repository: chronos_store::counterexample_repository::SessionStoreBackedCounterexampleRepository::new(std::sync::Arc::clone(&store)).into_arc(),
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

    let store = std::sync::Arc::new(SessionStore::in_memory().expect("in_memory store"));
    let engines: HashMap<String, QueryEngine> = HashMap::new();
    let engines = TokioMutex::new(engines);
    let hyp_ctx = HypothesisTestContext { engines: &engines };
    let ctx = CounterexampleContext {
        repository: chronos_store::counterexample_repository::SessionStoreBackedCounterexampleRepository::new(std::sync::Arc::clone(&store)).into_arc(),
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

    let store = std::sync::Arc::new(SessionStore::in_memory().expect("in_memory store"));
    let engines: HashMap<String, QueryEngine> = HashMap::new();
    let engines = TokioMutex::new(engines);
    let hyp_ctx = HypothesisTestContext { engines: &engines };
    let ctx = CounterexampleContext {
        repository: chronos_store::counterexample_repository::SessionStoreBackedCounterexampleRepository::new(std::sync::Arc::clone(&store)).into_arc(),
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

    let store = std::sync::Arc::new(SessionStore::in_memory().expect("in_memory store"));
    let engines: HashMap<String, QueryEngine> = HashMap::new();
    let engines = TokioMutex::new(engines);
    let hyp_ctx = HypothesisTestContext { engines: &engines };
    let ctx = CounterexampleContext {
        repository: chronos_store::counterexample_repository::SessionStoreBackedCounterexampleRepository::new(std::sync::Arc::clone(&store)).into_arc(),
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

    let store = std::sync::Arc::new(SessionStore::in_memory().expect("in_memory store"));
    let engines: HashMap<String, QueryEngine> = HashMap::new();
    let engines = TokioMutex::new(engines);
    let hyp_ctx = HypothesisTestContext { engines: &engines };
    let ctx = CounterexampleContext {
        repository: chronos_store::counterexample_repository::SessionStoreBackedCounterexampleRepository::new(std::sync::Arc::clone(&store)).into_arc(),
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

    let store = std::sync::Arc::new(SessionStore::in_memory().expect("in_memory store"));
    let engines: HashMap<String, QueryEngine> = HashMap::new();
    let engines = TokioMutex::new(engines);
    let hyp_ctx = HypothesisTestContext { engines: &engines };
    let ctx = CounterexampleContext {
        repository: chronos_store::counterexample_repository::SessionStoreBackedCounterexampleRepository::new(std::sync::Arc::clone(&store)).into_arc(),
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

    assert_eq!(
        loaded.schema_version,
        chronos_store::counterexample_storage::CURRENT_BUNDLE_SCHEMA_VERSION,
        "record.schema_version must equal CURRENT"
    );
    assert_eq!(
        loaded.summary.schema_version,
        chronos_store::counterexample_storage::CURRENT_BUNDLE_SCHEMA_VERSION,
        "summary.schema_version must equal CURRENT"
    );
}

// m9-02: save a bundle and verify events_count is persisted in the summary.
#[tokio::test]
async fn m9_02_save_persists_events_count_in_summary() {
    use crate::hypothesis_test::HypothesisTestContext;
    use chronos_domain::property::PropertyValue;
    use chronos_query::QueryEngine;
    use chronos_store::SessionStore;
    use std::collections::HashMap;
    use tokio::sync::Mutex as TokioMutex;

    let store = std::sync::Arc::new(SessionStore::in_memory().expect("in_memory store"));
    let engines: HashMap<String, QueryEngine> = HashMap::new();
    let engines = TokioMutex::new(engines);
    let hyp_ctx = HypothesisTestContext { engines: &engines };
    let ctx = CounterexampleContext {
        repository: chronos_store::counterexample_repository::SessionStoreBackedCounterexampleRepository::new(std::sync::Arc::clone(&store)).into_arc(),
        hypothesis_ctx: &hyp_ctx,
    };

    let saved = ChronosCounterexampleService::save(
        &ctx,
        "ws-events-count-test",
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

    // Load it back and verify events_count was persisted.
    let loaded = store
        .load_counterexample_bundle(&saved_id)
        .expect("load should succeed")
        .expect("bundle should exist");
    assert_eq!(
        loaded.summary.events_count, 0,
        "events_count must be 0 for bundle with no events"
    );
}

// m9-02: events_count returns summary.events_count when populated.
#[tokio::test]
async fn m9_02_events_count_reads_from_summary() {
    use crate::hypothesis_test::HypothesisTestContext;
    use chronos_domain::property::PropertyValue;
    use chronos_query::QueryEngine;
    use chronos_store::SessionStore;
    use std::collections::HashMap;
    use tokio::sync::Mutex as TokioMutex;

    let store = std::sync::Arc::new(SessionStore::in_memory().expect("in_memory store"));
    let engines: HashMap<String, QueryEngine> = HashMap::new();
    let engines = TokioMutex::new(engines);
    let hyp_ctx = HypothesisTestContext { engines: &engines };
    let ctx = CounterexampleContext {
        repository: chronos_store::counterexample_repository::SessionStoreBackedCounterexampleRepository::new(std::sync::Arc::clone(&store)).into_arc(),
        hypothesis_ctx: &hyp_ctx,
    };

    let saved = ChronosCounterexampleService::save(
        &ctx,
        "ws-events-count-summary",
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

    // events_count tool returns the summary value.
    let result = ChronosCounterexampleService::events_count(&ctx, &saved_id)
        .expect("events_count should succeed");
    match result {
        CounterexampleOutput::EventsCount { events_count, .. } => {
            assert_eq!(events_count, 0, "events_count must be 0");
        }
        _ => panic!("expected EventsCount variant"),
    }
}

// m9-04 §5: spec §5 services + replay chokepoint — save via ChronosCounterexampleService,
// then load events via bundle_events_or_legacy (the m9-02 chokepoint). Verifies the
// v3 side-table layout end-to-end through the services wrapper.
#[tokio::test]
async fn m9_04_save_load_roundtrip_through_services() {
    use crate::hypothesis_test::HypothesisTestContext;
    use chronos_domain::property::PropertyValue;
    use chronos_query::QueryEngine;
    use chronos_store::counterexample_storage::bundle_events_or_legacy;
    use chronos_store::SessionStore;
    use std::collections::HashMap;
    use tokio::sync::Mutex as TokioMutex;

    let store = std::sync::Arc::new(SessionStore::in_memory().expect("in_memory store"));
    let engines: HashMap<String, QueryEngine> = HashMap::new();
    let engines = TokioMutex::new(engines);
    let hyp_ctx = HypothesisTestContext { engines: &engines };
    let ctx = CounterexampleContext {
        repository: chronos_store::counterexample_repository::SessionStoreBackedCounterexampleRepository::new(std::sync::Arc::clone(&store)).into_arc(),
        hypothesis_ctx: &hyp_ctx,
    };

    // Build a bundle with 10 events.
    let events: Vec<_> = (0u64..10)
        .map(|id| {
            chronos_domain::TraceEvent::new(
                id,
                MonotonicNs::from(id * 100),
                1,
                chronos_domain::EventType::FunctionEntry,
                chronos_domain::SourceLocation::new("test.rs", 10, "fn", 0x1000 + id),
                chronos_domain::EventData::Function {
                    name: format!("fn_{id}"),
                    signature: None,
                    symbol_id: None,
                    invocation_id: None,
                    parent_invocation_id: None,
                },
            )
        })
        .collect();

    // Save via ChronosCounterexampleService::save — this writes v3 side-table chunks.
    let saved = ChronosCounterexampleService::save(
        &ctx,
        "ws-m9-04-integration",
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
        events.clone(),
    )
    .expect("save should succeed");
    let saved_id = match saved {
        CounterexampleOutput::Saved { summary, .. } => summary.bundle_id,
        _ => panic!("expected Saved variant"),
    };

    // Load the bundle record.
    let bundle = store
        .load_counterexample_bundle(&saved_id)
        .expect("load should succeed")
        .expect("bundle should exist");

    // Verify bundle.summary.events_count was set correctly.
    assert_eq!(
        bundle.summary.events_count, 10,
        "summary.events_count must match event count"
    );

    // Load events via bundle_events_or_legacy (the m9-02 chokepoint).
    let loaded_events =
        bundle_events_or_legacy(&store, &bundle).expect("bundle_events_or_legacy should succeed");

    assert_eq!(
        loaded_events.len(),
        10,
        "bundle_events_or_legacy must return all 10 events"
    );
    for (i, evt) in loaded_events.iter().enumerate() {
        assert_eq!(evt.event_id, i as u64, "event {i} must match original");
    }
}

// m9-91: closes m9-02-R4 — the new `events` accessor returns the full
// events stream via the service layer. Verifies (1) the load matches
// what was saved in order, (2) `next_offset == None` when the slice
// covers the full stream, (3) pagination via `limit` returns the
// correct next_offset, (4) a missing bundle returns LoadFailed.
#[tokio::test]
async fn m9_91_events_returns_full_stream_with_no_pagination() {
    use crate::hypothesis_test::HypothesisTestContext;
    use chronos_domain::property::PropertyValue;
    use chronos_query::QueryEngine;
    use chronos_store::SessionStore;
    use std::collections::HashMap;
    use tokio::sync::Mutex as TokioMutex;

    let store = std::sync::Arc::new(SessionStore::in_memory().expect("in_memory store"));
    let engines: HashMap<String, QueryEngine> = HashMap::new();
    let engines = TokioMutex::new(engines);
    let hyp_ctx = HypothesisTestContext { engines: &engines };
    let ctx = CounterexampleContext {
        repository: chronos_store::counterexample_repository::SessionStoreBackedCounterexampleRepository::new(std::sync::Arc::clone(&store)).into_arc(),
        hypothesis_ctx: &hyp_ctx,
    };

    // Build a bundle with 10 events.
    let events: Vec<chronos_domain::TraceEvent> = (0u64..10)
        .map(|id| {
            chronos_domain::TraceEvent::new(
                id,
                MonotonicNs::from(id * 100),
                1,
                chronos_domain::EventType::FunctionEntry,
                chronos_domain::SourceLocation::new("test.rs", 10, "fn", 0x1000 + id),
                chronos_domain::EventData::Function {
                    name: format!("fn_{id}"),
                    signature: None,
                    symbol_id: None,
                    invocation_id: None,
                    parent_invocation_id: None,
                },
            )
        })
        .collect();

    let saved = ChronosCounterexampleService::save(
        &ctx,
        "ws-m9-91-full",
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
        events.clone(),
    )
    .expect("save should succeed");
    let saved_id = match saved {
        CounterexampleOutput::Saved { summary, .. } => summary.bundle_id,
        _ => panic!("expected Saved variant"),
    };

    // Load via new `events` accessor (no pagination).
    let result = ChronosCounterexampleService::events(&ctx, &saved_id, None, None)
        .expect("events should succeed");
    match result {
        CounterexampleOutput::Events {
            bundle_id,
            events_count,
            returned_events,
            next_offset,
        } => {
            assert_eq!(bundle_id, saved_id);
            assert_eq!(events_count, 10, "events_count == total persisted");
            assert_eq!(returned_events.len(), 10, "default returns all events");
            assert_eq!(next_offset, None, "no more events after full slice");
            for (i, evt) in returned_events.iter().enumerate() {
                assert_eq!(evt.event_id, i as u64, "event {i} must match original");
            }
        }
        _ => panic!("expected Events variant"),
    }
}

#[tokio::test]
async fn m9_91_events_pagination_with_limit_and_offset() {
    use crate::hypothesis_test::HypothesisTestContext;
    use chronos_domain::property::PropertyValue;
    use chronos_query::QueryEngine;
    use chronos_store::SessionStore;
    use std::collections::HashMap;
    use tokio::sync::Mutex as TokioMutex;

    let store = std::sync::Arc::new(SessionStore::in_memory().expect("in_memory store"));
    let engines: HashMap<String, QueryEngine> = HashMap::new();
    let engines = TokioMutex::new(engines);
    let hyp_ctx = HypothesisTestContext { engines: &engines };
    let ctx = CounterexampleContext {
        repository: chronos_store::counterexample_repository::SessionStoreBackedCounterexampleRepository::new(std::sync::Arc::clone(&store)).into_arc(),
        hypothesis_ctx: &hyp_ctx,
    };

    // 10 events.
    let events: Vec<chronos_domain::TraceEvent> = (0u64..10)
        .map(|id| {
            chronos_domain::TraceEvent::new(
                id,
                MonotonicNs::from(id * 100),
                1,
                chronos_domain::EventType::FunctionEntry,
                chronos_domain::SourceLocation::new("test.rs", 10, "fn", 0x1000 + id),
                chronos_domain::EventData::Function {
                    name: format!("fn_{id}"),
                    signature: None,
                    symbol_id: None,
                    invocation_id: None,
                    parent_invocation_id: None,
                },
            )
        })
        .collect();
    let saved = ChronosCounterexampleService::save(
        &ctx,
        "ws-m9-91-page",
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
        events.clone(),
    )
    .expect("save should succeed");
    let saved_id = match saved {
        CounterexampleOutput::Saved { summary, .. } => summary.bundle_id,
        _ => panic!("expected Saved variant"),
    };

    // Page 1: offset=0, limit=3 -> events[0..3], next_offset=3.
    let p1 = ChronosCounterexampleService::events(&ctx, &saved_id, Some(3), Some(0))
        .expect("page 1 should succeed");
    match p1 {
        CounterexampleOutput::Events {
            events_count,
            returned_events,
            next_offset,
            ..
        } => {
            assert_eq!(events_count, 10);
            assert_eq!(returned_events.len(), 3);
            assert_eq!(returned_events[0].event_id, 0, "page 1 starts at event 0");
            assert_eq!(next_offset, Some(3));
        }
        _ => panic!("expected Events variant"),
    }

    // Page 2: offset=3, limit=3 -> events[3..6], next_offset=6.
    let p2 = ChronosCounterexampleService::events(&ctx, &saved_id, Some(3), Some(3))
        .expect("page 2 should succeed");
    match p2 {
        CounterexampleOutput::Events {
            events_count,
            returned_events,
            next_offset,
            ..
        } => {
            assert_eq!(events_count, 10);
            assert_eq!(returned_events.len(), 3);
            assert_eq!(returned_events[0].event_id, 3);
            assert_eq!(next_offset, Some(6));
        }
        _ => panic!("expected Events variant"),
    }

    // Page 4 (last partial): offset=9, limit=3 -> events[9..10],
    // next_offset=None.
    let p4 = ChronosCounterexampleService::events(&ctx, &saved_id, Some(3), Some(9))
        .expect("page 4 should succeed");
    match p4 {
        CounterexampleOutput::Events {
            events_count,
            returned_events,
            next_offset,
            ..
        } => {
            assert_eq!(events_count, 10);
            assert_eq!(returned_events.len(), 1, "only 1 event left after offset 9");
            assert_eq!(returned_events[0].event_id, 9);
            assert_eq!(next_offset, None);
        }
        _ => panic!("expected Events variant"),
    }

    // Out-of-range offset: offset=20 returns empty slice, next_offset=None.
    let p_oor = ChronosCounterexampleService::events(&ctx, &saved_id, Some(5), Some(20))
        .expect("out-of-range page should succeed");
    match p_oor {
        CounterexampleOutput::Events {
            events_count,
            returned_events,
            next_offset,
            ..
        } => {
            assert_eq!(events_count, 10);
            assert!(returned_events.is_empty());
            assert_eq!(next_offset, None);
        }
        _ => panic!("expected Events variant"),
    }
}

#[tokio::test]
async fn m9_91_events_missing_bundle_returns_load_failed() {
    use crate::hypothesis_test::HypothesisTestContext;
    use chronos_query::QueryEngine;
    use chronos_store::SessionStore;
    use std::collections::HashMap;
    use tokio::sync::Mutex as TokioMutex;

    let store = std::sync::Arc::new(SessionStore::in_memory().expect("in_memory store"));
    let engines: HashMap<String, QueryEngine> = HashMap::new();
    let engines = TokioMutex::new(engines);
    let hyp_ctx = HypothesisTestContext { engines: &engines };
    let ctx = CounterexampleContext {
        repository: chronos_store::counterexample_repository::SessionStoreBackedCounterexampleRepository::new(std::sync::Arc::clone(&store)).into_arc(),
        hypothesis_ctx: &hyp_ctx,
    };

    let result = ChronosCounterexampleService::events(&ctx, "no-such-bundle", None, None);
    match result {
        Err(crate::error::ServiceError::LoadFailed(msg)) => {
            assert!(
                msg.contains("no counterexample bundle with id `no-such-bundle`"),
                "error message must identify the missing bundle: {msg}",
            );
        }
        other => panic!("expected LoadFailed, got {other:?}"),
    }
}
