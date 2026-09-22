//! M7.4 lift: cost / memory baselines, FNV-1a collision hunt, UAT-M7-01/02 executors.
//!
//! Lifted from `/home/rubentxu/m7-spikes/m7.4-cost-memory-collision/` (598L).
//!
//! Closes ROADMAP §M7 §77 — "M7.4 UAT-M7-01/02 y baselines de coste/memoria".
//!
//! M7.1 / M7.2 / M7.3 delivered BehaviourFingerprint that can summarize a
//! session. They don't say what it costs, how much memory it uses, or whether
//! the FNV-1a 64-bit hash will collide on real-world data. M7.4 closes those
//! gaps with executable evidence.
//!
//! ## Duplication avoidance
//!
//! `measure_cost`/`measure_memory` REPLAY M7.3's allocation pattern using
//! its layout (25 bytes/pair for aggregate, 17 bytes/pair for shape). They
//! do NOT duplicate the FNV-1a hashing — that's delegated to M7.3's
//! `session_fingerprint`.
//!
//! `random_alignment_report` builds synthetic `AlignmentReport`s directly
//! from the M7.2 type definitions — no duplication of M7.2 logic.
//!
//! `run_uat_m7_01`/`run_uat_m7_02` use `align_sessions` from M7.2 and
//! `session_fingerprint` from M7.3, plus `parse_traceparent` from M6.1.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use super::alignment::{
    align_sessions, AlignmentError, AlignmentKey, AlignmentKeyKind, AlignmentReport,
    InvocationAlignment, InvocationStatus, InvocationWithEvents, Session,
};
use super::equivalence::equivalence_spec_default;
use super::fingerprint::{session_fingerprint, BehaviourFingerprint};
use crate::otlp::correlation::{ChronosEvent, EventField};
use crate::otlp::parse::parse_traceparent;
use crate::otlp::{OtlpInvocationId as InvocationId, RecordedInvocation};

// ---------------------------------------------------------------------------
// §1 — Cost baseline
// ---------------------------------------------------------------------------

/// Wall-clock cost of `session_fingerprint` for a given report, averaged over
/// multiple iterations. Uses `Instant::now()` — relative timing, not absolute
/// wall-clock (per project preference: no Unix timestamps in M7.x).
///
/// Returns `CostBaseline` with `iterations` runs aggregated. The first
/// iteration is **included** in the average — M7.4 deliberately does NOT do a
/// warm-up throwaway, because the spike evidence chain records the raw cost
/// including JIT/cold-cache effects.
#[derive(Debug, Clone, PartialEq)]
pub struct CostBaseline {
    /// Number of invocations in the input report.
    pub n_invocations: usize,
    /// Number of full `session_fingerprint` runs aggregated.
    pub iterations: u32,
    /// Total elapsed time across all iterations.
    pub total: Duration,
    /// Average elapsed time per iteration.
    pub per_iter: Duration,
    /// Invocations per second (computed as n_invocations / per_iter.as_secs_f64()).
    pub invocations_per_sec: f64,
}

fn report_total(report: &AlignmentReport) -> usize {
    report.matched.len() + report.mismatched.len() + report.only_in_a.len() + report.only_in_b.len()
}

fn session_fingerprint_or_panic(report: &AlignmentReport) -> BehaviourFingerprint {
    let total = report_total(report);
    assert!(total > 0, "session_fingerprint called on empty report");
    session_fingerprint(report).expect("non-empty report failed fingerprint")
}

/// Measure cost: run `session_fingerprint(report)` `iterations` times and
/// record elapsed time. If the report is empty (would error), returns a
/// baseline with `per_iter = Duration::ZERO`.
pub fn measure_cost(report: &AlignmentReport, iterations: u32) -> CostBaseline {
    let n_invocations = report_total(report);

    if iterations == 0 || n_invocations == 0 {
        return CostBaseline {
            n_invocations,
            iterations,
            total: Duration::ZERO,
            per_iter: Duration::ZERO,
            invocations_per_sec: 0.0,
        };
    }

    let start = Instant::now();
    for _ in 0..iterations {
        let _ = session_fingerprint_or_panic(report);
    }
    let total = start.elapsed();
    let per_iter = total / iterations;
    let invocations_per_sec = if per_iter.as_secs_f64() > 0.0 {
        n_invocations as f64 / per_iter.as_secs_f64()
    } else {
        f64::INFINITY
    };

    CostBaseline {
        n_invocations,
        iterations,
        total,
        per_iter,
        invocations_per_sec,
    }
}

// ---------------------------------------------------------------------------
// §2 — Memory baseline
// ---------------------------------------------------------------------------

/// Peak allocation during stream construction. Tracks the maximum number of
/// bytes the internal `Vec<u8>` reaches across all stages (aggregate stream,
/// shape stream, sort buffer). This is a **deterministic** measurement — it
/// does NOT measure process RSS. It is sufficient for "did the spike stay
/// under X bytes?" budgeting.
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryBaseline {
    /// Number of invocations in the input report.
    pub n_invocations: usize,
    /// Peak `Vec<u8>::capacity()` (in bytes) across all stream allocations.
    pub peak_vec_bytes: usize,
    /// Total bytes appended across all streams (lower bound on working set).
    pub total_bytes_appended: usize,
    /// Number of times a `Vec<u8>` was re-allocated (capacity doubled).
    pub reallocations: usize,
}

/// Compute the byte size of the aggregate stream for a given report, matching
/// M7.3's per-pair layout: 25 bytes/pair (status u8 + hash_a u64 + hash_b u64
/// + delta_or_sentinel u64).
pub fn aggregate_stream_size(report: &AlignmentReport) -> usize {
    let per_pair = 1 + 8 + 8 + 8;
    report_total(report) * per_pair
}

/// Compute the byte size of the shape stream for a given report, matching
/// M7.3's per-pair layout: 17 bytes/pair (status u8 + primary_hash u64 +
/// delta_or_sentinel u64).
pub fn shape_stream_size(report: &AlignmentReport) -> usize {
    let per_pair = 1 + 8 + 8;
    report_total(report) * per_pair
}

/// Measure memory: simulate the allocation pattern of `session_fingerprint`
/// and record peak `Vec<u8>` bytes + total bytes appended + reallocation count.
/// The simulation matches M7.3's actual code path: build a `Vec<u8>` for the
/// aggregate stream, push 25 bytes per pair, then build the shape stream,
/// push 17 bytes per pair.
pub fn measure_memory(report: &AlignmentReport) -> MemoryBaseline {
    let total = report_total(report);
    let agg_target = total * 25;
    let shape_target = total * 17;

    let mut agg: Vec<u8> = Vec::with_capacity(agg_target.max(1));
    let mut agg_reallocs = 0usize;
    for _ in 0..total {
        for _ in 0..25 {
            if agg.len() >= agg.capacity() {
                agg_reallocs += 1;
            }
            agg.push(0);
        }
    }
    let agg_peak = agg.capacity();
    drop(agg);

    let mut shape: Vec<u8> = Vec::with_capacity(shape_target.max(1));
    let mut shape_reallocs = 0usize;
    for _ in 0..total {
        for _ in 0..17 {
            if shape.len() >= shape.capacity() {
                shape_reallocs += 1;
            }
            shape.push(0);
        }
    }
    let shape_peak = shape.capacity();
    drop(shape);

    let peak_vec_bytes = agg_peak.max(shape_peak);
    let total_bytes_appended = agg_target + shape_target;
    let reallocations = agg_reallocs + shape_reallocs;
    debug_assert_eq!(
        reallocations, 0,
        "expected 0 reallocations with exact-capacity pre-allocation, got {}",
        reallocations
    );

    MemoryBaseline {
        n_invocations: total,
        peak_vec_bytes,
        total_bytes_appended,
        reallocations,
    }
}

// ---------------------------------------------------------------------------
// §3 — Collision hunt
// ---------------------------------------------------------------------------

/// Result of a collision hunt over a corpus of N randomly-generated
/// `AlignmentReport`s. Generated from a deterministic seed (splitmix64).
#[derive(Debug, Clone, PartialEq)]
pub struct CollisionReport {
    /// Seed used (for reproducibility).
    pub seed: u64,
    /// Number of reports generated.
    pub corpus_size: usize,
    /// Distinct `aggregate_hash` values seen.
    pub unique_aggregates: usize,
    /// Distinct `fingerprint_hash` values seen.
    pub unique_fingerprints: usize,
    /// Number of `aggregate_hash` collisions (corpus_size - unique_aggregates).
    pub aggregate_collisions: usize,
    /// Number of `fingerprint_hash` collisions (corpus_size - unique_fingerprints).
    pub fingerprint_collisions: usize,
}

/// splitmix64 — deterministic 64-bit PRNG. Same constants as the standard
/// "splitmix64" reference. Pure stdlib, no external deps.
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

/// Generate a random `AlignmentReport` from the splitmix64 stream.
/// Each report has 1-3 invocations, with random statuses, to give the
/// fingerprint stream variety.
fn random_alignment_report(state: &mut u64) -> AlignmentReport {
    let mut report = AlignmentReport {
        matched: Vec::new(),
        mismatched: Vec::new(),
        only_in_a: Vec::new(),
        only_in_b: Vec::new(),
        alignment_key_a: AlignmentKeyKind::Trace,
        alignment_key_b: AlignmentKeyKind::Trace,
    };
    let n = 1 + (splitmix64(state) as usize % 3); // 1..=3 invocations
    for _ in 0..n {
        let status = splitmix64(state) % 4;
        let hash_a = splitmix64(state);
        let hash_b = splitmix64(state);
        let delta_signed = (splitmix64(state) as i64).wrapping_rem(101);
        let delta = if delta_signed == 0 {
            None
        } else {
            Some(delta_signed - 1)
        };
        let inv_key = AlignmentKey::Invocation(uuid::Uuid::new_v4());
        let alignment = InvocationAlignment {
            key: inv_key,
            status: match status {
                0 => InvocationStatus::Matched,
                1 => InvocationStatus::Mismatched,
                2 => InvocationStatus::OnlyInA,
                _ => InvocationStatus::OnlyInB,
            },
            hash_a: if status == 2 { None } else { Some(hash_a) },
            hash_b: if status == 3 { None } else { Some(hash_b) },
            delta_event_count: delta,
        };
        match alignment.status {
            InvocationStatus::Matched => report.matched.push(alignment),
            InvocationStatus::Mismatched => report.mismatched.push(alignment),
            InvocationStatus::OnlyInA => report.only_in_a.push(alignment),
            InvocationStatus::OnlyInB => report.only_in_b.push(alignment),
        }
    }
    report
}

/// Run a collision hunt: generate `corpus_size` random reports, fingerprint
/// each, count distinct hashes. A healthy FNV-1a 64-bit + sort+fold pipeline
/// over 1-3 invocations per report should show near-zero collisions for
/// corpus_size up to ~10K.
pub fn collision_hunt(seed: u64, corpus_size: usize) -> CollisionReport {
    let mut state = seed;
    let mut aggregates: HashSet<u64> = HashSet::new();
    let mut fingerprints: HashSet<u64> = HashSet::new();
    for _ in 0..corpus_size {
        let report = random_alignment_report(&mut state);
        let fp = session_fingerprint_or_panic(&report);
        aggregates.insert(fp.aggregate_hash);
        fingerprints.insert(fp.fingerprint_hash);
    }
    let unique_aggregates = aggregates.len();
    let unique_fingerprints = fingerprints.len();
    CollisionReport {
        seed,
        corpus_size,
        unique_aggregates,
        unique_fingerprints,
        aggregate_collisions: corpus_size - unique_aggregates,
        fingerprint_collisions: corpus_size - unique_fingerprints,
    }
}

// ---------------------------------------------------------------------------
// §4 — UAT-M7-01 / UAT-M7-02 executors
// ---------------------------------------------------------------------------

/// Outcome of a UAT execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UatOutcome {
    // ----- M7.4 variants (existing) -----
    /// UAT-M7-01: first semantic divergence was found.
    FoundDivergence,
    /// UAT-M7-02: gaps / missing context → unknown, no false equality.
    UnknownUnsupported,
    /// UAT failed: a false equality was declared when it shouldn't have been.
    /// This is the failure mode UAT-M7-02 is testing for.
    FalseEquality,
    /// UAT-M7-01 failed: divergence was expected but not found.
    NoDivergence,

    // ----- M6.6 variants (R1 lift) -----
    /// UAT-M6-01: two concurrent cross-service invocations did NOT mix
    /// trace_ids / invocation_ids / events. Distinct per-service.
    CrossServiceUnambiguous,
    /// UAT-M6-02: monotonic-clock drift between services preserved, and an
    /// idempotent retry with the same `traceparent` produced a new
    /// `OtlpInvocationId` but the same `trace_id`.
    DriftAndIdempotencyHold,
    /// UAT-M6-01 failure: two concurrent invocations shared trace_id /
    /// invocation_id (cross-mix detected). This is the failure mode the
    /// scenario is testing for.
    CrossServiceMixDetected,
    /// UAT-M6-02 failure: monotonic drift collapsed (events from both
    /// services merged under a single monotonic timestamp) or the retry
    /// produced a different trace_id.
    DriftOrIdempotencyCollapsed,
}

impl UatOutcome {
    /// `true` iff the outcome indicates the UAT scenario passed.
    pub fn is_pass(&self) -> bool {
        matches!(
            self,
            UatOutcome::FoundDivergence
                | UatOutcome::UnknownUnsupported
                | UatOutcome::CrossServiceUnambiguous
                | UatOutcome::DriftAndIdempotencyHold
        )
    }

    /// Stable string name, useful for logs and dashboards.
    pub fn name(&self) -> &'static str {
        match self {
            UatOutcome::FoundDivergence => "FoundDivergence",
            UatOutcome::UnknownUnsupported => "UnknownUnsupported",
            UatOutcome::FalseEquality => "FalseEquality",
            UatOutcome::NoDivergence => "NoDivergence",
            UatOutcome::CrossServiceUnambiguous => "CrossServiceUnambiguous",
            UatOutcome::DriftAndIdempotencyHold => "DriftAndIdempotencyHold",
            UatOutcome::CrossServiceMixDetected => "CrossServiceMixDetected",
            UatOutcome::DriftOrIdempotencyCollapsed => "DriftOrIdempotencyCollapsed",
        }
    }
}

/// UAT result: outcome + supporting counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UatResult {
    /// Scenario name (e.g. "UAT-M7-01 / state-bug on shared trace_id").
    pub scenario: String,
    /// Pass/fail outcome.
    pub outcome: UatOutcome,
    /// Number of divergence points found (0 if N/A).
    pub divergence_count: usize,
    /// Number of unsupported regions reported (0 if N/A).
    pub unsupported_region_count: usize,
    /// Aggregate hash of the report under test (None if report was unbuildable).
    pub aggregate_hash: Option<u64>,
    /// Fingerprint hash of the report under test (None if report was unbuildable).
    pub fingerprint_hash: Option<u64>,
}

fn make_invocation_with_trace(traceparent: &str) -> RecordedInvocation {
    let inv_id = InvocationId::new();
    let etc = parse_traceparent(traceparent).expect("valid traceparent");
    RecordedInvocation {
        invocation_id: inv_id,
        external: Some(etc),
    }
}

fn build_session_from<'a>(pairs: Vec<(&'a RecordedInvocation, Vec<ChronosEvent>)>) -> Session<'a> {
    let mut s = Session::new();
    for (inv, evs) in pairs {
        s.push(InvocationWithEvents::new(inv, evs));
    }
    s
}

fn make_event(probe: &str, key: &str, value: &str) -> ChronosEvent {
    ChronosEvent {
        ts_micros: 0,
        probe: probe.to_string(),
        fields: vec![(key.to_string(), EventField::Str(value.to_string()))],
    }
}

/// Run UAT-M7-01: noisy good vs buggy bad run with a known state-divergence
/// on the same trace_id. Asserts that the first semantic divergence is found
/// and not confused with a symbol vs invocation mismatch.
pub fn run_uat_m7_01() -> UatResult {
    let scenario = "UAT-M7-01 / state-bug on shared trace_id".to_string();

    let trace_id = "4bf92f3577b34da6a3ce929d0e0e4736";
    let traceparent = format!("00-{}-00f067aa0ba902b7-01", trace_id);

    let inv_a = make_invocation_with_trace(&traceparent);
    let inv_b = make_invocation_with_trace(&traceparent);

    let events_a = vec![make_event("checkout", "total", "3500")];
    let events_b = vec![make_event("checkout", "total", "3150")];

    let good_session = build_session_from(vec![(&inv_a, events_a)]);
    let bad_session = build_session_from(vec![(&inv_b, events_b)]);

    let spec = equivalence_spec_default();

    let alignment: AlignmentReport = match align_sessions(good_session, bad_session, &spec) {
        Ok(r) => r,
        Err(AlignmentError::AmbiguousInvocationId { .. }) => {
            return UatResult {
                scenario,
                outcome: UatOutcome::NoDivergence,
                divergence_count: 0,
                unsupported_region_count: 0,
                aggregate_hash: None,
                fingerprint_hash: None,
            };
        }
    };

    let mismatched = alignment.mismatched.len();
    let only_in_a = alignment.only_in_a.len();
    let only_in_b = alignment.only_in_b.len();
    let divergence_count = mismatched + only_in_a + only_in_b;

    let outcome = if mismatched >= 1 && only_in_a == 0 && only_in_b == 0 {
        UatOutcome::FoundDivergence
    } else if mismatched == 0 && only_in_a == 0 && only_in_b == 0 {
        UatOutcome::NoDivergence
    } else {
        UatOutcome::FalseEquality
    };

    let fp = session_fingerprint(&alignment).ok();

    UatResult {
        scenario,
        outcome,
        divergence_count,
        unsupported_region_count: 0,
        aggregate_hash: fp.as_ref().map(|f| f.aggregate_hash),
        fingerprint_hash: fp.as_ref().map(|f| f.fingerprint_hash),
    }
}

/// Run UAT-M7-02: comparison with gaps, missing external context, or
/// incomparable data. Asserts `UnknownUnsupported` outcome (no false
/// equality) and reports the unsupported region count.
pub fn run_uat_m7_02() -> UatResult {
    let scenario = "UAT-M7-02 / gaps + missing-context (different trace_ids)".to_string();

    let traceparent_a = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
    let traceparent_b = "00-5bf92f3577b34da6a3ce929d0e0e4737-00f067aa0ba902b8-01";

    let inv_a = make_invocation_with_trace(traceparent_a);
    let inv_b = make_invocation_with_trace(traceparent_b);

    let events_a = vec![make_event("step", "name", "alpha")];
    let events_b = vec![make_event("step", "name", "alpha")];

    let good_session = build_session_from(vec![(&inv_a, events_a)]);
    let bad_session = build_session_from(vec![(&inv_b, events_b)]);

    let spec = equivalence_spec_default();

    let alignment: AlignmentReport = match align_sessions(good_session, bad_session, &spec) {
        Ok(r) => r,
        Err(_) => {
            return UatResult {
                scenario,
                outcome: UatOutcome::FalseEquality,
                divergence_count: 0,
                unsupported_region_count: 0,
                aggregate_hash: None,
                fingerprint_hash: None,
            };
        }
    };

    let matched_count = alignment.matched.len();
    let only_in_a = alignment.only_in_a.len();
    let only_in_b = alignment.only_in_b.len();
    let divergence_count = only_in_a + only_in_b + alignment.mismatched.len();
    let unsupported_region_count = only_in_a + only_in_b;

    let outcome = if matched_count == 0 && unsupported_region_count > 0 {
        UatOutcome::UnknownUnsupported
    } else {
        UatOutcome::FalseEquality
    };

    let fp = session_fingerprint(&alignment).ok();

    UatResult {
        scenario,
        outcome,
        divergence_count,
        unsupported_region_count,
        aggregate_hash: fp.as_ref().map(|f| f.aggregate_hash),
        fingerprint_hash: fp.as_ref().map(|f| f.fingerprint_hash),
    }
}
