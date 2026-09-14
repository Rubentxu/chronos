# Exploration Report — m9-93: TraceEvent JsonSchema derive

## Context

m9-91 (counterexample_bundle_events MCP tool, A-min, v0.7.93) opened
FIND-M9-91-TRACE-EVENT-NO-JSON-SCHEMA. The wire DTO
`CounterexampleBundleEventsOutputDto` exposes `Vec<TraceEvent>` but
`TraceEvent` doesn't implement `schemars::JsonSchema`. m9-91 applied
a pragmatic fix:

```rust
pub struct CounterexampleBundleEventsOutputDto {
    pub bundle_id: String,
    pub events_count: usize,
    #[schemars(skip)]   // ← m9-91 workaround
    pub returned_events: Vec<chronos_domain::TraceEvent>,
    pub next_offset: Option<usize>,
}
```

m9-92 handoff recommended m9-93 close this finding.

## Type graph

To add `JsonSchema` to `TraceEvent`, the following types must also
derive `JsonSchema` (compile error otherwise). All are in
`crates/chronos-domain/src/`:

| Type | File | Notes |
|---|---|---|
| `EventType` | trace/event.rs:19 | enum, ~22 variants |
| `Language` | trace/session.rs:9 | enum, 14 variants |
| `VariableScope` | value/typed.rs:8 | enum, ~5 variants |
| `VariableInfo` | value/typed.rs:31 | struct, ~10 fields (incl. `Vec<u8>` for raw_bytes) |
| `SourceLocation` | trace/location.rs:6 | struct, 5 Option fields |
| `SymbolId` | trace/event.rs:123 | struct with Language field |
| `InvocationId` | trace/event.rs:152 | tuple struct wrapping `Uuid` |
| `RegisterState` | trace/event.rs:419 | struct, 18 u64 fields |
| `WasmModuleInfo` | trace/event.rs:442 | struct with `Vec<WasmFunctionInfo>` |
| `WasmFunctionInfo` | trace/event.rs:457 | struct |
| `EventData` | trace/event.rs:231 | enum, 14 variants (incl. nested enum kinds) |
| `TraceEvent` | trace/event.rs:472 | struct, leaf target |

All member types use only primitives (`u64`, `String`, `Option<T>`,
`Vec<T>`, enums) that schemars 1.x auto-derives. No recursive
cycles. The `Uuid` in `InvocationId` is the only external dep.

## Uuid feature dependency

`uuid` v1.x provides `JsonSchema for Uuid` under the `jsonschema`
feature (per uuid docs). Currently the workspace uuid dep is:

```toml
uuid = { version = "1", features = ["v4", "v7", "serde"] }
```

Need to add `jsonschema`. This adds zero runtime cost (only impl
methods).

**Alternative**: write a manual `impl JsonSchema for InvocationId` that
delegates to Uuid's `json_schema(generator)`. More code; not needed.

**Decision**: enable the feature flag.

## Type-name → schema-name strategy

schemars derives `schema_name` from the Rust type name by default.
`VariableInfo` → `"VariableInfo"`, `TraceEvent` → `"TraceEvent"`, etc.
The MCP server publishes schemas under these names. Clients introspect
by `definitions.VariableInfo.properties.name` etc.

No renaming needed.

## Cargo build implications

Adding JsonSchema to 12 types in `chronos-domain` will trigger full
recompilation of:
- `chronos-domain` lib
- `chronos-services` (depends on chronos-domain)
- `chronos-mcp` (depends on chronos-services)
- `chronos-store` (depends on chronos-domain)
- `chronos-sandbox` (depends on chronos-mcp)
- `chronos-e2e` (depends on chronos-mcp)
- `chronos-sdd` (depends on chronos-services)
- `chronos-cli` / `chronos-bench` / etc.

Expected build time: 30-90 seconds (incremental). No `proc-macro`
changes elsewhere; only the derive attributes.

## Test coverage

Three new tests:
1. `chronos_domain::trace::event::tests::trace_event_implements_json_schema`
2. `chronos_domain::trace::event::tests::event_data_schema_is_oneof`
3. `chronos_services::output::bundle_events_schema_tests::dto_schema_includes_returned_events`

All existing tests should continue to pass — JsonSchema derive does
not change `Debug`/`Clone`/`PartialEq`/`Serialize`/`Deserialize`
behavior. The only "new" behavior is the `JsonSchema` impl itself.

## Risk analysis

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Compile error from JsonSchema derive on a type with an unsupported field | low | medium | Add `#[schemars(skip)]` to the specific field (not the whole type); document |
| `Uuid` jsonschema feature conflicts with another feature | very low | low | Check `cargo metadata` after feature add; revert if conflict |
| Generated schema leaks internal types (e.g., SymbolId) | medium | low | Use `#[schemars(rename = "...")]` if names are misleading; defer |
| CounterexampleBundleEventsOutputDto JSON Schema changes (MCP clients consuming old schema might break) | medium | medium | Document in release notes; the change is additive (a previously-skipped field is now visible) |

## Recommendation

Proceed with m9-93 as A-min. Single commit per phase (T1.1, T2.7, T3.2)
keeps the change reviewable.

## Out-of-scope (deferred to m10+)

- `CounterexampleBundleEventsOutputDto` field additions (e.g., `from_offset`, `to_offset`).
- Schema docs site auto-generation.
- `EventData` variant additions (e.g., `WasmStore`, `ErlangFrame`).
- JSON Schema `$ref` deduplication across `oneOf` (schemars 1.x emits
  inline schemas by default; would require `#[schemars(deny_unknown_fields)]`
  or similar to share).
