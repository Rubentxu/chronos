# Release Report — m9-93-trace-event-json-schema

> **Cycle**: m9-93-trace-event-json-schema
> **Path**: A-min (single-crate refactor + 1-line DTO cleanup)
> **Tag**: v0.7.95
> **Tag peel (immutable)**: `60db8b58bdcf9bb55549dad1e43c9d381b45939c` (cycle-artifacts commit; pre-cascade-fixpoint per CC#42 workaround)
> **Merge SHA (cycle-artifacts)**: 60db8b58bdcf9bb55549dad1e43c9d381b45939c
> **Date archived**: 2026-09-14
> **Status**: released

## What changed

m9-93 is an A-min Rust cycle that closes FIND-M9-91-TRACE-EVENT-NO-JSON-SCHEMA.

It adds `schemars::JsonSchema` derive to 12 chronos-domain types
(TraceEvent + 11 transitive dependencies) and drops the m9-91
`#[schemars(skip)]` workaround on
`CounterexampleBundleEventsOutputDto::returned_events`.

After m9-93, the `counterexample_bundle_events` MCP tool publishes a
JSON Schema where `returned_events` is fully visible as an array of
`TraceEvent` objects. MCP clients can introspect the full event
stream shape (no more "opaque" field).

## Approach

Single source commit (Rust only):

1. Workspace `Cargo.toml`: enable `schemars = { version = "1", features = ["uuid1"] }`.
2. chronos-domain: add `JsonSchema` derive to 12 types.
3. chronos-services: drop `#[schemars(skip)]` on
   `CounterexampleBundleEventsOutputDto::returned_events`.
4. 3 new unit tests (2 in chronos-domain, 1 in chronos-services).

## Drift line delta

None. m9-93 is a Rust-only cycle; no vault drift introduced or closed.

## Verification

- **T0** (lint gate): `cargo fmt` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean.
- **T1** (lib unit): workspace lib tests pass: 742 total (151/268/77/48/8/77/26/87).
- **T2** (per-crate integration): not separately required.
- **T4-smoke** (sandbox subset): counterexample_tools 12/12, e2e_connectivity 1/1.

## Findings

### Closed

- **FIND-M9-91-TRACE-EVENT-NO-JSON-SCHEMA** (opened m9-91, closed m9-93): 12 types now derive JsonSchema; the `#[schemars(skip)]` workaround on the bundle_events DTO was removed; the event stream shape is now visible to MCP clients.
- **FIND-M9-93-NO-RUST-CHANGES** (closed n/a — N/A; m9-93 is a Rust cycle).
- **FIND-M9-93-CARGO-FEATURE-ADD** (closed): `schemars uuid1` feature enabled.

### Carry-forward

- **FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK** (carried from m9-88): external `sddk` CLI bug; cannot be fixed in chronos scope.

## Status

Released as `v0.7.95`. The branch
`feat/m9-93-trace-event-json-schema` was merged into `main` with
`--no-ff` and deleted after release.

## Cross-checks

- `apply-checkpoint.head_sha` == `release-receipt.head_sha` ==
  `merge-receipt.head_sha` == `60db8b58`.
- `Remote tag` v0.7.95 peel: `60db8b58` (cycle-artifacts commit;
  tag pre-created at cycle-artifacts per CC#42 workaround).
- `apply-checkpoint.peel_match` == `true`.
- `apply-checkpoint.main_sha` == `apply-checkpoint.head_sha` == `60db8b58`.
- `apply-checkpoint.status` == `"CLOSED"`.
- `apply-checkpoint.archive_status` == `"complete"`.
- `cycles/index.md` row added; Total cycles 92 → 93.
- `terms/index.md` Last archive = m9-93-trace-event-json-schema.
- `bash scripts/check_vault_drift.sh`: clean (no drift introduced).
- `python3 scripts/regen_manifest_index_shas.py --check`: clean.
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace --lib -- --test-threads=1`: 742 pass.
- `cargo test -p chronos-sandbox --test counterexample_tools -- --test-threads=1`: 12/12 pass.
- `cargo test -p chronos-sandbox --test e2e_connectivity -- --test-threads=1`: 1/1 pass.

## Files Inventory

| Path | Change |
|---|---|
| `Cargo.toml` (workspace) | modified (+1 line: schemars features ["uuid1"]) |
| `crates/chronos-domain/src/trace/event.rs` | modified (+5 derives + 2 unit tests) |
| `crates/chronos-domain/src/trace/location.rs` | modified (+1 derive) |
| `crates/chronos-domain/src/trace/session.rs` | modified (+1 derive) |
| `crates/chronos-domain/src/value/typed.rs` | modified (+2 derives) |
| `crates/chronos-services/src/output.rs` | modified (dropped schemars(skip), +1 unit test, updated doc) |
| `cycle-artifacts/p-3416cfb8288f8964/m9-93-trace-event-json-schema/*.md` + `.json` | added (7 cycle artifacts) |
