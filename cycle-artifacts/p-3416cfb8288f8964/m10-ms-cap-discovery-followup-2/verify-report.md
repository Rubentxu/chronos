# Verify Report — m10-ms-cap-discovery-followup-2

> **Path**: A-min, single-file refactor

## Subject

`m10-ms-cap-discovery-followup-2` refactors
`crates/chronos-mcp/src/server.rs` to derive tool list assertions from
the live `#[tool_router]` registration instead of a hand-maintained
`REGISTERED_TOOLS` mirror constant.

## Evidence

- `cargo test -p chronos-mcp --lib all_tool_names -- --nocapture`:
  both `all_tool_names_count_61` and `all_tool_names_matches_router` pass.
- `cargo test -p chronos-mcp --lib --no-fail-fast`: 99/99 pass.
- `cargo test -p chronos-native --lib -- --test-threads=1`: 103/103 pass.
- `cargo test --workspace --lib --tests --exclude chronos-sandbox
  --exclude chronos-e2e --exclude chronos-native --no-fail-fast`: all pass.
- `cargo fmt --all -- --check`: clean.
- `cargo clippy -p chronos-mcp --all-targets -- -D warnings`: clean.

## Results

| Gate | Result |
|---|---|
| Net Rust delta relative to m10-m9-legacy base | 1 file, +71/-97 lines |
| T0 fmt | passed |
| T0 clippy | passed |
| T3 workspace lib + integration tests | passed |
| T4-serial chronos-native | passed |
| T4-smoke sandbox | N/A (no probe/MCP plumbing touched; refactor is internal to chronos-mcp) |
| T5 full sandbox | N/A |
| Manifest fixpoint (CC#4) | N/A (no manifest changes) |
| Vault drift CC sweep | clean (CC#48; CC#54 still exposes pre-existing CC#5 + CC#53) |

## Findings

### Closed

- **FIND-M10-MS-CAP-FU2-ROUTER-INTROSPECTION**: rmcp 1.5's
  `ToolRouter::list_all()` exposes the live tool list derived from
  every `#[tool]` attribute. No more hand-maintained mirrors.
- **FIND-M10-MS-CAP-FU2-REGISTERED-TOOLS-REMOVED**: the 61-entry
  `REGISTERED_TOOLS` mirror constant is deleted (was a maintenance
  liability — any `#[tool]` addition required a parallel edit to this
  list).
- **FIND-M10-MS-CAP-FU2-NEW-SYNC-TEST**: `all_tool_names_matches_router`
  asserts set equality between `ALL_TOOL_NAMES` and the live router.
  Any future drift is now caught at `cargo test` time.

### Carry-forward

None.

## Files Inventory

| Path | Change |
|---|---|
| `crates/chronos-mcp/src/server.rs` | `-97` (REGISTERED_TOOLS + 2 tests deleted), `+71` (router-derived helper + 2 tests + comments + vis = "pub") |

**Total**: 1 file, +71/-97 lines.

## Notes

- Single reviewable commit `e51d9e82`, `--no-ff` merge commit `91db3ad5`
  to main, tag v0.7.110 on the apply commit.
- Apply agent failed for the 5th consecutive cycle on the upstream
  endpoint; orchestrator executed apply directly via inline edits.

None — clean state. (m10-legacy-migration)
