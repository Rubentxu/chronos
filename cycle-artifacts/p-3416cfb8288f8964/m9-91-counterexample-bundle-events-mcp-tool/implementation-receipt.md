# Implementation Receipt — m9-91-counterexample-bundle-events-mcp-tool

## Summary

Closes the long-deferred m9-02-R4 backlog item by adding a full MCP
surface for reading the events stream of a counterexample bundle.

The m9-02 storage primitive
`counterexample_storage::bundle_events_or_legacy` had shipped unused
at the MCP layer since m9-02. m9-04 documents R4 explicitly:

> R4_m9_02_no_new_mcp_tool: counterexample_bundle_events tool stays
> m9+; m9-02 ships the storage primitive only.

m9-91 fixes that by adding:

- A service method (`ChronosCounterexampleService::events`).
- A wire DTO (`CounterexampleBundleEventsOutputDto`).
- An MCP tool (`counterexample_bundle_events`) with input params
  `{bundle_id, limit?, offset?}` and output envelope
  `{bundle_id, events_count, returned_events, next_offset}`.
- 3 unit tests covering: full-stream read, paginated read across 4
  pages (including out-of-range offset), and LoadFailed on missing
  bundle.

The pagination is in-memory: the service loads the full events via
`bundle_events_or_legacy`, then applies `offset`/`limit` and computes
`next_offset`. This is the same approach the spec called out as the
intended strategy and matches the existing `events_count` accessor's
read path.

## Files changed

### Added

- **Service variant**: `CounterexampleOutput::Events { bundle_id,
  events_count, returned_events: Vec<TraceEvent>, next_offset }` in
  `crates/chronos-services/src/counterexample.rs`.
- **Service method**: `ChronosCounterexampleService::events(ctx,
  bundle_id, limit, offset) -> Result<CounterexampleOutput,
  ServiceError>` (50 lines including doc comments). Verifies the
  bundle exists via `load_counterexample_bundle`, then loads events
  via `bundle_events_or_legacy`, applies offset + limit in memory,
  and computes `next_offset`.
- **Wire DTO**: `CounterexampleBundleEventsOutputDto` in
  `crates/chronos-services/src/output.rs`. Derives Debug + Clone +
  PartialEq + Serialize + Deserialize + JsonSchema. The
  `returned_events` Vec is marked `#[schemars(skip)]` because
  `chronos_domain::TraceEvent` does not (yet) implement JsonSchema —
  pragmatic fix; matches the convention for bundling opaque event
  payloads onto a wire DTO.
- **MCP params**: `CounterexampleBundleEventsParams { bundle_id,
  limit: Option<usize>, offset: Option<usize> }` in
  `crates/chronos-mcp/src/server.rs`.
- **MCP tool**: `#[tool(name = "counterexample_bundle_events")]`
  handler in `crates/chronos-mcp/src/server.rs`. Maps to the new
  service method and routes the result through
  `serialize_counterexample_output`.
- **Serialize arm**: `COut::Events { ... }` arm in
  `serialize_counterexample_output` →
  `CounterexampleBundleEventsOutputDto`.
- **Unit tests** (3 new): `m9_91_events_returns_full_stream_with_no_pagination`,
  `m9_91_events_pagination_with_limit_and_offset`,
  `m9_91_events_missing_bundle_returns_load_failed`.

## Service method contract

```rust
pub async fn events(
    ctx: &CounterexampleContext<'_>,
    bundle_id: &str,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<CounterexampleOutput, ServiceError>
```

- **load_counterexample_bundle(ctx.store, bundle_id)**: must return
  `Some(bundle)` or the method surfaces
  `ServiceError::LoadFailed("no counterexample bundle with id \`{id}\`")`.
- **bundle_events_or_legacy(ctx.store, &bundle)**: drives the load
  through the m9-02 v3 side-table (with legacy fallback to the
  blob).
- **offset defaults to 0; limit defaults to None (= full stream)**.
- **Pagination**: starts at `offset`, returns up to `limit` items,
  computes `next_offset = Some(off + returned_count)` when more
  remain, `None` otherwise.

## Wire envelope

```json
{
  "bundle_id": "...",
  "events_count": 10,
  "returned_events": [ ... TraceEvent ... ],
  "next_offset": 3  // or null when no more remain
}
```

`events_count` is the total persisted on disk — independent of the
slice. `next_offset` is `null` (or absent on the wire) when the
returned slice exhausted the stream, or when the offset was already
past the end.

## TraceEvent + JsonSchema

`chronos_domain::TraceEvent` derives `Debug, Clone, Serialize,
Deserialize, PartialEq` but **not** `JsonSchema`. Implementing
`JsonSchema` would require a recursive schema for `EventData`'s ~10
variants, an arbitrary-shape pattern that's painful to author by
hand. Pragmatic fix: the DTO marks `returned_events` with
`#[schemars(skip)]`. The events remain on the wire via their serde
representation — schema generation simply treats them as opaque.
This matches how the existing codebase handles opaque payloads
(e.g., `replay_tools` handles entire sessions, where the events
inside are likewise skipped from the schema).

## Risk and verification

- Build: `cargo build --workspace` succeeds, no warnings.
- Lint: `cargo clippy --workspace --all-targets -- -D warnings`
  clean.
- Format: `cargo fmt --all -- --check` clean.
- Tests:
  - `cargo test -p chronos-services --lib`: 267/267 (was 264, +3 new).
  - `cargo test --workspace --lib -- --test-threads=1`: all suites pass
    (services 267, store 77, mcp 87, native 103, plus all others).
- Sandbox smoke subset (T4-smoke):
  - `cargo test -p chronos-sandbox --test counterexample_tools
    -- --test-threads=1`: 12/12 (the suite most directly exercising
    the affected tool family).
  - `cargo test -p chronos-sandbox --test e2e_connectivity
    -- --test-threads=1`: 1/1 (server starts with new tool
    registered).
  - `cargo test -p chronos-sandbox --test analytics_tools
    -- --test-threads=1`: 4/4 (broad coverage, no regression in
    adjacent families).

## Carry-forward

- The `cc-001-god-module` debt remains open. `counterexample.rs`
  grew by ~330 lines this cycle (mostly tests). Refactor is out of
  scope for this cycle.
- `counterexample_storage.rs` (the original god-module target) was
  not touched in m9-91; the new method goes through the existing
  `bundle_events_or_legacy` chokepoint.
- FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK remains external-deferred
  (carried from m9-88).
