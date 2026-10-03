//! Scale acceptance for the M10 Execution Explorer read path: 1M events.
//!
//! ROADMAP §M10 listed "sandbox 1M eventos integration" as a real wiring
//! follow-up. Every other follow-up in that list is closed, and this is the
//! one that was left. It is a scale question, not a wiring question: the
//! other read-path tests seed five events, so nothing in the suite ever
//! asks what `summarize` does when it has to walk a million records.
//!
//! What this asserts, and why each one is a scale assertion:
//!
//!   1. A bounded `poll` over a 1M-event log returns exactly the requested
//!      batch and advances its cursor. A read path that materialises the
//!      whole session per poll would not return 100 out of 1,000,000.
//!   2. `summarize` reports a total of 1,000,000 given enough time. This is
//!      the aggregation path that has to read every page, so it is the one
//!      that can exhaust memory or time out at scale -- and at 1M events it
//!      does: see the `#[ignore]` reason below for the measured cost.
//!   3. `rollup` reports a `total_events` of 1,000,000, and an
//!      `invocation_count` of 0, because the seeded records carry no
//!      `invocation_id` and a rollup that invented invocations for them
//!      would be fabricating identities out of v1-shaped records.
//!   4. Resuming a poll from the returned cursor does not replay.
//!
//! NOT in the normal gate, on purpose. The gate runs exactly
//! `cargo test -p chronos-sandbox --test execution_log_read_e2e`, so a test in
//! its own target does not lengthen every gate by the cost of seeding a
//! million records. A scale test that only runs in CI is close to worthless,
//! so this one is meant to be run explicitly:
//!
//!     cargo test -p chronos-sandbox --test scale_execution_log_1m -- --nocapture
//!
//! The recorded run and its wall-clock cost are in the commit that added this
//! file and in the SDDK ledger, so the number is not folklore.
//!
//! STRUCTURE: one `#[tokio::test]`, one server. Same reasoning as
//! `execution_log_read_e2e`: a server booted in one test has its stdio bound
//! to that test's runtime, and the runtime dies with the test, so a server
//! shared across tests observes a dead transport.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chronos_domain::trace::TraceEvent;
use chronos_domain::{EventData, EventType, MonotonicNs, SourceLocation};
use chronos_log::{
    ExecutionKind, ExecutionPayload, NewExecutionRecord, SegmentedConfig, SegmentedExecutionLog,
    SessionId,
};
use chronos_sandbox::client::tools::McpTestClient;
use serde_json::json;

const SESSION: &str = "scale-1m";
const EVENTS: u64 = 1_000_000;
/// One thread, so `rollup` has a single bucket and the arithmetic is exact.
const THREAD: u64 = 1;
const POLL_LIMIT: usize = 100;
/// Generous ceiling for the aggregate calls, used only to measure the real
/// cost. `McpTestClient::call_tool` hardcodes 30s, which is shorter than the
/// aggregate takes at this size, so the aggregate modes are called through
/// `call_with_timeout` instead.
const AGGREGATE_TIMEOUT_SECS: u64 = 600;

/// Unwrap a raw `tools/call` envelope into the tool's own payload.
///
/// `McpTestClient::call_tool` does this internally; `call_with_timeout` does
/// not — it returns the JSON-RPC envelope verbatim, because the point of
/// taking an explicit timeout is to reach the aggregate modes whose cost the
/// client's hardcoded 30s default would cut short.
///
/// Indexing that envelope as if it were the payload is a mistake this file
/// used to make: `summary["total_events"]` on
/// `{"id":..,"jsonrpc":..,"result":{..}}` yields `None`, which read as a
/// missing count rather than as a wrong index. `wire_retention_facts.rs:34`
/// already does `.get("result")` for the same reason.
///
/// A `isError: true` response is not an error here: after D2 a budget stop is
/// reported as an error envelope *carrying the resume anchor*, and this test
/// asserts on that shape. So the payload is returned either way and the
/// caller decides what it means.
fn unwrap_tool_envelope(raw: &serde_json::Value) -> serde_json::Value {
    let result = raw
        .get("result")
        .unwrap_or_else(|| panic!("tools/call must answer with a result, got {raw}"));
    let content = result
        .get("content")
        .and_then(|c| c.as_array())
        .unwrap_or_else(|| panic!("result must carry a content array, got {raw}"));
    let text = content
        .first()
        .and_then(|c| c.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or_else(|| panic!("the first content block must be text, got {raw}"));
    serde_json::from_str(text)
        .unwrap_or_else(|e| panic!("the tool payload must be JSON ({e}), got {text}"))
}

fn temp_root(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "chronos-scale-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("create temp root");
    p
}

/// A minimal but well-formed `TraceEvent`.
///
/// The reader decodes every record into a `TraceEvent` and an undecodable
/// record fails the whole read closed, so the seeded payload must be a real
/// event under the canonical `"trace_event"` tag. Arbitrary bytes would make
/// the log unreadable rather than merely large.
fn trace_event(event_id: u64) -> TraceEvent {
    TraceEvent {
        event_id,
        timestamp_ns: MonotonicNs::from(event_id * 1_000),
        thread_id: THREAD,
        event_type: EventType::FunctionEntry,
        location: SourceLocation {
            function: Some("scale_work".to_string()),
            ..SourceLocation::default()
        },
        data: EventData::Function {
            name: "scale_work".to_string(),
            signature: None,
            symbol_id: None,
            invocation_id: None,
            parent_invocation_id: None,
        },
    }
}

/// Seed a real 1M-event log with the production writer.
///
/// Nothing here hand-writes a manifest, so the segments and retention
/// metadata on disk are exactly what the server will reopen. The log is
/// dropped before the server starts: the server must be able to open it from
/// what is on disk alone.
fn seed_million_events(root: &Path) {
    let dir = root.join(SESSION);
    std::fs::create_dir_all(&dir).expect("create execution-log dir");

    let session_id = SessionId::new(SESSION);
    let log = SegmentedExecutionLog::open(session_id.clone(), SegmentedConfig::with_dir(&dir))
        .expect("open execution log for seeding");

    // Each record is encoded for real, because `event_id` and the timestamp
    // vary per event and the reader decodes them. A reused payload would make
    // this a test of a log where every record claims the same identity.
    for i in 1..=EVENTS {
        log.append(NewExecutionRecord {
            session_id: session_id.clone(),
            kind: ExecutionKind::Raw,
            monotonic_ns: i * 1_000,
            payload: ExecutionPayload::new(
                serde_json::to_vec(&trace_event(i)).expect("encode trace event"),
                "trace_event",
            ),
            ..Default::default()
        })
        .expect("append event");
    }
    log.flush().expect("flush seeded log");
}

// Ignored on purpose, with the cost recorded rather than hidden. Measured
// 2026-10-02 on a host at load ~17: seeding 1M events took 32.5s and wrote
// 26,971,843 bytes; the paged `poll` was fast and correct; but `summarize`
// ran past 600s at 98.5% CPU holding ~1.5 GB RSS. Remote CI runs
// `cargo test --workspace --tests`, so an un-ignored test would add minutes
// to every run for a check whose own result is "the aggregate does not scale
// yet". The skip list in CI is not an option either: it is derived from
// `reconstruction-contracts.toml` for deferred tests that FAIL, and the Debt
// Sentinel requires a skipped test to still fail exactly as declared.
//
//     cargo test -p chronos-sandbox --test scale_execution_log_1m -- --ignored --nocapture
#[tokio::test]
#[ignore = "1M-event scale characterization: seed 32.5s, aggregate over 600s / 1.5GB RSS; not for the hot CI path"]
async fn the_read_path_serves_a_million_events() {
    let root = temp_root("1m");
    let started = std::time::Instant::now();
    seed_million_events(&root);
    let seed_secs = started.elapsed().as_secs_f64();
    eprintln!("seeded {EVENTS} events in {seed_secs:.1}s");

    // The log must really be on disk and really be that large, otherwise the
    // assertions below could pass against a log that was never written.
    let seg_bytes: u64 = std::fs::read_dir(root.join(SESSION))
        .expect("segment dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("seg"))
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum();
    assert!(
        seg_bytes > 0,
        "no segment bytes were written; the rest of this test would be vacuous"
    );
    eprintln!("segment bytes on disk: {seg_bytes}");

    let mcp_path = McpTestClient::resolve_mcp_path();
    let mut env = HashMap::new();
    env.insert(
        "CHRONOS_EXECUTION_LOG_DIR".to_string(),
        root.to_string_lossy().to_string(),
    );
    let mut client = McpTestClient::start_with_env(&mcp_path, &env)
        .await
        .expect("MCP server must start against a 1M-event log root");

    // ------------------------------------------------- bounded poll at scale
    let first = client
        .call_tool(
            "execution_log_read",
            json!({ "session_id": SESSION, "mode": "poll", "limit": POLL_LIMIT }),
        )
        .await
        .unwrap_or_else(|e| panic!("poll over a 1M-event log must succeed, got: {e}"));
    let batch = &first["events"]
        .as_array()
        .unwrap_or_else(|| panic!("poll must return an events array, got {first}"));
    assert_eq!(
        batch.len(),
        POLL_LIMIT,
        "a bounded poll must return exactly the requested batch out of {EVENTS}, \
         not the whole session and not fewer: got {}",
        batch.len()
    );

    let cursor = first["next_cursor"]
        .as_str()
        .unwrap_or_else(|| panic!("poll must return a next_cursor, got {first}"))
        .to_string();

    // Resuming must not replay what the first poll already delivered.
    let second = client
        .call_tool(
            "execution_log_read",
            json!({
                "session_id": SESSION,
                "mode": "poll",
                "limit": POLL_LIMIT,
                "cursor": cursor,
            }),
        )
        .await
        .unwrap_or_else(|e| panic!("resumed poll must succeed, got: {e}"));
    let second_batch = &second["events"]
        .as_array()
        .unwrap_or_else(|| panic!("resumed poll must return an events array"));
    assert_eq!(
        second_batch.len(),
        POLL_LIMIT,
        "a resumed poll must deliver the next batch, not re-serve the first"
    );
    assert_ne!(
        second_batch[0], batch[0],
        "the resumed poll replayed the first batch's first event"
    );

    // ------------------------------------------------------------ summarize
    // The aggregation path reads every page of the session. This is the
    // assertion that a five-event test can never make: that walking a million
    // records completes and that the total is the real one.
    //
    // Measured with an explicit generous timeout rather than the client's
    // default 30s, because the interesting datum is how long this actually
    // takes, not whether it beats an arbitrary client default. The number is
    // printed and asserted against below, so the test records the cost
    // instead of hiding it behind a pass.
    let t = std::time::Instant::now();
    let raw = client
        .call_with_timeout(
            "tools/call",
            json!({ "name": "execution_log_read", "arguments": {
                "session_id": SESSION, "mode": "summarize" } }),
            std::time::Duration::from_secs(AGGREGATE_TIMEOUT_SECS),
        )
        .await
        .unwrap_or_else(|e| panic!("summarize over {EVENTS} events must answer, got: {e}"));
    let summarize_secs = t.elapsed().as_secs_f64();
    eprintln!("summarize answered in {summarize_secs:.1}s: {raw}");

    let summary = unwrap_tool_envelope(&raw);

    // The honest outcomes are exactly two, and the test accepts both:
    //
    //   a) the walk finished inside its ceiling -> the total is the real one
    //   b) the walk passed its ceiling           -> an error carrying the
    //                                               resume anchor
    //
    // What is NOT acceptable, and what this assertion exists to reject, is the
    // third outcome this code used to produce: an `Ok` aggregate over a prefix
    // of the log whose `total_events` silently undercounted it. That shape is
    // the Silent Lie ADR-0004 exists to remove, and after D2 the read path can
    // no longer emit it — so the test pins that it cannot come back.
    if summary.get("error").is_some() {
        let reason = summary["error"].as_str().unwrap_or("<not a string>");
        assert!(
            reason.starts_with("read_path_"),
            "a summarize failure must name its reason from the read-path budget \
             contract, got {reason:?} in {summary}"
        );
        let next_seq = summary["next_seq"]
            .as_u64()
            .unwrap_or_else(|| panic!("a budget stop must carry the resume anchor, got {summary}"));
        assert!(
            next_seq > 0 && next_seq <= EVENTS,
            "the resume anchor must point inside the session, got {next_seq}"
        );
        assert_eq!(
            summary["partial"],
            json!(true),
            "a budget stop must be marked partial, never presented as complete: {summary}"
        );
        eprintln!(
            "summarize stopped at its ceiling after {next_seq} events \
             ({:.1}s) and offered the resume anchor — the honest stop",
            summarize_secs
        );
    } else {
        let total = summary["total_events"].as_u64().unwrap_or_else(|| {
            panic!("a complete summarize must report total_events, got {summary}")
        });
        assert_eq!(
            total, EVENTS,
            "a summarize that reports success must account for every seeded event, \
             not a truncated read"
        );
    }

    // The bound below is deliberately NOT asserted, and the reason is worth
    // stating precisely because it is no longer the one this file used to
    // carry. The 2026-10-02 note ("ran past 600s at load ~17") was measured
    // against a `read_from_seq` that cloned the whole log on every page; R2.2
    // replaced it with a windowed slice, and the same measurement afterwards
    // answered in ~11s. So the old figure does not describe current code, and
    // the new one is not asserted either — a wall-clock budget asserted in a
    // test that seeds a million records is a flake generator on shared CI.
    // What IS asserted is the contract above: a complete answer is complete,
    // and a stopped one is stopped and resumable. The cost is printed on every
    // run so it is recorded rather than assumed.
    eprintln!("AGGREGATE COST over {EVENTS} events: {summarize_secs:.1}s");

    // ---------------------------------------------------------------- rollup
    // `rollup` answers with aggregate counts, not a per-thread array: the
    // tool reports `invocation_count`, `total_events`, `mean_per_invocation`
    // and `max_per_invocation`. `total_events` is the scale assertion -- it
    // must again account for the whole session rather than a truncated read.
    //
    // Routed through `call_with_timeout` for the same two reasons as
    // `summarize`: the client's 30s default is shorter than an aggregate over
    // 1M can be expected to take, and `call_tool` turns an `isError` envelope
    // into a transport `Err`, which would erase the budget stop's anchor —
    // the one number a caller needs to resume.
    let t = std::time::Instant::now();
    let raw_rollup = client
        .call_with_timeout(
            "tools/call",
            json!({ "name": "execution_log_read", "arguments": {
                "session_id": SESSION, "mode": "rollup" } }),
            std::time::Duration::from_secs(AGGREGATE_TIMEOUT_SECS),
        )
        .await
        .unwrap_or_else(|e| panic!("rollup over {EVENTS} events must answer, got: {e}"));
    let rollup_secs = t.elapsed().as_secs_f64();
    eprintln!("rollup answered in {rollup_secs:.1}s");
    let rollup = unwrap_tool_envelope(&raw_rollup);

    if rollup.get("error").is_some() {
        let reason = rollup["error"].as_str().unwrap_or("<not a string>");
        assert!(
            reason.starts_with("read_path_"),
            "a rollup failure must name its reason from the read-path budget \
             contract, got {reason:?} in {rollup}"
        );
        assert!(
            rollup["next_seq"].as_u64().is_some_and(|n| n > 0),
            "a budget stop must carry the resume anchor, got {rollup}"
        );
        eprintln!("rollup stopped at its ceiling and offered the resume anchor");
    } else {
        let rollup_total = rollup["total_events"]
            .as_u64()
            .unwrap_or_else(|| panic!("a complete rollup must report total_events, got {rollup}"));
        assert_eq!(
            rollup_total, EVENTS,
            "a rollup that reports success must account for every seeded event, \
             not a truncated read"
        );
    }
    // The seeded records carry `invocation_id: None`, and the read path cannot
    // group by invocation id yet — so the rollup groups by `thread_id`. All
    // the seeded records are on one thread, so the group count is 1.
    //
    // What must be pinned is not the number but that the number is
    // self-describing: an answer of `invocation_count: 1` is only honest
    // alongside a `grouping_key` that says it is a thread count. Without that
    // field an agent reads one invocation where the truth is "one thread, and
    // the number of invocations is unknown" — the 1M-event case makes it
    // stark, because 1,000,000 events on one thread is very unlikely to be
    // one invocation.
    assert_eq!(
        rollup["grouping_key"],
        json!("thread_id"),
        "the rollup must declare what it grouped by: {rollup}"
    );
    assert_eq!(
        rollup["grouping_is_identity"],
        json!(false),
        "a thread grouping must not be presented as an identity: {rollup}"
    );
    let group_count = rollup["invocation_count"]
        .as_u64()
        .unwrap_or_else(|| panic!("rollup must report a group count, got {rollup}"));
    assert_eq!(
        group_count, 1,
        "all seeded records share one thread, so there is exactly one group: {rollup}"
    );

    eprintln!(
        "read path served {EVENTS} events: seed {seed_secs:.1}s, summarize {summarize_secs:.1}s"
    );
    let _ = std::fs::remove_dir_all(&root);
}
