# Proposal — m9-93: TraceEvent JsonSchema derive

## Identification

| Field | Value |
|---|---|
| Cycle ID | `m9-93-trace-event-json-schema` |
| Workspace | `p-3416cfb8288f8964` |
| Path | A-min (single crate, schemars-schema-on-existing-types refactor) |
| Status | proposed |
| Branch | `feat/m9-93-trace-event-json-schema` |
| Tag | `v0.7.95` |

## Subject

Close FIND-M9-91-TRACE-EVENT-NO-JSON-SCHEMA. Add `schemars::JsonSchema`
derive to `chronos_domain::TraceEvent` + transitive dependency chain
(EventData, RegisterState, WasmModuleInfo, WasmFunctionInfo, SymbolId,
InvocationId, SourceLocation, VariableInfo, Language, VariableScope,
EventType). Then drop the `#[schemars(skip)]` on
`CounterexampleBundleEventsOutputDto::returned_events` introduced
pragmatically in m9-91.

## Problem

In m9-91 (counterexample_bundle_events MCP tool) the wire DTO
`CounterexampleBundleEventsOutputDto` exposed `Vec<TraceEvent>` as a
field, but `TraceEvent` does not implement `schemars::JsonSchema`
because `EventData`'s ~14 variants contain nested types that lack
the derive. Pragmatic fix in m9-91:

```rust
pub struct CounterexampleBundleEventsOutputDto {
    pub bundle_id: String,
    pub events_count: usize,
    #[schemars(skip)]                              // ← m9-91 pragmatic fix
    pub returned_events: Vec<chronos_domain::TraceEvent>,
    pub next_offset: Option<usize>,
}
```

This excludes `returned_events` from the JSON Schema published by the
MCP server's `tool_input_schema`. Clients cannot introspect the
shape of the event stream. Documented as
FIND-M9-91-TRACE-EVENT-NO-JSON-SCHEMA in the m9-91 handoff.

## Approach

A-min: single-crate refactor in `chronos-domain` + 1-line cleanup in
`chronos-services::output::CounterexampleBundleEventsOutputDto`. No
architectural change, no new domain concept, no probe/mcp touched.

### Types to add `JsonSchema` to (transitive chain)

| Type | File | Reason |
|---|---|---|
| `EventType` | trace/event.rs | TraceEvent field |
| `Language` | trace/session.rs | SymbolId field |
| `VariableScope` | value/typed.rs | VariableInfo field |
| `VariableInfo` | value/typed.rs | TraceEvent::data nested (Vec<VariableInfo>) |
| `SourceLocation` | trace/location.rs | TraceEvent field |
| `SymbolId` | trace/event.rs | EventData::Function field |
| `InvocationId` | trace/event.rs | EventData::Function field; wraps Uuid |
| `RegisterState` | trace/event.rs | EventData::Registers |
| `WasmModuleInfo` | trace/event.rs | (only used in code as-is; not in EventData but consistency) |
| `WasmFunctionInfo` | trace/event.rs | WasmModuleInfo field |
| `EventData` | trace/event.rs | TraceEvent::data field |
| `TraceEvent` | trace/event.rs | leaf target |

12 types total. All have only primitive fields (u64, String, Option<T>,
Vec<T>, enum variants) that auto-derive under `schemars 1.x`.

### Uuid (in InvocationId)

`InvocationId(pub Uuid)` requires `uuid` to expose a `JsonSchema` impl.
The `uuid` crate provides this under the `jsonschema` feature. Two
options:

1. Add `jsonschema` feature to workspace uuid dep. **Recommended.**
2. Manual `impl JsonSchema for InvocationId` that delegates to Uuid.

Option 1 is preferred (less code, idiomatic). The `jsonschema` feature
adds no runtime cost (only a `schema_name()` and `json_schema(generator)`
method).

### Cleanup in chronos-services

After JsonSchema lands on `TraceEvent`, drop `#[schemars(skip)]` from
`CounterexampleBundleEventsOutputDto::returned_events`. The field
will appear in the published schema with full introspection
(EventData variants visible as `oneOf`).

## Out-of-scope

- Schema docs generation (m9-93 just adds the derive; the docs site
  publishing path is a separate cycle).
- Tracing any new variants (no EventData variants added).
- `#[serde(...)]` attribute changes (no wire-shape changes).

## Files changed

| Bucket | Files | Notes |
|---|---|---|
| `Cargo.toml` (workspace) | 1 modified | +1 uuid feature (`jsonschema`) |
| `crates/chronos-domain/src/trace/event.rs` | 1 modified | +12 derives on EventType, EventData, RegisterState, WasmModuleInfo, WasmFunctionInfo, TraceEvent, SymbolId, InvocationId |
| `crates/chronos-domain/src/trace/session.rs` | 1 modified | +1 derive on Language |
| `crates/chronos-domain/src/trace/location.rs` | 1 modified | +1 derive on SourceLocation |
| `crates/chronos-domain/src/value/typed.rs` | 1 modified | +2 derives on VariableScope, VariableInfo |
| `crates/chronos-services/src/output.rs` | 1 modified | -1 attribute on `CounterexampleBundleEventsOutputDto::returned_events` |

**Total**: 6 files, ~20 lines (mostly derive attributes + 1 cargo
feature flag).

## Tier

A-min: T0 + T2.

- **T0**: `cargo fmt --all -- --check` + `cargo clippy --workspace
  --all-targets -- -D warnings`.
- **T2**: `cargo test --workspace --lib` (chronos-domain unit tests
  + chronos-services unit tests for `CounterexampleBundleEvents*`).
- **T4-smoke**: not strictly required for schemars-only change, but
  a `counterexample_tools.rs` smoke test would confirm the MCP
  server's published schema now includes `returned_events`. Recommend
  1 smoke test in T4-smoke subset.

## Verification

- New unit test in `chronos-domain`: `trace_event_implements_json_schema`
  (asserts `schema_for!(TraceEvent)` produces a non-null schema with
  `properties.data` showing `oneOf` for the EventData variants).
- New unit test in `chronos-services::output`: `counterexample_bundle_events_dto_schema_includes_returned_events`
  (asserts `schema_for!(CounterexampleBundleEventsOutputDto)` includes
  `returned_events` field).
- Existing m9-91 tests must continue to pass (no behavior regression).
