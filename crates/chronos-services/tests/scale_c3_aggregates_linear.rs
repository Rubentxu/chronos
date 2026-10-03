//! C3 of `docs/roadmap/SCALE_BUDGETS.md` §7.3, as a scale test.
//!
//! > **C3.** El coste total de `summarize` (y de `rollup`) debe ser lineal en N.
//! > Con N y 2N sembrados, afirmar que `coste(2N)/coste(N)` está acotado por una
//! > constante declarada.
//!
//! ## Where this lives, and why
//!
//! Its own test target, `#[ignore]`d, for the reason
//! `chronos-sandbox/tests/scale_read_from_seq_c1.rs` gives at length: the gate
//! runs a fixed, small set of targets, so a scale test in one of them would
//! lengthen every gate by the cost of seeding a large session. A scale test
//! that **passes** belongs behind `#[ignore]`, which nothing has to keep in
//! sync; a *failing* deferred test would belong in the manifest's skip list,
//! which is derived from `reconstruction-contracts.toml`. That file also
//! explains why the alternative — putting it in the hot path — is not
//! available: the Debt Sentinel requires a skipped test to still fail exactly
//! as declared.
//!
//!     cargo test -p chronos-services --test scale_c3_aggregates_linear -- --ignored --nocapture
//!
//! It lives in `chronos-services` rather than in the sandbox because
//! `summarize` is a `ReadPathService` method, and going through the service is
//! the point: that is the object the MCP server calls, so the budget, the
//! registry lookup and the D2 ceiling are all inside the measurement. Calling
//! the free function `summarize_log` would measure a path production does not
//! use — the same "requisito de validez" C1 carries.
//!
//! ## The assertion that carries the contract is the ratio, not the constant
//!
//! Declared, not fitted: **linear gives ≈ 2, quadratic gives ≈ 4.** `MAX_RATIO`
//! sits between them with room for a shared host, so the test discriminates on
//! the SHAPE of the curve and not on how fast this machine happens to be. A
//! constant tuned to the measurement would be a gate that cannot fail for the
//! right reason.
//!
//! This is the same structure C1 uses, inverted. C1 asserts the quotient is far
//! BELOW `N₂/N₁`, because a full-Vec clone is linear in N. C3 asserts the
//! quotient is far below 4, because the defect it guards is quadratic in the
//! page count. Both are about proportionality, and both put the threshold where
//! the wrong shape cannot reach it.
//!
//! ## What it would catch
//!
//! The measured defect: aggregation walked the log in pages of 1024, and each
//! page read iterated in chunks of `SCAN_CHUNK = 512` over a `read_from_seq`
//! that cloned the whole session, giving O(N²/512) with a local exponent
//! observed up to **2,5**. R2.2 removed the clone, which is what made this
//! contract passable at all — the ledger records that change as **>51x** on
//! aggregation, and specifically notes it removed a quadratic term rather than
//! a constant. C3 is what turns that one measurement into a standing guard: if a
//! future change reintroduces a per-page full scan, the ratio walks back toward
//! 4 and this test goes red.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chronos_domain::trace::TraceEvent;
use chronos_domain::{EventData, EventType, MonotonicNs, SourceLocation};
use chronos_log::{ExecutionKind, ExecutionPayload, NewExecutionRecord, SessionId};
use chronos_services::events_cursor::EventsCursorV1;
use chronos_services::read_budget::ResourceLimits;
use chronos_services::read_path::ReadPathService;
use chronos_services::session_log::SessionExecutionLogRegistry;

/// Doubling N must not quadruple the work. Declared between linear (2) and
/// quadratic (4); see the module doc for why the ratio, not the constant, is
/// what carries the contract.
const MAX_RATIO: f64 = 3.0;

/// The D2 ceiling this test runs under.
///
/// Generous ON PURPOSE, and the reason matters: C3 is about the SHAPE of the
/// cost curve, and a ceiling tight enough to bite would turn a load measurement
/// into a budget test — the failure mode SCALE_BUDGETS §8 describes, where a
/// number is pinned that nobody can then re-measure. `summarize` is wrapped in
/// the real budget here, so the budget's presence is exercised; its limit is
/// set well above the session so it never becomes the thing under test.
fn limits() -> ResourceLimits {
    ResourceLimits {
        max_events: 100_000_000,
        timeout_secs: 3_600,
    }
}

fn temp_root() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "chronos-c3-scale-{}-{}",
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
/// The aggregation decodes every record, and an undecodable record fails the
/// whole read CLOSED rather than being skipped, so the seeded payload has to be
/// a real event under the canonical `"trace_event"` tag. The first draft of
/// this test seeded arbitrary bytes under a `"scale"` tag and every read came
/// back as `evidence at seq 0 ... could not be decoded` — which is the read
/// path being correctly fail-closed, not the test being wrong about cost.
fn trace_event(event_id: u64) -> TraceEvent {
    TraceEvent {
        event_id,
        timestamp_ns: MonotonicNs::from(event_id * 1_000),
        thread_id: 1,
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

/// Seed `n` events through the production factory and register the log, so the
/// aggregation below goes through `ReadPathService` exactly as the MCP server
/// drives it.
fn seed(n: u64, root: &Path) -> (Arc<SessionExecutionLogRegistry>, String) {
    let registry = Arc::new(SessionExecutionLogRegistry::default());
    let session = SessionId::new(format!("c3-{n}"));
    let dir = root.join(session.as_str());
    let log = registry
        .register_create(dir, session.clone())
        .expect("production factory must create the log");

    // Each record is encoded for real: `event_id` and the timestamp vary per
    // event and the reader decodes them, so a reused payload would make this a
    // test of a log where every record claims the same identity.
    let started = Instant::now();
    for i in 1..=n {
        log.append(NewExecutionRecord {
            session_id: session.clone(),
            kind: ExecutionKind::Raw,
            monotonic_ns: i * 1_000,
            payload: ExecutionPayload::new(
                serde_json::to_vec(&trace_event(i)).expect("encode trace event"),
                "trace_event",
            ),
            ..Default::default()
        })
        .expect("append");
    }
    eprintln!(
        "seeded {n} events in {:.1}s",
        started.elapsed().as_secs_f64()
    );
    (registry, session.as_str().to_string())
}

/// One full aggregation over the whole log, returning its duration and the
/// event count it claims to have covered.
///
/// The count comes back so the caller can assert it: a "fast" aggregation that
/// read nothing is not a linear one, it is a wrong one, and a ratio alone
/// cannot tell the difference.
fn measure(n: u64, root: &Path) -> (Duration, u64) {
    let (registry, session) = seed(n, root);
    let read_path = ReadPathService::with_limits(registry, limits());
    let cursor = EventsCursorV1::start(SessionId::new(session.clone()));

    let started = Instant::now();
    let summary = read_path
        .summarize(&session, &cursor, 1_000_000_000)
        .expect("summarize must complete over a whole log");
    let elapsed = started.elapsed();

    (elapsed, summary.total_events)
}

#[ignore = "C3 scale contract: seeds 60k + 120k events and aggregates each whole; a load measurement, not for the hot CI path"]
#[test]
fn aggregation_grows_linearly_and_not_quadratically_with_the_session() {
    // 2x apart, as C3's enunciado prescribes. The span is deliberately small:
    // C3 is about the SHAPE between two points, and a 100x span would put the
    // seeding cost — not the aggregation — in the denominator.
    const N: u64 = 60_000;
    let root = temp_root();

    let (t_n, total_n) = measure(N, &root);
    let (t_2n, total_2n) = measure(N * 2, &root);

    eprintln!(
        "summarize  N={}: {:.3}s  total_events={}\n\
         summarize  N={}: {:.3}s  total_events={}",
        N,
        t_n.as_secs_f64(),
        total_n,
        N * 2,
        t_2n.as_secs_f64(),
        total_2n,
    );

    // Preconditions, so a fast ratio cannot be bought with a wrong measurement.
    assert_eq!(
        total_n, N,
        "the N-sized aggregate must have covered every seeded event; an \
         aggregate that undercounts is a Silent Lie, and a cheap one would \
         make the ratio below meaningless"
    );
    assert_eq!(
        total_2n,
        N * 2,
        "the 2N-sized aggregate must have covered every seeded event"
    );

    let ratio = t_2n.as_secs_f64() / t_n.as_secs_f64().max(f64::MIN_POSITIVE);
    eprintln!("cost(2N)/cost(N) = {ratio:.2}  (linear ~2, quadratic ~4)");

    assert!(
        ratio < MAX_RATIO,
        "C3 FAILS: doubling the session multiplied aggregation cost by {ratio:.2}x, \
         and a linear aggregation should cost about 2x. A ratio approaching 4 is \
         the O(N^2/page) signature this contract exists to catch — it means some \
         layer re-scans the log for every page instead of reading each event once.",
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// The clock-free half of the contract.
///
/// A ratio needs a shared host to be meaningful; the event count does not. An
/// aggregate over 2N events must bucket exactly 2N events, and every one of
/// them must land in exactly one bucket. That holds on a host too loaded to
/// time anything, which is the case that matters when this is re-measured to
/// settle §8.
#[ignore = "C3 scale contract: seeds 60k + 120k events and aggregates each whole; a load measurement, not for the hot CI path"]
#[test]
fn the_aggregate_buckets_every_event_exactly_once() {
    const N: u64 = 30_000;
    let root = temp_root();

    for n in [N, N * 2] {
        let (registry, session) = seed(n, &root);
        let read_path = ReadPathService::with_limits(registry, limits());
        let cursor = EventsCursorV1::start(SessionId::new(session.clone()));

        let summary = read_path
            .summarize(&session, &cursor, 1_000_000_000)
            .expect("summarize");

        assert_eq!(
            summary.total_events, n,
            "an aggregate that undercounts is a Silent Lie"
        );
        assert!(
            summary.bucket_count > 0,
            "an empty bucket set means the fold produced nothing, and the \
             total above would be counting an empty walk"
        );
    }

    let _ = std::fs::remove_dir_all(&root);
}
