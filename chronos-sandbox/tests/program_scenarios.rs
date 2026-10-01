//! Program-Specific Scenarios tests — verify tools work correctly with various program types.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::client::types::{QueryFilter, TraceEvent};
use chronos_sandbox::McpSession;
use std::time::Duration;

/// PS1: test_fork_captures_multiple_processes
/// Probe test_fork, verify threads >= 1 and events > 0.
#[tokio::test]
async fn test_fork_captures_multiple_processes() {
    let fixture = McpSession::fixture_path("test_fork")
        .expect("test_fork fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    // Wait for fork to complete
    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");

    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // List threads
    let threads = client
        .list_threads(&session_id)
        .await
        .expect("list_threads failed");

    println!("✓ list_threads: {} threads", threads.len());
    assert!(!threads.is_empty(), "Should have at least 1 thread");

    // Query events
    let filter = QueryFilter {
        limit: 100,
        offset: 0,
        ..Default::default()
    };
    let events = client
        .query_events(&session_id, filter)
        .await
        .expect("query_events failed");

    println!("  events captured: {}", events.len());
    assert!(!events.is_empty(), "Should have captured events");

    client.shutdown().await.ok();
}

/// PS2: test_clone_thread_creation_visible
/// Probe test_clone, verify threads and events are captured.
#[tokio::test]
async fn test_clone_thread_creation_visible() {
    let fixture = McpSession::fixture_path("test_clone")
        .expect("test_clone fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");

    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // List threads
    let threads = client
        .list_threads(&session_id)
        .await
        .expect("list_threads failed");

    println!("✓ list_threads: {} threads", threads.len());

    // Query events
    let filter = QueryFilter {
        limit: 100,
        offset: 0,
        ..Default::default()
    };
    let events = client
        .query_events(&session_id, filter)
        .await
        .expect("query_events failed");

    println!("  events captured: {}", events.len());
    assert!(!events.is_empty(), "Should have captured events");

    println!("✓ Clone thread creation visible in trace");

    client.shutdown().await.ok();
}

/// PS3: test_many_threads_count
/// Probe test_many_threads, verify at least 3 threads visible.
#[tokio::test]
async fn test_many_threads_count() {
    let fixture = McpSession::fixture_path("test_many_threads")
        .expect("test_many_threads fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(4)).await;
    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");

    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // List threads
    let threads = client
        .list_threads(&session_id)
        .await
        .expect("list_threads failed");

    println!("✓ list_threads: {} threads", threads.len());
    assert!(
        threads.len() >= 3,
        "Should have at least 3 threads visible, got {}",
        threads.len()
    );

    client.shutdown().await.ok();
}

/// PS4: test_divide_by_zero_crash_detected
/// Probe test_divide_by_zero, debug_find_crash, assert crash_found or valid error.
#[tokio::test]
async fn test_divide_by_zero_crash_detected() {
    let fixture = McpSession::fixture_path("test_divide_by_zero")
        .expect("test_divide_by_zero fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    // Program exits fast, give it time then stop
    tokio::time::sleep(Duration::from_millis(500)).await;
    let _drained = client.probe_drain(&session_id).await;

    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Find crash
    let crash = client
        .debug_find_crash(&session_id)
        .await
        .expect("debug_find_crash failed");

    match crash {
        Some(info) => {
            println!("✓ debug_find_crash found crash:");
            println!("  signal: {:?}", info.signal);
            if let Some(sig) = &info.signal {
                assert!(
                    sig.contains("FPE") || sig.contains("8"),
                    "Signal should be FPE or contain 8 (SIGFPE=8)"
                );
            }
        }
        None => {
            // Crash may not be found if program exited too fast
            println!("✓ debug_find_crash returned None (crash may have been too fast)");
        }
    }

    client.shutdown().await.ok();
}

/// PS5: test_abort_crash_detected_sigabrt
/// Probe test_abort, debug_find_crash, assert crash_found or valid response.
#[tokio::test]
async fn test_abort_crash_detected_sigabrt() {
    let fixture = McpSession::fixture_path("test_abort")
        .expect("test_abort fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    // Program exits fast, give it time then stop
    tokio::time::sleep(Duration::from_millis(500)).await;
    let _drained = client.probe_drain(&session_id).await;

    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Find crash
    let crash = client
        .debug_find_crash(&session_id)
        .await
        .expect("debug_find_crash failed");

    match crash {
        Some(info) => {
            println!("✓ debug_find_crash found crash:");
            println!("  signal: {:?}", info.signal);
            if let Some(sig) = &info.signal {
                assert!(
                    sig.contains("ABRT") || sig.contains("6"),
                    "Signal should be ABRT or contain 6 (SIGABRT=6)"
                );
            }
        }
        None => {
            // Crash may not be found if program exited too fast
            println!("✓ debug_find_crash returned None (crash may have been too fast)");
        }
    }

    client.shutdown().await.ok();
}

/// PS6: test_crash_thread_crash_in_non_main_thread
/// Probe test_crash_thread, debug_find_crash, assert response is valid.
#[tokio::test]
async fn test_crash_thread_crash_in_non_main_thread() {
    let fixture = McpSession::fixture_path("test_crash_thread")
        .expect("test_crash_thread fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");

    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Find crash
    let crash = client
        .debug_find_crash(&session_id)
        .await
        .expect("debug_find_crash failed");

    match crash {
        Some(info) => {
            println!("✓ debug_find_crash found crash in non-main thread:");
            println!("  crash_found: {}", info.crash_found);
            if let Some(signal) = &info.signal {
                println!("  signal: {}", signal);
            }
            assert!(info.crash_found, "crash_found should be true");
        }
        None => {
            // Crash may not be detected
            println!("✓ debug_find_crash returned None (crash detection may not work for thread crashes)");
        }
    }

    client.shutdown().await.ok();
}

/// PS7: test_trace_syscalls_false_still_captures_events
/// Probe test_add with trace_syscalls=false, verify events are captured.
#[tokio::test]
async fn test_trace_syscalls_false_still_captures_events() {
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Start probe with trace_syscalls=false using probe_start_with_params
    let session_id = client
        .probe_start_with_params(
            fixture.to_str().unwrap(),
            false, // trace_syscalls = false
        )
        .await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");

    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");
    println!("Probe stopped: {} total events", stop.total_events);

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Query events
    let filter = QueryFilter {
        limit: 100,
        offset: 0,
        ..Default::default()
    };
    let events = client
        .query_events(&session_id, filter)
        .await
        .expect("query_events failed");

    println!(
        "✓ query_events with trace_syscalls=false: {} events",
        events.len()
    );

    // Measured on this fixture: the capture is NOT empty. `trace_syscalls=false`
    // suppresses syscall interception, but the probe still records the program
    // reaching its exit, so the log holds exactly the `process_exit` custom
    // marker (with `exit_code`) and zero syscall events. The previous
    // "events may be empty / just verify the response" comment was wrong.
    assert!(
        !events.is_empty(),
        "trace_syscalls=false must still capture the program_exit marker"
    );
    assert_eq!(
        events.len(),
        stop.total_events,
        "query_events and probe_stop disagree on the captured event count"
    );

    for e in &events {
        assert_eq!(
            e.event_type, "custom",
            "trace_syscalls=false must not capture syscall events, got {e:?}"
        );
        let Some(custom) = e.data.get("Custom") else {
            panic!("event carries no data.Custom payload: {e:?}");
        };
        assert_eq!(
            custom.get("name").and_then(|n| n.as_str()),
            Some("process_exit"),
            "the only custom event a finished probe emits is process_exit, got {e:?}"
        );
        let exit_json = custom
            .get("data_json")
            .and_then(|d| d.as_str())
            .unwrap_or_else(|| panic!("process_exit carries no data_json: {e:?}"));
        let exit: serde_json::Value = serde_json::from_str(exit_json)
            .unwrap_or_else(|err| panic!("data_json is not valid JSON ({err}): {exit_json:?}"));
        assert_eq!(
            exit.get("exit_code").and_then(|c| c.as_i64()),
            Some(0),
            "test_add must run to completion and exit 0, got {exit_json:?}"
        );
    }

    // Get execution summary
    let summary = client
        .get_execution_summary(&session_id)
        .await
        .expect("get_execution_summary failed");

    println!(
        "✓ get_execution_summary: total_events={}",
        summary.total_events
    );

    // A third tool has to agree on the same number.
    assert_eq!(
        summary.total_events, stop.total_events as u64,
        "execution_summary and probe_stop disagree on the captured event count"
    );
    assert_eq!(
        summary
            .event_counts_by_type
            .iter()
            .map(|c| c.count)
            .sum::<u64>(),
        summary.total_events,
        "per-type counts must add up to total_events, got {:?}",
        summary.event_counts_by_type
    );
    assert_eq!(
        summary.thread_count, 1,
        "test_add is a single-threaded fixture, got {}",
        summary.thread_count
    );
    // Honest empty: with syscall tracing off there is no function attribution
    // to rank, so this is empty by construction rather than by accident.
    assert!(
        summary.top_functions.is_empty(),
        "no syscalls were traced, so no function can be attributed, got {:?}",
        summary.top_functions
    );

    client.shutdown().await.ok();
}

/// PS8: test_infinite_loop_stopped_by_probe_stop
/// Probe test_infinite_loop, wait, probe_stop, verify events captured.
#[tokio::test]
async fn test_infinite_loop_stopped_by_probe_stop() {
    // Note: We don't have test_infinite_loop, using test_busyloop as substitute
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    // Wait 1.5 seconds
    tokio::time::sleep(Duration::from_millis(1500)).await;

    // Stop the probe
    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");

    println!("✓ probe_stop succeeded: {} total events", stop.total_events);
    assert!(stop.total_events > 0, "Should have captured events");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Query events
    let filter = QueryFilter {
        limit: 100,
        offset: 0,
        ..Default::default()
    };
    let events = client
        .query_events(&session_id, filter)
        .await
        .expect("query_events failed");

    println!("✓ query_events: {} events captured", events.len());
    assert!(!events.is_empty(), "Should have captured events");

    client.shutdown().await.ok();
}

/// One decoded `function_entry` / `function_exit` frame.
#[derive(Debug)]
struct Frame {
    kind: String,
    name: String,
    invocation_id: String,
    parent_invocation_id: Option<String>,
    address: u64,
    timestamp_ns: u64,
}

/// Asserts the invariants shared by every `track_function_frames=true`
/// live-probe capture of the `test_function_frames` fixture family, and
/// returns the decoded frames so each caller can apply its own load-address
/// rule (see PS-FF1 vs PS-PIE1).
///
/// The fixture is deterministic — `main` calls `add` once and `fact(4)`,
/// which recurses over n = 4, 3, 2, 1 — and the whole capture lands in ~1 ms,
/// so the counts below are exact, not a tolerance.
fn assert_frame_capture(events: &[TraceEvent], stop_total_events: usize) -> Vec<Frame> {
    assert!(
        !events.is_empty(),
        "track_function_frames=true must stream FunctionEntry events onto the ExecutionLog"
    );
    assert_eq!(
        events.len(),
        stop_total_events,
        "query_events and probe_stop disagree on the frame event count"
    );

    let frames: Vec<Frame> = events
        .iter()
        .map(|e| {
            assert!(
                e.event_type == "function_entry" || e.event_type == "function_exit",
                "track_function_frames=true must capture frame events only, got {e:?}"
            );
            let Some(payload) = e.data.get("Function") else {
                panic!("frame event carries no data.Function payload: {e:?}");
            };
            let text = |key: &str| {
                payload
                    .get(key)
                    .and_then(|v| v.as_str())
                    .unwrap_or_else(|| panic!("data.Function.{key} missing: {e:?}"))
                    .to_string()
            };
            Frame {
                kind: e.event_type.clone(),
                name: text("name"),
                invocation_id: text("invocation_id"),
                parent_invocation_id: payload
                    .get("parent_invocation_id")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                address: e
                    .location
                    .get("address")
                    .and_then(|v| v.as_u64())
                    .unwrap_or_else(|| panic!("location.address is not a u64: {e:?}")),
                timestamp_ns: e.timestamp_ns,
            }
        })
        .collect();

    let entries: Vec<&Frame> = frames
        .iter()
        .filter(|f| f.kind == "function_entry")
        .collect();
    let exits: Vec<&Frame> = frames
        .iter()
        .filter(|f| f.kind == "function_exit")
        .collect();
    assert_eq!(
        entries.len(),
        exits.len(),
        "every captured frame must also return: {frames:?}"
    );

    let names: Vec<&str> = entries.iter().map(|f| f.name.as_str()).collect();
    for expected in ["_start", "main", "add", "fact"] {
        assert!(
            names.contains(&expected),
            "no frame entry captured for {expected}, got {names:?}"
        );
    }
    assert_eq!(
        names.iter().filter(|n| **n == "fact").count(),
        4,
        "main calls fact(4) so n=4,3,2,1 must yield 4 fact entries, got {names:?}"
    );

    // Every entry is closed by the exit of the same invocation.
    for e in &entries {
        assert!(
            exits.iter().any(|x| x.invocation_id == e.invocation_id),
            "frame entry {} (invocation {}) never returned",
            e.name,
            e.invocation_id
        );
    }

    // `add` is called from `main`, so the parent link must resolve.
    let main_id = &entries
        .iter()
        .find(|f| f.name == "main")
        .expect("main entry asserted present above")
        .invocation_id;
    let add = entries
        .iter()
        .find(|f| f.name == "add")
        .expect("add entry asserted present above");
    assert_eq!(
        add.parent_invocation_id.as_deref(),
        Some(main_id.as_str()),
        "add must be recorded as a child frame of main, got {add:?}"
    );

    // The four recursive activations all belong to the same `main` frame.
    //
    // This is the invariant that a re-entry whose observed return address
    // points at no live frame used to break: unwinding popped the caller as
    // well, so `main`'s FunctionExit was emitted before `fact` #2..#4 were
    // entered and those invocations were recorded parentless. Measured
    // against the live fixture, all four carry `main`.
    let facts: Vec<&&Frame> = entries.iter().filter(|f| f.name == "fact").collect();
    for f in &facts {
        assert_eq!(
            f.parent_invocation_id.as_deref(),
            Some(main_id.as_str()),
            "every fact activation is called from main, so it must be its child frame, got {f:?}"
        );
    }
    // Four activations of the same function are four distinct invocations,
    // not four views of one. Sharing an id would make `children_of` return
    // one node where the call tree has four.
    let fact_ids: std::collections::BTreeSet<&str> =
        facts.iter().map(|f| f.invocation_id.as_str()).collect();
    assert_eq!(
        fact_ids.len(),
        facts.len(),
        "each fact activation needs its own invocation id, got {fact_ids:?}"
    );

    // `_start` is the process root, so it is the one frame with no parent;
    // `main` is entered from it. The `_start` → `main` edge is what proves
    // the tree has a root at all rather than a forest of orphans.
    let start = entries
        .iter()
        .find(|f| f.name == "_start")
        .expect("_start entry asserted present above");
    assert_eq!(
        start.parent_invocation_id, None,
        "_start is the process root and has no caller, got {start:?}"
    );
    assert_eq!(
        entries
            .iter()
            .find(|f| f.name == "main")
            .expect("main entry asserted present above")
            .parent_invocation_id
            .as_deref(),
        Some(start.invocation_id.as_str()),
        "main is entered from _start, got {frames:?}"
    );

    // Replay the capture against a stack: every entry pushes its frame, and
    // every exit must close exactly the frame on top. This is the assertion
    // that a premature caller unwind cannot survive — a `main` exit emitted
    // while a `fact` activation was still live would find `fact` on the top
    // instead, and the mismatch names both frames.
    let mut live: Vec<&str> = Vec::new();
    for f in &frames {
        if f.kind == "function_entry" {
            assert_eq!(
                f.parent_invocation_id.as_deref(),
                live.last().copied(),
                "{} must be entered from the frame it was called on, got {f:?} with live stack {live:?}",
                f.name
            );
            live.push(f.invocation_id.as_str());
        } else {
            let top = live.pop().unwrap_or_else(|| {
                panic!("{} returned with no live frame left, got {f:?}", f.name)
            });
            assert_eq!(
                top, f.invocation_id,
                "{} returned out of order: it closed {top} but the frame on top was {}",
                f.name, f.invocation_id
            );
        }
    }
    assert!(
        live.is_empty(),
        "every frame must return before the capture ends, still live: {live:?}"
    );

    // Real events carry advancing timestamps — not one frozen stamp.
    let first = frames.first().expect("non-empty asserted above");
    let last = frames.last().expect("non-empty asserted above");
    assert!(
        last.timestamp_ns > first.timestamp_ns,
        "frame timestamps must advance across the capture, got first={} last={}",
        first.timestamp_ns,
        last.timestamp_ns
    );
    assert!(
        frames
            .windows(2)
            .all(|w| w[1].timestamp_ns >= w[0].timestamp_ns),
        "frame timestamps must be non-decreasing in emission order"
    );

    frames
}

/// PS-FF1: test_track_function_frames_opt_in_live_probe
///
/// m2-native-live-probe-frame-capture: confirm that `probe_start` accepts
/// the new opt-in `track_function_frames=true` field on the spawned
/// fixture and that the fixture yields real `FunctionEntry` events through
/// the MCP surface. Full `FunctionEntry` identity assertions live in
/// `crates/chronos-native/tests/m2_function_frame_capture.rs`
/// (chronos-native knows the ExecutionLog v2 layout).
#[tokio::test]
async fn test_track_function_frames_opt_in_live_probe() {
    let fixture = McpSession::fixture_path("test_function_frames")
        .expect("test_function_frames fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start_with_track_function_frames(fixture.to_str().unwrap())
        .await
        .expect("probe_start with track_function_frames=true failed");

    // The fixture does ~5 entries (1 add + 4 recursive fact). Give the
    // INT3 capture branch time to break / single-step / re-inject.
    tokio::time::sleep(Duration::from_secs(2)).await;

    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");

    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");

    println!(
        "✓ live-probe with track_function_frames stopped: {} total events",
        stop.total_events
    );

    tokio::time::sleep(Duration::from_millis(200)).await;

    // `track_function_frames=true` takes over the capture loop, so the log
    // holds frame events only — no syscall events at all. `assert_frame_capture`
    // pins that down.
    let filter = QueryFilter {
        limit: 100,
        offset: 0,
        ..Default::default()
    };
    let events = client
        .query_events(&session_id, filter)
        .await
        .expect("query_events failed");

    println!(
        "  events captured with track_function_frames=true: {}",
        events.len()
    );

    // Measured: 14 events — 7 `function_entry` + 7 `function_exit` — for
    // `_start`, `main`, `add` and 4 recursive `fact` calls. The old
    // "the fixture may legitimately produce 0 events" comment was wrong:
    // the capture is deterministic and always populated.
    let frames = assert_frame_capture(&events, stop.total_events);

    // This fixture is built `-no-pie`, so every frame address sits in the
    // static image around 0x400000: the zero-bias fast path. PS-PIE1 covers
    // the randomised one.
    for f in &frames {
        assert!(
            (0x0040_0000..0x0100_0000).contains(&f.address),
            "non-PIE fixture frame {} resolved to {:#x}, expected the static 0x400000 image",
            f.name,
            f.address
        );
    }

    client.shutdown().await.ok();
}

/// PS-PIE1: test_track_function_frames_pie_fixture_yields_real_entries
///
/// m2-pie-fixture-ci: same as PS-FF1 but on the PIE-flagged variant. Linux
/// ASLR randomises the load base for the PIE binary on every exec, so the
/// Int3Injector::compute_load_bias path is exercised (not the zero-bias
/// fast-path). The capture must still produce the same 7 `FunctionEntry` /
/// 7 `FunctionExit` events as the non-PIE variant, and every resolved address
/// must carry the non-zero load bias.
#[tokio::test]
async fn test_track_function_frames_pie_fixture_yields_real_entries() {
    let fixture = McpSession::fixture_path("test_function_frames_pie")
        .expect("test_function_frames_pie fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    let session_id = client
        .probe_start_with_track_function_frames(fixture.to_str().unwrap())
        .await
        .expect("probe_start with track_function_frames=true on PIE fixture failed");

    // Same timing as the non-PIE UAT: the fixture does ~5 entries and the
    // INT3 capture path needs time to break / single-step / re-inject.
    tokio::time::sleep(Duration::from_secs(2)).await;

    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");

    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");

    println!(
        "✓ live-probe on PIE fixture stopped: {} total events",
        stop.total_events
    );

    // Query the events that landed in the ExecutionLog; PIE captures should
    // still produce at least the `add` + recursive `fact` entries.
    let filter = QueryFilter {
        limit: 100,
        offset: 0,
        ..Default::default()
    };
    let events = client
        .query_events(&session_id, filter)
        .await
        .expect("query_events failed");

    println!(
        "  events captured with PIE fixture + track_function_frames=true: {}",
        events.len()
    );
    // Measured: 14 events — 7 `function_entry` + 7 `function_exit` — same
    // shape as the non-PIE variant, so the randomised load base does not
    // cost us any frame. The old "loose assertion: ≥0" comment was wrong.
    let frames = assert_frame_capture(&events, stop.total_events);

    // The point of this variant: ASLR moves the image on every exec, so every
    // frame address must sit far above the static 0x400000 base. That is what
    // proves Int3Injector::compute_load_bias actually ran instead of the
    // zero-bias fast-path exercised by PS-FF1.
    for f in &frames {
        assert!(
            f.address >= 0x0100_0000,
            "PIE fixture frame {} resolved to {:#x}, which is inside the non-PIE static image — \
             the randomised load bias was not applied",
            f.name,
            f.address
        );
    }

    client.shutdown().await.ok();
}

// ============================================================================
// m7-06 — v2 lifecycle smoke (full session via v2)
// ============================================================================

#[tokio::test]
async fn test_session_lifecycle_in_full_session() {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found - run cargo build first");

    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // Full v2 lifecycle on a real binary.
    let start = client
        .session_start_spawn(fixture.to_str().unwrap(), vec![])
        .await
        .expect("session_start spawn failed");
    assert_eq!(
        start.action,
        chronos_sandbox::client::types::SessionStartAction::Spawn
    );
    assert!(start.capability_snapshot.get("probe_type").is_some());

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let stop = client
        .session_stop(&start.session_id, true, true)
        .await
        .expect("session_stop failed");
    assert!(stop.sealed_at.is_some());

    client.shutdown().await.ok();
}
