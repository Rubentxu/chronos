//! The p95 of `summarize` and `rollup` at 1M — the budget `SCALE_BUDGETS` §8
//! says cannot exist today.
//!
//! ## Why this target exists at all
//!
//! §8 is explicit that its blocker is *cost*, not difficulty:
//!
//! > Una sola llamada a `summarize` a 1M cuesta ~1.223 s. Una distribución p95
//! > exige 20 muestras como mínimo: 20 × 1.222,7 s ≈ 6,8 h por operación.
//!
//! and it names the way out as a design requirement to be resolved first:
//!
//! > **Opciones para el gate de R2.1** … o arreglar §4.6 primero y re-medir.
//!
//! R2.2 fixed §4.6 — it removed the per-page full clone behind
//! `read_from_seq`, and the ledger records that change as **>51x** on
//! aggregation, specifically noting it removed a *quadratic term* rather than
//! a constant. So the option §8 named was taken, and the number the whole
//! argument rested on is no longer the number in the tree.
//!
//! Measured after that fix, `summarize` over 1M events answers in **≈11,8 s** —
//! a hundredfold change, not a rounding. Twenty samples is therefore minutes
//! per operation, not hours. **The blocker dissolved because the quantity it
//! was computed from changed**, which is the honest way for a cost blocker to
//! go away: not by reinterpreting it, but by removing the thing that made it
//! expensive.
//!
//! ## What this measures, and what it deliberately does not
//!
//! Twenty samples per operation on a single seeded 1M session, reporting p50
//! and p95. It is a **load characterisation on one host at one size**, not a
//! portable budget: §0 forbids pinning a number nobody can re-measure, and a
//! p95 measured on a shared machine is exactly that. What this target buys is
//! the *distribution* — whether the cost is a steady 12 s or a bimodal one
//! with a tail — which is the thing §8 said was missing and could not be
//! obtained.
//!
//! Two properties are asserted, and they are the ones that make the numbers
//! mean something:
//!
//!   * **Every sample is a whole-log aggregation** that reports the full
//!     `total_events`. A "fast" sample that folded nothing is not a fast
//!     aggregate, it is a wrong one, and a p95 over wrong samples is a
//!     beautifully precise lie. A budget stop is rejected outright rather
//!     than counted: it is a property of the walk's deadline, not of the
//!     operation's cost.
//!   * **The spread is reported, not just the percentile.** p95 alone hides
//!     bimodality; `p50`, `p95` and `max` together show the shape.
//!
//!     cargo test -p chronos-sandbox --test scale_aggregate_p95_1m -- --ignored --nocapture

mod common;

use common::{seed, start_server, temp_root, AggregateOutcome, AggregationOp, EVENTS};

/// The p95 budget of `SCALE_BUDGETS` §8, as a sample count.
///
/// Twenty is the smallest count for which a 95th percentile is not just the
/// maximum: at 20 samples the nearest-rank p95 is the 19th of 20, so a single
/// pathological sample does not become the whole number.
const SAMPLES: usize = 20;

/// The shape of a sample distribution, not just one number out of it.
///
/// `p95` alone cannot distinguish "slow" from "variable", and those are
/// different problems: a slow aggregate is a cost issue, a bimodal one points
/// at a path that sometimes skips work. `all` keeps the raw samples in
/// acquisition order so drift over the run is visible too.
#[derive(Debug)]
struct SampleStats {
    p50: f64,
    p95: f64,
    max: f64,
    min: f64,
    /// The samples in acquisition order, not sorted.
    all: Vec<f64>,
}

impl SampleStats {
    /// Nearest-rank percentiles over `samples`.
    ///
    /// Nearest-rank, not interpolated: with 20 real measurements there is no
    /// honest reading between the 10th and 11th sorted sample, and inventing
    /// one would put a number in the report that no run ever produced.
    fn of(samples: &[f64]) -> Self {
        assert!(
            !samples.is_empty(),
            "a percentile over no samples is not a percentile"
        );
        let mut sorted = samples.to_vec();
        sorted.sort_by(f64::total_cmp);
        let rank = |q: f64| -> f64 {
            let idx = (q * sorted.len() as f64).ceil().max(1.0) as usize - 1;
            sorted[idx.min(sorted.len() - 1)]
        };
        Self {
            p50: rank(0.50),
            p95: rank(0.95),
            max: *sorted.last().expect("non-empty"),
            min: sorted[0],
            all: samples.to_vec(),
        }
    }
}

/// The host-fingerprint fields the numbers below are only valid for. §0's
/// rule is that a budget must be re-measurable, and a percentile is only
/// meaningful next to the thing that produced it.
fn host_note() -> String {
    let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".into());
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(0);
    format!("kernel {kernel}, {cpus} logical cpus, debug build")
}

/// Prove the arithmetic of the steadiness guard before spending 1M events on
/// it, and pin the two properties `SAMPLES` was chosen for.
///
/// This runs in the normal suite because it costs nothing. A guard whose
/// threshold has never been shown to reject a bad distribution is a comment
/// with an `assert!` attached.
#[test]
fn the_steadiness_threshold_separates_steady_from_bimodal() {
    // A perfectly steady cost satisfies the guard.
    let steady: Vec<f64> = vec![12.0; SAMPLES];
    let s = SampleStats::of(&steady);
    assert_eq!(
        (s.p50, s.p95, s.max, s.min),
        (12.0, 12.0, 12.0, 12.0),
        "a constant sample set must report itself as constant"
    );
    assert!(
        s.p95 <= s.p50 * 2.0,
        "a steady cost must satisfy the guard, got p50 {} p95 {}",
        s.p50,
        s.p95
    );

    // Two slow samples in twenty are bimodality, and the guard must reject
    // it: this is the shape §8 could not distinguish from "slow".
    let mut bimodal: Vec<f64> = vec![12.0; SAMPLES - 2];
    bimodal.extend([60.0, 60.0]);
    let b = SampleStats::of(&bimodal);
    assert_eq!(b.p50, 12.0, "the fast mode must still be the median");
    assert_eq!(b.p95, 60.0, "the slow mode must be what p95 finds");
    assert!(
        b.p95 > b.p50 * 2.0,
        "the guard exists to reject exactly this: p95 {} vs p50 {}",
        b.p95,
        b.p50
    );

    // One pathological sample out of twenty is not bimodality, and this is
    // the property that sample count was chosen for: at 20 samples the
    // nearest-rank p95 is the 19th, so a single outlier lands in `max` and
    // does not become the reported cost of the whole operation. Nothing is
    // hidden — `max` still carries it.
    let mut one_outlier: Vec<f64> = vec![12.0; SAMPLES - 1];
    one_outlier.push(60.0);
    let o = SampleStats::of(&one_outlier);
    assert_eq!(
        (o.p50, o.p95, o.max),
        (12.0, 12.0, 60.0),
        "a lone outlier must move max, not p95 — that is what 20 samples buys"
    );

    // The raw samples stay in acquisition order, so a cost that drifts over
    // the run is visible in the printed distribution instead of being sorted
    // into a shape that never happened.
    let drifting = vec![10.0, 30.0, 10.0];
    assert_eq!(
        SampleStats::of(&drifting).all,
        vec![10.0, 30.0, 10.0],
        "the printed distribution must be chronological, not sorted"
    );
}

#[tokio::test]
#[ignore = "1M-event p95 characterisation: one seed plus 42 whole-log aggregates; not for the hot CI path"]
async fn summarize_and_rollup_have_a_measurable_p95_at_1m() {
    eprintln!(
        "host: {} | {SAMPLES} samples per operation over one seeded 1M session",
        host_note()
    );

    let root = temp_root("p95");
    let seed_secs = seed(&root);
    eprintln!("fixture ready in {seed_secs:.1}s");
    let mut client = start_server(&root).await;

    for op in [AggregationOp::Summarize, AggregationOp::Rollup] {
        // Warm-up, discarded: the first aggregate pays page-cache and
        // allocator growth that no later sample pays, and folding it into a
        // p95 would measure the cold-start of a process rather than the
        // operation. The 1M lane's single-shot numbers included it, which is
        // one of the reasons they are not comparable to these.
        let _ = op.once(&mut client).await;

        let mut samples = Vec::with_capacity(SAMPLES);
        for i in 0..SAMPLES {
            let secs = match op.once(&mut client).await {
                AggregateOutcome::Complete {
                    total_events, secs, ..
                } => {
                    assert_eq!(
                        total_events, EVENTS,
                        "sample {i} of {} reported {total_events} events instead of {EVENTS}. A p95 \
                         over aggregates that do not cover the log is a precise lie, not a \
                         measurement.",
                        op.name()
                    );
                    secs
                }
                AggregateOutcome::Stopped {
                    reason,
                    next_seq,
                    secs,
                } => panic!(
                    "{} sample {i} stopped at its read-path budget ({reason}) at seq {next_seq} \
                     after {secs:.1}s. A p95 measured partly from walks that never finished \
                     describes the budget, not the operation — raise the ceiling or shrink the \
                     session before reading anything into these numbers.",
                    op.name()
                ),
            };
            samples.push(secs);
        }

        let stats = SampleStats::of(&samples);
        eprintln!(
            "{} at 1M over {SAMPLES} samples: p50 {:.3}s | p95 {:.3}s | max {:.3}s (min {:.3}s)",
            op.name(),
            stats.p50,
            stats.p95,
            stats.max,
            stats.min,
        );
        eprintln!(
            "   distribution: {}",
            stats
                .all
                .iter()
                .map(|s| format!("{s:.2}"))
                .collect::<Vec<_>>()
                .join(" ")
        );

        // The guard this target is really for: a p95 that only exists if the
        // operation is steady. §8 could not distinguish "slow" from
        // "variable", and a bimodal aggregate is a different problem from a
        // slow one — it points at a path that sometimes skips work. Asserting
        // that p95 stays near p50 says the cost is a property of the operation
        // and not of which path this particular call took.
        assert!(
            stats.p95 <= stats.p50 * 2.0,
            "{}: p95 {:.3}s is more than 2x p50 {:.3}s, so the cost is not steady. A bimodal \
             aggregate usually means some call takes a different path than most — check the \
             per-sample distribution printed above before reading the p95 as the cost of the \
             operation.",
            op.name(),
            stats.p95,
            stats.p50,
        );
    }

    let _ = std::fs::remove_dir_all(&root);
}
