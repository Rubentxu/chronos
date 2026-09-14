# Release Report — m9-91-counterexample-bundle-events-mcp-tool

## Identification

| Field | Value |
|---|---|
| Cycle | m9-91-counterexample-bundle-events-mcp-tool |
| Path | A-min |
| Branch | feat/m9-91-counterexample-bundle-events-mcp-tool |
| Date | 2026-09-14 |
| Base SHA | 5cdb4e1a2d38b53d53addf3eb04e25650fd01b9d |
| Head SHA | fdcb0dcd9c59864eaf8e8c8b942f1867d5d5b18c |
| Remote tag | v0.7.93 (pre-created at cycle-artifacts commit fdcb0dc) |
| ff_merged | false (--no-ff merge commit 40a7ba23082c11f5dc2ec6a25a9a8d12cb9a4cfa) |

## Cycle value

| Field | Value |
|---|---|
| Cycle | m9-91-counterexample-bundle-events-mcp-tool |
| Tier required | T2 |
| Tiers run | T0, T1 (workspace lib), T2 (services), T4-smoke (counterexample_tools + e2e_connectivity + analytics_tools) |
| Status | applied → verified → released (pending merge + cascade + push) |

## Path

A-min (single new MCP tool + matching service method + DTO + tests;
two crates touched ��� chronos-services + chronos-mcp).

## Cycle

m9-91-counterexample-bundle-events-mcp-tool closes m9-02-R4
(m9-02's deferred `counterexample_bundle_events` MCP tool exposure).
The storage primitive `bundle_events_or_legacy` had shipped unused at
the MCP layer for 89 cycles (~6 weeks of work) before this cycle.

## What shipped

- **Service variant**: `CounterexampleOutput::Events { bundle_id,
  events_count, returned_events: Vec<TraceEvent>, next_offset }`.
- **Service method**: `ChronosCounterexampleService::events(ctx,
  bundle_id, limit, offset)`. Verifies the bundle exists via
  `load_counterexample_bundle`, loads events via
  `bundle_events_or_legacy`, applies offset + limit in memory,
  computes `next_offset`.
- **Wire DTO**: `CounterexampleBundleEventsOutputDto { bundle_id,
  events_count, returned_events, next_offset }`. `#[schemars(skip)]`
  on the events Vec because `TraceEvent` lacks `JsonSchema` (see
  FIND-M9-91-TRACE-EVENT-NO-JSON-SCHEMA).
- **MCP tool**: `#[tool(name = "counterexample_bundle_events")]`
  registered with `Parameters<CounterexampleBundleEventsParams>`,
  response routed through `serialize_counterexample_output`'s
  `COut::Events` arm.
- **3 unit tests**: full-stream (1), pagination across 4 pages (1),
  missing-bundle LoadFailed (1).

## What did NOT ship

- No new public API beyond the 4 listed items (tool/DTO/method/variant).
- No public API removed.
- No changes to `counterexample_storage.rs` (the m9-91 code reuses
  the existing `bundle_events_or_legacy` chokepoint from m9-02).
- `JsonSchema` impl on `chronos_domain::TraceEvent` — out of scope
  for m9-91 (a schema-generator for `EventData`'s ~10 variants is a
  sizable PR in its own right; finding recorded as
  FIND-M9-91-TRACE-EVENT-NO-JSON-SCHEMA for a future cycle).
- No CLI plumbing changes — the new tool is MCP-only.
- No sandbox-e2e or ptrace work — out of scope.

## Cross-checks / Verification

| Tier | Command | Result |
|---|---|---|
| T0 | `cargo fmt --all -- --check` | passed |
| T0 | `cargo clippy --workspace --all-targets -- -D warnings` | passed |
| T1 | `cargo test --workspace --lib -- --test-threads=1` | all suites pass (services 267, store 77, mcp 87, native 103) |
| T2 | `cargo test -p chronos-services --lib` | 267/267 (was 264, +3 new) |
| T4-smoke | counterexample_tools | 12/12 |
| T4-smoke | e2e_connectivity | 1/1 |
| T4-smoke | analytics_tools | 4/4 |
| Build | `cargo build --workspace` | success, no warnings |
| Build | `cargo build --bin chronos-mcp` | success, no warnings |

### Cross-checks

- **apply-checkpoint.base_sha** == **release-receipt.base_sha** ==
  `5cdb4e1a2d38b53d53addf3eb04e25650fd01b9d`. **PASS.**
- **apply-checkpoint.head_sha** == **release-receipt.head_sha** ==
  `18e2a28d0fc1ade564d1c884d9c7228f5864a6f0`. **PASS (cycle HEAD
  pre-merge).** Final HEAD after SHA cascade will differ; that
  cascade is documented in release-receipt.sh's "Final fixpoint HEAD"
  row.
- **cycles/index.md** will have m9-91 row referencing
  `5cdb4e1a2d38b53d53addf3eb04e25650fd01b9d` once the cascade
  completes. Total cycles 90 → 91.
- **v0.7.93** tag peel == cycle-artifacts commit
  `fdcb0dcd9c59864eaf8e8c8b942f1867d5d5b18c` (current), eventually
  moving through cascade commits. **PASS (CC#42 fixpoint-cascade
  workaround).**

## Files Inventory

- `crates/chronos-services/src/counterexample.rs` — modified (+329 lines).
- `crates/chronos-services/src/output.rs` — modified (+27 lines).
- `crates/chronos-mcp/src/server.rs` — modified (+87 lines).
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-91-counterexample-bundle-events-mcp-tool/` —
  exploration-report, proposal, spec, tasks.
- `cycle-artifacts/p-3416cfb8288f8964/m9-91-counterexample-bundle-events-mcp-tool/` —
  7 cycle artifacts.
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-91-counterexample-bundle-events-mcp-tool/archive-manifest.md`
  — archive manifest (added during archive phase).
- Tag `v0.7.93` to be created at the cycle-artifacts commit (TBD SHA).

## Carry-forward

- `cc-001-god-module` debt remains open. `counterexample.rs` grew
  ~330 lines this cycle (mostly tests). Refactor for that file is
  out of scope for m9-91.
- FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK remains external-deferred
  (carried from m9-88).
- A future cycle could add `JsonSchema` for `TraceEvent` to drop the
  `#[schemars(skip)]` on `CounterexampleBundleEventsOutputDto`'s
  events field. Recorded as FIND-M9-91-TRACE-EVENT-NO-JSON-SCHEMA.

## Artifacts

- `cycle-artifacts/p-3416cfb8288f8964/m9-91-counterexample-bundle-events-mcp-tool/`:
  apply-checkpoint.json, implementation-receipt.md, merge-receipt.md,
  release-receipt.md, release-report.md, verify-findings.json,
  verify-report.md.
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-91-counterexample-bundle-events-mcp-tool/`:
  exploration-report.md, proposal.md, spec.md, tasks.md.
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-91-counterexample-bundle-events-mcp-tool/archive-manifest.md`
  (to be added at archive).
- Tag `v0.7.93` at the cycle-artifacts commit (TBD SHA, pre-cascade).
