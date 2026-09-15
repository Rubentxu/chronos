# Implementation Receipt — m10-ms-cap-discovery-followup-2

| Field | Value |
|---|---|
| Cycle | `m10-ms-cap-discovery-followup-2` |
| Route | A-min |
| Date | 2026-09-15 |

## Summary

Refactor `crates/chronos-mcp/src/server.rs` to derive tool list assertions
from the live `#[tool_router]` registration instead of a hand-maintained
`REGISTERED_TOOLS` mirror.

## Files Changed (1 file, +71/-97)

### `crates/chronos-mcp/src/server.rs`

1. **Promoted `tool_router()` to public**: `#[rmcp::tool_router]` →
   `#[rmcp::tool_router(vis = "pub")]`.

2. **Updated `pub const ALL_TOOL_NAMES` doc comment** to flag it as a
   const mirror with the router as the authoritative source.

3. **Rewrote `all_tool_names_count_61`** to derive the expected count
   from `ChronosServer::tool_router().list_all().len()` instead of a
   hardcoded 61. Failure message now points to the actual router-vs-const
   length delta.

4. **Replaced the `toolset_sync_check` module**:
   - Deleted the 61-entry `REGISTERED_TOOLS` mirror constant.
   - Deleted `all_tool_names_covers_registrations` and
     `all_tool_names_have_registration` (they depended on the mirror).
   - Added `router_tool_names()` helper that returns the live router
     tool list.
   - Added `all_tool_names_matches_router` test (set equality).
   - Added `router_has_expected_minimum_tool_count` test (≥50 tripwire).

## Tier Results

| Tier | Status | Notes |
|---|---|---|
| T0 fmt | clean | one minor whitespace fix applied |
| T0 clippy | clean | no warnings |
| T2 (cargo test workspace, excluding chronos-sandbox/chronos-e2e/chronos-native) | pass | 99 chronos-mcp tests + workspace tests |
| T4-serial chronos-native | pass | 103/103 |

## Honest Gaps

- Apply agent (MiniMax-M2.7-highspeed) failed for the 5th consecutive
  cycle on the upstream endpoint. Orchestrator executed apply directly
  via inline edits + `git commit` + `git tag`. Recommend switching the
  apply model or handling A-min cycles directly without an apply agent.
- The `pub const ALL_TOOL_NAMES` const is now a hand-maintained mirror
  (intentionally — kept for `&'static` semantics in
  `build_tool_availability`). The drift risk is fully eliminated by the
  new `all_tool_names_matches_router` test, which catches any future
  divergence.
