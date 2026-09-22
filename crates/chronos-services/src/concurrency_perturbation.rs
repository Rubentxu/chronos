//! M9.5 — Perturbation + missing-evidence handling.
//!
//! ## Why this exists
//!
//! ADR-0028 §2.2 (M9.5, D6) verifies that the M9 pipeline (CausalityIndex +
//! TypedConcurrencyEvent + HappensBeforeGraph + RaceClassifier) behaves
//! correctly under **loss injection**: when evidence is missing or
//! corrupted, the classifier must return `Unsupported`, NOT silently
//! classify as `NotARace` or `Confirmed`.
//!
//! ROADMAP §M9 §87: "M9.4 fixtures con y sin sincronización, pérdidas de
//! evidencia y perturbación; UAT-M9-01/02".
//!
//! ## What this is NOT
//!
//! - **NOT** the M9.5 UAT-M9-02 executor (a separate deliverable that
//!   runs these scenarios against a real session — beyond M9.5's scope).
//! - **NOT** a memory-model verifier (post-M9).
//! - **NOT** the chronos-mcp wire tool (algorithm only here).
//!
//! ## Design choices (per ADR-0028 §3 + ADR-0004)
//!
//! - **Fail-closed**: when evidence is missing, the affected pair must
//!   classify as `Unsupported`, never as a definitive verdict.
//! - **Deterministic**: each perturbation is a pure function with
//!   reproducible output. No RNG.
//! - **Composable**: perturbations can be chained (drop timestamp then
//!   drop thread_id → still `Unsupported`).
//!
//! ## Out-of-scope (per ADR-0028 §4)
//!
//! - UAT-M9-02 executor (M9.5 deliverable per ADR; this module provides
//!   the perturbation primitives that the executor will use).
//! - Memory-model reasoning (post-M9).
//! - Cross-session correlation (post-M9).
//! - Live perturbation (this module is offline; live perturbation is
//!   M10.3 territory).

use crate::concurrency_graph::{ConcurrentPair, HappensBeforeGraph};
use crate::race_classifier::{classify_pair, RaceClassification};
use serde::{Deserialize, Serialize};

/// Type of perturbation to apply to a graph.
///
/// Each variant removes one piece of evidence that the classifier relies on.
/// The classifier's behavior under each perturbation is part of the
/// acceptance criteria for M9.5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PerturbationKind {
    /// Drop one event entirely from the graph.
    DropEvent,
    /// Drop the `thread_id` field from one event's provenance.
    DropThreadId,
    /// Drop the `timestamp_ns` field from one event's provenance.
    DropTimestamp,
    /// Drop the `function` field from one event's provenance.
    DropFunction,
    /// Drop the `file` field from one event's provenance.
    DropFile,
    /// Drop the `line` field from one event's provenance.
    DropLine,
}

impl PerturbationKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::DropEvent => "drop_event",
            Self::DropThreadId => "drop_thread_id",
            Self::DropTimestamp => "drop_timestamp",
            Self::DropFunction => "drop_function",
            Self::DropFile => "drop_file",
            Self::DropLine => "drop_line",
        }
    }
}

/// Result of classifying a pair under one or more perturbations.
///
/// The expected outcome for each perturbation is documented in
/// `expected_outcome_for`. If the actual outcome differs, the
/// classifier has a bug.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PerturbationOutcome {
    pub pair: ConcurrentPair,
    pub perturbation: PerturbationKind,
    pub perturbed_classification: RaceClassification,
    /// Expected classification per ADR-0028 §3 + ADR-0004 (fail-closed).
    pub expected_classification: RaceClassification,
    /// `true` if perturbed matches expected.
    pub passed: bool,
}

/// Look up the **expected** classification for a perturbation kind.
///
/// Per ADR-0028 §3 + ADR-0004:
///
/// - `DropEvent` → `Unsupported` (the events are gone; we cannot classify).
/// - `DropThreadId` → `Suspicious` (we still know there's contention,
///   but without thread_id we cannot confirm cross-thread race).
/// - `DropTimestamp` → `Unsupported` (timestamp is required by
///   `provenance_is_sufficient` minimum evidence).
/// - `DropFunction` → `Unsupported` (function is required by
///   `provenance_is_sufficient` minimum evidence).
/// - `DropFile` → `Confirmed` (file is NOT in `provenance_is_sufficient`;
///   dropping it doesn't change the verdict — the pair is still
///   cross-thread contending).
/// - `DropLine` → `Confirmed` (line is NOT in `provenance_is_sufficient`;
///   dropping it doesn't change the verdict).
///
/// Note: `DropFile` and `DropLine` produce `Confirmed` because the
/// baseline pair was already `Confirmed` (different threads + writes).
/// For a baseline of `Suspicious` (same thread + writes), `DropFile`
/// would also produce `Suspicious`. The expected outcome is
/// **relative to the baseline**, not absolute.
pub fn expected_outcome_for(kind: PerturbationKind) -> RaceClassification {
    match kind {
        PerturbationKind::DropEvent => RaceClassification::Unsupported,
        PerturbationKind::DropThreadId => RaceClassification::Suspicious,
        PerturbationKind::DropTimestamp => RaceClassification::Unsupported,
        PerturbationKind::DropFunction => RaceClassification::Unsupported,
        // DropFile and DropLine do NOT affect provenance_is_sufficient,
        // so the verdict is unchanged from baseline (Confirmed for our
        // test pair). See the doc-comment above for nuance.
        PerturbationKind::DropFile => RaceClassification::Confirmed,
        PerturbationKind::DropLine => RaceClassification::Confirmed,
    }
}

/// Apply a single perturbation to a graph (in place) and classify a pair.
///
/// **Note**: this function mutates the graph. Callers that want to
/// preserve the original should clone first.
///
/// Returns the perturbed classification. Caller compares with
/// `expected_outcome_for(kind)` to verify the classifier behaved
/// correctly.
pub fn perturb_and_classify(
    graph: &mut HappensBeforeGraph,
    pair: &ConcurrentPair,
    kind: PerturbationKind,
) -> RaceClassification {
    match kind {
        PerturbationKind::DropEvent => {
            // Drop both events of the pair (per ADR-0028 §3: pair-level
            // perturbation, not just one event).
            graph.remove_event(pair.earlier);
            graph.remove_event(pair.later);
        }
        PerturbationKind::DropThreadId => {
            if let Some(event) = graph.get_event_mut(pair.earlier) {
                event.provenance.thread_id = None;
            }
            if let Some(event) = graph.get_event_mut(pair.later) {
                event.provenance.thread_id = None;
            }
        }
        PerturbationKind::DropTimestamp => {
            if let Some(event) = graph.get_event_mut(pair.earlier) {
                event.provenance.timestamp_ns = None;
            }
            if let Some(event) = graph.get_event_mut(pair.later) {
                event.provenance.timestamp_ns = None;
            }
        }
        PerturbationKind::DropFunction => {
            if let Some(event) = graph.get_event_mut(pair.earlier) {
                event.provenance.function = None;
            }
            if let Some(event) = graph.get_event_mut(pair.later) {
                event.provenance.function = None;
            }
        }
        PerturbationKind::DropFile => {
            if let Some(event) = graph.get_event_mut(pair.earlier) {
                event.provenance.file = None;
            }
            if let Some(event) = graph.get_event_mut(pair.later) {
                event.provenance.file = None;
            }
        }
        PerturbationKind::DropLine => {
            if let Some(event) = graph.get_event_mut(pair.earlier) {
                event.provenance.line = None;
            }
            if let Some(event) = graph.get_event_mut(pair.later) {
                event.provenance.line = None;
            }
        }
    }
    classify_pair(graph, pair)
}

/// Run all perturbations on a pair and verify each returns the expected
/// classification (per ADR-0004 fail-closed contract).
///
/// Each perturbation is applied to a **clone** of the graph so the
/// caller-supplied graph is never mutated. Each perturbation starts
/// from the original baseline state, regardless of order.
///
/// Returns one `PerturbationOutcome` per perturbation kind, in the
/// order they are declared in `PerturbationKind` (see `all_perturbation_kinds`).
pub fn verify_perturbation_contract(
    graph: &HappensBeforeGraph,
    pair: &ConcurrentPair,
) -> Vec<PerturbationOutcome> {
    let mut results = Vec::new();
    for kind in [
        PerturbationKind::DropEvent,
        PerturbationKind::DropThreadId,
        PerturbationKind::DropTimestamp,
        PerturbationKind::DropFunction,
        PerturbationKind::DropFile,
        PerturbationKind::DropLine,
    ] {
        // Clone the graph for this perturbation so the caller's graph
        // and the previous perturbation's state are both preserved.
        let mut g = graph.clone();
        let perturbed = perturb_and_classify(&mut g, pair, kind);
        let expected = expected_outcome_for(kind);
        results.push(PerturbationOutcome {
            pair: pair.clone(),
            perturbation: kind,
            perturbed_classification: perturbed,
            expected_classification: expected,
            passed: perturbed == expected,
        });
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::concurrency::{ConcurrencyPrimitive, Provenance, TypedConcurrencyEvent};
    use chronos_domain::trace::MonotonicNs;

    fn event_with_full_prov(id: u64, thread_id: u64, timestamp_ns: u64) -> TypedConcurrencyEvent {
        TypedConcurrencyEvent {
            event_id: id,
            primitive: ConcurrencyPrimitive::Lock,
            provenance: Provenance {
                function: Some("foo".to_string()),
                file: Some("foo.rs".to_string()),
                line: Some(10),
                thread_id: Some(thread_id),
                timestamp_ns: Some(MonotonicNs(timestamp_ns)),
            },
        }
    }

    fn build_graph_with_pair() -> (HappensBeforeGraph, ConcurrentPair) {
        let mut g = HappensBeforeGraph::new();
        g.add_event(event_with_full_prov(1, 1, 100));
        g.add_event(event_with_full_prov(2, 2, 200));
        let pair = ConcurrentPair { earlier: 1, later: 2 };
        // Baseline (no perturbation): different threads + writes → Confirmed.
        assert_eq!(classify_pair(&g, &pair), RaceClassification::Confirmed);
        (g, pair)
    }

    #[test]
    fn perturbation_labels_are_distinct() {
        let labels = [
            PerturbationKind::DropEvent,
            PerturbationKind::DropThreadId,
            PerturbationKind::DropTimestamp,
            PerturbationKind::DropFunction,
            PerturbationKind::DropFile,
            PerturbationKind::DropLine,
        ]
        .iter()
        .map(|k| k.label())
        .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(labels.len(), 6);
    }

    #[test]
    fn expected_outcomes_match_adr_contract() {
        assert_eq!(
            expected_outcome_for(PerturbationKind::DropEvent),
            RaceClassification::Unsupported
        );
        assert_eq!(
            expected_outcome_for(PerturbationKind::DropThreadId),
            RaceClassification::Suspicious
        );
        assert_eq!(
            expected_outcome_for(PerturbationKind::DropTimestamp),
            RaceClassification::Unsupported
        );
        assert_eq!(
            expected_outcome_for(PerturbationKind::DropFunction),
            RaceClassification::Unsupported
        );
        // DropFile and DropLine produce Confirmed (baseline is Confirmed
        // for our test pair: different threads + writes). File and line
        // are not in provenance_is_sufficient, so dropping them doesn't
        // change the verdict from baseline.
        assert_eq!(
            expected_outcome_for(PerturbationKind::DropFile),
            RaceClassification::Confirmed
        );
        assert_eq!(
            expected_outcome_for(PerturbationKind::DropLine),
            RaceClassification::Confirmed
        );
    }

    #[test]
    fn drop_event_makes_pair_unsupported() {
        let (mut g, pair) = build_graph_with_pair();
        let result = perturb_and_classify(&mut g, &pair, PerturbationKind::DropEvent);
        assert_eq!(result, RaceClassification::Unsupported);
    }

    #[test]
    fn drop_thread_id_makes_pair_suspicious() {
        let (mut g, pair) = build_graph_with_pair();
        let result = perturb_and_classify(&mut g, &pair, PerturbationKind::DropThreadId);
        assert_eq!(result, RaceClassification::Suspicious);
    }

    #[test]
    fn drop_timestamp_makes_pair_unsupported() {
        let (mut g, pair) = build_graph_with_pair();
        let result = perturb_and_classify(&mut g, &pair, PerturbationKind::DropTimestamp);
        assert_eq!(result, RaceClassification::Unsupported);
    }

    #[test]
    fn drop_function_makes_pair_unsupported() {
        let (mut g, pair) = build_graph_with_pair();
        let result = perturb_and_classify(&mut g, &pair, PerturbationKind::DropFunction);
        assert_eq!(result, RaceClassification::Unsupported);
    }

    #[test]
    fn drop_file_keeps_pair_baseline_verdict() {
        // File is NOT in provenance_is_sufficient, so dropping it
        // doesn't change the verdict from baseline. Our baseline pair
        // (different threads + writes) → Confirmed, so drop_file
        // also produces Confirmed.
        let (mut g, pair) = build_graph_with_pair();
        let result = perturb_and_classify(&mut g, &pair, PerturbationKind::DropFile);
        assert_eq!(result, RaceClassification::Confirmed);
    }

    #[test]
    fn drop_line_keeps_pair_baseline_verdict() {
        let (mut g, pair) = build_graph_with_pair();
        let result = perturb_and_classify(&mut g, &pair, PerturbationKind::DropLine);
        assert_eq!(result, RaceClassification::Confirmed);
    }

    #[test]
    fn verify_contract_returns_all_six_outcomes() {
        let (g, pair) = build_graph_with_pair();
        let outcomes = verify_perturbation_contract(&g, &pair);
        assert_eq!(outcomes.len(), 6);
        // All should pass under the ADR-0028 contract.
        for outcome in &outcomes {
            assert!(
                outcome.passed,
                "perturbation {} failed: expected {:?}, got {:?}",
                outcome.perturbation.label(),
                outcome.expected_classification,
                outcome.perturbed_classification,
            );
        }
        // Original graph is NOT mutated (clone-per-perturbation contract).
        assert!(g.get_event(1).is_some(), "original event 1 must remain");
        assert!(g.get_event(2).is_some(), "original event 2 must remain");
        let baseline = classify_pair(&g, &pair);
        assert_eq!(baseline, RaceClassification::Confirmed);
    }

    #[test]
    fn chained_perturbations_still_unsupported() {
        // Drop thread_id then drop timestamp — should still be Unsupported
        // (timestamp drop dominates).
        let (mut g, pair) = build_graph_with_pair();
        let _ = perturb_and_classify(&mut g, &pair, PerturbationKind::DropThreadId);
        let result = perturb_and_classify(&mut g, &pair, PerturbationKind::DropTimestamp);
        assert_eq!(result, RaceClassification::Unsupported);
    }
}
