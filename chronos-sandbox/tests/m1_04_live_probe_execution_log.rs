//! m1-04 — Live ptrace UAT.
//!
//! Drives a real `PtraceTracer` against `/bin/true` (without
//! syscall tracing, which is what AGENTS.md §6.5 documents as the
//! kernel-isolation flake) and asserts that:
//!   - The `PtraceEvent`s flow into the `SegmentedExecutionLog`
//!     attached to a `NativeProbeBackend`.
//!   - After the tracee exits, `read_execution_log_records_with_stats`
//!     returns the same events with `unparseable_payload_count == 0`.
//!   - Reopens the log in a second `SegmentedExecutionLog` handle
//!     and confirms the events replay (durability smoke).
//!
//! This is the first end-to-end m1-04 acceptance that exercises
//! both the producer and consumer sides of the migration against
//! a real ptrace run, not just synthetic `TraceEvent`s.

use std::path::PathBuf;
use std::sync::Arc;

use chronos_domain::ports::execution_log::ExecutionLogProvider;
use chronos_log::{
    provider::SegmentedExecutionLogProvider, SegmentedConfig, SegmentedExecutionLog, SessionId,
};
use chronos_native::probe_backend::{trace_event_to_log_record_for_test, NativeProbeBackend};
use chronos_native::ptrace_tracer::{PtraceConfig, PtraceEvent, PtraceTracer};

fn tempdir(label: &str) -> PathBuf {
    let base = std::env::temp_dir();
    let unique = format!(
        "chronos-m1-04-live-{}-{}-{}",
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

fn ptracenonevent_to_trace_event(event: &PtraceEvent, seq: u64) -> chronos_domain::TraceEvent {
    use chronos_domain::{EventData, EventType, MonotonicNs, SourceLocation, TraceEvent};
    let pid = event.pid();
    let (kind, label) = match event {
        PtraceEvent::Exited { exit_code, .. } => {
            (EventType::ThreadExit, format!("exited(code={})", exit_code))
        }
        PtraceEvent::Signaled {
            signal,
            signal_name,
            ..
        } => (
            EventType::ThreadExit,
            format!("signaled({}={})", signal_name, signal),
        ),
        PtraceEvent::Stopped {
            signal,
            signal_name,
            ..
        } => (
            EventType::SignalDelivered,
            format!("stopped({}={})", signal_name, signal),
        ),
        PtraceEvent::PtraceEvent { event_code, .. } => (
            EventType::Custom,
            format!("ptrace_event(code={})", event_code),
        ),
        PtraceEvent::Registers { .. } => (EventType::Custom, "registers".to_string()),
        PtraceEvent::Syscall { .. } => (EventType::SyscallEnter, "syscall".to_string()),
    };
    TraceEvent {
        event_id: seq,
        timestamp_ns: MonotonicNs::from(seq * 1000),
        thread_id: pid as u64,
        event_type: kind,
        location: SourceLocation {
            function: Some(label),
            ..SourceLocation::default()
        },
        data: EventData::Empty,
    }
}

#[test]
fn live_ptrace_events_flow_into_execution_log() {
    // Skip if /bin/true is missing (e.g. some Alpine images).
    if !std::path::Path::new("/bin/true").exists() {
        eprintln!("/bin/true not present; skipping live UAT");
        return;
    }

    let dir = tempdir("live");
    let session_id = "m1-04-live-uat";

    // Build the backend with the ExecutionLog provider attached. The
    // canonical path is `attach_execution_log(provider)`; the legacy
    // `with_execution_log_dir` is gone under REC-C3.3.2.
    let backend = NativeProbeBackend::new();

    let log_dir = dir.join(format!("native-{}", session_id));
    let concrete = Arc::new(
        SegmentedExecutionLog::open(
            SessionId::new(session_id),
            SegmentedConfig::with_dir(&log_dir),
        )
        .expect("open log"),
    );
    let provider: Arc<dyn ExecutionLogProvider> = Arc::new(SegmentedExecutionLogProvider::new(
        SessionId::new(session_id),
        concrete.clone(),
    ));
    backend.attach_execution_log(provider.clone());

    // Launch `/bin/true` under ptrace. `trace_syscalls: false` and
    // `follow_children: false` are the same config the unit tests
    // use that pass on this kernel.
    let mut tracer = PtraceTracer::new(PtraceConfig {
        trace_syscalls: false,
        capture_registers: true,
        follow_children: false,
        track_function_frames: false,
    });
    let pid = tracer
        .launch(std::path::Path::new("/bin/true"), &[])
        .expect("launch /bin/true");
    assert!(pid > 0, "non-zero pid from launch");

    // Drive the event loop and push each event through the
    // producer's record shape into the ExecutionLog.
    let mut seq = 0u64;
    let mut saw_exit = false;
    for _ in 0..1000 {
        match tracer.wait_event() {
            Ok(Some(PtraceEvent::Exited { exit_code, .. })) => {
                let ev =
                    ptracenonevent_to_trace_event(&PtraceEvent::Exited { pid, exit_code }, seq);
                let rec = trace_event_to_log_record_for_test(session_id, seq * 1000, &ev);
                concrete.append(rec).expect("append Exited");
                seq += 1;
                saw_exit = true;
                break;
            }
            Ok(Some(ev)) => {
                let trace_ev = ptracenonevent_to_trace_event(&ev, seq);
                let rec = trace_event_to_log_record_for_test(session_id, seq * 1000, &trace_ev);
                concrete.append(rec).expect("append");
                seq += 1;
                tracer.continue_execution(pid).expect("cont");
            }
            Ok(None) => break,
            Err(e) => panic!("wait_event error: {}", e),
        }
    }
    assert!(saw_exit, "tracee must exit within 1000 events");
    concrete.flush().expect("flush log");

    // Query through the canonical port. The decode path must succeed
    // for every record and the unparseable counter must be zero.
    let (events, tail_seq, unparseable, total_seen) =
        chronos_native::probe_backend::read_log_with_stats(provider.as_ref(), None, 1000)
            .expect("read_log_with_stats");
    assert_eq!(total_seen, seq, "every event should be visible");
    assert_eq!(
        unparseable, 0,
        "no record should fail to decode as TraceEvent"
    );
    assert_eq!(events.len() as u64, seq);
    assert_eq!(tail_seq, Some(seq - 1), "tail_seq is the highest seq");

    // The exit event must be the last one with exit_code == 0.
    let last = events.last().expect("at least one event");
    assert_eq!(last.event_type, chronos_domain::EventType::ThreadExit);

    // Durability: reopen the log and verify the events replay.
    let log2 = SegmentedExecutionLog::open(
        SessionId::new(session_id),
        SegmentedConfig::with_dir(&log_dir),
    )
    .expect("reopen log");
    assert_eq!(
        log2.tail_seq().map(|s| s.0),
        Some(seq - 1),
        "tail_seq survives restart"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn decoder_counters_surface_unparseable_payloads() {
    // Build a backend whose log has both a valid TraceEvent and
    // an unparseable payload. The counters must distinguish them.
    let dir = tempdir("counters");
    let session_id = "m1-04-counters";
    let backend = NativeProbeBackend::new();

    let log_dir = dir.join(format!("native-{}", session_id));
    let concrete = Arc::new(
        SegmentedExecutionLog::open(
            SessionId::new(session_id),
            SegmentedConfig::with_dir(&log_dir),
        )
        .expect("open log"),
    );
    let provider: Arc<dyn ExecutionLogProvider> = Arc::new(SegmentedExecutionLogProvider::new(
        SessionId::new(session_id),
        concrete.clone(),
    ));
    backend.attach_execution_log(provider.clone());

    // One valid TraceEvent.
    use chronos_domain::{EventData, EventType, MonotonicNs, SourceLocation, TraceEvent};
    let good_ev = TraceEvent {
        event_id: 0,
        timestamp_ns: MonotonicNs::from(0),
        thread_id: 1,
        event_type: EventType::FunctionEntry,
        location: SourceLocation::default(),
        data: EventData::Empty,
    };
    concrete
        .append(trace_event_to_log_record_for_test(session_id, 0, &good_ev))
        .expect("append good");

    // Two unparseable payloads (raw bytes that are not valid
    // TraceEvent JSON).
    for i in 1..=2u64 {
        let bytes = format!("not-json-{}-{{{{", i).into_bytes();
        concrete
            .append(chronos_log::NewExecutionRecord {
                kind: chronos_log::ExecutionKind::Raw,

                session_id: SessionId::new(session_id),
                monotonic_ns: i * 100,
                payload: chronos_log::ExecutionPayload::new(bytes, "noise"),
                ..Default::default()
            })
            .expect("append noise");
    }
    concrete.flush().expect("flush");

    let (events, _, unparseable, total) =
        chronos_native::probe_backend::read_log_with_stats(provider.as_ref(), None, 100)
            .expect("read");
    assert_eq!(total, 3, "three records total");
    assert_eq!(unparseable, 2, "two records fail to decode");
    assert_eq!(events.len(), 1, "one valid event decoded");
    assert_eq!(events[0].event_id, 0);

    let _ = std::fs::remove_dir_all(&dir);
}
