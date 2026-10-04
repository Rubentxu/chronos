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

use common::{
    seed, seed_with, start_server, temp_root, AggregateOutcome, AggregationOp, EVENTS, SESSION,
};
use serde_json::json;

const POLL_LIMIT: usize = 100;

/// Resident set size, in KiB, of one process — the **server**, not the harness.
///
/// `SCALE_BUDGETS` §9.5 carries four RSS figures for the same 1M aggregate that
/// disagree by ~2,5x, and every one of them came from wrapping the whole
/// `cargo test` in `/usr/bin/time -v`. That reads the tree's high-water mark
/// and reports the **largest single process** in it, not a sum — so the number
/// is real, but the tree has two members with opposite jobs: this harness,
/// which holds a million `TraceEvent`s while seeding them, and the server
/// subprocess, which reads them back a page at a time. Which of the two was the
/// larger is not recoverable from the figure, which is why §9.5 declines to
/// pick a winner. Reading one process's own `statm` recovers it directly
/// instead of arguing about it.
///
/// Deliberately **not** a re-export of `chronos_services::process_metrics::
/// process_rss_kb`, which is the canonical implementation: `chronos-sandbox`
/// does not depend on `chronos-services`, and adding that edge would invert the
/// layering, since the harness sits *below* the services it exercises. The
/// format and the page count match and both are pinned to the same `man 5 proc`
/// contract; if the kernel's `statm` layout ever changes, both have to move
/// together.
///
/// Returns `None` off Linux, or once the process is gone.
fn process_rss_kb(pid: u32) -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        const PAGE_SIZE_KB: u64 = 4; // man 5 proc: fixed 4 KiB on every arch we target
        let raw = std::fs::read_to_string(format!("/proc/{pid}/statm")).ok()?;
        let mut fields = raw.split_whitespace();
        fields.next()?; // field (1) is total program size; RSS is field (2)
        let resident_pages: u64 = fields.next()?.parse().ok()?;
        Some(resident_pages.saturating_mul(PAGE_SIZE_KB))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        None
    }
}

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

/// A session **one event over** the D2 event ceiling stops honestly, and
/// resuming from its anchor reconstructs the whole thing.
///
/// The sibling test above seeds exactly 1.000.000, which is exactly
/// `ResourceLimits::default().max_events`, so it lands on the *good* side of a
/// boundary and says nothing about the other one. `SCALE_BUDGETS` §9.3 records
/// that the default cap sits precisely on R2.1's 1M target with no headroom —
/// which means "1M events within budget" is true **at** the boundary and not
/// past it. This is the measurement of the other side, and the thing it has to
/// establish is not that the walk stops (that is D2's job, proven elsewhere) but
/// that stopping is **lossless**: the anchor plus a resumed walk accounts for
/// every seeded event, with nothing double-counted and nothing dropped.
///
/// That is the Silent Lie boundary. An aggregate over a prefix that reported
/// itself as complete is the defect ADR-0004 exists to remove, and a ceiling
/// that truncates is exactly where it would come back. So the assertions are
/// arithmetic, not narrative: `stopped_at + resumed == seeded`.
///
/// ## The cursor is minted here, and that is a finding
///
/// A budget stop returns a bare `next_seq`. Turning it into a resumable call
/// requires an `EventsCursorV1`, whose wire form is
/// `ecv1:<schema>:<len>:<session>:<next_seq>` — and that form appears nowhere in
/// the tool description, the response, or the error. A client can derive it (the
/// `poll` response shows the shape and the schema version is 1), and that is
/// what this test does, but "derivable by a determined agent" is not the same as
/// "documented", and a stop that hands over an anchor the caller cannot spend
/// without reverse-engineering the format is half a contract. Recorded rather
/// than papered over: changing the wire format is a contract decision, not
/// something a characterisation lane decides on its own.
#[tokio::test]
#[ignore = "1M+1-event ceiling characterisation: a second 1M seed plus a stop and a resume; not for the hot CI path"]
async fn one_event_over_the_event_ceiling_stops_honestly_and_resumes_to_the_whole() {
    /// One past the default `max_events`, which is what makes this the other
    /// side of the boundary rather than a bigger version of the same case.
    const OVER: u64 = EVENTS + 1;

    // Cost, measured rather than guessed, because it is the reason this is
    // `#[ignore]`d: seeding 1.000.001 events took **34 s** onto the tmpfs and
    // **907 s** onto the NVMe volume this run happened to use, with the walk
    // itself at 11,3 s. The 26x is the filesystem under load, not the fixture
    // — which is the honest reason to record it and not to budget it: the
    // figure nobody can re-measure is the one §0 of SCALE_BUDGETS forbids
    // pinning. Point TMPDIR somewhere that can hold ~27 MB and can be written
    // fast; see the scratch-space note in `tests/common/mod.rs`.
    let root = temp_root("1m-plus-1");
    let seed_secs = seed_with(&root, OVER);
    eprintln!("seeded {OVER} events in {seed_secs:.1}s");
    let mut client = start_server(&root).await;

    // ------------------------------------------------------------ the stop
    let (stopped_at, stop_secs) = match AggregationOp::Summarize.once(&mut client).await {
        AggregateOutcome::Stopped { next_seq, secs, .. } => {
            // The ceiling is charged per page, so it fires on the page that
            // crosses the line rather than at the exact event — and on this
            // fixture that page is the LAST one. 976 full pages plus a partial
            // one cover all 1.000.001, so the walk is refused having already
            // read the whole session, with the anchor at its very end.
            //
            // Measured, not assumed: the stop reported
            // `events_scanned 1000001 over 977 pages … stopped at
            // next_seq=1000001`, and the resume below returns an empty
            // aggregate.
            //
            // That is recorded rather than asserted away, because it is the
            // interesting part. `read_budget`'s doc declares that an operation
            // "may overshoot by at most one page" as an accepted trade; here the
            // overshoot is the entire log, so the slack is not benign — it
            // discards a result the walk had already computed. Whether that
            // should change is a **D2 contract decision**, not something a
            // characterisation lane settles by asserting, and the existing
            // `d2_summarize_stops_at_its_deadline_instead_of_running_on` pins
            // the stop itself. So this asserts only what must hold either way.
            assert!(
                next_seq > EVENTS,
                "the ceiling must be enforced: it fired at {next_seq}, at or below {EVENTS}"
            );
            assert!(
                next_seq <= OVER,
                "the anchor must point inside the session, never past its end: {next_seq} > {OVER}"
            );
            eprintln!("summarize stopped at seq {next_seq} after {secs:.1}s");
            (next_seq, secs)
        }
        AggregateOutcome::Complete {
            total_events,
            payload,
            ..
        } => panic!(
            "a session one event over the ceiling must not be aggregated whole: it reported \
             {total_events} in {payload}"
        ),
    };

    // ---------------------------------------------------------- the resume
    // `poll` is the only response that hands a client a cursor, so the resume
    // cursor is built from the server's own encoding rather than a literal
    // format written out here — which also means this test fails if the shape
    // ever changes, instead of quietly minting something the server rejects.
    let first = client
        .call_tool(
            "execution_log_read",
            json!({ "session_id": SESSION, "mode": "poll", "limit": 1 }),
        )
        .await
        .expect("poll must answer against the seeded session");
    let template = first["next_cursor"]
        .as_str()
        .expect("poll must return a next_cursor")
        .to_string();
    let parts: Vec<&str> = template.split(':').collect();
    assert_eq!(
        parts.len(),
        5,
        "the cursor is documented as five colon-separated parts; a different shape means this \
         test is minting something else: {template}"
    );
    let last = parts.len() - 1;
    let resume_cursor = format!("{}:{stopped_at}", parts[..last].join(":"));

    let resumed = match AggregationOp::Summarize
        .once_from(&mut client, Some(&resume_cursor))
        .await
    {
        AggregateOutcome::Complete {
            total_events,
            payload,
            secs,
        } => {
            eprintln!("resumed summarize answered in {secs:.1}s: {payload}");
            total_events
        }
        AggregateOutcome::Stopped { next_seq, .. } => panic!(
            "the remainder is far below the ceiling, so the resume must finish; it stopped again \
             at {next_seq}"
        ),
    };

    // ------------------------------------------------------------ the arithmetic
    // This is the whole point: the stop plus the resume must account for every
    // seeded event. A ceiling that dropped events, or an anchor that re-read
    // them, breaks this sum — and either would be a silent lie at the exact size
    // the roadmap's headline claim is about.
    //
    // Measured, not designed: the stop accounts for **all** of it and the
    // resume accounts for **none**, because the ceiling fires on the last page
    // and the anchor lands on the last seq. The sum still has to hold — that is
    // the property that says no events are lost — but neither half may be
    // assumed to be non-empty, so the assertions bound rather than predict.
    assert!(
        stopped_at <= OVER,
        "the stop cannot claim to have read more than the session holds: {stopped_at} > {OVER}"
    );
    assert_eq!(
        stopped_at + resumed,
        OVER,
        "stop ({stopped_at}) plus resume ({resumed}) must reconstruct the seeded {OVER} exactly: \
         a ceiling that drops events or an anchor that replays them breaks this sum"
    );

    // The resume covering **nothing** is the finding, and it is pinned on
    // purpose. Leaving it unasserted would let a later change quietly make this
    // test weaker; asserting it means that revising D2 — returning the
    // aggregate once the walk has exhausted the log — has to come through here,
    // where the first arm's panic already says what the new behaviour would
    // mean. A characterisation that stops characterising is worse than none.
    assert_eq!(
        resumed, 0,
        "measured: the anchor is the last seq, so the resume aggregates nothing. If D2 is \
         revised to return an aggregate the walk already completed, this is the assertion that \
         must change — deliberately."
    );

    eprintln!(
        "one event over the ceiling: seed {seed_secs:.1}s, stop at {stopped_at} after \
         {stop_secs:.1}s, resume covered {resumed}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// `SCALE_BUDGETS` §9.5 carries four RSS figures for the same 1M aggregate that
/// disagree by ~2,5x, and it explicitly declines to pick a winner. The stated
/// reason is "probably route, not code" — an unmeasured guess.
///
/// This lane measures the route, because the guess was checkable, and **the
/// check refuted it**. Every one of those four numbers came from wrapping the
/// whole `cargo test` in `/usr/bin/time -v`, which reports the largest single
/// process in the tree. The tree has two members with opposite jobs: this
/// harness, which holds a million `TraceEvent`s while seeding them, and the
/// server subprocess, which reads them back. The natural guess is that the
/// seeder dominates and the tree-level figure was mostly test rig.
///
/// **Measured, it is the other way round: the server is the larger of the two
/// by 2,1x** — 949.204 KB against the harness's 442.724 KB, on the same run,
/// in the same units, at the same moment. The reading path is what costs the
/// memory, and §9.5's four figures were describing the product all along.
///
/// ## Why that also explains the number §9.4 could not place
///
/// §9.5's fourth recording quotes a "~949 MB peak observed in the server
/// process" while the tree-level figure for the same lane is 602.208 KB. Those
/// are consistent under `time -v` **only if the server is not in the tree
/// accounting**: the sandbox client reaps the server with an external `kill -9`
/// (`client/process.rs`, `force_kill`), so the grandchild is never `wait4`-ed
/// by cargo and its resident size never enters the reported maximum. The
/// figure is therefore the *harness's*, and the server was invisible to every
/// tree-level measurement in the table.
///
/// That is a property of the **measurement rig**, not of the read path, which
/// is why the fix is to attribute the number rather than to re-run it.
///
/// ## What this asserts, and what it deliberately does not
///
/// The ordering plus a ratio floor. Both are properties of the code rather than
/// of a host, which is what `SCALE_BUDGETS` §0 allows to be pinned; the
/// absolute figures are not, and are printed rather than asserted.
#[tokio::test]
#[ignore = "1M-event memory attribution: a third 1M seed; pairs with the p95 lane, not a replacement"]
async fn the_read_path_not_the_seeder_owns_the_million_event_footprint() {
    let root = temp_root("rss");
    let seed_secs = seed(&root);
    let mut client = start_server(&root).await;

    let server_pid = client
        .server_pid()
        .expect("a live server process must have a pid: without one nothing here is measurable");

    // The harness's own footprint, sampled at the same moment as the server's
    // and in the same units, because two numbers from different moments or
    // different units would reproduce the very ambiguity this lane removes.
    let harness_rss = process_rss_kb(std::process::id())
        .expect("this process must have a readable /proc/self/statm on Linux");

    // Aggregate first: the question is what serving the session costs, and a
    // server measured before it has read anything has not answered it.
    let _ = AggregationOp::Summarize.once(&mut client).await;

    let server_rss = process_rss_kb(server_pid)
        .expect("the server's /proc/<pid>/statm must be readable while it is running");

    eprintln!(
        "1M footprint — server {server_rss} KB, harness {harness_rss} KB \
         (ratio {:.2}x, seed {seed_secs:.1}s)",
        server_rss as f64 / harness_rss as f64
    );

    // Non-vacuity first. A zero or absent read on either side would let the
    // ratio below pass for the wrong reason, and the failure would look like a
    // pass rather than like a broken measurement — the same shape as the
    // envelope bug R2.9 fixed, where a parse of the wrong content block read as
    // a missing field.
    assert!(
        server_rss > 0 && harness_rss > 0,
        "both RSS readings must be real: server {server_rss} KB, harness {harness_rss} KB"
    );

    // The attribution, measured at 2,1x. The floor is well under the observed
    // value so it does not encode this host's allocator behaviour, but it is
    // above parity: a tree-level measurement that could not tell the two apart
    // is only worth anything if being able to tell them apart changes the
    // answer, and it does. A server that stopped exceeding the seeder — by
    // streaming rather than materialising, say — would cross this, and that
    // would be an improvement worth its own recording rather than a failure.
    assert!(
        server_rss > harness_rss * 3 / 2,
        "the server ({server_rss} KB) should exceed the seeder ({harness_rss} KB) by a clear \
         margin: the read path carries the session, the seeder only wrote it. Measured 2,1x. If \
         this no longer holds, the read path changed shape and §9.5 needs re-recording."
    );

    let _ = std::fs::remove_dir_all(&root);
}
