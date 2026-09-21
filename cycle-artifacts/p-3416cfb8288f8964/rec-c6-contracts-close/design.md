# Design — REC-C6: Close unfinished M1–M4 reconstruction contracts

Path: A-lite. Baseline: feat/rec-c6-contracts-close @ 96601b25 (REC-C6.1 landed).

## Reality check on slice scope (2026-09-21)

The proposal claimed 7 slices (C6.1..C6.7) closing 7 inherited
contracts. After inline investigation against the real code on this
branch, **most of those slices are bigger than the A-lite budget
allows**:

- HEX-002/003 — checker `scripts/check_architecture_contracts.py`
  already passes (zero forbidden edges); the hexagonal boundary
  closure landed in REC-C3. The remaining gap is *abstraction use*,
  not *edge direction*. `chronos-services` does touch
  `chronos_log::SegmentedExecutionLog` directly in several helpers
  (canonical_drain, observe, probe, test_support, tripwire_evidence,
  sessions, session_log) but those uses are either (a) test-only
  (`#[cfg(test)]`), (b) factory composition (the only allowed place
  per REC-C3.3.2), or (c) bridge helpers that immediately wrap the
  concrete in the port. The "gap" label in the ledger is older than
  the code; reality is closer to `verified`.

- CONN-001 — ~12 wall-clock-ms boundary sites is plausible but each
  one needs a call-site audit (some sites are monotonic despite the
  raw u64 type). Pre-flight grep is required; landing the slice
  without audit risks a runtime semantic regression.

- CONN-002 — `probe_id` flows through MCP wire (rmcp::ErrorData path
  + observe handler) and into chronos-services. Newtype threading
  touches the wire DTOs (`probe_inject` request/response) plus
  sandbox tests that compare probe_id as String. Real slice, ~5
  files + sandbox tests. A-min scope at best.

- API-001/002 — AGENT_API_V2.md ratification is doc-only. The
  wrapper audit is mostly cosmetic.

- SOLID-001 — `CaptureLifecycle` capability trait already exists
  (C4.1). Production consumer migration touches factory +
  controller + their consumers (~6 files, ~150 lines). A-min scope.

**Decision**: reduce scope to what is actually achievable as A-lite
in one cycle without scope creep. The remaining work for HEX-002,
HEX-003, CONN-001, CONN-002, API-001/002, SOLID-001, M4A-001, M4B-001
is carved into follow-on cycles or stays blocked with honest deferral.

## Revised slices

- **C6.2 HEX-002/003 reality check** (docs + contracts.toml only).
  Re-audit the 8 services files that still import `chronos_log::*`
  and confirm each use is one of: (a) `#[cfg(test)]`, (b) factory
  composition at the boundary, or (c) bridge helper that wraps the
  concrete in the port. Flip HEX-002 from `gap` to `verified` if all
  sites pass the audit; flip to `partial` with notes if any site is
  a real consumer of the concrete type outside composition. Same
  audit for HEX-003 on chronos-store. ~2 files (contracts.toml +
  audit notes in apply-checkpoint). Tier: T0.

- **C6.3 AGENT_API_V2.md ratification** (docs only). Update the doc
  to formally ratify the 41-tool surface (12 v2 + 29 not-yet-converged
  neighbours), the alias sunset policy applied by REC-C5, and the
  future migration order for the 29 neighbours. Flip API-001 from
  `partial` to `verified` once ratified. ~1 file. Tier: T0.

- **C6.4 SOLID-001 narrow migration** (Rust code, A-min).
  Migrate ONE production consumer (the smaller of
  `native_probe_factory` and `native_probe_controller`) to bind on
  `CaptureLifecycle`. Keep the legacy `TraceAdapter` surface for the
  other consumer; document the carve-out. Flip SOLID-001 from
  `partial` to `partial+` (status stays `partial` but notes explain
  the partial carve-out). ~2-3 files. Tier: T0 + T1 + T2 + T3.

- **C6.5 CONN-002 minimal ProbeId newtype** (Rust code, A-min).
  Introduce `ProbeId` newtype in `chronos-domain::ports::identity`
  and thread it through chronos-services and chronos-mcp. Sandbox
  tests that compare probe_id as String get updated in the same
  slice. Flip CONN-002 from `partial` to `partial+`. ~4-5 files.
  Tier: T0 + T1 + T2 + T3 + T4-smoke (sandbox tests will fail
  otherwise).

- **C6.6 API-002 wrapper audit** (Rust code, A-min). Grep the
  sandbox wrapper file (`chronos-sandbox/src/client/tools.rs`) for
  inline adapter calls beyond pure transport mapping. Remove the
  pure-shim ones; keep the ones that are real wire translations.
  Flip API-002 from `partial` to `verified` if the audit clears.
  ~1-2 files. Tier: T0 + T1 + T2 + T4-smoke (e2e_connectivity +
  analytics_tools).

- **C6.7 CONN-001 / HEX-003 / API-001 deferred notes** (docs only).
  CONN-001 wall-clock-ms boundary sites: pre-flight grep + audit
  notes; status stays `partial` with a deferred-to-REC-C7 marker.
  HEX-003 chronos-store decoupling: status stays `gap` with note
  about the indirect path; deferred. ~1 file (contracts.toml).

## Hexagonal boundary preservation

Every C6.x slice MUST keep `known_dependency_violations = []` in
`reconstruction-contracts.toml [architecture]` and
`python3 scripts/check_architecture_contracts.py` PASSED after each
slice lands.

## Slice ordering

C6.2 -> C6.3 -> C6.7 (doc-only or audit-only — leaves repo green)
-> C6.4 -> C6.5 -> C6.6 (Rust slices — order so each slice's
failures don't block later slices).

C6.4 and C6.5 are parallel-safe (different crates). C6.6 follows
C6.5 because the wrapper audit benefits from knowing the new ProbeId
type to look for inline `String`-to-ProbeId conversions.

## Pre-flight grep recipes (per slice)

- C6.2 HEX-002/003 audit:
  ```
  grep -nE 'use chronos_log::|chronos_log::SegmentedExecutionLog' \
    crates/chronos-services/src/ crates/chronos-store/src/ \
    --include='*.rs'
  ```
  Classify each hit as (test-only | factory-composition | bridge-helper
  | real-consumer). If `real-consumer` count > 0, HEX-002/003 stay at
  `partial`/`gap` with notes.

- C6.4 SOLID-001 native_probe_factory migration:
  ```
  grep -nE 'TraceAdapter|CaptureLifecycle' \
    crates/chronos-native/src/native_probe_factory.rs \
    crates/chronos-native/src/native_probe_controller.rs
  ```

- C6.5 CONN-002 ProbeId threading:
  ```
  grep -rn 'probe_id: String\|probe_id = String' \
    crates/ --include='*.rs' | grep -v target
  ```

- C6.6 API-002 wrapper audit:
  ```
  grep -nE 'fn .*\(.*\).*\{' \
    chronos-sandbox/src/client/tools.rs | head -50
  ```
  Identify wrappers whose body is a 2-line adapter call beyond
  pure transport mapping.

## Tier mapping

| slice | tier | rationale |
|---|---|---|
| C6.2 | T0 | doc + audit only |
| C6.3 | T0 | doc only |
| C6.4 | T0+T1+T2+T3 | Rust change in chronos_native |
| C6.5 | T0+T1+T2+T3+T4-smoke | newtype threading touches MCP + sandbox tests |
| C6.6 | T0+T1+T2+T4-smoke | wrapper file + sandbox smoke |
| C6.7 | T0 | doc only |

## Smoke subset

Per AGENTS.md §2 pick the 2-4 sandbox suites that match the touched
code paths:

- `cargo test -p chronos-sandbox --test e2e_connectivity`
- `cargo test -p chronos-sandbox --test analytics_tools`
- `cargo test -p chronos-sandbox --test probe_inject` (CONN-002
  smoke; C6.5 sandbox tests break if ProbeId threading is wrong)
- `cargo test -p chronos-native --lib --test-threads=1` (SOLID-001
  smoke; C6.4 must not regress the ptrace lib tests)

## Pre-approved gates

All `human_gate`s pre-approved per the user's standing instruction
(auto-run mode). No deep-research callout unless a slice surfaces
a real blocker.

## Risk register

| id | severity | mitigation |
|---|---|---|
| C6-R1 HEX-002/003 audit reveals a real consumer of SegmentedExecutionLog outside composition | high | Flip status to `partial` (not `verified`); file a follow-up cycle; do not silently close. |
| C6-R2 SOLID-001 factory migration breaks the chronos-native probe lifecycle | high | Run `cargo test -p chronos-native --lib --test-threads=1`; if regressions, revert the slice and file a follow-up. |
| C6-R3 CONN-002 ProbeId newtype breaks sandbox tests that compare probe_id as String | high | Update the sandbox tests in the same slice; record each updated test in apply-checkpoint. |
| C6-R4 CONN-002 newtype changes the rmcp wire DTO shape | medium | Add a serde-transparent attribute or wrap-projection to preserve the wire format. |
| C6-R5 API-002 wrapper audit reveals a substantive adapter call that should be moved to chronos-services | medium | Move the call to chronos-services and update the wrapper to pure transport; record the move in apply-checkpoint. |

## Out of scope (deferred)

- Real Go instrumentation substrate (M4A-001): needs a Go crate,
  future gate.
- Real Rust instrumentation bridge (M4B-001): composition-root
  adapter wiring + capture-path integration + eBPF/USDT/XRay
  decision module, future gate.
- chronos-store decoupling from chronos-native tracer (HEX-003):
  needs a StorageOpenError port + adapter swap, A-min scope, future
  cycle.
- CONN-001 wall-clock-ms boundary sites in chronos-log: needs
  per-site audit (some sites are monotonic despite raw u64),
  A-min scope, future cycle.
- API-001 41-tool surface ratification is in C6.3; the 29
  not-yet-converged neighbours remain as separate cycles (one
  per neighbour group) once their canonical v2 equivalents are
  designed.
