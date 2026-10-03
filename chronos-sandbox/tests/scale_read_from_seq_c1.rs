//! C1 of `docs/roadmap/SCALE_BUDGETS.md` §7.1, as a scale test.
//!
//! > **C1.** Sea `L` un límite de ventana fijo y `C` un cursor. El coste de una
//! > llamada a `execution_log_read(mode=poll, limit=L, cursor=C)` **no crece con
//! > N**, el número total de eventos de la sesión, para ninguna posición de `C`.
//!
//! ## Where this lives, and why
//!
//! Its own test target, `#[ignore]`d. That is the repo's convention for scale
//! work and §7.4 names it: the gate runs
//! `cargo test -p chronos-sandbox --test execution_log_read_e2e`, so a test in
//! its own target does not lengthen every gate by the cost of seeding a
//! million records. The sibling `scale_execution_log_1m.rs` is placed and
//! annotated the same way, and says why the other option is not available:
//! the skip list in CI is derived from `reconstruction-contracts.toml` for
//! deferred tests that **FAIL**, and the Debt Sentinel requires a skipped test
//! to still fail exactly as declared. A scale test that **passes** does not
//! belong in that list — it belongs behind `#[ignore]`, which no manifest has
//! to keep in sync. That is also why this file touches nothing outside its own
//! target: `reconstruction-contracts.toml` is not this change's to edit.
//!
//! ## What it asserts, and the part that does not depend on a constant
//!
//! Seed the same logical session at N₁ = 10⁴, N₂ = 10⁵, N₃ = 10⁶ with `L` fixed
//! and the cursor at the head, take the median of `REPS` reads at each size, and
//! assert `c(N₃)/c(N₁) <= K` for a declared `K` independent of N.
//!
//! The assertion that actually carries the contract is the one next to it: the
//! quotient must not be *proportional* to `N₃/N₁` = 100. With the full-Vec
//! clone the cost is linear in N, so the quotient IS ≈ 100 and `K` is
//! irrelevant — it fails on the proportionality, not on the constant. Measured
//! on the change that closed C1, in the same harness the fix was measured with:
//! 399x before, 1.04x after.
//!
//! ## The log is the production one
//!
//! C1 carries a "requisito de validez": a test that measured the purely
//! in-memory provider would prove a path production does not use. This builds
//! the log through `SegmentedExecutionLogFactory` — the only
//! `ExecutionLogFactory` implementation in the tree (`factory.rs:42`) — and
//! reads through the `ExecutionLogProvider` port it hands back.
//!
//!     cargo test -p chronos-sandbox --test scale_read_from_seq_c1 -- --ignored --nocapture

use std::path::{Path, PathBuf};
use std::time::Instant;

use chronos_domain::ports::execution_log_factory::ExecutionLogFactory;
use chronos_log::factory::SegmentedExecutionLogFactory;
use chronos_log::EventSeq;

/// The three sizes C1 prescribes. The span is 100x, which is what makes the
/// proportionality assertion meaningful.
const SIZES: [u64; 3] = [10_000, 100_000, 1_000_000];
/// Fixed window, as C1 requires. Also the value the §3 baseline used.
const LIMIT: usize = 100;
/// C1's own tolerance for the per-size median.
const REPS: usize = 7;
/// Declared bound, independent of N: a windowed read must not grow with the
/// session. The defect's quotient was ≈ 100 and ≈ 399 measured, so anything in
/// single digits can only be noise or a regression that is not O(N) yet.
const MAX_QUOTIENT: f64 = 4.0;

fn temp_root() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "chronos-c1-scale-{}-{}",
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

/// Seed `n` events through the production factory and return the median
/// duration of one `read_from_seq` at the HEAD of the log, plus the same at the
/// TAIL. Both positions, because C1 says "para ninguna posición de `C`", and
/// because a full-Vec clone is position-independent for the wrong reason: it
/// walks the whole session either way.
fn measure(n: u64, root: &Path) -> (u128, u128) {
    let session = chronos_log::SessionId::new(format!("c1-{n}"));
    let dir = root.join(session.as_str());
    let factory = SegmentedExecutionLogFactory::new();
    let bundle = factory
        .create(dir, session.clone())
        .expect("production factory must create the log");
    let log = &bundle.evidence;

    let started = Instant::now();
    for i in 0..n {
        log.append(chronos_log::NewExecutionRecord {
            session_id: session.clone(),
            kind: chronos_log::ExecutionKind::Raw,
            monotonic_ns: i * 1_000,
            payload: chronos_log::ExecutionPayload::new(
                format!("{{\"e\":{i}}}").into_bytes(),
                "scale",
            ),
            ..Default::default()
        })
        .expect("append");
    }
    let seed_secs = started.elapsed().as_secs_f64();

    // The read must really be over n events, or the ratios below are measuring
    // an empty log that got faster.
    let total = log
        .read_from_seq(EventSeq::ZERO, n as usize)
        .expect("read all")
        .records
        .len();
    assert_eq!(
        total, n as usize,
        "the seeded session must really hold n events"
    );

    let median_at = |from: EventSeq| -> u128 {
        let mut samples = Vec::with_capacity(REPS);
        for _ in 0..REPS {
            let t = Instant::now();
            let page = log
                .read_from_seq(from, LIMIT)
                .expect("windowed read over the production log");
            samples.push(t.elapsed().as_nanos());
            assert_eq!(
                page.records.len(),
                LIMIT.min(n as usize),
                "a bounded read must return the requested window out of {n}"
            );
        }
        samples.sort_unstable();
        samples[samples.len() / 2]
    };

    let head = median_at(EventSeq::ZERO);
    let tail = median_at(EventSeq::new(n.saturating_sub(LIMIT as u64)));
    eprintln!(
        "N={n} seed={seed_secs:.1}s head p50={:.3}ms tail p50={:.3}ms",
        head as f64 / 1e6,
        tail as f64 / 1e6
    );
    (head, tail)
}

#[test]
#[ignore = "C1 scale contract: seeds up to 1,000,000 events per size (~30s each) — measured, not on the hot CI path"]
fn poll_cost_does_not_grow_with_the_session_size() {
    let root = temp_root();
    let mut head = Vec::new();
    let mut tail = Vec::new();
    for n in SIZES {
        let (h, t) = measure(n, &root);
        head.push(h);
        tail.push(t);
    }
    let _ = std::fs::remove_dir_all(&root);

    for (label, series) in [("head", &head), ("tail", &tail)] {
        let c_small = *series.first().expect("three sizes") as f64;
        let c_large = *series.last().expect("three sizes") as f64;
        let quotient = c_large / c_small;
        let n_quotient = SIZES[2] as f64 / SIZES[0] as f64;
        eprintln!("C1 at {label}: c(1M)/c(10k) = {quotient:.2}x  (N grew {n_quotient:.0}x)");

        assert!(
            quotient <= MAX_QUOTIENT,
            "C1 FAILS at the {label}: c(1M)/c(10k) = {quotient:.2}x exceeds the declared \
             {MAX_QUOTIENT}x. The session grew {n_quotient:.0}x, so a windowed read is still \
             paying for the session."
        );
        // The assertion that does not depend on MAX_QUOTIENT: the quotient must
        // not be proportional to the size span. Under the clone it was, which is
        // why this fails whatever constant the other assertion demands.
        assert!(
            quotient < n_quotient / 10.0,
            "C1 FAILS at the {label}: the cost quotient {quotient:.2}x is proportional to the \
             size span {n_quotient:.0}x — the cost is the session, not the window."
        );
    }

    // The sizes in between are the curve, not an assertion: they are what makes
    // the two quotients above legible if one of them ever fails.
    eprintln!("C1 curve (head, ms): {head:?}");
    eprintln!("C1 curve (tail, ms): {tail:?}");
}
