# m9-91 Tasks — counterexample_bundle_events MCP Tool

A-min cycle. 5 tasks.

## Task 1: Service layer — `ChronosCounterexampleService::events`

In `crates/chronos-services/src/counterexample.rs`:

1. Add a new `CounterexampleOutput::Events` variant:
   ```rust
   Events {
       bundle_id: String,
       events_count: usize,
       returned_events: Vec<TraceEvent>,
       next_offset: Option<usize>,
   }
   ```

2. Add `pub fn events(...)` method to `ChronosCounterexampleService`:
   ```rust
   pub fn events(
       ctx: &CounterexampleContext<'_>,
       bundle_id: &str,
       limit: Option<usize>,
       offset: Option<usize>,
   ) -> Result<CounterexampleOutput, ServiceError>
   ```
   - First call `ctx.store.load_counterexample_bundle_events(bundle_id)`
   - Verify the bundle exists (return `LoadFailed` if 0 events and
     bundle summary is also missing — actually call
     `load_counterexample_bundle` first to verify existence, then load events).
   - Apply `offset` and `limit` in memory.
   - Compute `next_offset = offset + returned_count` if more events
     remain.
   - Return the `Events` variant.

3. Unit tests for the new method.

## Task 2: Wire DTO — `CounterexampleBundleEventsOutputDto`

In `crates/chronos-services/src/output.rs`:

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, JsonSchema)]
#[schemars(rename_all = "snake_case")]
pub struct CounterexampleBundleEventsOutputDto {
    pub bundle_id: String,
    pub events_count: usize,
    pub returned_count: usize,
    pub next_offset: Option<usize>,
    pub events: Vec<TraceEvent>,
}
```

## Task 3: MCP tool — `counterexample_bundle_events`

In `crates/chronos-mcp/src/server.rs`:

1. Add `CounterexampleBundleEventsParams` struct (near
   `CounterexampleEventsCountParams`).

2. Add the `#[tool]` handler in the `impl ChronosServer` block
   (near `counterexample_events_count`).

3. Extend `serialize_counterexample_output` with a `COut::Events`
   arm that produces the DTO and wraps it in `session_envelope` for
   degraded-field consistency.

4. Unit test in `mod tests`:
   - Construct a ChronosServer
   - Save a bundle
   - Call the new tool via direct method invocation
   - Assert returned events match what was saved

## Task 4: Verification

- `cargo fmt --all -- --check`: clean
- `cargo clippy --workspace --all-targets -- -D warnings`: clean
- `cargo test --workspace --lib -- --test-threads=1`: passing (1036 +
  N new tests)
- `cargo test -p chronos-mcp --tests --no-fail-fast`: passing
- `cargo test -p chronos-services --tests --no-fail-fast`: passing
  (covers new service-level unit tests)

## Task 5: Release

- Write cycle artifacts: apply-checkpoint.json, verify-report.md,
  verify-findings.json, merge-receipt.md, release-receipt.md,
  release-report.md, implementation-receipt.md.
- Merge --no-ff into main.
- Pre-create tag `v0.7.93` at cycle-artifacts commit, move to merge
  commit per CC#42.
- Write archive-manifest.md + handoff.md.
- Push to origin; delete cycle branch.

**Acceptance**: All artifacts present; tag `v0.7.93` pushed;
`cycles/index.md` updated to Total cycles 91; sandbox smoke tests
pass.
