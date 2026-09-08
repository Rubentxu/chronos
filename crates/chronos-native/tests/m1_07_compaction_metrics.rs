//! Integration tests for m1-07 — exposing `chronos-log`'s
//! `CompactionMetrics` through `NativeProbeBackend::compaction_metrics()`
//! so the new MCP tool `probe_compaction_metrics` can read them.
//!
//! These tests do NOT spawn a live probe. They construct a
//! `NativeProbeBackend`, attach a pre-built `SegmentedExecutionLog`
//! via the `#[doc(hidden)]` test slot (mirroring what the M1
//! integration test files do), drive a few records + a
//! `compact_up_to`, and assert the metrics surface is accurate.
//!
//! The sandbox UAT for the actual MCP tool lives in
//! `chronos-sandbox/tests/m1_07_compaction_metrics.rs` (T4 smoke).

use chronos_domain::{EventData, EventType, SourceLocation, TraceEvent};
use chronos_log::{
    EventSeq, ExecutionPayload, LogConsumerId, NewExecutionRecord, SegmentedConfig,
    SegmentedExecutionLog, SessionId,
};
use chronos_native::probe_backend::NativeProbeBackend;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;

fn tempdir(label: &str) -> PathBuf {
    let base = std::env::temp_dir();
    let unique = format!(
        "chronos-m1-07-{}-{}-{}",
        label,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let p = base.join(unique);
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// `compaction_metrics()` returns `Ok(None)` when the backend was
/// not configured with `with_execution_log_dir` (and no log was
/// attached afterwards).
#[test]
fn compaction_metrics_returns_none_when_no_log_attached() {
    let bus = chronos_domain::bus::EventBus::new_shared(1024);
    let backend = NativeProbeBackend::new(bus);
    let m = backend.compaction_metrics().expect("call");
    assert!(m.is_none(), "no log attached ⇒ None");
}

/// `compaction_metrics()` returns `Ok(Some(zeros))` when a log is
/// attached but no compaction runs have happened yet.
#[test]
fn compaction_metrics_returns_zeros_when_log_attached() {
    let dir = tempdir("zeros");
    let bus = chronos_domain::bus::EventBus::new_shared(1024);
    let backend = NativeProbeBackend::new(bus).with_execution_log_dir(Some(dir.clone()));

    let log_session = "native-compaction-zeros";
    let log_dir = dir.join(log_session);
    let log = Arc::new(
        SegmentedExecutionLog::open(
            SessionId::new(log_session),
            SegmentedConfig::with_dir(&log_dir),
        )
        .expect("open log"),
    );
    {
        let mut slot = backend
            .execution_log_slot_for_test()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *slot = Some(log.clone());
    }

    let m = backend
        .compaction_metrics()
        .expect("call")
        .expect("attached");
    assert_eq!(m.segments_removed_total, 0);
    assert_eq!(m.bytes_reclaimed_total, 0);
    assert_eq!(m.compaction_runs_total, 0);

    let _ = std::fs::remove_dir_all(&dir);
}

/// After a real `compact_up_to` on the attached log, the
/// `compaction_metrics` snapshot reflects the run.
#[test]
fn compaction_metrics_reflects_real_compaction_runs() {
    let dir = tempdir("runs");
    let bus = chronos_domain::bus::EventBus::new_shared(1024);
    let backend = NativeProbeBackend::new(bus).with_execution_log_dir(Some(dir.clone()));

    let log_session = "native-compaction-runs";
    let log_dir = dir.join(log_session);
    // Force tight flush threshold so 4 records ⇒ 2 segments.
    let mut cfg = SegmentedConfig::with_dir(&log_dir);
    cfg.flush_threshold = NonZeroUsize::new(2).unwrap();
    let log =
        Arc::new(SegmentedExecutionLog::open(SessionId::new(log_session), cfg).expect("open"));

    {
        let mut slot = backend
            .execution_log_slot_for_test()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *slot = Some(log.clone());
    }

    // 4 records → 2 segments. Encode one TraceEvent so the JSON
    // payload is realistic; payload bytes do not need to round-
    // trip because the consumer here is just the snapshot accessor.
    for i in 0..4u64 {
        let ev = TraceEvent {
            event_id: i,
            timestamp_ns: i * 10,
            thread_id: 1,
            event_type: EventType::FunctionEntry,
            location: SourceLocation::default(),
            data: EventData::Function {
                name: format!("fn-{}", i),
                signature: None,
                symbol_id: None,
                invocation_id: None,
                parent_invocation_id: None,
            },
        };
        let bytes = serde_json::to_vec(&ev).expect("encode");
        log.append(NewExecutionRecord {
            session_id: SessionId::new(log_session),
            monotonic_ns: i * 10,
            payload: ExecutionPayload::new(bytes, "FunctionEntry"),
            ..Default::default()
        })
        .expect("append");
    }
    log.flush().expect("flush");
    assert_eq!(log.flushed_segments().len(), 2);

    // Commit a cursor that covers the first segment (seq 0..1),
    // then compact.
    let consumer = LogConsumerId::new("slow");
    log.commit_cursor(&consumer, EventSeq::new(1)).unwrap();
    let removed = log.compact_up_to(EventSeq::new(1)).unwrap();
    assert_eq!(removed.len(), 1, "one segment should be removed");

    let m = backend
        .compaction_metrics()
        .expect("call")
        .expect("attached");
    assert_eq!(m.segments_removed_total, 1, "1 segment removed");
    assert_eq!(m.compaction_runs_total, 1, "1 run");
    assert!(
        m.bytes_reclaimed_total > 0,
        "size snapshot should be > 0 (was {})",
        m.bytes_reclaimed_total
    );

    // Idempotent re-call: still the same numbers.
    let removed_again = log.compact_up_to(EventSeq::new(1)).unwrap();
    assert!(removed_again.is_empty());
    let m2 = backend
        .compaction_metrics()
        .expect("call")
        .expect("attached");
    assert_eq!(m2.segments_removed_total, 1);
    assert_eq!(m2.compaction_runs_total, 1);

    let _ = std::fs::remove_dir_all(&dir);
}
