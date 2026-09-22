//! Integration tests for the `concurrency_wire` adapter (R3.1 CONC-001 wiring).
//!
//! These tests prove that the chronos-mcp `concurrency_wire` module
//! is a **real consumer** of `chronos_services::race_classifier`:
//!
//! 1. The adapter composes the algorithm (does NOT re-implement it).
//! 2. The wire shape is stable across `Serialize` + `Deserialize`
//!    round-trips (so an MCP client can build a request and parse a
//!    response without losing data).
//! 3. The adapter reproduces the canonical classification results
//!    from `race_classifier`'s own unit tests on identical input.
//!    (Composition fidelity: if we hand the adapter the same events,
//!    edges, and pair as `race_classifier`'s tests do, we get the same
//!    `RaceClassification` + reasons.)
//!
//! ## What this is NOT
//!
//! - **NOT** a test of the algorithm. The algorithm is tested in
//!   `chronos-services::race_classifier` (the source module). This
//!   file only verifies the wire layer composes it correctly.
//! - **NOT** an MCP `#[tool]` integration test. Tool registration
//!   requires a running `ChronosServer`; that path is deferred to
//!   a future cycle that also adds Context plumbing.
//!
//! ## CONC-001 wiring rationale
//!
//! Before R3, `race_classifier` was only re-exported from
//! `chronos-services::lib.rs` — nothing in `chronos-mcp` invoked
//! `classify_pair` or `explain_classification`. This file is the
//! first chronos-mcp test that proves the algorithm is reached
//! from the wire adapter, closing the ledger's CONC-001 gap from
//! `partial` → `verified`.

use chronos_domain::concurrency::{ConcurrencyPrimitive, Provenance, TypedConcurrencyEvent};
use chronos_domain::trace::MonotonicNs;
use chronos_mcp::concurrency_wire::{
    build_graph_from_wire, classify_pair_from_json, classify_pair_via_wire, wire_version,
    GraphEdgeWire, RaceClassificationWire,
};
use chronos_services::concurrency_graph::{ConcurrentPair, EdgeKind, HappensBeforeGraph};
use chronos_services::race_classifier::{
    classify_pair, explain_classification, ClassificationExplanation, ClassificationReason,
    RaceClassification,
};
use serde_json::{json, Value};

// -------------------------------------------------------------------------
// Canonical fixture builder (mirrors `race_classifier::tests::event_with_prov`)
// -------------------------------------------------------------------------

fn ev(id: u64, thread_id: u64, ts_ns: u64) -> TypedConcurrencyEvent {
    TypedConcurrencyEvent {
        event_id: id,
        primitive: ConcurrencyPrimitive::Lock,
        provenance: Provenance {
            function: Some("foo".to_string()),
            file: Some("foo.rs".to_string()),
            line: Some(10),
            thread_id: Some(thread_id),
            timestamp_ns: Some(MonotonicNs(ts_ns)),
        },
    }
}

fn empty_prov_event(id: u64) -> TypedConcurrencyEvent {
    TypedConcurrencyEvent::new(id, ConcurrencyPrimitive::Unknown, Provenance::empty())
}

fn pair(earlier: u64, later: u64) -> ConcurrentPair {
    ConcurrentPair { earlier, later }
}

// =========================================================================
// Section 1 — wire_version() and module surface
// =========================================================================

#[test]
fn wire_version_is_v1() {
    // Pinned contract: any future schema bump must be detectable.
    assert_eq!(wire_version(), "v1");
}

#[test]
fn graph_edge_wire_serializes_snake_case() {
    let edge = GraphEdgeWire {
        earlier: 1,
        later: 2,
        kind: EdgeKind::LockReleaseAcquire,
    };
    let json = serde_json::to_value(&edge).expect("serialize");
    assert_eq!(
        json,
        json!({
            "earlier": 1,
            "later": 2,
            "kind": "lock_release_acquire"
        })
    );
}

#[test]
fn race_classification_wire_envelope_carries_version() {
    let events = vec![ev(1, 1, 100), ev(2, 2, 200)];
    let edges: Vec<GraphEdgeWire> = vec![];
    let result: RaceClassificationWire = classify_pair_via_wire(&events, &edges, &pair(1, 2));
    assert_eq!(result.wire_version, "v1");
    assert_eq!(result.classification, RaceClassification::Confirmed);
    assert!(!result.explanation.reasons.is_empty());
}

// =========================================================================
// Section 2 — Composition fidelity (adapter == algorithm on identical input)
// =========================================================================

#[test]
fn adapter_reproduces_missing_event_unsupported() {
    // Mirrors `missing_event_in_graph_returns_unsupported` in race_classifier.
    let events: Vec<TypedConcurrencyEvent> = vec![];
    let edges: Vec<GraphEdgeWire> = vec![];
    let wire = classify_pair_via_wire(&events, &edges, &pair(1, 2));
    assert_eq!(wire.classification, RaceClassification::Unsupported);
    assert!(wire
        .explanation
        .reasons
        .contains(&ClassificationReason::InsufficientProvenance));
}

#[test]
fn adapter_reproduces_empty_provenance_unsupported() {
    // Mirrors `empty_provenance_returns_unsupported`.
    let events = vec![empty_prov_event(1), empty_prov_event(2)];
    let edges: Vec<GraphEdgeWire> = vec![];
    let wire = classify_pair_via_wire(&events, &edges, &pair(1, 2));
    assert_eq!(wire.classification, RaceClassification::Unsupported);
    assert!(wire
        .explanation
        .reasons
        .contains(&ClassificationReason::InsufficientProvenance));
}

#[test]
fn adapter_reproduces_cross_thread_confirmed() {
    // Mirrors `different_threads_same_address_returns_confirmed`.
    let events = vec![ev(1, 1, 100), ev(2, 2, 200)];
    let edges: Vec<GraphEdgeWire> = vec![];
    let wire = classify_pair_via_wire(&events, &edges, &pair(1, 2));
    assert_eq!(wire.classification, RaceClassification::Confirmed);
    assert!(wire.explanation.summary.contains("confirmed"));
}

#[test]
fn adapter_reproduces_same_thread_suspicious() {
    // Mirrors `same_thread_different_timestamps_returns_suspicious`.
    let events = vec![ev(1, 1, 100), ev(2, 1, 200)];
    let edges: Vec<GraphEdgeWire> = vec![];
    let wire = classify_pair_via_wire(&events, &edges, &pair(1, 2));
    assert_eq!(wire.classification, RaceClassification::Suspicious);
}

#[test]
fn adapter_explanation_matches_direct_classify_pair_call() {
    // Fidelity: the wire adapter's `explanation.classification` and
    // `explanation.reasons` must equal what `race_classifier::classify_pair`
    // + `explain_classification` produce directly on the same graph.
    let events = vec![ev(1, 7, 500), ev(2, 9, 600)];
    let edges: Vec<GraphEdgeWire> = vec![];
    let p = pair(1, 2);

    // Direct (no wire layer).
    let direct_g = {
        let mut g = HappensBeforeGraph::new();
        for e in &events {
            g.add_event(e.clone());
        }
        g
    };
    let direct_classification = classify_pair(&direct_g, &p);
    let direct_explanation: ClassificationExplanation = explain_classification(&direct_g, &p);

    // Via wire layer.
    let wire = classify_pair_via_wire(&events, &edges, &p);

    assert_eq!(wire.classification, direct_classification);
    assert_eq!(
        wire.explanation.classification,
        direct_explanation.classification
    );
    assert_eq!(wire.explanation.reasons, direct_explanation.reasons);
    assert_eq!(wire.explanation.pair, direct_explanation.pair);
}

// =========================================================================
// Section 3 — Graph construction from wire
// =========================================================================

#[test]
fn build_graph_from_wire_adds_events_and_edges() {
    let events = vec![ev(1, 1, 100), ev(2, 2, 200), ev(3, 1, 300)];
    let edges = vec![
        GraphEdgeWire {
            earlier: 1,
            later: 2,
            kind: EdgeKind::LockReleaseAcquire,
        },
        GraphEdgeWire {
            earlier: 2,
            later: 3,
            kind: EdgeKind::AtomicStoreLoad,
        },
    ];
    let graph = build_graph_from_wire(&events, &edges);
    assert_eq!(graph.event_count(), 3);
    assert_eq!(graph.edge_count(), 2);
    assert!(graph.get_event(1).is_some());
    assert!(graph.get_event(3).is_some());
}

#[test]
fn build_graph_from_wire_with_no_edges_yields_no_relationships() {
    let events = vec![ev(1, 1, 100), ev(2, 2, 200)];
    let edges: Vec<GraphEdgeWire> = vec![];
    let graph = build_graph_from_wire(&events, &edges);
    assert_eq!(graph.event_count(), 2);
    assert_eq!(graph.edge_count(), 0);
}

// =========================================================================
// Section 4 — JSON convenience wrapper
// =========================================================================

#[test]
fn classify_pair_from_json_round_trip_cross_thread_confirmed() {
    let request = json!({
        "events": [
            {
                "event_id": 1,
                "primitive": "lock",
                "provenance": {
                    "function": "foo",
                    "file": "foo.rs",
                    "line": 10,
                    "thread_id": 1,
                    "timestamp_ns": 100
                }
            },
            {
                "event_id": 2,
                "primitive": "lock",
                "provenance": {
                    "function": "bar",
                    "file": "bar.rs",
                    "line": 20,
                    "thread_id": 2,
                    "timestamp_ns": 200
                }
            }
        ],
        "edges": [],
        "pair": { "earlier": 1, "later": 2 }
    });

    let response: Value =
        classify_pair_from_json(request).expect("valid wire shape must parse + classify");
    assert_eq!(response["wire_version"], "v1");
    assert_eq!(response["classification"], "confirmed");
    assert!(response["explanation"]["summary"]
        .as_str()
        .expect("summary is string")
        .contains("confirmed"));
}

#[test]
fn classify_pair_from_json_missing_pair_field_returns_error() {
    // Defensive contract: malformed wire shape → Err(String), never panic.
    let request = json!({
        "events": [],
        "edges": []
    });
    let result = classify_pair_from_json(request);
    assert!(result.is_err(), "missing `pair` field must surface as Err");
    let err = result.expect_err("err");
    assert!(
        err.contains("missing field") || err.contains("wire shape parse error"),
        "error message must explain parse failure, got: {err}"
    );
}

#[test]
fn classify_pair_from_json_invalid_edge_kind_returns_error() {
    // EdgeKind is a closed enum; an unknown discriminator must be rejected.
    let request = json!({
        "events": [],
        "edges": [
            { "earlier": 1, "later": 2, "kind": "not_a_real_kind" }
        ],
        "pair": { "earlier": 1, "later": 2 }
    });
    let result = classify_pair_from_json(request);
    assert!(result.is_err(), "unknown EdgeKind must surface as Err");
}

// =========================================================================
// Section 5 — Module surface stability (structural pin)
// =========================================================================

#[test]
fn lib_rs_exposes_concurrency_wire_symbols() {
    // Structural check: the symbols re-exported in lib.rs are reachable
    // through the crate root, not just through the `concurrency_wire`
    // module path. Guards against accidental re-export removal.
    //
    // We assert by referencing each item at the crate root in a way
    // that requires both `use` resolution AND type/value visibility:
    //   - functions are referenced via the function-pointer turbofish
    //   - types are referenced via `_` annotation that requires the
    //     name to resolve
    use chronos_mcp::{
        build_graph_from_wire, classify_pair_from_json, classify_pair_via_wire,
        concurrency_wire_version, GraphEdgeWire, RaceClassificationWire,
    };

    // Function-pointer references (require items to be callable).
    let _: fn(&[TypedConcurrencyEvent], &[GraphEdgeWire]) -> HappensBeforeGraph =
        build_graph_from_wire;
    let _: fn(
        &[TypedConcurrencyEvent],
        &[GraphEdgeWire],
        &ConcurrentPair,
    ) -> RaceClassificationWire = classify_pair_via_wire;
    let _: fn(serde_json::Value) -> Result<serde_json::Value, String> = classify_pair_from_json;
    let _: fn() -> &'static str = concurrency_wire_version;

    // Type references (require types to resolve at the crate root).
    let _: Option<GraphEdgeWire> = None;
    let _: Option<RaceClassificationWire> = None;
}
