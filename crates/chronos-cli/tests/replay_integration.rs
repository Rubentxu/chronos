// Integration tests for `chronos test replay` that exercise the m9-04 v3 key layout
// and v2 fallback through the full CLI replay path.
//
// These tests live in `tests/` (per-crate integration) rather than in the `src/`
// module tree because they open independent redb database files and call
// `run_replay` as a public async function that requires a clean database file per test.

use std::path::PathBuf;

fn temp_db_path(label: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "chronos-cli-replay-integration-{label}-{}.db",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&p);
    p
}

/// m9-04 §5: spec §5 replay chokepoint — a bundle saved via v3 layout (post-m9-04)
/// is replayed through run_replay. The replay must successfully load events from the
/// v3 side table (bundle_events_or_legacy → load_counterexample_bundle_events →
/// collect_bundle_chunks range scan) and produce a structured ReplayReport.
#[tokio::test]
async fn m9_04_replay_uses_v3_layout() {
    use chronos_cli::replay::run_replay;
    use chronos_domain::property::PropertyValue;
    use chronos_store::counterexample_storage::bundle_events_or_legacy;
    use chronos_store::SessionStore;

    let path = temp_db_path("v3-replay");

    // Save a bundle with events through the SessionStore (writes v3 side-table chunks).
    // Use bundle_events_or_legacy to verify events are stored in v3 layout.
    {
        let store = SessionStore::open(&path).expect("open store");

        // Build events manually (same shape as m9-04 v3 save).
        let events: Vec<_> = (0u64..5)
            .map(|id| {
                chronos_domain::TraceEvent::new(
                    id,
                    id * 100,
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

        let bundle_id = "b-v3-replay-test";
        let rec = chronos_store::counterexample_storage::CounterexampleBundleRecord {
            summary: chronos_store::counterexample_storage::CounterexampleBundleSummary {
                bundle_id: bundle_id.into(),
                property_kind: "invariant".into(),
                workspace_id: "ws-v3-test".into(),
                created_at_ms: 0,
                rounds_used: 1,
                has_full_bundle: true,
                schema_version: 1,
                events_count: 5,
            },
            events,
            minimised: Some(
                chronos_store::counterexample_storage::MinimisedPayload::Constant(
                    PropertyValue::Number(0.0),
                ),
            ),
            event_cas_hashes: vec![],
            target_hypothesis: None,
            schema_version: 1,
        };
        store
            .save_counterexample_bundle(rec)
            .expect("save v3 bundle");

        // Verify v3 side table has the events via bundle_events_or_legacy.
        let bundle = store
            .load_counterexample_bundle(bundle_id)
            .expect("load bundle")
            .expect("bundle exists");
        let side_events =
            bundle_events_or_legacy(&store, &bundle).expect("bundle_events_or_legacy");
        assert_eq!(
            side_events.len(),
            5,
            "v3 side table must contain all 5 events"
        );
    } // store dropped; redb lock released

    // Replay the v3 bundle — events must be loaded from v3 side table.
    let report = run_replay(&path, "b-v3-replay-test")
        .await
        .expect("replay should succeed");

    assert_eq!(report.bundle_id, "b-v3-replay-test");
    assert_eq!(report.property_kind, "invariant");
    assert_eq!(
        report.events_in_bundle, 5,
        "replay must load 5 events from v3"
    );
    assert!(
        matches!(
            report.replay_verdict.as_str(),
            "unsupported" | "violation" | "pass"
        ),
        "replay must produce a valid verdict, got {}",
        report.replay_verdict
    );

    let _ = std::fs::remove_file(&path);
}

/// m9-04 §5: spec §5 replay v2 fallback — a bundle injected with v2 legacy chunks
/// (pre-m9-04 format, never re-saved) is replayed through run_replay. The replay
/// must successfully load events from the v2 fallback path
/// (collect_bundle_chunks → v2 fallback when v3 is empty AND events_count > 0).
#[tokio::test]
async fn m9_04_replay_v2_bundle_uses_legacy_path() {
    use chronos_cli::replay::run_replay;
    use chronos_store::SessionStore;

    let path = temp_db_path("v2-fallback-replay");
    let bundle_id = "b-v2-fallback-test";

    // Inject a v2 bundle directly: v2 chunk in side table + bundle record with
    // events_count > 0 (to trigger D7 fallback guard) and empty events blob.
    {
        let store = SessionStore::open(&path).expect("open store");

        // Write a v2 chunk to the side table via the m9-05 R4 test chokepoint
        // (replaces the previous raw db().begin_write() + open_table(...).insert(...) path).
        let v2_events = vec![chronos_domain::TraceEvent::new(
            42,
            4200,
            1,
            chronos_domain::EventType::FunctionEntry,
            chronos_domain::SourceLocation::new("legacy.rs", 1, "legacy_fn", 0x2000),
            chronos_domain::EventData::Function {
                name: "legacy_fn".into(),
                signature: None,
                symbol_id: None,
                invocation_id: None,
                parent_invocation_id: None,
            },
        )];
        store
            .insert_v2_chunk_for_test(bundle_id, 0, &v2_events)
            .expect("insert v2 chunk");

        // Write the bundle record with events_count > 0 (triggers v2 fallback).
        // Inject directly — save_counterexample_bundle overwrites events_count
        // based on events.len(), which would reset it to 0 here.
        let rec = chronos_store::counterexample_storage::CounterexampleBundleRecord {
            summary: chronos_store::counterexample_storage::CounterexampleBundleSummary {
                bundle_id: bundle_id.into(),
                property_kind: "invariant".into(),
                workspace_id: "ws-v2-test".into(),
                created_at_ms: 0,
                rounds_used: 1,
                has_full_bundle: true,
                schema_version: 1,
                events_count: 1, // D7 guard: events_count > 0 triggers v2 fallback
            },
            events: vec![], // events are in v2 side table, not blob
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
            .insert_bundle_record_for_test(&rec)
            .expect("insert bundle record");
    } // store dropped; redb lock released

    // Replay the v2 bundle — events must be loaded from v2 fallback path.
    let report = run_replay(&path, bundle_id)
        .await
        .expect("replay should succeed");

    assert_eq!(report.bundle_id, bundle_id);
    assert_eq!(
        report.events_in_bundle, 1,
        "replay must load 1 event from v2 fallback"
    );
    // Verify that the v3 side table has no v3 chunks for this bundle
    // (via the m9-05 R4 test chokepoint).
    {
        let store = SessionStore::open(&path).expect("open store");
        let v3_count = store
            .count_v3_chunks_for_test(bundle_id)
            .expect("count v3 chunks");
        assert_eq!(v3_count, 0, "v2 fallback bundle must have no v3 chunks");
    }

    let _ = std::fs::remove_file(&path);
}
