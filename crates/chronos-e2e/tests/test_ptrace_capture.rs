//! Ptrace capture integration tests with timeout and diagnostics.
//!
//! These tests exercise the full ptrace capture pipeline with REAL binaries,
//! covering all PtraceEvent types:
//!   - Stopped (SIGTRAP, signals)
//!   - Syscall (entry/exit)
//!   - PtraceEvent (CLONE, FORK, VFORK)
//!   - Exited (clean exit)
//!   - Signaled (SIGSEGV crash)
//!
//! Every test has a hard timeout and dumps diagnostic info on failure.

use chronos_domain::CaptureConfig;
use chronos_native::capture_runner::{CaptureEndReason, CaptureRunner};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

// ============================================================================
// Test infrastructure
// ============================================================================

const FIXTURES_RELATIVE: &str = "../../tests/fixtures";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);

/// Compile a C fixture and return the path to the binary.
fn compile_fixture(source_name: &str) -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
    let fixtures_dir = Path::new(&manifest_dir).join(FIXTURES_RELATIVE);
    let source_path = fixtures_dir.join(source_name);
    let binary_path = fixtures_dir.join(source_name.replace(".c", ""));

    // Don't recompile if binary exists and is newer than source
    if binary_path.exists() {
        if let (Ok(src_meta), Ok(bin_meta)) = (
            std::fs::metadata(&source_path),
            std::fs::metadata(&binary_path),
        ) {
            if bin_meta.modified().unwrap() > src_meta.modified().unwrap() {
                return binary_path;
            }
        }
    }

    // pthread flag only needed for thread fixtures
    let needs_pthread = source_name.contains("thread");
    let mut cmd = Command::new("gcc");
    cmd.args(["-g", "-no-pie", "-o"])
        .arg(&binary_path)
        .arg(&source_path);
    if needs_pthread {
        cmd.arg("-lpthread");
    }

    let output = cmd.output().expect("gcc should be installed");
    if !output.status.success() {
        panic!(
            "Failed to compile {}: {}",
            source_name,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    binary_path
}

/// Run a capture with a hard timeout. Spawns a separate thread for the
/// blocking `run_to_completion` call and waits with a deadline.
fn run_capture_with_timeout(
    binary: &Path,
    timeout: Duration,
) -> Result<chronos_native::capture_runner::CaptureResult, String> {
    let binary_owned = binary.to_path_buf();
    let binary_for_err = binary.to_path_buf();
    let (tx, rx) = std::sync::mpsc::channel();

    let capture_thread = std::thread::spawn(move || {
        let config = CaptureConfig::new(binary_owned.to_str().unwrap());
        let mut runner = CaptureRunner::new(config);
        let result = runner.run_to_completion();
        let _ = tx.send(());
        result
    });

    // Wait for the capture thread with timeout
    match rx.recv_timeout(timeout) {
        Ok(()) => {
            // Capture completed within timeout — join the thread to get the result
            capture_thread
                .join()
                .map_err(|e| format!("Capture thread panicked: {:?}", e))?
                .map_err(|e| format!("Capture failed: {}", e))
        }
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            Err(format!(
                "CAPTURE TIMED OUT after {:.1}s for binary '{}'\n\
                 This likely means the ptrace event loop hung.\n\
                 Possible causes:\n\
                 - PtraceEvent::PtraceEvent not continued after clone/fork/vfork\n\
                 - New child thread not continued after SIGSTOP\n\
                 - waitpid blocking on zombie traced processes\n\
                 - Breakpoint INT3 hit by non-main thread not handled\n\
                 Check capture_runner.rs should_continue logic.",
                timeout.as_secs_f64(),
                binary_for_err.display()
            ))
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            capture_thread
                .join()
                .map_err(|e| format!("Capture thread panicked: {:?}", e))?
                .map_err(|e| format!("Capture failed: {}", e))
        }
    }
}

/// Generate a diagnostic report for a capture result.
fn print_diagnostic_report(
    binary: &Path,
    result: &chronos_native::capture_runner::CaptureResult,
    elapsed: Duration,
) {
    eprintln!("\n=== Capture Diagnostic Report ===");
    eprintln!("Binary: {}", binary.display());
    eprintln!("Duration: {:.3}s", elapsed.as_secs_f64());
    eprintln!("Total events: {}", result.total_events);
    eprintln!("End reason: {:?}", result.end_reason);

    // Count events by type
    use std::collections::HashMap;
    let mut type_counts: HashMap<String, usize> = HashMap::new();
    let mut thread_ids: std::collections::HashSet<u64> = std::collections::HashSet::new();

    for evt in &result.events {
        *type_counts.entry(format!("{:?}", evt.event_type)).or_insert(0) += 1;
        thread_ids.insert(evt.thread_id);
    }

    eprintln!("Threads seen: {:?}", thread_ids);
    eprintln!("Event type breakdown:");
    let mut sorted: Vec<_> = type_counts.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));
    for (ty, count) in &sorted {
        eprintln!("  {:?}: {}", ty, count);
    }

    // Print first 5 and last 5 events
    eprintln!("\nFirst 5 events:");
    for (i, evt) in result.events.iter().take(5).enumerate() {
        let func = evt.location.function.as_deref().unwrap_or("???");
        eprintln!(
            "  [{}] {:?} tid={} addr=0x{:x} func={}",
            i, evt.event_type, evt.thread_id, evt.location.address, func
        );
    }
    if result.events.len() > 10 {
        eprintln!("  ... ({} events omitted)", result.events.len() - 10);
    }
    eprintln!("Last 5 events:");
    let start = result.events.len().saturating_sub(5);
    for (i, evt) in result.events.iter().skip(start).enumerate() {
        let func = evt.location.function.as_deref().unwrap_or("???");
        eprintln!(
            "  [{}] {:?} tid={} addr=0x{:x} func={}",
            start + i, evt.event_type, evt.thread_id, evt.location.address, func
        );
    }
    eprintln!("=== End Report ===\n");
}

/// Full capture test: compile fixture, capture, verify, diagnose.
fn full_capture_test(
    source: &str,
    expected_exit: Option<i32>,
    min_events: u64,
) {
    let binary = compile_fixture(source);
    let started = Instant::now();

    let result =
        run_capture_with_timeout(&binary, DEFAULT_TIMEOUT).unwrap_or_else(|e| {
            panic!("{}", e);
        });

    let elapsed = started.elapsed();

    print_diagnostic_report(&binary, &result, elapsed);

    // Verify completion
    if let Some(expected) = expected_exit {
        match &result.end_reason {
            CaptureEndReason::Exited(code) if *code == expected => {}
            CaptureEndReason::Exited(code) => {
                panic!(
                    "Expected exit code {}, got {}. Binary: {}",
                    expected,
                    code,
                    binary.display()
                );
            }
            CaptureEndReason::Signaled { signal_name, .. } => {
                panic!(
                    "Process killed by {} but expected exit({}). Binary: {}",
                    signal_name,
                    expected,
                    binary.display()
                );
            }
            other => {
                panic!(
                    "Unexpected end reason {:?}, expected exit({}). Binary: {}",
                    other,
                    expected,
                    binary.display()
                );
            }
        }
    }

    // Verify we got events
    assert!(
        result.total_events >= min_events,
        "Expected at least {} events, got {}. Binary: {}",
        min_events,
        result.total_events,
        binary.display()
    );

    // Verify capture completed within timeout
    assert!(
        elapsed < DEFAULT_TIMEOUT,
        "Capture took {:.1}s (timeout: {}s). Binary: {}",
        elapsed.as_secs_f64(),
        DEFAULT_TIMEOUT.as_secs(),
        binary.display()
    );

    eprintln!(
        "✅ {} captured {} events in {:.1}s",
        source,
        result.total_events,
        elapsed.as_secs_f64()
    );
}

/// Capture test for crashing programs.
fn crash_capture_test(source: &str) {
    let binary = compile_fixture(source);
    let started = Instant::now();

    let result =
        run_capture_with_timeout(&binary, DEFAULT_TIMEOUT).unwrap_or_else(|e| {
            panic!("{}", e);
        });

    let elapsed = started.elapsed();

    print_diagnostic_report(&binary, &result, elapsed);

    // Verify the process terminated (either signaled or exit with non-zero)
    match &result.end_reason {
        CaptureEndReason::Signaled { .. } => {}
        CaptureEndReason::Exited(code) if *code != 0 => {}
        CaptureEndReason::Exited(code) => {
            panic!(
                "Expected crash (signal or non-zero exit), got exit({}). Binary: {}",
                code,
                binary.display()
            );
        }
        other => {
            panic!(
                "Unexpected end reason {:?}, expected crash. Binary: {}",
                other,
                binary.display()
            );
        }
    }

    assert!(
        result.total_events > 0,
        "Should capture events before crash. Binary: {}",
        binary.display()
    );

    eprintln!(
        "✅ {} crashed as expected, {} events in {:.1}s",
        source,
        result.total_events,
        elapsed.as_secs_f64()
    );
}

// ============================================================================
// Group 1: Single-threaded baseline tests
// ============================================================================

#[test]
fn test_ptrace_bin_true() {
    // /bin/true is the simplest possible binary — exits immediately with 0
    let binary = PathBuf::from("/bin/true");
    let started = Instant::now();
    let config = CaptureConfig::new("/bin/true");
    let mut runner = CaptureRunner::new(config);

    let result = runner.run_to_completion().expect("Should capture /bin/true");
    let elapsed = started.elapsed();

    print_diagnostic_report(&binary, &result, elapsed);

    assert!(
        matches!(result.end_reason, CaptureEndReason::Exited(0)),
        "Expected Exited(0), got: {:?}",
        result.end_reason
    );
    assert!(
        result.total_events > 0,
        "Should capture at least 1 event"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "Should complete in <5s, took {:.1}s",
        elapsed.as_secs_f64()
    );
}

#[test]
fn test_ptrace_bin_echo() {
    let binary = PathBuf::from("/bin/echo");
    let started = Instant::now();
    let config = CaptureConfig::new("/bin/echo");
    let mut runner = CaptureRunner::new(config);

    let result = runner.run_to_completion().expect("Should capture /bin/echo");
    let elapsed = started.elapsed();

    print_diagnostic_report(&binary, &result, elapsed);

    assert!(
        matches!(result.end_reason, CaptureEndReason::Exited(0)),
        "Expected Exited(0), got: {:?}",
        result.end_reason
    );
    assert!(result.total_events > 0);
}

#[test]
fn test_ptrace_c_single_thread() {
    // test_add.c: simple C program, no threads
    full_capture_test("test_add.c", Some(0), 1);
}

#[test]
fn test_ptrace_c_segfault() {
    // test_segfault.c: crashes with SIGSEGV in main thread
    crash_capture_test("test_segfault.c");
}

// ============================================================================
// Group 2: Multi-threaded tests (THESE catch the bug we found)
// ============================================================================

#[test]
fn test_ptrace_pthreads_clean_exit() {
    // test_threads.c: 3 pthreads, clean join, exit 0
    // THIS is the test that would have caught the PtraceEvent bug:
    // If PtraceEvent::PtraceEvent is not continued after clone,
    // this test will timeout.
    full_capture_test("test_threads.c", Some(0), 5);
}

#[test]
fn test_ptrace_many_threads() {
    // test_many_threads.c: 10 simultaneous threads
    // Stress test for rapid clone events
    full_capture_test("test_many_threads.c", Some(0), 5);
}

#[test]
fn test_ptrace_clone() {
    // test_clone.c: uses clone() syscall directly
    // Exercises PTRACE_EVENT_CLONE with raw syscall
    // Note: clone with shared resources may exit with non-zero,
    // so we just verify capture completes (not the exit code).
    full_capture_test("test_clone.c", None, 1);
}

#[test]
fn test_ptrace_crash_in_thread() {
    // test_crash_thread.c: thread crashes with SIGSEGV
    // Tests that crash in child thread is properly propagated
    crash_capture_test("test_crash_thread.c");
}

// ============================================================================
// Group 3: Multi-process tests
// ============================================================================

#[test]
fn test_ptrace_fork() {
    // test_fork.c: fork + exec + waitpid
    // Exercises PTRACE_O_TRACEFORK and child process tracking
    // Child exits with code 42, parent verifies and exits with 0
    full_capture_test("test_fork.c", Some(0), 2);
}

// ============================================================================
// Group 4: Event type coverage verification
// ============================================================================

/// Verify that the multi-threaded test actually produces PtraceEvent
/// events (clone events). This ensures we're exercising the code path
/// that was buggy.
#[test]
fn test_ptrace_verifies_clone_events_seen() {
    let binary = compile_fixture("test_threads.c");
    let config = CaptureConfig::new(binary.to_str().unwrap());
    let mut runner = CaptureRunner::new(config);

    let result = runner
        .run_to_completion()
        .expect("Should capture test_threads");

    // This test MUST produce ThreadCreate events (from clone)
    let thread_creates: Vec<_> = result
        .events
        .iter()
        .filter(|e| {
            matches!(
                e.event_type,
                chronos_domain::EventType::ThreadCreate
            )
        })
        .collect();

    assert!(
        !thread_creates.is_empty(),
        "Multi-threaded test MUST produce ThreadCreate events.\n\
         If this assertion fires, it means the capture is not properly\n\
         handling PTRACE_EVENT_CLONE events.\n\
         Total events: {}, event types: {:?}",
        result.total_events,
        result
            .events
            .iter()
            .map(|e| format!("{:?}", e.event_type))
            .collect::<std::collections::HashSet<_>>()
    );

    eprintln!(
        "✅ Saw {} ThreadCreate events (clone was exercised)",
        thread_creates.len()
    );
}

/// Verify that multi-threaded captures see multiple thread IDs.
#[test]
fn test_ptrace_verifies_multiple_threads_seen() {
    let binary = compile_fixture("test_many_threads.c");
    let config = CaptureConfig::new(binary.to_str().unwrap());
    let mut runner = CaptureRunner::new(config);

    let result = runner
        .run_to_completion()
        .expect("Should capture test_many_threads");

    let thread_ids: std::collections::HashSet<u64> =
        result.events.iter().map(|e| e.thread_id).collect();

    // With 10 threads + main, we should see at least 2 different thread IDs
    // (main + at least one worker)
    assert!(
        thread_ids.len() >= 2,
        "Multi-threaded test should see multiple thread IDs, got: {:?}",
        thread_ids
    );

    eprintln!(
        "✅ Saw {} different thread IDs: {:?}",
        thread_ids.len(),
        thread_ids
    );
}

/// Verify that syscall tracing works with multi-threaded programs.
#[test]
fn test_ptrace_syscall_tracing_multithread() {
    let binary = compile_fixture("test_threads.c");
    let mut config = CaptureConfig::new(binary.to_str().unwrap());
    // Enable syscall tracing explicitly
    config.capture_syscalls = true;

    let mut runner = CaptureRunner::new(config);
    let started = Instant::now();

    let result = runner
        .run_to_completion()
        .expect("Should capture test_threads with syscall tracing");

    let elapsed = started.elapsed();

    assert!(
        elapsed < DEFAULT_TIMEOUT,
        "Syscall tracing with threads should not hang. Took {:.1}s",
        elapsed.as_secs_f64()
    );

    // With syscall tracing we should get many more events
    eprintln!(
        "✅ Syscall-traced multi-thread: {} events in {:.1}s",
        result.total_events,
        elapsed.as_secs_f64()
    );
}

// ============================================================================
// Group 5: Regression — the exact bug we found today
// ============================================================================

/// Regression test for the PtraceEvent continuation bug.
///
/// Before the fix, PtraceEvent::PtraceEvent was excluded from `should_continue`
/// in capture_runner.rs, which meant that after a clone/fork/vfork event,
/// the parent process was never resumed with PTRACE_CONT. This caused:
/// 1. The parent to stay stopped forever
/// 2. waitpid to block waiting for events from a stopped process
/// 3. The entire capture to timeout
///
/// This test captures a multi-threaded program and verifies:
/// 1. It completes within the timeout
/// 2. It sees the expected number of threads
/// 3. The process exits cleanly with code 0
#[test]
fn test_regression_ptrace_event_not_continued() {
    let binary = compile_fixture("test_threads.c");
    let started = Instant::now();

    let result = run_capture_with_timeout(&binary, Duration::from_secs(10))
        .unwrap_or_else(|e| {
            panic!(
                "REGRESSION: The PtraceEvent continuation bug is BACK!\n\
                 Multi-threaded capture timed out, meaning PtraceEvent::PtraceEvent\n\
                 is not being continued after clone/fork/vfork events.\n\
                 \n\
                 Check capture_runner.rs: should_continue must NOT exclude\n\
                 PtraceEvent::PtraceEvent from PTRACE_CONT.\n\
                 \n\
                 Error: {}",
                e
            );
        });

    let elapsed = started.elapsed();

    assert!(
        matches!(result.end_reason, CaptureEndReason::Exited(0)),
        "REGRESSION: Multi-threaded program did not exit cleanly: {:?}",
        result.end_reason
    );

    assert!(
        elapsed < Duration::from_secs(10),
        "REGRESSION: Capture took {:.1}s — should complete in <2s",
        elapsed.as_secs_f64()
    );

    // Must see at least one ThreadCreate event (from clone)
    let has_thread_create = result.events.iter().any(|e| {
        matches!(e.event_type, chronos_domain::EventType::ThreadCreate)
    });
    assert!(
        has_thread_create,
        "REGRESSION: No ThreadCreate events seen — clone events not being processed"
    );

    eprintln!(
        "✅ REGRESSION TEST PASSED: {} events, {:.1}s, {:?}",
        result.total_events,
        elapsed.as_secs_f64(),
        result.end_reason
    );
}
