# Release Receipt — m9-93-trace-event-json-schema

## Identification

| Field | Value |
|---|---|
| Cycle | m9-93-trace-event-json-schema |
| Path | A-min |
| Branch | feat/m9-93-trace-event-json-schema |
| Date | 2026-09-14 |

## Release details

| Field | Value |
|---|---|
| Base SHA | 1b8164d209b521e341b6cbc433000d3a4f57aede |
| Head SHA | 60db8b58bdcf9bb55549dad1e43c9d381b45939c |
| Main SHA | 60db8b58bdcf9bb55549dad1e43c9d381b45939c |
| Remote tag | v0.7.95 |
| Remote tag_peel | 60db8b58bdcf9bb55549dad1e43c9d381b45939c |
| Peel match | true |

## SHAs (canonical table)

| Field | Value |
|---|---|
| Branch | feat/m9-93-trace-event-json-schema |
| Date | 2026-09-14 |
| Base SHA | 1b8164d209b521e341b6cbc433000d3a4f57aede |
| Head SHA | 60db8b58bdcf9bb55549dad1e43c9d381b45939c |
| Remote tag | v0.7.95 |
| Remote tag_peel | 60db8b58bdcf9bb55549dad1e43c9d381b45939c |
| Peel match | true |

## Release notes

- A-min Rust cycle. Closes FIND-M9-91-TRACE-EVENT-NO-JSON-SCHEMA.
- Adds `schemars::JsonSchema` derive to 12 chronos-domain types
  (TraceEvent + 11 transitive dependencies).
- Drops the m9-91 `#[schemars(skip)]` on
  `CounterexampleBundleEventsOutputDto::returned_events`. The
  `returned_events` array is now visible in the published JSON
  Schema; MCP clients can introspect the full event stream shape.
- Enables schemars's `uuid1` feature so `InvocationId(pub Uuid)` can
  derive JsonSchema.
- 3 new unit tests added (2 in chronos-domain, 1 in chronos-services).
- Workspace lib tests: 742 pass (no regressions, +3 vs m9-92).
- T4-smoke: counterexample_tools 12/12 + e2e_connectivity 1/1.

## Cross-checks

- `bash scripts/check_vault_drift.sh`: clean (no vault drift
  introduced; m9-93 is a Rust-only cycle).
- `python3 scripts/regen_manifest_index_shas.py --check`: clean.
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace --lib -- --test-threads=1`: 742 pass.
- `cargo test -p chronos-sandbox --test counterexample_tools -- --test-threads=1`: 12/12 pass.
- `cargo test -p chronos-sandbox --test e2e_connectivity -- --test-threads=1`: 1/1 pass.
- `apply-checkpoint.peel_match == true`.
- `apply-checkpoint.head_sha == release-receipt.Head SHA == 60db8b58`.
- `Remote tag` v0.7.95 peel: `60db8b58` (cycle-artifacts commit).
