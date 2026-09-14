# m9-91 Spec — counterexample_bundle_events MCP Tool

## Behavior

A new MCP tool `counterexample_bundle_events` that reads the events
stream for a `bundle_id` from the `counterexample_bundle_events` side
table, returning the events as a JSON array.

### Tool name

`counterexample_bundle_events` (snake_case to match existing tools).

### Input

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CounterexampleBundleEventsParams {
    /// Bundle ID whose events stream to load.
    pub bundle_id: String,
    /// Optional maximum number of events to return.
    /// `None` means "return all events" (no truncation).
    #[serde(default)]
    pub limit: Option<usize>,
    /// Optional number of events to skip.
    /// Used together with `limit` for pagination.
    #[serde(default)]
    pub offset: Option<usize>,
}
```

### Output

```json
{
  "bundle_id": "bundle-abc",
  "events_count": 42,
  "returned_count": 42,
  "next_offset": null,
  "events": [
    {"event_id": 1, "timestamp_ns": 100, "thread_id": 0, "event_type": "function_entry", ...},
    ...
  ]
}
```

If `limit` + `offset` truncate, `events_count` is the **total**
count, `returned_count` is the number in this response, and
`next_offset` is `Some(offset + returned_count)` if more events
remain, `None` otherwise.

### Errors

- `bundle_id` does not exist → `LoadFailed("no counterexample bundle with id `<id>`")`
- `limit` exceeds events_count → silently clamped (returns all
  available events).

### Implementation outline

1. New `ChronosCounterexampleService::events` method:
   ```rust
   pub fn events(
       ctx: &CounterexampleContext<'_>,
       bundle_id: &str,
       limit: Option<usize>,
       offset: Option<usize>,
   ) -> Result<CounterexampleOutput, ServiceError>
   ```
   - Loads events via `ctx.store.load_counterexample_bundle_events(bundle_id)`.
   - Applies offset/limit in memory.
   - Returns `CounterexampleOutput::Events { bundle_id, events_count, returned_events, next_offset }`.

2. New `CounterexampleOutput::Events` variant.

3. New wire DTO `CounterexampleBundleEventsOutputDto` in
   `chronos-services::output`:
   ```rust
   pub struct CounterexampleBundleEventsOutputDto {
       pub bundle_id: String,
       pub events_count: usize,
       pub returned_count: usize,
       pub next_offset: Option<usize>,
       pub events: Vec<TraceEvent>,
   }
   ```

4. New MCP tool handler in `chronos-mcp::server`:
   ```rust
   #[tool(
       name = "counterexample_bundle_events",
       description = "Load the persisted events stream for a counterexample bundle. ..."
   )]
   async fn counterexample_bundle_events(
       &self,
       params: Parameters<CounterexampleBundleEventsParams>,
   ) -> Result<CallToolResult, rmcp::ErrorData>
   ```

5. New `COut::Events` arm in `serialize_counterexample_output`.

## Scenarios

### Scenario 1: load events for an existing bundle

```
$ chronos-mcp-tool counterexample_bundle_events bundle_id=bundle-abc
{
  "bundle_id": "bundle-abc",
  "events_count": 42,
  "returned_count": 42,
  "next_offset": null,
  "events": [...]
}
```

### Scenario 2: bundle_id does not exist

```
$ chronos-mcp-tool counterexample_bundle_events bundle_id=nope
error: "counterexample_events failed: no counterexample bundle with id `nope`"
```

### Scenario 3: pagination

```
$ chronos-mcp-tool counterexample_bundle_events bundle_id=bundle-abc limit=10 offset=20
{
  "bundle_id": "bundle-abc",
  "events_count": 42,
  "returned_count": 10,
  "next_offset": 30,
  "events": [...]  // events[20..30]
}
```

## Acceptance criteria

1. New tool `counterexample_bundle_events` appears in the
   `list_tools` MCP response.
2. `cargo test -p chronos-mcp --tests --no-fail-fast` includes a new
   test `test_counterexample_bundle_events_roundtrip` that:
   - Saves a bundle with N events
   - Calls the new tool via the rmcp test client
   - Verifies returned events match what was saved (in order)
3. Sandbox `e2e_connectivity` test still passes (no regression on
   existing tools).
4. `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings` clean.
5. `cargo test --workspace --lib -- --test-threads=1` passes all
   existing tests + new test.
