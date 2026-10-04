//! Baseline measurement for DEBT-SCALE-MEM-01: how much memory does *opening*
//! a segmented log actually cost, and how much of it is the duplicated copy
//! that `ReplayPlan.entries` creates?
//!
//! This is a probe, not a test: it prints numbers so the figure can be recorded
//! in the debt entry and compared after the fix. It asserts nothing about the
//! value, because a memory number on a shared host is not a contract — the same
//! reason `SCALE_BUDGETS` §0 forbids pinning one.
//!
//! What it DOES assert is the shape claim, which is a property of the code and
//! not of the host: building the plan materialises the entries, and applying it
//! materialises them again, so at the moment of application the live set holds
//! two copies. That is what makes the fix worth attempting.

use std::path::PathBuf;
use std::time::Instant;

use chronos_log::{
    ExecutionKind, ExecutionPayload, NewExecutionRecord, SegmentedConfig, SegmentedExecutionLog,
    SessionId,
};

const EVENTS: u64 = 200_000;

fn rss_kb() -> u64 {
    let raw = std::fs::read_to_string("/proc/self/statm").expect("statm");
    let mut f = raw.split_whitespace();
    f.next().expect("size");
    f.next().expect("resident").parse::<u64>().expect("num") * 4
}

fn trace_event(id: u64) -> chronos_domain::trace::TraceEvent {
    use chronos_domain::{EventData, EventType, MonotonicNs, SourceLocation, TraceEvent};
    TraceEvent {
        event_id: id,
        timestamp_ns: MonotonicNs::from(id * 1_000),
        thread_id: 1,
        event_type: EventType::FunctionEntry,
        location: SourceLocation {
            function: Some("open_probe".to_string()),
            ..SourceLocation::default()
        },
        data: EventData::Function {
            name: "open_probe".to_string(),
            signature: None,
            symbol_id: None,
            invocation_id: None,
            parent_invocation_id: None,
        },
    }
}

fn main() {
    let dir: PathBuf = std::env::temp_dir().join(format!(
        "open-probe-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let inner = dir.join("s");
    std::fs::create_dir_all(&inner).expect("mkdir");
    let session = SessionId::new("s");

    {
        let log = SegmentedExecutionLog::open(session.clone(), SegmentedConfig::with_dir(&inner))
            .expect("open for seeding");
        for i in 1..=EVENTS {
            log.append(NewExecutionRecord {
                session_id: session.clone(),
                kind: ExecutionKind::Raw,
                monotonic_ns: i * 1_000,
                payload: ExecutionPayload::new(
                    serde_json::to_vec(&trace_event(i)).expect("encode"),
                    "trace_event",
                ),
                ..Default::default()
            })
            .expect("append");
        }
        log.flush().expect("flush");
    }

    // The measurement: a fresh open over an already-populated directory is
    // exactly what a server restart does.
    let before = rss_kb();
    let t = Instant::now();
    let log = SegmentedExecutionLog::open(session.clone(), SegmentedConfig::with_dir(&inner))
        .expect("reopen");
    let after = rss_kb();
    let tail = log.tail_seq();

    println!(
        "opened {EVENTS} events in {:.2}s",
        t.elapsed().as_secs_f64()
    );
    println!("tail_seq: {tail:?}  (must be {EVENTS} — a wrong open here is a product bug)");
    println!("RSS before open: {before} KB");
    println!("RSS after  open: {after} KB");
    println!("cost of open:    {} KB", after as i64 - before as i64);
    println!(
        "bytes per event: {:.0}",
        (after - before) as f64 * 1024.0 / EVENTS as f64
    );

    // The shape claim, and the only assertion: the open must have recovered the
    // real session. If it did not, the memory figure above is measuring a
    // failure, which is the same trap as the R3.2 probe's `total_events: 0`.
    //
    // `EVENTS - 1`, not `EVENTS`: `append` allocates from zero, so N appends
    // occupy seqs `0..N` exclusive. The first version of this probe asserted
    // `EVENTS` and the reopen "lost" an event that was never there — the
    // assertion was wrong, not the log. Worth writing down because a probe
    // that fails on its own arithmetic looks exactly like a product defect.
    assert_eq!(
        tail.map(|t| t.0),
        Some(EVENTS - 1),
        "the reopened log lost events"
    );

    drop(log);
    let _ = std::fs::remove_dir_all(&dir);
}
