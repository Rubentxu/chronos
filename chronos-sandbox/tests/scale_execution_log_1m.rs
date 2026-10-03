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
//!      that can exhaust memory or time out at scale.
//!   3. `rollup` reports a `total_events` of 1,000,000, and a group count of
//!      1, because the seeded records all share one `thread_id` and a rollup
//!      that invented invocations for them would be fabricating identities
//!      out of v1-shaped records.
//!   4. Resuming a poll from the returned cursor does not replay.
//!
//! The fixture — the 1M-event log, the server, and the two aggregate calls —
//! lives in `tests/common`, shared with `scale_aggregate_p95_1m.rs`. What
//! stays here is what only *this* lane asserts: the wire behaviour of the
//! read path at scale.
//!
//! NOT in the normal gate, on purpose. The gate runs exactly
//! `cargo test -p chronos-sandbox --test execution_log_read_e2e`, so a test in
//! its own target does not lengthen every gate by the cost of seeding a
//! million records. A scale test that only runs in CI is close to worthless,
//! so this one is meant to be run explicitly:
//!
//!     cargo test -p chronos-sandbox --test scale_execution_log_1m -- --ignored --nocapture
//!
//! The recorded run and its wall-clock cost are in the commit that added this
//! file and in the SDDK ledger, so the number is not folklore.
//!
//! STRUCTURE: one `#[tokio::test]`, one server. Same reasoning as
//! `execution_log_read_e2e`: a server booted in one test has its stdio bound
//! to that test's runtime, and the runtime dies with the test, so a server
//! shared across tests observes a dead transport.

mod common;

use common::{seed, start_server, temp_root, AggregateOutcome, AggregationOp, EVENTS, SESSION};
use serde_json::json;

const POLL_LIMIT: usize = 100;

#[tokio::test]
#[ignore = "1M-event scale characterization: seed ~32s plus two whole-log aggregates; not for the hot CI path"]
async fn the_read_path_serves_a_million_events() {
    let root = temp_root("1m");
    let seed_secs = seed(&root);
    let mut client = start_server(&root).await;

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

    // ------------------------------------------------------- whole-log modes
    // The aggregation path reads every page of the session. This is the
    // assertion that a five-event test can never make: that walking a million
    // records completes and that the total is the real one.
    //
    // The honest outcomes are exactly two, and the test accepts both:
    //
    //   a) the walk finished inside its ceiling -> the total is the real one
    //   b) the walk passed its ceiling           -> a budget stop carrying the
    //                                               resume anchor
    //
    // What is NOT acceptable, and what these assertions exist to reject, is
    // the third outcome this code used to produce: an `Ok` aggregate over a
    // prefix of the log whose `total_events` silently undercounted it. That
    // shape is the Silent Lie ADR-0004 exists to remove, and after D2 the
    // read path can no longer emit it — so the test pins that it cannot come
    // back.
    let summarize_secs = match AggregationOp::Summarize.once(&mut client).await {
        AggregateOutcome::Complete {
            total_events,
            secs,
            payload,
        } => {
            eprintln!("summarize answered in {secs:.1}s: {payload}");
            assert_eq!(
                total_events, EVENTS,
                "a summarize that reports success must account for every seeded \
                 event, not a truncated read"
            );
            secs
        }
        AggregateOutcome::Stopped {
            reason,
            next_seq,
            secs,
        } => {
            eprintln!(
                "summarize stopped at its ceiling after {next_seq} events ({secs:.1}s) \
                 and offered the resume anchor ({reason}) — the honest stop"
            );
            secs
        }
    };

    // `rollup` answers with aggregate counts, not a per-thread array: the tool
    // reports `invocation_count`, `total_events`, `mean_per_invocation` and
    // `max_per_invocation`. `total_events` is again the scale assertion — it
    // must account for the whole session rather than a truncated read.
    let rollup = match AggregationOp::Rollup.once(&mut client).await {
        AggregateOutcome::Complete {
            total_events,
            payload,
            secs,
        } => {
            eprintln!("rollup answered in {secs:.1}s");
            assert_eq!(
                total_events, EVENTS,
                "a rollup that reports success must account for every seeded \
                 event, not a truncated read"
            );
            payload
        }
        AggregateOutcome::Stopped { next_seq, secs, .. } => {
            eprintln!("rollup stopped at its ceiling after {next_seq} events ({secs:.1}s)");
            // A stopped rollup carries no group statistics, so there is
            // nothing left to pin. The stop itself was already checked against
            // the read-path budget contract in `AggregationOp::once`.
            let _ = std::fs::remove_dir_all(&root);
            return;
        }
    };

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

    // The wall-clock bound is deliberately NOT asserted, and the reason is
    // worth stating precisely because it is no longer the one this file used
    // to carry. The 2026-10-02 note ("ran past 600s at load ~17") was
    // measured against a `read_from_seq` that cloned the whole log on every
    // page; R2.2 replaced it with a windowed slice, and the same measurement
    // afterwards answered in ~11s. So the old figure does not describe current
    // code, and the new one is not asserted either — a wall-clock budget
    // asserted in a test that seeds a million records is a flake generator on
    // shared CI. What IS asserted is the contract above: a complete answer is
    // complete, and a stopped one is stopped and resumable. The cost is
    // printed on every run so it is recorded rather than assumed.
    eprintln!(
        "read path served {EVENTS} events: seed {seed_secs:.1}s, summarize {summarize_secs:.1}s"
    );
    let _ = std::fs::remove_dir_all(&root);
}
