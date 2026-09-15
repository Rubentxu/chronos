# Change: m9-93 TraceEvent JsonSchema derive

## Summary

Cycle closed with no follow-up debt. Schema and drift sweep clean.

## Identification

| Field | Value |
|---|---|
| Cycle ID | `m9-93-trace-event-json-schema` |
| Workspace | `p-3416cfb8288f8964` |
| Path | A-min (single-crate refactor + 1-line DTO cleanup) |
| Status | CLOSED |
| Branch | `feat/m9-93-trace-event-json-schema` |
| Tag | `v0.7.95` |

## Subject

Close FIND-M9-91-TRACE-EVENT-NO-JSON-SCHEMA. Add
`schemars::JsonSchema` derive to `chronos_domain::TraceEvent` and 11
transitive dependencies, then drop the `#[schemars(skip)]` on
`CounterexampleBundleEventsOutputDto::returned_events` introduced
pragmatically in m9-91.

## Problem

m9-91 (counterexample_bundle_events MCP tool, A-min, v0.7.93) opened
FIND-M9-91-TRACE-EVENT-NO-JSON-SCHEMA. The wire DTO
`CounterexampleBundleEventsOutputDto` exposes `Vec<TraceEvent>` but
`TraceEvent` did not implement `schemars::JsonSchema`. m9-91 applied a
pragmatic fix using `#[schemars(skip)]` on the field, which hid the
event stream from MCP clients' introspection.

The m9-92 closure handoff recommended m9-93 close this finding.

## Approach

A-min: single-crate refactor in `chronos-domain` + 1-line cleanup in
`chronos-services::output::CounterexampleBundleEventsOutputDto`. No
architectural change, no new domain concept, no probe/mcp touched.

### Types updated (12 total)

`chronos-domain` types receiving `JsonSchema` derive:
1. `Language` (enum, 14 variants) — trace/session.rs
2. `VariableScope` (enum) — value/typed.rs
3. `VariableInfo` (struct) — value/typed.rs
4. `SourceLocation` (struct) — trace/location.rs
5. `EventType` (enum, ~22 variants) — trace/event.rs
6. `SymbolId` (struct) — trace/event.rs
7. `InvocationId` (struct, wraps Uuid) — trace/event.rs
8. `EventData` (enum, ~14 variants) — trace/event.rs
9. `RegisterState` (struct, 18 u64 fields) — trace/event.rs
10. `WasmModuleInfo` (struct) — trace/event.rs
11. `WasmFunctionInfo` (struct) — trace/event.rs
12. `TraceEvent` (struct, leaf target) — trace/event.rs

### Workspace dep change

`Cargo.toml`: enabled `schemars = { version = "1", features = ["uuid1"] }`
so `InvocationId(pub Uuid)` can derive `JsonSchema` (the impl lives
in schemars, not in uuid).

### DTO cleanup

`crates/chronos-services/src/output.rs`: removed
`#[schemars(skip)]` from
`CounterexampleBundleEventsOutputDto::returned_events`.

### Tests added (3)

| Test | Crate | Validates |
|---|---|---|
| `trace_event_implements_json_schema` | chronos-domain | REQ-m9-93-1: schema has 6 expected properties |
| `event_data_schema_is_oneof` | chronos-domain | REQ-m9-93-2: schema is a oneOf with ≥10 variants |
| `bundle_events_dto_schema_includes_returned_events` | chronos-services | REQ-m9-93-5: DTO schema includes returned_events array |

## Files changed

| Bucket | Files | Lines | Notes |
|---|---|---|---|
| Cargo.toml (workspace) | 1 modified | 1 | `schemars` features ["uuid1"] |
| chronos-domain | 4 modified | ~70 | +12 derives + 2 unit tests |
| chronos-services | 1 modified | ~25 | dropped skip, +1 unit test, updated doc comment |

**Total**: 6 files modified, ~95 net lines added.

## Drift delta

None — m9-93 does not affect any CC. The cycle is purely a Rust
refactor + JSON Schema introspection improvement. No vault drift
introduced or closed.

## Out-of-scope

- **FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK** (carried from m9-88):
  external `sddk` CLI bug; cannot be fixed in chronos scope.
- Schema docs site auto-generation (separate cycle).
- EventData variant additions.
- `cc-001-god-module-keys-split` / `cc-004-implicit-io-toctou`
  (deferred to m10+).

## Cross-check

- **CC#3**: m9-93 cycle apply-checkpoint `head_sha`,
  `remote_tag_peel`, `tag_peel_sha`, and `main_sha` all equal
  `2e2b7bd` (cycle source); `peel_match` = `true`. Tag stays at
  pre-cascade-fixpoint commit per CC#42 workaround.
- **CC#4**: `python3 scripts/regen_manifest_index_shas.py --check`
  exits 0 (no stale rows after SHA fixpoint cascade).
- **CC#8**: All m9-93 SHAs (`base_sha`, `head_sha`, `main_sha`,
  `remote_tag_peel`) exist in the local git object store.
- **CC#11**: m9-93 apply-checkpoint `status` = `CLOSED`,
  `archived_at` = `2026-09-14`,
  `findings_closed` = `["FIND-M9-91-TRACE-EVENT-NO-JSON-SCHEMA"]`,
  `peel_match` = `true`.
- **CC#22**: m9-93 release-receipt Head SHA matches apply-checkpoint
  head_sha; Remote tag_peel matches.
- **CC#23**: m9-93 merge-receipt Head SHA matches apply-checkpoint
  head_sha; Base SHA matches base_sha.
- **CC#39**: cycles/index.md Total cycles = 93 (matches actual row
  count after m9-93 row added).
- **CC#42**: m9-93 release-receipt Remote tag_peel matches
  `git rev-parse v0.7.95^{commit}`.
- **CC#43**: m9-93 release-receipt Head SHA matches
  apply-checkpoint head_sha (`2e2b7bd`).
- **CC#47**: m9-93 apply-checkpoint `base_sha` (`1b8164d`) exists in
  git as a commit object (was m9-92 final cascade HEAD).
- **CC#51**: cycles/index.md cycle (m9-93) has
  cycle-artifacts/ folder (with 7 artifacts).
- **CC#53**: cycle branch `feat/m9-93-trace-event-json-schema`
  deleted after `--no-ff` merge into main.

## Files

- Source commit: `2e2b7bd` (m9-93: add JsonSchema derive to
  TraceEvent + transitive deps).
- Cycle-artifacts commit: TBD (cycle-artifacts/ dir + change-entry +
  spec/tasks/exploration-report).
- Merge commit: TBD (`--no-ff` merge into main).
- Cascade commits: typically 1-2 SHA-bookkeeping commits per CC#42
  fixpoint-cascade workaround.
