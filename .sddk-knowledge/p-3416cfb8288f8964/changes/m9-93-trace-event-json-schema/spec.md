# Spec — m9-93: TraceEvent JsonSchema derive

## ADDED Requirements

### REQ-m9-93-1: TraceEvent must implement `schemars::JsonSchema`

`chronos_domain::TraceEvent` (and all types reachable through its
field graph) must derive `schemars::JsonSchema`. After this derive,
`schema_for!(TraceEvent)` must produce a non-null `Schema` object
exposing the `event_id`, `timestamp_ns`, `thread_id`, `event_type`,
`location`, and `data` fields with their respective subschemas.

**Rationale**: m9-91 introduced `CounterexampleBundleEventsOutputDto`
with `Vec<TraceEvent>` field. The `schemars::skip` attribute on that
field hides the event stream from MCP clients' introspection. Dropping
the skip requires TraceEvent to implement JsonSchema.

**Acceptance criterion**: A unit test compiles and runs:
```rust
let _schema = schemars::schema_for!(TraceEvent);
```
without compile error.

### REQ-m9-93-2: EventData must implement `schemars::JsonSchema`

`chronos_domain::EventData` enum (14 variants) must derive
`schemars::JsonSchema`. The generated schema must represent the
enum as a `oneOf` (the schemars default for enums with data).

**Acceptance criterion**:
```rust
let schema = schemars::schema_for!(EventData);
// schema must have a "oneOf" or "anyOf" subschema enumerating the variants
```

### REQ-m9-93-3: Transitive dependencies must implement JsonSchema

The following types must all derive `JsonSchema` (required for
TraceEvent to derive):

| Type | Location |
|---|---|
| `EventType` | `crates/chronos-domain/src/trace/event.rs` |
| `Language` | `crates/chronos-domain/src/trace/session.rs` |
| `VariableScope` | `crates/chronos-domain/src/value/typed.rs` |
| `VariableInfo` | `crates/chronos-domain/src/value/typed.rs` |
| `SourceLocation` | `crates/chronos-domain/src/trace/location.rs` |
| `SymbolId` | `crates/chronos-domain/src/trace/event.rs` |
| `InvocationId` | `crates/chronos-domain/src/trace/event.rs` |
| `RegisterState` | `crates/chronos-domain/src/trace/event.rs` |
| `WasmModuleInfo` | `crates/chronos-domain/src/trace/event.rs` |
| `WasmFunctionInfo` | `crates/chronos-domain/src/trace/event.rs` |

**Rationale**: schemars derive requires all field types to implement
JsonSchema. The compile error is the obvious feedback mechanism.

### REQ-m9-93-4: Uuid feature `jsonschema` must be enabled

The workspace `uuid` dependency must include the `jsonschema` feature
to enable `JsonSchema` impl for `uuid::Uuid`, which `InvocationId`
wraps.

**Rationale**: `InvocationId(pub Uuid)` needs Uuid's JsonSchema impl.
The `uuid` crate ships this under the `jsonschema` feature flag.

**Acceptance criterion**: `Cargo.toml` workspace dependency reads:
```toml
uuid = { version = "1", features = ["v4", "v7", "serde", "jsonschema"] }
```

### REQ-m9-93-5: CounterexampleBundleEventsOutputDto must expose returned_events

The `#[schemars(skip)]` attribute on
`CounterexampleBundleEventsOutputDto::returned_events` (introduced
pragmatically in m9-91) must be removed. The field must appear in the
generated JSON Schema.

**Rationale**: After REQ-m9-93-1..4 land, the skip is no longer
necessary and unnecessarily hides the event stream from MCP clients.

**Acceptance criterion**: Unit test confirms
`schema_for!(CounterexampleBundleEventsOutputDto)` includes a
`returned_events` property (not skipped).

## MODIFIED Requirements

None.

## REMOVED Requirements

None.

## Scenarios

### Scenario S1: TraceEvent schema_for! compiles

**Given**: chronos-domain built with m9-93 changes
**When**: A test calls `schemars::schema_for!(TraceEvent)`
**Then**: Compilation succeeds and the resulting `Schema` exposes
properties for `event_id`, `timestamp_ns`, `thread_id`, `event_type`,
`location`, and `data`.

### Scenario S2: EventData schema is a oneOf

**Given**: chronos-domain built with m9-93 changes
**When**: A test introspects `schema_for!(EventData)`
**Then**: The top-level schema contains a `oneOf` (or `anyOf`) key
with at least 10 subschemas (one per EventData variant).

### Scenario S3: CounterexampleBundleEventsOutputDto schema includes returned_events

**Given**: chronos-services built with m9-93 changes
**When**: A test introspects `schema_for!(CounterexampleBundleEventsOutputDto)`
**Then**: The schema's `properties` map contains a `returned_events`
key whose schema is an `array` of `TraceEvent` objects.

### Scenario S4: No regression in existing tests

**Given**: chronos-domain + chronos-services built with m9-93 changes
**When**: `cargo test --workspace --lib -- --test-threads=1` runs
**Then**: All existing tests pass with no failures attributable to
the schema derives (serde roundtrip tests, m9-91 bundle_events tests,
mcp-server tests).

## Out-of-scope

- Schema docs site publishing (separate cycle).
- EventData variant additions.
- CounterexampleBundleEventsOutputDto field additions.

## Acceptance criteria (summary)

1. `cargo fmt --all -- --check` clean.
2. `cargo clippy --workspace --all-targets -- -D warnings` clean.
3. `cargo test --workspace --lib -- --test-threads=1` passes.
4. New unit tests `trace_event_implements_json_schema` +
   `counterexample_bundle_events_dto_schema_includes_returned_events`
   added and passing.
5. `CounterexampleBundleEventsOutputDto` no longer uses
   `#[schemars(skip)]` on `returned_events`.
