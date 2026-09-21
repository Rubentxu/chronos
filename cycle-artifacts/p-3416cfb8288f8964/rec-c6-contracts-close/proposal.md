# Proposal — REC-C6: Close unfinished M1–M4 reconstruction contracts

Path: A-lite. Baseline: origin/main @ e5dfe63b (REC-C5 closeout pushed).

## Intent

REC-C5 closed the canonical Agent API surface (wire 63 → 41 tools,
ALL_TOOL_NAMES synced with the `#[tool]` router). REC-C6 advances the
convergence sequence to the next active gate by closing as many of the
nine still-unverified contracts as the available evidence supports,
without manufacturing test coverage or pretending scope we don't have.

The three contracts owned by REC-C6 per `reconstruction-contracts.toml`:

- **PROP-001** (`verified`, M3 order-total) — already closed.
- **M4A-001** (`gap`) — Go adaptive instrumentation: OTel discovery,
  OBI, Auto SDK and otelc. Title only; no evidence, no UAT, no
  verify recipe. Requires a Go-side adapter that does not exist in
  this workspace (no Go crate, no Go capture path).
- **M4B-001** (`partial`) — Rust adaptive instrumentation: telemetry
  reuse, eBPF, XRay/USDT decisions and semantic probes. The
  `chronos_domain::ports::telemetry::TelemetryReceiver` port is
  declared (`NoopTelemetry`, `InMemoryTelemetry` impls present) but
  the integration with capture probes (`chronos_native`,
  `chronos_services::observe`, `probe_inject` path) is not wired.

## Evidence (from inline explore, session panda, 2026-09-21)

Workspace-wide grep for adaptive-instrumentation vocabulary on
2026-09-21 (post REC-C5 merge to main):

```
$ grep -rln "opentelemetry|USDot|USDotProvider|XRay|usdt|telemetry|\
   AutoInstrument|OBI|otelc" --include="*.rs" crates/
crates/chronos-domain/tests/common/mod.rs
crates/chronos-domain/tests/telemetry_ports.rs
crates/chronos-domain/src/ports/telemetry.rs
crates/chronos-domain/src/ports/mod.rs
```

Four files, all in `chronos-domain` (the port-side owner). Zero files
under `crates/chronos-services`, `crates/chronos-native`, or
`crates/chronos-mcp` even mention the vocabulary. The `TelemetryReceiver`
port is declared and tested in isolation but no adapter is wired at
the composition root (`chronos_mcp::composition`) and no call site in
the capture path (`chronos_native::native_probe_*`,
`chronos_services::observe`, `probe_inject`) consumes the port.

For M4A-001 (Go): no Go crate exists in the workspace. The
`crates/` tree is Rust-only. No `Cargo.toml` `[dependencies]` entry,
no `build.rs`, no Go-side adapter under any path. M4A-001 has no
substrate in this repo to land against.

Inherited debt (still open after REC-C3/REC-C4/REC-C5 close):

| id | owner_gate | status | what remains |
|---|---|---|---|
| HEX-002 | REC-C3 | gap | chronos-services still consumes concrete `SegmentedExecutionLog` from chronos_log (the ports exist; the binding does not). |
| HEX-003 | REC-C3 | gap | chronos-store transitively pulls the native tracer implementation (storage -> native dep via `chrono-store::store_open` failure path). |
| SOLID-001 | REC-C4 | partial | TraceAdapter split into CaptureLifecycle + DebugInspect (C4.1 landed); production consumers (factory, ebpf) still bind to the legacy TraceAdapter surface. |
| CONN-001 | REC-C4 | partial | MonotonicNs + WallClockMs newtypes landed (C4.2); ~12 wall-clock-ms boundary constructors in chronos-log still typed as raw u64. |
| CONN-002 | REC-C4 | partial | SubscriptionId newtype landed (C4.3); `probe_id` is still `String` at MCP / services boundaries. |
| API-001 | REC-C5 | partial | Canonical v2 agent API is now 41 tools and `ALL_TOOL_NAMES` is synced (REC-C5 closeout); remaining gap: AGENT_API_V2.md still lists 12 v2 tools and does not formally ratify the 29 not-yet-converged neighbours. |
| API-002 | REC-C5 | partial | MCP wrappers contain transport mapping only (REC-C5 closeout confirmed); remaining gap: a few wrappers still inline a 2-line adapter call beyond pure transport (audit by grep). |

## Slices

The slices below are ordered so each one leaves the repo in a
green state and advances the contracts ledger monotonically.

- **C6.1 M4 contracts reclassification** (doc + contracts.toml only).
  Flip M4A-001 status from `gap` to `blocked` and add notes that
  document the substrate absence (no Go crate in the workspace).
  Flip M4B-001 from `partial` to `blocked` and add notes that
  document the port declaration + missing composition-root binding.
  Honest deferral: these contracts cannot be closed without an
  architecture cycle that lands real instrumentation work. ~2 files.
  Tier: T0 (lint + contracts.toml schema valid). No Rust changes.

- **C6.2 HEX-002 closeout** — chronos-services consumes
  `ExecutionLogProvider` port (already declared in chronos-domain
  per REC-C3.1) instead of `SegmentedExecutionLog`. Concrete
  adapter wiring at the composition root. ~3-5 files
  (chronos-services/src/lib.rs, chronos-services/src/projection.rs,
  crates/chronos-mcp/src/composition.rs). Tier: T0 + T1 + T2 + T3.
  Smoke subset: `cargo test -p chronos-services --lib`,
  `cargo test -p chronos-mcp --lib`, plus `chronos-sandbox --test
  rec_c1_*` (the projection restart-equivalence tests).

- **C6.3 HEX-003 closeout** — chronos-store storage path
  decouples from the native tracer. The current direct dep comes
  through `chrono-store::store_open` failure-path code that
  imports from chronos-native. Replace with a `StorageOpenError`
  enum that does not pull the tracer implementation. ~2-3 files
  (chronos-store + Cargo.toml). Tier: T0 + T1 + T2 + T3.

- **C6.4 SOLID-001 closeout** — migrate the two remaining
  production consumers (`native_probe_factory`,
  `native_probe_controller`) to bind on the `CaptureLifecycle`
  capability trait instead of the legacy `TraceAdapter` surface.
  Trims the public surface of `chronos_native`. ~4-6 files. Tier:
  T0 + T1 + T2 + T3.

- **C6.5 CONN-001 closeout** — type the ~12 wall-clock-ms
  boundary constructors in `chronos-log` to `WallClockMs`. ~3
  files (`crates/chronos-log/src/segmented.rs`, possibly
  `crates/chronos-log/src/memory.rs`). Tier: T0 + T1 + T2 + T3.

- **C6.6 CONN-002 closeout** — introduce a `ProbeId` newtype in
  `chronos-domain::ports` and thread it through the MCP / services
  boundaries. Replaces `String` probe_id. ~4-5 files. Tier: T0 +
  T1 + T2 + T3.

- **C6.7 API-001/002 closeout** — update AGENT_API_V2.md to
  formally ratify the 41-tool surface (12 v2 + 29 not-yet-converged
  neighbours); audit wrapper files (`chronos-sandbox/src/client/
  tools.rs`) for the remaining 2-line adapter inline calls beyond
  transport mapping; remove any pure shim. ~2-3 files. Tier: T0 +
  T1 + T2 + T4-smoke (`cargo test -p chronos-sandbox --test
  e2e_connectivity --test analytics_tools`).

## Non-goals

- We do not propose closing M4A-001 / M4B-001 in this cycle. The
  workspace has no Go substrate and the Rust telemetry integration
  is not yet a composition-root binding. Those need a future cycle
  with real instrumentation work; we only reclassify and document
  honestly here.
- We do not propose re-doing REC-C5 work. Wire surface is 41 tools
  and locked; the regression test `chronos-mcp/tests/alias_deletion.rs`
  is green; `toolset_sync_check` is green.
- We do not propose a release tag for REC-C6. REC-C6 is a gate
  cycle (contracts closeout), not a shipped deliverable. The next
  `v0.x.y` tag lands with REC-C7 (convergence close).
- We do not touch `docs/propuestas/` (legacy, frozen by repo
  convention).
- We do not propose a `#[allow(...)]` cascade for any clippy noise
  touched by these slices; if clippy surfaces drift on a site we did
  not write, it goes in as a follow-up cycle commit, not an inline
  allow.
- We do not delete or rewrite a test to make it pass. If a slice
  breaks an existing test, the test gets re-baselined in
  `apply-checkpoint.json` notes.

## Pre-approved gates

All `human_gate`s in this cycle are pre-approved per the user's
standing instruction (auto-run mode, complete the full roadmap
without per-cycle approval). No deep-research callout is expected
unless a slice surfaces an unforeseen architectural blocker during
apply; in that case the orchestrator reports the blocker and the
slice is held.

## Risk register

| id | severity | mitigation |
|---|---|---|
| C6-R1 HEX-002 binding breaks the chronos-services -> chronos_log direct edge | high | Pre-flight grep before apply; if the binding is structural, abort the slice and reclassify as `planned` with a follow-up cycle commit. |
| C6-R2 chronos-store decoupling reveals a missing port for store-open failure reporting | medium | Add `StorageOpenError` enum + port in chronos-domain as part of the slice; do not invent a side-channel. |
| C6-R3 ProbeId newtype breaks existing sandbox tests that compare probe_id as String | high | Grep the sandbox tests first; if comparison is structural, update them in the same slice (recorded in apply-checkpoint). |
| C6-R4 WallClockMs boundary sites reveal a semantic mismatch (some sites are monotonic, not wall-clock) | medium | Re-classify the site to MonotonicNs if appropriate; record the reclassification in apply-checkpoint notes. |

## Sandbox smoke subset

- `cargo test -p chronos-sandbox --test e2e_connectivity`
- `cargo test -p chronos-sandbox --test analytics_tools`
- `cargo test -p chronos-sandbox --test rec_c1_*` (projection
  restart-equivalence family)

Rationale: each slice touches capture probes / projection / store
open; these three suites cover the integration boundaries that
REC-C6 can break without meaning to.
