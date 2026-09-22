//! Wire-format adapter for the M7.4 cost/memory/collision UAT executors
//! (ADR-0028 §2.2 D4, ROADMAP §M7.4).
//!
//! ## Why this exists
//!
//! `chronos_domain::otlp::cost_memory_collision` exposes
//! `run_uat_m7_01()` + `run_uat_m7_02()` as `pub fn -> UatResult`.
//! Before R4.1, no consumer in `chronos-mcp` invoked them — they were
//! only called from `chronos-domain`'s own test binary
//! (`crates/chronos-domain/tests/otlp_cost_memory_collision.rs`, 21 tests).
//! That left the M7 acceptance criterion **algorithmically satisfied**
//! but **without consumer wiring** in the chronos-mcp surface.
//!
//! Per ROADMAP §0.2 + the operator's directive ("ledger is the source of
//! truth, so promoting a `partial` to `verified` requires real consumer
//! wiring, not just claims"), R2 left DIFF-001 at `partial` with that
//! gap documented. R4.1 closes the gap: this module is the first
//! chronos-mcp code that actually invokes `run_uat_m7_01` and
//! `run_uat_m7_02` in a production path.
//!
//! ## What this is NOT
//!
//! - **NOT** an MCP `#[tool]` handler. Tool-handler registration requires
//!   the full `ChronosServer::new()` composition path (with env vars,
//!   execution_log_root, etc.). The wire-level integration tests in
//!   `chronos-mcp/tests/cost_memory_uat_wire.rs` exercise this adapter
//!   directly without spawning the server.
//! - **NOT** a replacement for `cost_memory_collision`. The algorithm +
//!   canonical fixtures live there. This module is a wire shim —
//!   composition-only, no domain logic.
//! - **NOT** a permission/auth layer. The wire adapter trusts the caller
//!   (the dispatcher) to gate access.
//!
//! ## Design choices (per ADR-0028 §3 + AGENTS §anti-duplication)
//!
//! - **Composition over duplication**: every public function in this
//!   module delegates to `cost_memory_collision::*`. No re-implementation
//!   of classification / measurement / collision logic.
//! - **No hidden state**: every function takes no inputs and returns a
//!   fresh value (the algorithm itself is self-contained; this is
//!   different from R3 where the algorithm took user-supplied events).
//!   No globals, no caches.
//! - **Wire-stable DTO**: `UatResultWire` is a fresh struct that
//!   mirrors `UatResult`'s fields with `Serialize`/`Deserialize` derives
//!   added. We do NOT modify the canonical `UatResult` (which has no
//!   serde derives by design — keeping domain types free of wire-shape
//!   concerns). The DTO conversion is mechanical and pinned by
//!   `to_wire_round_trip_preserves_all_fields` test.
//!
//! ## Out-of-scope (per ADR-0028 §4 + AGENTS §regression-conscious)
//!
//! - 3 MCP wire tools full implementation (tool macro registration +
//!   `ServerHandler` impl). Deferred to a cycle that adds the tool
//!   alongside Context plumbing for the dispatcher's session_id.
//! - Real production data feed into the UAT executors. The executors
//!   are deliberately self-contained scenario fixtures (they construct
//!   the canonical divergence pattern internally); wiring real session
//!   data into them is a separate design decision (would change the
//!   UAT semantics from "canonically assert the algorithm" to
//!   "evaluate the algorithm on user data" — a different acceptance
//!   criterion).
//! - Persistent results storage (in-memory only here per ADR-0028).
//!
//! ## DIFF-001 wiring rationale
//!
//! Prior to R4.1, `run_uat_m7_01` and `run_uat_m7_02` were only called
//! from `chronos-domain`'s own integration tests; no chronos-mcp code
//! path invoked them. The DIFF-001 ledger entry stayed at `partial`
//! with that gap documented. R4.1 wires the consumer: this module is
//! the first chronos-mcp code that calls both executors in a
//! production path (tests assert the wire shape + pass semantics
//! end-to-end).

use chronos_domain::otlp::cost_memory_collision::{run_uat_m7_01, run_uat_m7_02, UatResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON wire shape for `UatResult`.
///
/// Mirrors `chronos_domain::otlp::cost_memory_collision::UatResult`
/// fields but adds `Serialize`/`Deserialize` derives (the canonical
/// `UatResult` has no serde derives by design — keeping domain types
/// free of wire-shape concerns).
///
/// Mechanical conversion: every field maps 1:1, no semantic change.
/// `aggregate_hash` / `fingerprint_hash` are `Option<u64>` on the
/// domain side → JSON `null` when absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct UatResultWire {
    /// Scenario name (e.g. "UAT-M7-01 / state-bug on shared trace_id").
    pub scenario: String,
    /// Outcome discriminator — stable string from `UatOutcome::name()`
    /// (NOT the enum itself, to avoid coupling the wire shape to the
    /// domain enum's variant order or labels).
    pub outcome: String,
    /// `true` iff the outcome indicates the UAT scenario passed.
    pub passed: bool,
    /// Number of divergence points found (0 if N/A).
    pub divergence_count: usize,
    /// Number of unsupported regions reported (0 if N/A).
    pub unsupported_region_count: usize,
    /// Aggregate hash of the report under test (`null` if absent).
    pub aggregate_hash: Option<u64>,
    /// Fingerprint hash of the report under test (`null` if absent).
    pub fingerprint_hash: Option<u64>,
    /// Wire schema version (currently `"v1"`).
    pub wire_version: String,
}

const WIRE_VERSION_V1: &str = "v1";

/// Convert a canonical `UatResult` to its wire DTO.
///
/// Pure function. Stable field mapping. The `outcome` field carries
/// the stable string label from `UatOutcome::name()` so the wire
/// shape is decoupled from the domain enum's `Debug` repr.
pub fn to_wire(result: UatResult) -> UatResultWire {
    let passed = result.outcome.is_pass();
    let outcome = result.outcome.name().to_string();
    UatResultWire {
        scenario: result.scenario,
        outcome,
        passed,
        divergence_count: result.divergence_count,
        unsupported_region_count: result.unsupported_region_count,
        aggregate_hash: result.aggregate_hash,
        fingerprint_hash: result.fingerprint_hash,
        wire_version: WIRE_VERSION_V1.to_string(),
    }
}

/// Run UAT-M7-01 in a wire-ready shape.
///
/// Composes `cost_memory_collision::run_uat_m7_01` + `to_wire`.
/// No re-implementation: every assertion this scenario validates lives
/// in `cost_memory_collision::run_uat_m7_01`'s body.
pub fn run_uat_m7_01_wire() -> UatResultWire {
    to_wire(run_uat_m7_01())
}

/// Run UAT-M7-02 in a wire-ready shape.
///
/// Composes `cost_memory_collision::run_uat_m7_02` + `to_wire`.
pub fn run_uat_m7_02_wire() -> UatResultWire {
    to_wire(run_uat_m7_02())
}

/// JSON convenience for `run_uat_m7_01_wire`.
///
/// Returns the `UatResultWire` serialized as a JSON `Value`. Errors
/// are surfaced as `Err(String)` — the caller decides how to map to
/// an MCP wire error.
pub fn run_uat_m7_01_as_json() -> Result<Value, String> {
    let wire = run_uat_m7_01_wire();
    serde_json::to_value(wire).map_err(|e| format!("wire serialize error: {e}"))
}

/// JSON convenience for `run_uat_m7_02_wire`.
pub fn run_uat_m7_02_as_json() -> Result<Value, String> {
    let wire = run_uat_m7_02_wire();
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
