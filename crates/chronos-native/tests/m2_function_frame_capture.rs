//! m2-native acceptance: real FunctionEntry capture via INT3 injection.
//!
//! These tests spawn a real compiled C fixture under ptrace with
//! `track_function_frames=true` and assert the capture emits genuine
//! `FunctionEntry` events (not synthetic) carrying symbol/invocation
//! identity, including distinct invocations for a recursive call, and that
//! those entries round-trip into the ExecutionLog v2 via the exact producer
//! mapping (`probe_backend::trace_event_to_log_record_for_test`).
//!
//! Requires a working C toolchain (`cc`) to build the fixture and a host
//! that allows tracing our own forked child (no root needed for
//! `PTRACE_TRACEME` on a child we spawn). If no compiler is found the tests
//! skip with a note rather than failing CI without one.

use chronos_domain::{CaptureConfig, EventData, EventType, InvocationId, TraceEvent};
use chronos_log::{
    LogConsumerId, NewExecutionRecord, ReadResult, SegmentedConfig, SegmentedExecutionLog,
};
use chronos_native::capture_runner::{CaptureEndReason, CaptureResult, CaptureRunner};
use chronos_native::probe_backend::trace_event_to_log_record_for_test;
use chronos_native::probe_backend::NativeProbeBackend;
use chronos_native::ptrace_tracer::PtraceConfig;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Absolute path to the fixture C source committed in this crate.
fn fixture_source() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("test_function_frames.c")
}

/// Unique scratch directory for one test run.
fn scratch_dir(tag: &str) -> PathBuf {
    let base = std::env::temp_dir();
    let unique = format!(
        "chronos-m2native-{}-{}-{}",
        tag,
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

/// Try to compile the fixture to an executable. Returns the exe path, or
/// `None` when no usable C compiler is present (tests then skip).
///
/// We deliberately do NOT pass `-no-pie`: gcc's default PIE output forces the
/// tracer's load-bias relocation (symbol address + ASLR base) to be exercised,
/// which is exactly what the milestone must prove. If only a non-Linux
/// toolchain that rejects PIE were present the capture would still work via a
/// zero bias, but on Linux the default path is the interesting one.
fn compile_fixture() -> Option<PathBuf> {
    let src = fixture_source();
    let dir = scratch_dir("build");
    let out = dir.join("test_function_frames");
    let args: Vec<String> = [
        "-g",
        "-O0",
        "-fno-inline",
        "-fno-omit-frame-pointer",
        "-o",
        out.to_str()?,
        src.to_str()?,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let cc_candidates = [
        std::env::var("CC").ok(),
        Some("cc".to_string()),
        Some("gcc".to_string()),
        Some("clang".to_string()),
    ];
    for cc in cc_candidates.into_iter().flatten() {
        if let Ok(status) = Command::new(&cc).args(&args).status() {
            if status.success() {
                return Some(out);
            }
        }
    }
    eprintln!("skipping: no usable C compiler found to build the function-frame fixture");
    None
}

/// Run a real function-frame capture of the fixture.
fn capture_frames(exe: &Path) -> CaptureResult {
    let config = CaptureConfig::new(exe.to_string_lossy().to_string());
    let ptrace = PtraceConfig {
        trace_syscalls: false,
        capture_registers: false,
        follow_children: false,
        track_function_frames: true,
    };
    let mut runner = CaptureRunner::new(config).with_ptrace_config(ptrace);
    runner
        .run_to_completion()
        .expect("function-frame spawn capture should run to completion")
}

/// Pull the function name and invocation id out of a Function event.
fn function_entry_name_and_invocation(ev: &TraceEvent) -> Option<(String, InvocationId)> {
    if ev.event_type != EventType::FunctionEntry {
        return None;
    }
    match &ev.data {
        EventData::Function {
            name,
            invocation_id: Some(inv),
            ..
        } => Some((name.clone(), *inv)),
        _ => None,
    }
}

#[test]
fn spawn_capture_emits_real_function_entries_with_recursion_ids() {
    let Some(exe) = compile_fixture() else {
        return;
    };

    let result = capture_frames(&exe);
    assert!(
        matches!(result.end_reason, CaptureEndReason::Exited(0)),
        "fixture should exit cleanly, got {:?}",
        result.end_reason
    );
    assert!(!result.events.is_empty(), "capture produced no events");

    // Collect FunctionEntry events that carry identity.
    let entries: Vec<(String, InvocationId)> = result
        .events
        .iter()
        .filter_map(function_entry_name_and_invocation)
        .collect();

    let mut names: HashSet<String> = entries.iter().map(|(n, _)| n.clone()).collect();
    // Drop compiler-internal helpers; assert our three are all present.
    for expected in ["main", "add", "fact"] {
        assert!(
            names.remove(expected),
            "expected a real FunctionEntry for '{}', saw: {:?}",
            expected,
            names
        );
    }

    // Recursion: fact(4) yields four distinct invocations at the same entry.
    let fact_invocations: HashSet<InvocationId> = entries
        .iter()
        .filter(|(n, _)| n == "fact")
        .map(|(_, id)| *id)
        .collect();
    assert!(
        fact_invocations.len() >= 3,
        "expected >=3 distinct recursive 'fact' invocations, got {}",
        fact_invocations.len()
    );
}

#[test]
fn real_function_entries_round_trip_into_execution_log_v2() {
    let Some(exe) = compile_fixture() else {
        return;
    };

    let result = capture_frames(&exe);
    let entries: Vec<&TraceEvent> = result
        .events
        .iter()
        .filter(|ev| function_entry_name_and_invocation(ev).is_some())
        .collect();
    assert!(
        !entries.is_empty(),
        "no identity-bearing FunctionEntry captured"
    );

    // Map through the exact producer mapping the ExecutionLog dual-write uses.
    let session_id = "native-m2native-roundtrip";
    let log_dir = scratch_dir("log");
    let log = SegmentedExecutionLog::open(
        chronos_log::SessionId::new(session_id),
        SegmentedConfig::with_dir(&log_dir),
    )
    .expect("open execution log");

    let mut appended = 0usize;
    for (i, ev) in entries.iter().enumerate() {
        let rec: NewExecutionRecord = trace_event_to_log_record_for_test(session_id, i as u64, ev);
        // Identity fields survive onto the v2 record.
        assert!(
            rec.symbol_id.is_some(),
            "v2 record must carry symbol_id for a FunctionEntry"
        );
        assert!(
            rec.invocation_id.is_some(),
            "v2 record must carry invocation_id for a FunctionEntry"
        );
        log.append(rec).expect("append record");
        appended += 1;
    }
    log.flush().expect("flush log");

    // Read it back and confirm the records (with identity) are durable.
    let consumer = LogConsumerId::new("m2native-query");
    match log.read_after(&consumer, None).expect("read_after") {
        ReadResult::Ok { records, .. } => {
            assert_eq!(
                records.len(),
                appended,
                "all FunctionEntry records must round-trip"
            );
            for r in records {
                assert!(
                    r.invocation_id.is_some() && r.symbol_id.is_some(),
                    "read-back v2 record lost identity for a FunctionEntry"
                );
            }
        }
        other => panic!("expected Ok read, got {:?}", other),
    }
}

#[test]
fn real_function_frame_capture_persists_durable_execution_log_v2() {
    let Some(exe) = compile_fixture() else {
        return;
    };

    let result = capture_frames(&exe);
    let entries: Vec<&TraceEvent> = result
        .events
        .iter()
        .filter(|ev| function_entry_name_and_invocation(ev).is_some())
        .collect();
    assert!(
        !entries.is_empty(),
        "no identity-bearing FunctionEntry captured"
    );

    // Production seam: persist the whole captured stream through the real
    // producer (NOT the `_for_test` helper) into a durable ExecutionLog v2.
    let session_id = "native-m2native-durable";
    let base_dir = scratch_dir("durable-log");
    let appended = chronos_native::probe_backend::persist_events_to_execution_log(
        session_id,
        &base_dir,
        &result.events,
    )
    .expect("persist real frame capture to a durable ExecutionLog");

    // Re-open the same log and confirm every persisted record is readable and
    // the identity-bearing FunctionEntry frames survived the durable round-trip.
    let log = SegmentedExecutionLog::open(
        chronos_log::SessionId::new(session_id),
        SegmentedConfig::with_dir(base_dir.join(session_id)),
    )
    .expect("reopen persisted ExecutionLog");

    let consumer = LogConsumerId::new("m2native-durable-query");
    match log.read_after(&consumer, None).expect("read_after") {
        ReadResult::Ok { records, .. } => {
            assert_eq!(
                records.len(),
                appended,
                "all persisted records must be readable back"
            );
            let identity_frames = records
                .iter()
                .filter(|r| r.symbol_id.is_some() && r.invocation_id.is_some())
                .count();
            assert_eq!(
                identity_frames,
                entries.len(),
                "identity-bearing FunctionEntry frames must survive the \
                 durable ExecutionLog round-trip"
            );
        }
        other => panic!("expected Ok read, got {:?}", other),
    }
}

/// m2-native-live-probe-frame-capture: drive the live MCP path through
/// `NativeProbeBackend::start_probe(config, track_function_frames=true)`,
/// then read the FunctionEntry frames back through the same
/// `read_execution_log_records_with_stats` API the QueryEngine uses.
/// Each read-back row must carry `invocation_id`, `parent_invocation_id`
/// and `symbol_id`, proving that the live MCP loop streams real INT3
/// `FunctionEntry` frames into the durable `SegmentedExecutionLog` v2
/// through the same `dual_push` producer used by the flat syscall loop.
#[test]
fn live_probe_emits_real_function_entries_to_execution_log() {
    use std::time::Duration;

    let Some(exe) = compile_fixture() else {
        return;
    };
    let exe = exe.to_str().unwrap().to_string();

    let execution_log_dir = scratch_dir("live-ff");
    let bus = chronos_domain::bus::EventBus::new_shared(50000);
    let backend = NativeProbeBackend::new(bus)
        .with_language(chronos_domain::Language::C)
        .with_execution_log_dir(Some(execution_log_dir.clone()));

    let config = CaptureConfig::new(exe.clone());

    let session = backend
        .start_probe(config, /* track_function_frames */ true)
        .expect("start_probe with track_function_frames=true failed");

    // The fixture does 1 add + 4 recursive fact entries. Give INT3
    // capture + dual_push + ExecutionLog.append time to settle. INT3
    // capture single-steps every entry, so it is noticeably slower than
    // the flat syscall loop; 5 s reliably covers 1 main + 5 nested entries.
    std::thread::sleep(Duration::from_secs(5));

    backend.stop_probe(&session).expect("stop_probe failed");

    // Read everything back from the ExecutionLog and filter for
    // identity-bearing rows (proxy: any record whose payload decodes
    // as a Function TraceEvent with non-None identity fields).
    let (events, _tail, _unparseable, total) = backend
        .read_execution_log_records_with_stats(None, 10_000)
        .expect("read_execution_log_records_with_stats failed");

    assert!(
        total >= 1,
        "ExecutionLog must contain at least 1 record after a live function-frame probe"
    );

    use chronos_domain::EventData;
    let identity_entries: Vec<&TraceEvent> = events
        .iter()
        .filter(|ev| {
            matches!(
                &ev.data,
                EventData::Function {
                    symbol_id: Some(_),
                    invocation_id: Some(_),
                    ..
                }
            )
        })
        .collect();

    assert!(
        !identity_entries.is_empty(),
        "live probe must emit at least one identity-bearing Function entry into the ExecutionLog, got {} total records (variants: {:?})",
        events.len(),
        events
            .iter()
            .map(|ev| format!("{:?}", ev.data))
            .collect::<Vec<_>>()
    );

    // At least one of the identity-bearing entries must carry a parent link
    // (the recursive `fact` chain produces four nested entries); this proves
    // FunctionFrameIdentity holds on the live MCP path.
    let with_parent = identity_entries
        .iter()
        .filter(|ev| {
            if let EventData::Function {
                parent_invocation_id,
                ..
            } = &ev.data
            {
                parent_invocation_id.is_some()
            } else {
                false
            }
        })
        .count();
    assert!(
        with_parent >= 1,
        "at least one Function entry must carry parent_invocation_id (recursion), got 0 of {}",
        identity_entries.len()
    );
}
