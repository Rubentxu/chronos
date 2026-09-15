# Release Report — m10-ms-cap-discovery-followup-2

| Field | Value |
|---|---|
| Cycle | `m10-ms-cap-discovery-followup-2` |
| Path | A-min |
| Tag | `v0.7.110` |

## What changed

Refactor `crates/chronos-mcp/src/server.rs` to derive tool list assertions
from the live `#[tool_router]` registration instead of a hand-maintained
`REGISTERED_TOOLS` mirror constant. Eliminates the parallel-list drift
risk for capability discovery.

1. **Promoted `tool_router()` to public** via `vis = "pub"` on the
   `#[rmcp::tool_router]` macro attribute.
2. **Replaced the 61-entry `REGISTERED_TOOLS` mirror** with a router-
   derived assertion that calls `ChronosServer::tool_router().list_all()`.
3. **Rewrote `all_tool_names_count_61`** to assert length equality
   against the live router count (was hardcoded 61).
4. **Added `router_has_expected_minimum_tool_count`** as a tripwire
   against accidental bulk deletion of `#[tool]` registrations.
5. **Updated the `ALL_TOOL_NAMES` doc comment** to flag it as a const
   mirror with the router as the authoritative source.

## Verification

- T0 fmt + clippy: clean
- T2 cargo test: 99/99 chronos-mcp + workspace tests pass
- T4-serial chronos-native: 103/103 pass
- HEAD = origin/main = `91db3ad5` (merge commit)
- Tag v0.7.110 peels to apply commit `e51d9e82` (A-lite convention)

## Carry-forward

None. The cycle closes a single targeted refactor; no follow-up required.
