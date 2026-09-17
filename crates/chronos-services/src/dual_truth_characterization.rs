//! REC-C1.7.1 — Dual-truth characterization.
//!
//! RED on `main` by construction. These tests prove the diagnosis that
//! `execution_query`, `state_query`, and `trace_slice` can disagree with
//! `SessionExecutionLog` whenever the in-memory `QueryEngine` map was
//! built from a `drain_raw_events` snapshot rather than from the durable
//! log.
//!
//! After C1.7.2 lands (`chronos_services::projection::build_engine`
//! becomes the single canonical builder and is invoked on every read
//! path), these tests flip GREEN.
//!
//! The diagnosis is the user's original sketch:
//!
//! ```text
//! ExecutionLog
//!    │
//!    └── events_read     ← verdad canónica
//!
//! EngineMap (from drain_raw_events)
//!    │
//!    ├── execution_query
//!    ├── state_query
//!    └── trace_slice
//! ```
//!
//! Until the engine is rebuilt from the log, the two trees disagree on
//! at least three operations.

// These tests are intentionally RED on `main` by construction. They
// panic before reaching the lines that consume several of the imports
// above. Clippy therefore flags the imports as unused, but they ARE
// required for the GREEN path that the test would take once the
// divergence is closed by the wrapper-side gate. The `#[allow]` keeps
// `-D warnings` clean without papering over the test design.
//
// Applied file-wide because the items below are referenced from tests
// that abort before reaching the use sites; this is a documented
// divergence characterization, not dead code.
#![allow(unused_imports, dead_code)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use chronos_domain::trace::TraceEvent;
use chronos_log::{ExecutionPayload, NewExecutionRecord, SessionId};
use chronos_query::QueryEngine;
use serde_json::json;
use tokio::sync::Mutex as TokioMutex;

use crate::execution_query::{
    ChronosExecutionQueryService, ExecutionQueryContext, ExecutionQueryInput,
};
use crate::output::{ExecutionQueryKind, TraceSliceKind};
use crate::session_log::SessionExecutionLog;
use crate::trace_slice::{ChronosTraceSliceService, TraceSliceContext, TraceSliceInput};

/// Stdlib-only tempdir helper (matches the pattern in
/// `session_export.rs::tests` so we don't add a `tempfile` dev-dep).
fn make_tempdir(tag: &str) -> PathBuf {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let p = std::env::temp_dir().join(format!("chronos-c1-7-{tag}-{pid}-{nanos}"));
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// Build a session log with `n` records at deterministic seqs.
///
/// Each record's payload is JSON-encoded so the canonical
/// `events_log_read::decode` helper would round-trip it back to a
/// `TraceEvent`. We never call `decode` here (this is the services
/// layer, not the read path); we use the records only to count what
/// the log actually contains.
fn session_log_with(n: u64, dir: &std::path::Path) -> (SessionExecutionLog, SessionId) {
    let session_id = SessionId::new("dual-truth-fixture");
    let log = SessionExecutionLog::create(dir, session_id.clone()).expect("create log");

    for i in 0..n {
        let ev = TraceEvent::new(
            40 + i,                  // event_id (deliberately ≠ seq)
            10_000_500 + i * 1_000, // timestamp_ns
            1,                      // thread_id
            chronos_domain::EventType::FunctionEntry,
            chronos_domain::SourceLocation::default(),
            chronos_domain::EventData::Empty,
        );
        let payload =
            ExecutionPayload::new(serde_json::to_vec(&ev).expect("encode"), "trace_event");
        let handle = log.handle();
        handle
            .append(NewExecutionRecord {
                session_id: session_id.clone(),
                monotonic_ns: 10_000_500 + i * 1_000,
                payload,
                invocation_id: None,
                parent_invocation_id: None,
                symbol_id: None,
            })
            .expect("append");
    }
    log.handle().flush().ok();
    (log, session_id)
}

/// A `QueryEngine` synthesized from `events` — this is what
/// `ProbeService::stop` (today) or `chronos_services::projection::build_engine`
/// (C1.7.2+) would feed into the engines map.
#[allow(dead_code)] // used once C1.7.2 introduces projection::build_engine
fn engine_from(events: Vec<TraceEvent>) -> QueryEngine {
    QueryEngine::new(events)
}

#[tokio::test]
async fn dual_truth_execution_query_says_session_not_found_while_log_has_records() {
    // 1. Durable log holds N records.
    let dir = make_tempdir("notfound");
    let (_log, session_id) = session_log_with(10, &dir);
    let session_id_str = session_id.as_str().to_string();

    // 2. Engines map is EMPTY: nothing has called `build_and_store_engine`
    // yet (no probe ever ran, no stop drain happened, no projection).
    let engines: Arc<TokioMutex<HashMap<String, QueryEngine>>> =
        Arc::new(TokioMutex::new(HashMap::new()));
    let empty_meta: Arc<TokioMutex<HashMap<String, crate::projection::ProjectionMeta>>> =
        Arc::new(TokioMutex::new(HashMap::new()));
    let ctx = ExecutionQueryContext {
        engines: &engines,
        projection_meta: &empty_meta,
    };

    // 3. Agent calls `execution_query` (kind=execution_summary). Today
    // the engine map is the only source, so this returns
    // SessionNotFound — even though the SessionExecutionLog holds
    // 10 records that events_read could answer.
    let input = ExecutionQueryInput {
        session_id: session_id_str.clone(),
        kind: ExecutionQueryKind::ExecutionSummary,
        event_id: None,
        max_depth: None,
        threshold_ns: None,
        top_n: None,
        saliency_limit: None,
    };
    let result = ChronosExecutionQueryService::query(&ctx, input).await;

    // RED: today this fails with SessionNotFound. After C1.7.2 +
    // C1.7.3 this returns ExecutionSummary{total_events: 10, ...}.
    assert!(
        result.is_ok(),
        "execution_query must derive from SessionExecutionLog; \
         got SessionNotFound while log holds 10 records: {:?}",
        result.err()
    );
    let summary = result.unwrap();
    let value = serde_json::to_value(&summary).expect("encode");
    assert_eq!(
        value
            .get("summary")
            .and_then(|v| v.get("total_events"))
            .cloned(),
        Some(json!(10)),
        "summary.total_events must match the durable log"
    );
}

#[tokio::test]
async fn dual_truth_trace_slice_says_session_not_found_while_log_has_records() {
    let dir = make_tempdir("slice-notfound");
    let (_log, session_id) = session_log_with(5, &dir);
    let session_id_str = session_id.as_str().to_string();

    let engines: Arc<TokioMutex<HashMap<String, QueryEngine>>> =
        Arc::new(TokioMutex::new(HashMap::new()));
    let empty_meta: Arc<TokioMutex<HashMap<String, crate::projection::ProjectionMeta>>> =
        Arc::new(TokioMutex::new(HashMap::new()));
    let ctx = TraceSliceContext {
        engines: &engines,
        projection_meta: &empty_meta,
    };

    let input = TraceSliceInput {
        session_id: session_id_str,
        slice_kind: TraceSliceKind::Crash, // no extra params required
        variable_name: None,
        address: None,
        limit: 100,
    };
    let result = ChronosTraceSliceService::slice(&ctx, input).await;

    // RED: SessionNotFound today. After C1.7.2 + C1.7.3 this returns
    // a Crash slice that knows there are zero crashes in the log
    // (no Crash events in the fixture), but the session IS queryable.
    assert!(
        result.is_ok(),
        "trace_slice must derive from SessionExecutionLog; \
         got SessionNotFound while log holds 5 records: {:?}",
        result.err()
    );
}

#[tokio::test]
async fn dual_truth_engine_built_before_late_append_misses_late_records() {
    // This is the most damning form of the divergence: the engine
    // exists and the log exists, but they disagree because the engine
    // was built before later records were appended. Today this is the
    // default state of any session whose probe has stopped (the
    // drain happened at stop time) and whose log subsequently
    // received records from a late append (eBPF tail, post-stop
    // async events, retentions that re-anchor, etc.).

    let dir = make_tempdir("late-append");
    let (log, session_id) = session_log_with(3, &dir);
    let session_id_str = session_id.as_str().to_string();

    // 1. Engine is built from the FIRST snapshot — only 3 events.
    let first_snapshot: Vec<TraceEvent> = (0..3)
        .map(|i| {
            TraceEvent::new(
                40 + i,
                10_000_500 + i * 1_000,
                1,
                chronos_domain::EventType::FunctionEntry,
                chronos_domain::SourceLocation::default(),
                chronos_domain::EventData::Empty,
            )
        })
        .collect();
    let engine = engine_from(first_snapshot);
    let engines: Arc<TokioMutex<HashMap<String, QueryEngine>>> =
        Arc::new(TokioMutex::new(HashMap::new()));
    let empty_meta: Arc<TokioMutex<HashMap<String, crate::projection::ProjectionMeta>>> =
        Arc::new(TokioMutex::new(HashMap::new()));
    engines.lock().await.insert(session_id_str.clone(), engine);

    // 2. THREE MORE records are appended to the durable log after the
    // engine snapshot was taken.
    for i in 3..6 {
        let ev = TraceEvent::new(
            40 + i,
            10_000_500 + i * 1_000,
            1,
            chronos_domain::EventType::FunctionEntry,
            chronos_domain::SourceLocation::default(),
            chronos_domain::EventData::Empty,
        );
        let payload =
            ExecutionPayload::new(serde_json::to_vec(&ev).expect("encode"), "trace_event");
        log.handle()
            .append(NewExecutionRecord {
                session_id: session_id.clone(),
                monotonic_ns: 10_000_500 + i * 1_000,
                payload,
                invocation_id: None,
                parent_invocation_id: None,
                symbol_id: None,
            })
            .expect("append");
    }
    log.handle().flush().ok();

    // 3. Agent queries execution_query. Today the engine (3 events)
    // wins; the log (6 events) is ignored. After C1.7.2 the engine
    // is rebuilt from the log and returns 6.
    let ctx = ExecutionQueryContext {
        engines: &engines,
        projection_meta: &empty_meta,
    };
    let input = ExecutionQueryInput {
        session_id: session_id_str,
        kind: ExecutionQueryKind::ExecutionSummary,
        event_id: None,
        max_depth: None,
        threshold_ns: None,
        top_n: None,
        saliency_limit: None,
    };
    let summary = ChronosExecutionQueryService::query(&ctx, input)
        .await
        .expect("query ok");

    let value = serde_json::to_value(&summary).expect("encode");
    assert_eq!(
        value
            .get("summary")
            .and_then(|v| v.get("total_events"))
            .cloned(),
        Some(json!(6)),
        "execution_query must reflect the durable log, not a stale engine snapshot"
    );
}
