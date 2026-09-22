//! Wire-format adapter for the M9.4 race classifier (ADR-0028 §2.2 D3+D5).
//!
//! ## Why this exists
//!
//! `chronos-services::race_classifier` exposes the algorithm
//! (`classify_pair` + `explain_classification`) but the module's own
//! doc states (race_classifier.rs §"What this is NOT"):
//!
//! > NOT the 3 MCP wire tools (only the core `classify_pair` + the
//! > explainer DTO are here; full MCP wire-up is chronos-mcp's job).
//!
//! This module is the **thin chronos-mcp layer** that:
//!
//! 1. Accepts wire-format `TypedConcurrencyEvent`s + `EdgeKind`s from JSON
//!    (the on-the-wire shape that an MCP client would send).
//! 2. Builds a `HappensBeforeGraph` from them.
//! 3. Invokes `explain_classification` to produce a
//!    `ClassificationExplanation` (the diagnostic trace).
//! 4. Returns the result in a wire-stable envelope (JSON-ready).
//!
//! ## What this is NOT
//!
//! - **NOT** an MCP `#[tool]` handler. Tool-handler registration requires
//!   the full `ChronosServer::new()` composition path (with env vars,
//!   execution_log_root, etc.). The wire-level integration tests in
//!   `chronos-mcp/tests/concurrency_race_classifier_wire.rs` exercise this
//!   adapter directly without spawning the server.
//! - **NOT** a replacement for `chronos-services::race_classifier`. The
//!   algorithm + canonical fixtures live there. This module is a wire
//!   shim — composition-only, no domain logic.
//! - **NOT** a permission/auth layer. The wire adapter trusts the caller
//!   (the dispatcher) to gate access.
//!
//! ## Design choices
//!
//! - **Composition over duplication**: every public function in this
//!   module delegates to `race_classifier::*`. No re-implementation of
//!   classification rules.
//! - **No hidden state**: every function takes the graph (or builds it
//!   from the input) and returns a fresh value. No globals, no caches.
//! - **Deterministic**: same inputs → same outputs. The JSON shape is
//!   pinned by tests; the algorithm is pinned by `race_classifier`'s
//!   own tests.
//!
//! ## Out-of-scope (per ADR-0028 §4)
//!
//! - 3 MCP wire tools full implementation (tool macro registration +
//!   `ServerHandler` impl). Deferred to a cycle that adds the tool
//!   alongside Context plumbing for the dispatcher's session_id.
//! - Loss injection / perturbation (M9.5; lives in
//!   `chronos-services::concurrency_perturbation`).
//! - Memory-model reasoning (post-M9).
//! - Cross-session correlation (post-M9).
//!
//! ## CONC-001 wiring rationale
//!
//! Prior to R3, `race_classifier` was only re-exported from
//! `chronos-services::lib.rs` — no consumer in `chronos-mcp`,
//! `chronos-cli`, or `chronos-sandbox` invoked `classify_pair` or
//! `explain_classification`. The `UatOutcome`-style wiring tests for
//! UAT-M9-01/02 lived in the durable M9.4 spike
//! (`/home/rubentxu/m9-spikes/`) but were never lifted to product.
//!
//! R3 wires the consumer: this module is the first chronos-mcp code
//! that actually calls `race_classifier::explain_classification` in a
//! production path (tests assert the wire shape end-to-end).

use chronos_domain::concurrency::TypedConcurrencyEvent;
use chronos_services::concurrency_graph::{ConcurrentPair, EdgeKind, HappensBeforeGraph};
use chronos_services::race_classifier::{
    classify_pair, explain_classification, ClassificationExplanation,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON wire shape for `HappensBeforeGraph::add_edge` arguments.
///
/// `earlier` and `later` are `event_id`s (u64). `kind` is the
/// `EdgeKind` snake_case discriminator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GraphEdgeWire {
    pub earlier: u64,
    pub later: u64,
    pub kind: EdgeKind,
}

/// Envelope returned by `classify_pair_via_wire`.
///
/// Distinct from `ClassificationExplanation` only by adding a
/// `wire_version` field so future schema changes are detectable by
/// clients without breaking deserialization.
///
/// NOTE: Does NOT derive `Eq` because `ClassificationExplanation` is
/// only `PartialEq` (the underlying reason enum includes string
/// payloads). We compare with `PartialEq` only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RaceClassificationWire {
    /// Wire schema version (currently `"v1"`).
    pub wire_version: String,
    /// The classification result from `race_classifier::classify_pair`.
    pub classification: chronos_services::race_classifier::RaceClassification,
    /// The full explanation trace (reasons + summary).
    pub explanation: ClassificationExplanation,
}

const WIRE_VERSION_V1: &str = "v1";

/// Build a `HappensBeforeGraph` from a list of typed events and edges.
///
/// Caller is responsible for ensuring that every `earlier`/`later` in
/// `edges` refers to an event_id present in `events`. The function does
/// NOT validate cross-references — `HappensBeforeGraph::add_edge` will
/// silently drop edges for unknown event_ids (see concurrency_graph.rs
/// §"Edge insertion semantics"). Pre-validation is the caller's job.
pub fn build_graph_from_wire(
    events: &[TypedConcurrencyEvent],
    edges: &[GraphEdgeWire],
) -> HappensBeforeGraph {
    let mut g = HappensBeforeGraph::new();
    for ev in events {
        g.add_event(ev.clone());
    }
    for edge in edges {
        g.add_edge(edge.earlier, edge.later, edge.kind);
    }
    g
}

/// Classify a single concurrent pair from wire events + edges.
///
/// Composes:
/// - `build_graph_from_wire` (this module)
/// - `race_classifier::classify_pair` (chronos-services)
/// - `race_classifier::explain_classification` (chronos-services)
///
/// Returns the classification label AND the full explanation trace.
pub fn classify_pair_via_wire(
    events: &[TypedConcurrencyEvent],
    edges: &[GraphEdgeWire],
    pair: &ConcurrentPair,
) -> RaceClassificationWire {
    let graph = build_graph_from_wire(events, edges);
    let classification = classify_pair(&graph, pair);
    let explanation = explain_classification(&graph, pair);
    RaceClassificationWire {
        wire_version: WIRE_VERSION_V1.to_string(),
        classification,
        explanation,
    }
}

/// JSON convenience wrapper for `classify_pair_via_wire`.
///
/// Accepts a JSON object with the shape:
/// ```json
/// {
///   "events": [ <TypedConcurrencyEvent JSON>, ... ],
///   "edges":  [ {"earlier": <u64>, "later": <u64>, "kind": "<EdgeKind snake_case>"}, ... ],
///   "pair":   {"earlier": <u64>, "later": <u64>}
/// }
/// ```
///
/// Returns the `RaceClassificationWire` serialized as a JSON `Value`.
/// Errors are surfaced as `Err(String)` with the underlying
/// `serde_json` error message — the caller decides whether to surface
/// to the MCP client as a wire error or to log and recover.
pub fn classify_pair_from_json(input: Value) -> Result<Value, String> {
    #[derive(Deserialize)]
    #[serde(rename_all = "snake_case")]
    struct WireRequest {
        events: Vec<TypedConcurrencyEvent>,
        edges: Vec<GraphEdgeWire>,
        pair: ConcurrentPair,
    }

    let req: WireRequest =
        serde_json::from_value(input).map_err(|e| format!("wire shape parse error: {e}"))?;
    let wire = classify_pair_via_wire(&req.events, &req.edges, &req.pair);
    serde_json::to_value(wire).map_err(|e| format!("wire serialize error: {e}"))
}

/// Returns the canonical wire schema version (`"v1"`).
///
/// Exposed so callers (e.g. an MCP tool handler in a future cycle)
/// can declare the schema version in their handler description without
/// hard-coding the string in two places.
pub fn wire_version() -> &'static str {
    WIRE_VERSION_V1
}
