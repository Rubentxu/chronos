//! `telemetry_wire` — composition-root binding for the `TelemetryReceiver`
//! port.
//!
//! ## What this is
//!
//! A thin composition-root seam for the `TelemetryReceiver` port declared in
//! `chronos_domain::ports::telemetry`. The canonical port contract (see
//! `crates/chronos-domain/src/ports/telemetry.rs`) defines two impls:
//!
//! - `NoopTelemetry` — always accepts and discards (AD-5; production default;
//!   the port is fire-and-forget by contract).
//! - `InMemoryTelemetry` — per-session counts in a `HashMap<String, Counters>`
//!   guarded by a `Mutex`. Used by tests and degraded mode.
//!
//! This module **does not re-implement** either of those. It exposes them
//! behind two factory functions that mirror the composition-root pattern
//! already established in `crates/chronos-mcp/src/composition.rs`
//! (`default_session_archive` / `in_memory_session_archive`,
//! `default_counterexample_repository` / `in_memory_counterexample_repository`).
//!
//! ## Wire surface
//!
//! - `default_telemetry_receiver()` — production default. Returns
//!   `Arc<dyn TelemetryReceiver>` wrapping `NoopTelemetry`. Telemetry is
//!   fire-and-forget by contract; the Noop impl is the canonical choice for
//!   "telemetry disabled" mode (per `telemetry.rs` AD-5).
//! - `in_memory_telemetry()` — returns `Arc<dyn TelemetryReceiver>` wrapping
//!   `InMemoryTelemetry`. For tests and degraded mode (e.g. when the
//!   operator wants per-session captured/dropped/snapshots_published counts
//!   without standing up Prometheus/OTLP).
//!
//! ## What this is NOT
//!
//! - **Not a `#[tool]` wrapper.** Adding an MCP `#[tool]` handler that exposes
//!   telemetry over the wire shape would require plumbing
//!   `rmcp::handler::server::tool::ToolCallContext` (or equivalent) through
//!   the dispatcher — that's deferred to a later cycle, mirroring R3.1 / R4.1
//!   honest-scope for `concurrency_wire` / `cost_memory_wire`.
//! - **Not wired into `ChronosServer`.** This is the composition-root seam
//!   only. `server.rs` does not hold a `TelemetryReceiver` field yet; that
//!   wiring happens in a follow-up once the consumer-side hot-path
//!   integration pattern lands in `chronos-services::observe` (out of R5.1
//!   scope per the audit-driven `M4B-001` honest definition).
//! - **Not an eBPF / USDT / XRay decision module.** Those runtime probe
//!   decision modules are out of scope for the bounded composition-root
//!   wiring; per the M4B-001 ledger entry they stay env-blocked (eBPF
//!   requires `CAP_BPF`/privileged kernel, USDT probes require Linux
//!   privileged host) and are deferred to a dedicated cycle.
//! - **Not a Prometheus/OTLP adapter.** Those belong in their own crates
//!   (mirroring the `chronos-webhook` and `chronos-browser` pattern for
//!   `NotificationSink` and `BrowserProbeFactory`). NoopTelemetry +
//!   InMemoryTelemetry are the canonical port impls; this module just
//!   surfaces them.
//!
//! ## Verification surface (R5.1)
//!
//! Tests live in `crates/chronos-mcp/tests/telemetry_receiver_wire.rs`. They
//! pin:
//!
//! 1. **Surface** — `default_telemetry_receiver_is_noop_accepts_and_drains_to_zero`;
//!    `in_memory_telemetry_records_counters_and_drains`.
//! 2. **Composition fidelity** — every `Metric` variant (`Captured`,
//!    `Dropped`, `SnapshotsPublished`) bumps the matching counter inside
//!    `InMemoryTelemetry`; `default_telemetry_receiver` drains to zero.
//! 3. **Error contract** — `InMemoryTelemetry::record` with empty
//!    `session_id` returns `TelemetryError::EmptySessionId`; after
//!    `close()`, `record` returns `TelemetryError::Closed`; `NoopTelemetry`
//!    never emits any error (fire-and-forget semantics).
//! 4. **Structural pin** — `lib_rs_exposes_telemetry_receiver_wire_symbols`
//!    asserts at compile time that `pub use telemetry_wire::{...}` re-exports
//!    are present in `lib.rs`.
//!
//! ## Honest scope of M4B-001 (post-R5.1)
//!
//! Closing the "no composition-root binding" gap is R5.1. The remaining
//! honest gaps for `M4B-001` to reach `verified` are documented in
//! `reconstruction-contracts.toml` and depend on future cycles (consumer
//! wiring at hot paths + decision module + privileged-environment work).
//! R5.1 promotes this entry from `partial` (composition-root binding missing)
//! to a state where the seam exists + is reachable + the canonical port
//! contract is pinned end-to-end through the wire surface, mirroring R3's
//! pattern for `CONC-001` and R4's pattern for `DIFF-001`.

use std::sync::Arc;

use chronos_domain::ports::{InMemoryTelemetry, NoopTelemetry, TelemetryReceiver};

/// Composition-root default for telemetry: `NoopTelemetry` (AD-5).
///
/// Production default. Telemetry is fire-and-forget by contract (see
/// `crates/chronos-domain/src/ports/telemetry.rs`); the Noop impl is the
/// canonical choice when telemetry is disabled or not yet wired into a
/// downstream exporter (Prometheus, OTLP — those belong in dedicated
/// crates).
///
/// Returns `Arc<dyn TelemetryReceiver>` so the caller can hold it behind
/// a trait object without caring about the concrete type — same shape as
/// `default_session_archive` / `default_counterexample_repository`.
pub fn default_telemetry_receiver() -> Arc<dyn TelemetryReceiver> {
    Arc::new(NoopTelemetry::new())
}

/// Composition-root in-memory telemetry for tests and degraded mode.
///
/// Mirrors `in_memory_session_archive` / `in_memory_counterexample_repository`:
/// when the operator wants per-session captured / dropped /
/// `snapshots_published` counts without standing up a downstream exporter,
/// the in-memory receiver buffers counts in a `HashMap<String, Counters>`.
///
/// Tests that need to assert telemetry was actually called use this
/// factory instead of mocking the trait — the test calls `record` /
/// `drain` and reads the resulting `Counters`.
pub fn in_memory_telemetry() -> Arc<dyn TelemetryReceiver> {
    Arc::new(InMemoryTelemetry::new())
}
