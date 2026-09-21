# Release Receipt — g0.1-events-read-kind-contract-align

## Identification

| Field | Value |
|---|---|
| Cycle | g0.1-events-read-kind-contract-align |
| Path | A-min, evidence-backed enum contract alignment |
| Base SHA | 2ee989ba45c049e07d7757d9ca3d282c2b4c9b76 |
| Head SHA | 0f773810ee9a06291b4cdd40952f3600fdd18db0 |
| Main SHA | 0f773810ee9a06291b4cdd40952f3600fdd18db0 |
| Remote tag | v0.7.112 |
| Remote tag_peel | 0be2ec2d53d9698956ae705938b32b80d7365ad7 |

## Release result

G0.1 ships a 2-line alignment in
`crates/chronos-services/src/output.rs:1587` that adds `Serialize` to
the `EventsReadKind` enum derive and applies `#[serde(rename_all =
"snake_case")]`. This brings the serde contract into agreement with
the existing JsonSchema naming, removing a wire-shape drift where the
server accepted `"mode": "Query"` (PascalCase) instead of the canonical
`"mode": "query"` snake_case.

9 contract tests in
`crates/chronos-services/tests/events_read_kind.rs` assert the
breaking change explicitly (PascalCase is rejected). Wire smoke
`/tmp/g0.1-wire-smoke` (282 lines, JSON-RPC manual, no rmcp)
confirms the binary now emits `"unknown variant 'Query', expected
'query' or 'by_id'"` for the rejected case.

The cycle does NOT create a new release tag. Canonical reference
remains `v0.7.112` peel `0be2ec2d` (intact).

| Peel match | true |
