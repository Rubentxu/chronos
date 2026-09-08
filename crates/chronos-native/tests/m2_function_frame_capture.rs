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
