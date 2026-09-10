# M6-05 — `session_export` net-new v2 tool

**Status:** PROPOSED
**Branch:** `feat/m6-05-session-export`
**Milestone:** M6 (v2-spec surface reduction)
**Close-report ref:** §4.2 item 1, item 5 (line 143 + 164)

## Intent

Add a `session_export` v2 tool to the chronos agent API surface so that an
external agent (or test harness) can materialize a full session bundle
**metadata + trace events + properties snapshot** to a portable file on disk.
This is the second of two net-new capabilities identified in the M5 close
report (§4.2): it has no v1 analogue, so the cycle ships a brand-new
dispatcher + DTOs + MCP wrapper without touching any deprecation path.

## Scope

**In scope (this cycle):**
- New dispatcher module `chronos_services::session_export` (~16th service
  module), zero-impact on existing 16 services.
- New DTOs in `chronos_services::output`:
  - `ExportFormat` (enum: `Json`, `OtlpJson`)
  - `ExportBundle` (the payload shape, also re-usable as a JSON value)
  - `ExportPropertiesEntry` (name + `PropertyValue`)
  - `ExportResult { path, bytes_written, format }`
- New `SessionExportContext` (`&'a Mutex<HashMap<String, QueryEngine>>`,
  no inner `Arc` — same pattern established in m6-03/m6-04).
- New MCP wrapper `session_export` + `SessionExportParams` in
  `chronos-mcp/src/server.rs`.
- Atomic file write (tmp path + rename) — guarantees no partial files if
  the write fails midway.

**Out of scope (deferred):**
- `ExportFormat::ZipJson` — would require pulling in `zip` crate as a
  dependency, plus packaging metadata+events+properties+manifest into a
  single `.zip`. Reserved for m7+ if user demand emerges. The DTO is a
  plain enum so it can be extended later without breaking callers.
- OTLP full proto compatibility (gRPC + protobuf) — out of scope by
  design; we ship **OtlpJson** which is the JSON wire-format the OpenTelemetry
  collectors already accept (a list of `ResourceSpans` with one
  `ScopeSpans` containing one `Span` per event). This is enough for an
  agent to feed traces into Jaeger/Tempo/Honeycomb via their JSON
  receivers without committing to protobuf dependencies.

## Shapes

### `ExportFormat` (enum, JSON-friendly string)

```
Json       →  { "format": "json" }
OtlpJson   →  { "format": "otlp_json" }
```

Used as both a typed enum (services layer) and a JSON-friendly string
(MCP wrapper) — same pattern established by `HypothesisKind` in m6-04.

### `ExportBundle` (the payload)

```
ExportBundle {
  schema_version: "v2-export.1",
  metadata: SessionMetadata,
  events: Vec<TraceEvent>,
  properties_snapshot: Vec<ExportPropertiesEntry>,
}
```

- `schema_version` is a literal string so future migrations can be
  detected by consumers.
- `metadata` reuses `SessionStore::SessionMetadata` directly (already
  `Serialize + Deserialize`).
- `events` is `Vec<TraceEvent>` re-using the domain's trace event type.
- `properties_snapshot` is the property table as of export time, taken
  from the in-memory engine's `evaluate_all` view (same shape used by
  `execution_query` and `analytics` dispatchers).

### `ExportPropertiesEntry`

```
ExportPropertiesEntry {
  name: String,
  value: PropertyValue,
}
```

A pair of name + value, NOT a map, so that order is preserved in the
JSON output (matters when humans diff two exports).

### `ExportResult`

```
ExportResult {
  path: PathBuf,
  bytes_written: u64,
  format: ExportFormat,
}
```

## Dispatcher

`ChronosSessionExportService::export(session_id, format, output_path, ctx)`
returns `Result<ExportResult, ServiceError>`.

Flow:
1. Lock engines, fetch the engine for `session_id`. Error:
   `SessionNotInMemory`.
2. Pull `engine.get_all_events()`. Error: `EmptySession` if 0 events.
3. Compute `properties_snapshot = engine.evaluate_all()` (best-effort,
   empty vec on failure — properties are advisory, not load-bearing).
4. Build `metadata` (same fields as `SessionsService::save_session`).
5. Build `ExportBundle`.
6. Serialize the bundle into the requested format:
   - `Json` → `serde_json::to_vec_pretty` of `ExportBundle`.
   - `OtlpJson` → map `ExportBundle` events to OTLP JSON shape
     (`{"resourceSpans":[{...}]}`), `metadata` becomes `resource.attributes`,
     `properties_snapshot` becomes `scope.attributes`.
7. Atomic write: write to `<output_path>.tmp.<pid>`, fsync, rename to
   `output_path`. Error: `ExportFailed` carrying the underlying io error.

## MCP wrapper

```
tool session_export {
  params SessionExportParams {
    session_id: String,
    format: String,           // "json" | "otlp_json"
    output_path: String,      // absolute or relative to MCP server CWD
  }
  result ExportResult
}
```

`SessionExportParams` uses the same `#[derive(JsonSchema)]` pattern as
`HypothesisTestParams` and the rest of the v2 wrappers. The `format` is
validated via a small `parse_export_format` helper that maps strings to
the typed enum.

## Error handling

Reuse existing `ServiceError` variants where possible:

| Failure | Variant |
|---|---|
| session_id not in engines | `SessionNotInMemory` (already exists) |
| 0 events | `EmptySession` (already exists) |
| format string unparseable | `InvalidParameter` (already exists) |
| io error during write | `ExportFailed(String)` — **new variant** |
| path not absolute / parent missing | `ExportFailed(String)` |

`ExportFailed` is the only new variant added in this cycle.

## Tests

`crates/chronos-services/src/session_export.rs` ships with **13 unit
tests** mirroring the coverage matrix used in m6-04 (`hypothesis_test`):

1. happy path: Json format
2. happy path: OtlpJson format
3. session not in memory
4. empty session (0 events)
5. properties snapshot populated (3 properties)
6. properties snapshot empty on engine without `evaluate_all`
7. atomic write fails when output_path parent does not exist
8. atomic write succeeds (tmp file removed after rename)
9. schema_version is "v2-export.1" in Json output
10. schema_version is "v2-export.1" in OtlpJson output (also appears in
    `resource.attributes["schema.version"]`)
11. metadata round-trip (write → read → metadata fields equal)
12. events round-trip (write → read → events count equal, first/last
    timestamps equal)
13. bytes_written > 0 for non-empty session

Fixtures reuse `chronos_index::builder::IndexBuilder` (same pattern as
`hypothesis_test`).

## Out of path / not affected

- `chronos-store::SessionStore::save_session` — unchanged. Export is
  read-only with respect to the store.
- v1 dispatcher list — unchanged. `session_export` is net-new.
- `chronos-native::capture_runner`, `ptrace_tracer` — excluded from
  validation per AGENTS.md §6.5.

## Success criteria

- All tier gates pass: T0 (fmt + clippy), T1 (lib unit, 73/73 mcp +
  143→≥156 services), T2 (mcp 11/11 + services 2/2), T3 (workspace
  --lib stable ≥ 244 across 25 runs), T4-smoke (`e2e_connectivity` 1/1
  confirms server starts and the new tool registers without panicking).
- Post-merge 5-run sanity: workspace --lib stable ≥ 244, fails = 0.
- FF-merged to `main` with cycle receipt.
- `apply-checkpoint.json` updated with cycle entry + new head_sha.
