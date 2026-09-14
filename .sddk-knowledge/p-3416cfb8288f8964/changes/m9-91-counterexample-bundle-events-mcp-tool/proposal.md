# m9-91 Proposal — counterexample_bundle_events MCP Tool

## Why this cycle

Closes the long-deferred `m9-02-R4` backlog item:
> `counterexample_bundle_events` MCP tool deferred to m9+ (m9-02 ships
> storage primitive only)

The `counterexample_bundle_events` side table has existed since
m9-02 (schema_version=2), and `chronos_store::SessionStore` has
exposed `load_counterexample_bundle_events` and
`count_counterexample_bundle_events` since m9-04 / m9-05. But the
MCP tool surface only exposes the per-bundle summary
(`counterexample_events_count`) — the actual events stream is
inaccessible to MCP clients.

## Goal

Add a single new MCP tool `counterexample_bundle_events` that reads
the events stream for a bundle_id from the `counterexample_bundle_events`
side table, returning the events as JSON.

## Path

A-min. Bounded scope (single tool + service function + tests).

## Deliverables

1. `chronos-services::counterexample::ChronosCounterexampleService::events()`
   new method that calls
   `chronos_store::SessionStore::load_counterexample_bundle_events`.
2. New `CounterexampleContext::events(...)` (returns `Vec<TraceEvent>`
   or a typed envelope) + matching `CounterexampleOutput::Events { ... }`
   variant.
3. `chronos-mcp` server.rs: new `#[tool] counterexample_bundle_events`
   handler + `CounterexampleBundleEventsParams` struct.
4. New wire DTO serializer in `server.rs::serialize_counterexample_output`.
5. Sandbox test asserting the round-trip: save a bundle, list events
   via the new tool, compare to the events the engine produced.
6. Cycle artifacts + knowledge artifacts + archive + handoff.
7. Release `v0.7.93`.

## Tag

`v0.7.93` (patch bump from `v0.7.92`).

## Out of scope

- Pagination / streaming: out-of-scope. The events_count is bounded
  by the bundle's `summary.events_count` (typically < 10k). Future
  cycles can add `limit` + `offset` if needed.
- Filtering by event_type / thread_id / timestamp: out-of-scope. The
  existing `events_read` tool already covers per-session filtered
  reads. Bundle events are typically small and read in full.
- Save events endpoint: deferred (no caller needs it; the
  `save_counterexample_bundle` already writes events atomically).
- Schema migration: not needed; the side table schema is unchanged
  since m9-02.

## Verification

- T0: fmt + clippy clean.
- T1: `cargo test --workspace --lib -- --test-threads=1` passing,
  including new tests for `CounterexampleService::events` and the
  MCP round-trip.
- T2: `cargo test -p chronos-mcp --tests --no-fail-fast` for the
  MCP server-side test.
- T4-smoke: run sandbox `e2e_connectivity` + `analytics_tools` (or
  add a new `counterexample_tools` test) to confirm the new tool
  appears in the tool listing and round-trips a bundle.
