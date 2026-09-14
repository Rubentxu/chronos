# m9-91 Exploration Report — counterexample_bundle_events MCP Tool

## Scope

Close `m9-02-R4`:
> `counterexample_bundle_events` MCP tool deferred to m9+ (m9-02 ships
> storage primitive only)

Add an MCP tool surface for reading the events stream of a
counterexample bundle (already stored in the
`counterexample_bundle_events` side table since m9-02).

## Discovery

### Existing storage primitive

The side table was added in m9-02 (`crates/chronos-store/src/ce_schema.rs`):

```rust
pub const COUNTEREXAMPLE_BUNDLE_EVENTS: TableDefinition<&str, Vec<u8>> =
    TableDefinition::new("counterexample_bundle_events");
```

Store-level accessor methods exist since m9-04/m9-05
(`crates/chronos-store/src/ce_read.rs`):

- `pub fn load_counterexample_bundle_events(&self, bundle_id: &str) -> Result<Vec<TraceEvent>, StoreError>`
- `pub fn count_counterexample_bundle_events(&self, bundle_id: &str) -> Result<u64, StoreError>`

### Existing MCP tool surface (counterexample-only)

The MCP server has exactly one counterexample tool today
(`crates/chronos-mcp/src/server.rs:5517`):

- `counterexample_events_count` — returns only the events_count
  (lightweight accessor). Implemented via
  `chronos_services::counterexample::ChronosCounterexampleService::events_count`,
  which calls `ctx.store.load_counterexample_bundle(bundle_id)` and
  reads `summary.events_count`.

The actual events stream is **not** reachable from MCP. To verify a
saved bundle's events, an MCP client must fall back to
`load_counterexample_bundle` (which only returns the bundle metadata,
not the events) or the in-memory engine (which only works for live
sessions).

### Service layer

`ChronosCounterexampleService` is the natural home for the new
method. Existing methods:

- `get`, `events_count`, `list`, `save` (4 methods total).

The new `events` method slots in next to `events_count`.

### Output DTOs

`crates/chronos-services/src/output.rs` has a DTO per
`CounterexampleOutput` variant. The new `Events` variant needs a
matching `CounterexampleBundleEventsOutputDto`.

### MCP tool registration

Tools are registered via `#[rmcp::tool_router]` on `impl ChronosServer`.
Adding a new `#[tool]` attribute + handler method is sufficient;
rmcp discovers and exposes it at startup.

## Constraints

- The events stream is bounded by `summary.events_count` (typically
  < 10k, but can be larger for long-running sessions). A pagination
  knob (`limit` + `offset`) is included for safety, but the default
  is "return all events".
- `TraceEvent` already derives `serde::Serialize`. The DTO can hold
  `Vec<TraceEvent>` directly without per-field DTOs.
- The MCP envelope helper (`session_envelope(degraded, value)`)
  injects a top-level `degraded: bool`. The new tool should use it
  to remain consistent with `save_session` / `list_sessions` etc.

## Out-of-scope

- Save events endpoint: deferred (no caller needs it; `save_counterexample_bundle`
  already writes events atomically as part of the save flow).
- Streaming: rmcp doesn't have a streaming tool pattern; full JSON
  return is sufficient for bounded event streams.
- Per-event filtering: the existing `events_read` tool already
  covers per-session filtered reads.
- Schema migration: not needed; the side table schema is unchanged.

## Decision

A-min path. Bounded scope (single new MCP tool + matching service
method + DTO + tests). No architectural changes.
