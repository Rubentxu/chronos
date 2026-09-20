//! REC-C2.1.7 (DoD 7) — the firing count is reconstructed from reopened
//! durable evidence, with no runtime state involved.

use std::path::PathBuf;
use std::sync::Arc;

use chronos_domain::trace::TraceEvent;
use chronos_domain::MonotonicNs;
use chronos_domain::{EventData, EventType, SourceLocation};
use chronos_domain::{TripwireCondition, TripwireId};
use chronos_log::{
    tripwire_evidence_codec as codec, ExecutionKind, ExecutionPayload, NewExecutionRecord,
    SegmentedConfig, SegmentedExecutionLog, SessionId, TripwireFiredEvidence,
};
use chronos_services::session_log::SessionExecutionLog;
use chronos_services::tripwire_evidence::{firing_count_snapshot, FiringCountStatus};

fn tempdir(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "chronos-c21-count-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("create tempdir");
    p
}

fn trace_event(event_id: u64) -> TraceEvent {
    TraceEvent {
        event_id,
        timestamp_ns: MonotonicNs::from(event_id * 1000),
        thread_id: 1,
        event_type: EventType::FunctionEntry,
        location: SourceLocation {
            function: Some("main_work".to_string()),
            ..SourceLocation::default()
        },
        data: EventData::Function {
            name: "main_work".to_string(),
            signature: None,
            symbol_id: None,
            invocation_id: None,
            parent_invocation_id: None,
        },
    }
}

#[test]
fn firing_count_is_reconstructed_from_reopened_evidence() {
    let dir = tempdir("reopen");
    let session = SessionId::new("rec-c2-1-count-reopen");

    // --- Produce three firings, then drop every runtime handle. ---
    {
        let log = SegmentedExecutionLog::open(session.clone(), SegmentedConfig::with_dir(&dir))
            .expect("open");
        for i in 1..=3u64 {
            let source = log
                .append(NewExecutionRecord {
                    session_id: session.clone(),
                    kind: ExecutionKind::Raw,
                    monotonic_ns: i * 1000,
                    payload: ExecutionPayload::new(
                        serde_json::to_vec(&trace_event(i)).expect("encode"),
                        "trace_event",
                    ),
                    ..Default::default()
                })
                .expect("append raw");
            let evidence = TripwireFiredEvidence {
                tripwire_id: TripwireId(42),
                source_seq: source,
                source_event_id: Some(i),
                condition: TripwireCondition::FunctionName {
                    pattern: "main*".to_string(),
                },
                label: None,
                source_timestamp_ns: i * 1000,
                source_thread_id: 1,
            };
            log.append(NewExecutionRecord {
                session_id: session.clone(),
                kind: ExecutionKind::TripwireFired,
                monotonic_ns: i * 1000,
                payload: codec::encode(&evidence).expect("encode evidence"),
                ..Default::default()
            })
            .expect("append firing");
        }
        log.flush().expect("flush");
    }

    // --- Reopen: no manager, no runtime state, only the durable log. ---
    let reopened = SegmentedExecutionLog::open(session.clone(), SegmentedConfig::with_dir(&dir))
        .expect("reopen");
    let adopted = SessionExecutionLog::from_segmented_log(
        session.clone(),
        Arc::new(reopened),
        Some(dir.clone()),
    );

    let snapshot = firing_count_snapshot(&adopted, 100_000).expect("snapshot");
    assert_eq!(
        snapshot.count_for(TripwireId(42)),
        3,
        "the count is reconstructed from evidence, not from any counter"
    );
    assert_eq!(snapshot.status(), FiringCountStatus::Complete);
    assert!(snapshot.exhausted);
    assert!(!snapshot.history_truncated, "nothing was retired");
    assert_eq!(snapshot.from_seq.0, 0);

    let _ = std::fs::remove_dir_all(&dir);
}
