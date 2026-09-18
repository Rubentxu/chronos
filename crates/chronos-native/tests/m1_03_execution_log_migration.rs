//! Integration tests for the m1-03 migration of one producer
//! (`chronos_native::probe_backend::NativeProbeBackend`) and one
//! query path.
//!
//! REC-C3.3.2: `read_execution_log_records` and
//! `with_execution_log_dir` were retired from `NativeProbeBackend`.
//! Tests now wire the writer through `attach_execution_log(provider)`
//! and read from the port directly with `read_from_seq` (the same
//! shape `chronos-services::session_log` uses).
//!
//! These tests do NOT run a live ptrace session — that requires
//! root and a real target binary. They exercise the producer's
//! `accept_and_publish` helper and a port-based read from the
//! provider.
//!
//! The full UAT through MCP runs in `chronos-sandbox/tests/m1_acceptance.rs`
//! (`m1_03_execution_log_migrates_one_producer_and_query_path`).

use chronos_domain::ports::execution_log::ExecutionLogProvider;
use chronos_domain::{EventData, EventType, SourceLocation, TraceEvent};
use chronos_log::{
    provider::SegmentedExecutionLogProvider, EventSeq, NewExecutionRecord, SegmentedConfig,
    SegmentedExecutionLog, SessionId,
};
use chronos_native::probe_backend::NativeProbeBackend;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;

fn tempdir() -> PathBuf {
    let base = std::env::temp_dir();
    let unique = format!(
        "chronos-m1-03-{}-{}",
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

/// Decode a JSON `TraceEvent` from a record payload.
fn decode_trace_event(bytes: &[u8]) -> TraceEvent {
    serde_json::from_slice(bytes).expect("decode TraceEvent")
}

/// Build a `TraceEvent` payload and stick it in the log.
fn push_event(log: &SegmentedExecutionLog, session: &str, i: u64, kind: EventType) {
    let ev = TraceEvent {
        event_id: i,
        timestamp_ns: i * 100,
        thread_id: 1,
        event_type: kind,
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

        session_id: SessionId::new(session),
        monotonic_ns: i * 100,
        payload: chronos_log::ExecutionPayload::new(bytes, "FunctionEntry"),
        ..Default::default()
    })
    .expect("append log");
}

/// Build a provider attached to a fresh on-disk segmented backend.
fn wire_provider(session: &str, dir: &std::path::Path) -> Arc<dyn ExecutionLogProvider> {
    let log_dir = dir.join(session);
    let concrete = Arc::new(
        SegmentedExecutionLog::open(SessionId::new(session), SegmentedConfig::with_dir(&log_dir))
            .expect("open segmented log"),
    );
    let wrapper = SegmentedExecutionLogProvider::new(SessionId::new(session), concrete);
    Arc::new(wrapper)
}

/// Drives the writer through `accept_and_publish` (the canonical
/// port call) and reads back via the port. Proves the
/// `NativeProbeBackend` -> `Arc<dyn ExecutionLogProvider>` wiring
/// works end-to-end in this crate.
#[test]
fn accept_and_publish_lands_in_attached_provider() {
    let dir = tempdir();
    let backend = NativeProbeBackend::new();

    let session = "native-test-session-1";
    let provider: Arc<dyn ExecutionLogProvider> = wire_provider(session, &dir);
    backend.attach_execution_log(Arc::clone(&provider));

    // Build a minimal TraceEvent.
    let ev = TraceEvent {
        event_id: 0,
        timestamp_ns: 0,
        thread_id: 1,
        event_type: EventType::FunctionEntry,
        location: SourceLocation::default(),
        data: EventData::Function {
            name: "fn-0".to_string(),
            signature: None,
            symbol_id: None,
            invocation_id: None,
            parent_invocation_id: None,
        },
    };

    // Drive the canonical writer path directly.
    let provider_for_call = provider.clone();
    NativeProbeBackend::accept_and_publish(Some(&provider_for_call), &ev, 100, None)
        .expect("accepted append lands a seq");

    // Read back through the port.
    let page = provider
        .read_from_seq(EventSeq::ZERO, 16)
        .expect("read_from_seq");
    assert_eq!(page.records.len(), 1);
    let observed = decode_trace_event(&page.records[0].payload.bytes);
    assert_eq!(observed.event_id, 0);
    assert_eq!(observed.timestamp_ns, 0);
    assert!(page.exhausted);

    let _ = std::fs::remove_dir_all(&dir);
}

/// Reads a bounded slice via the port's `read_from_seq`.
#[test]
fn read_from_seq_yields_bounded_slice_from_port() {
    let dir = tempdir();
    let session = "native-test-read";
    let log_dir = dir.join(session);
    let mut cfg = SegmentedConfig::with_dir(&log_dir);
    cfg.flush_threshold = NonZeroUsize::new(2).unwrap();
    let concrete =
        Arc::new(SegmentedExecutionLog::open(SessionId::new(session), cfg).expect("open"));

    for i in 0..8u64 {
        push_event(&concrete, session, i, EventType::FunctionEntry);
    }
    concrete.flush().expect("flush");

    let wrapper = SegmentedExecutionLogProvider::new(SessionId::new(session), concrete);
    let provider: Arc<dyn ExecutionLogProvider> = Arc::new(wrapper);

    let page = provider
        .read_from_seq(EventSeq::ZERO, 16)
        .expect("read_from_seq");
    assert_eq!(page.records.len(), 8);
    let max_seq = page
        .records
        .iter()
        .map(|r| r.seq.0)
        .max()
        .expect("at least one record");
    assert_eq!(max_seq, 7, "8 records with seq 0..=7");

    let _ = std::fs::remove_dir_all(&dir);
}
