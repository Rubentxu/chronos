# Change: m10-ms-evt-typed — typed event filter

## Subject

- **Cycle**: m10-ms-evt-typed
- **Path**: B-direct (T0+T2+T4-smoke)
- **Base SHA**: `9078ec3136faf07704ff4845d29a901348744988`
- **Head SHA**: `0998aa734541e6675ed1645745e2b26060a8a645`
- **Status**: completed

## Files changed

- `crates/chronos-domain/src/trace/event.rs`: + `EventType::from_snake_case` (21 variants, single owner of the string->enum mapping).
- `crates/chronos-mcp/src/server.rs`: `EventsReadParams.event_types` flipped to typed `Vec<EventType>`; `parse_event_type` deleted; v1 shims delegate to the domain mapping; 21-variant round-trip test.

## Evidence

ADR-0003 acceptance met: `grep parse_event_type crates/chronos-mcp/src/server.rs` returns 0; unknown names on the v2 wire are rejected at JSON-RPC parse time (typed schema error, no silent lies); v1 shims keep string params but now accept all 21 variants.

## Cross-check

- `cargo fmt --all -- --check`: passed.
- `cargo clippy -p chronos-mcp -p chronos-domain --all-targets -- -D warnings`: passed.
- T2: mcp lib 88 ok, domain lib 153 ok.
- T4-smoke: e2e_connectivity 1/1, analytics_tools 4/4.
