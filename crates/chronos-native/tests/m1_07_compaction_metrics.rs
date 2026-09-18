//! Integration tests for m1-07 — verifying that
//! `SegmentedExecutionLog::compaction_metrics()` exposes the truth of
//! `chronos-log`'s compaction run, so the new MCP tool
//! `probe_compaction_metrics` can read it.
//!
//! REC-C3.3.2: `compaction_metrics()` is no longer a method on
//! `NativeProbeBackend` — the canonical owner of that capability is
//! the `ExecutionLogProvider`. Because the port intentionally
//! excludes storage-maintenance surface area
//! (`ExecutionLogProvider` does NOT expose `compaction_metrics`,
//! `flush`, or `compact_up_to`), production readers go through the
//! `SessionExecutionLog` wrapper in `chronos-services`, and the
//! concrete accessor that exposes the snapshot directly is
//! `SegmentedExecutionLog::compaction_metrics()`.
//!
//! This file exercises that concrete accessor in the way the
//! production wrapper does. Sandbox UAT for the MCP tool itself
//! lives in `chronos-sandbox/tests/m1_07_compaction_metrics.rs`
//! (T4 smoke).

use chronos_domain::{EventData, EventType, SourceLocation, TraceEvent};
use chronos_log::{
    EventSeq, ExecutionPayload, LogConsumerId, NewExecutionRecord, SegmentedConfig,
    SegmentedExecutionLog, SessionId,
};
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

/// A freshly-opened `SegmentedExecutionLog` exposes the all-zero
/// `compaction_metrics` snapshot — there have been no runs.
#[test]
fn compaction_metrics_returns_zeros_when_log_freshly_opened() {
    let dir = tempdir("zeros");
    let log_session = "native-compaction-zeros";
    let log_dir = dir.join(log_session);
    let log: Arc<SegmentedExecutionLog> = Arc::new(
        SegmentedExecutionLog::open(
            SessionId::new(log_session),
            SegmentedConfig::with_dir(&log_dir),
        )
        .expect("open log"),
    );

    let m = log.compaction_metrics();
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
    let log_session = "native-compaction-runs";
    let log_dir = dir.join(log_session);
    // Force tight flush threshold so 4 records ⇒ 2 segments.
    let mut cfg = SegmentedConfig::with_dir(&log_dir);
    cfg.flush_threshold = NonZeroUsize::new(2).unwrap();
    let log =
        Arc::new(SegmentedExecutionLog::open(SessionId::new(log_session), cfg).expect("open"));

    // 4 records → 2 segments. Encode one TraceEvent so the JSON
    // payload is realistic; payload bytes do not need to round-trip
    // because the consumer here is just the snapshot accessor.
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
            kind: chronos_log::ExecutionKind::Raw,

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

    let m = log.compaction_metrics();
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
    let m2 = log.compaction_metrics();
    assert_eq!(m2.segments_removed_total, 1);
    assert_eq!(m2.compaction_runs_total, 1);

    let _ = std::fs::remove_dir_all(&dir);
}
