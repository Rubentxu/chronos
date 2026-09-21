# Design: REC-C5 C5.3 — Delete 22 deprecated alias handlers (split into C5.3.1 + C5.3.2)

## Technical Approach

Two-pass delete. C5.3.1 retargets the 22 v1 sandbox client wrappers at the v2
dispatchers (no observable behavior change). C5.3.2 then deletes the 22 alias
handlers and the two legacy service shims (`diff.rs:246` literal, `events_read.rs:33`
offset/limit comment). Both passes land on the same branch; tier gates run only
between passes and at the end.

## Why two passes (not one)

`chronos-mcp/src/server.rs` exposes 22 alias handlers (`#[tool] fn
query_events`, `get_event`, `get_call_stack`, …, `forensic_memory_audit`,
`tripwire_*`, `probe_inject`). The C5.3 working tree deletes them in one shot.
But `chronos-sandbox/src/client/tools.rs` re-exposes the same 22 names as
wrapper methods used by 14 sandbox test files (23 call sites). C5.2 explicitly
deferred those wrappers ("belong to a dedicated slice"). One commit breaks
the workspace end-to-end: `cargo check --workspace --all-targets` fails on
`chronos-mcp/tests/sessions_tools.rs:6637` (`server.forensic_memory_audit()`)
and the 14 affected sandbox tests can no longer link.

C5.3.1 makes the wrappers v2 internally. C5.3.2 then deletes the alias
handlers — by then no caller in the workspace references them.

## Architecture Decisions

### Decision: Wrapper-first, then delete

| Choice | Migrate the 22 client wrappers to call v2 dispatchers, THEN delete the
22 server handlers.
| Alternatives | (a) Keep alias handlers as thin passthroughs (rejected: the goal
of REC-C5 is to shrink MCP surface to 12 + 29 = 41, was 63). (b) Delete first,
migrate tests in a follow-up cycle (rejected: leaves the workspace red for
hours and erodes the baseline invariant that `cargo check --workspace` is
green at every commit).
| Rationale | C5.3.1 is non-observable; C5.3.2 is mechanical delete; both gates
pass cleanly in between. Matches the C5.2 commit message's promise ("belong
to a dedicated slice"). No `#[deprecated]` marker is needed because the
wrappers are sandbox-private (not a published API).

### Decision: Migration table from the C5.1 sunset policy

Source: `docs/specs/AGENT_API_V2.md` (added in C5.1, commit `f9fa14b1`). The
v2 dispatchers are: `events_read`, `state_query`, `execution_query`,
`trace_slice`, `session_compare`, `observe`, `session_export`. Wrapper
rewrites forward to one of these. Example pairings used by the migration:

- `query_events` → `events_read(mode=Query, filter, cursor, limit)`
- `get_event`    → `events_read(mode=ById, session_id, event_id)`
- `state_diff`   → `state_query(kind=RegisterDiff, session_id, ts_a, ts_b)`
- `evaluate_expression` → `state_query(kind=ExpressionEval, ...)`
- `get_call_stack`/`get_execution_summary`/`debug_call_graph`/`debug_detect_races`/`debug_expand_hotspot`/`debug_get_saliency_scores` → `execution_query(kind=...)`
- `inspect_causality`/`debug_find_crash`/`debug_find_variable_origin`/`forensic_memory_audit` → `trace_slice(kind=...)`
- `debug_get_memory`/`debug_get_registers`/`debug_analyze_memory` → `state_query(kind=...)`
- `tripwire_create/list/delete/query` and `probe_inject` → `observe(verb=...)` (C5.2 already used this pattern in `m0_acceptance.rs` and `probe_drain_canonical.rs`).

### Decision: ALL_TOOL_NAMES length

| Choice | After C5.3 the const must enumerate exactly 12 v2 + 29 not-yet-converged = 41 tools (was 63).
| Alternatives | Hardcode `41` (rejected: brittle). Compute at runtime from the router (rejected: the test in `toolset_sync_check` already asserts the constant format).
| Rationale | Hand-edit the const to the 41-entry list; the
`toolset_sync_check` test will fail if the constant and router drift.

## Data Flow

```
sandbox test          sandbox client             chronos-mcp             chronos-services
─────────────         ────────────────           ─────────────           ────────────────
client.query_events() wrapper.query_events() ──→ events_read handler ──→ EventsReadService
client.state_diff()   wrapper.state_diff()   ──→ state_query handler  ──→ StateQueryService
... (22 wrappers)     ...                       ...                     ...
```

After C5.3.2: the 22 wrapper bodies are unchanged (still call
`call_tool(<v2_name>, …)`), but the v1 handlers behind them are gone. The
wiring above is preserved; only the first server-level hop is renamed.

## File Changes

| File | Action | Description |
|---|---|---|
| `chronos-sandbox/src/client/tools.rs` | Modify | 22 v1 wrappers retargeted to v2 params. No public-API rename (tests stay untouched). |
| `chronos-mcp/src/server.rs` | Modify | C5.3.2: delete 22 alias `#[tool] fn`s and ALL_TOOL_NAMES rows; update `ALL_TOOL_NAMES` to 41 entries. |
| `chronos-services/src/diff.rs` | Modify | C5.3.2: drop the literal-error reconstruct hint comment (already partially done in working tree). |
| `chronos-services/src/events_read.rs` | Modify | C5.3.2: drop "Offset/limit compatibility" comment, rename to "Pagination" (already in working tree). |
| `chronos-mcp/tests/sessions_tools.rs` | Modify | C5.3.2: delete the `server.forensic_memory_audit(...)` test or retarget it. |
| `chronos-mcp/tests/toolset_sync_check.rs` (impl) | Verify | Pass after C5.3.2 (41 entries in `ALL_TOOL_NAMES` matches 41 `#[tool]`s). |

## Interfaces / Contracts

The sandbox client surface (`McpTestClient`) does NOT change. All 22 method
names are preserved. Only their *bodies* are rewritten to invoke v2 tool
names. Public test files do not need editing.

The MCP wire surface shrinks from 63 → 41 tool names. Sandbox tests that
called raw `call_tool("query_events", …)` were already migrated in C5.2.

## Testing Strategy

| Layer | What | How |
|---|---|---|
| Unit (T1) | `chronos-mcp --lib`, `chronos-services --lib` | `cargo test -p chronos-mcp --lib -p chronos-services --lib --no-fail-fast` |
| Lib toolset invariant (T1) | ALL_TOOL_NAMES = 41 entries, every entry has a router `#[tool]` | `toolset_sync_check` test inside `chronos-mcp` |
| Sandbox smoke (T4-smoke) | Real MCP server boots and answers v2 names; client wrappers reach v2 dispatchers | `cargo build --bin chronos-mcp` then `cargo test -p chronos-sandbox --test m0_acceptance --test concurrency_stress --test e2e_connectivity --test analytics_tools -- --test-threads=1` (subset chosen per AGENTS.md §2: covers start, drain, stop, analytics; m0_acceptance and concurrency_stress were the C5.2 migration targets). |
| Sandbox subset (T4-smoke extended) | Aliases are actually gone | A new test `chronos-mcp/tests/alias_deletion.rs` asserts that calling `query_events` via raw RPC returns `method_not_found`. |

## Migration / Rollout

Two commits on `feat/rec-c5-api-convergence`:

1. `feat(sandbox): migrate McpTestClient v1 wrappers to v2 dispatchers (REC-C5-C5.3.1)`
   - 22 wrapper body rewrites in `tools.rs`.
   - Tier gates: T0 + T2 (`chronos-mcp`, `chronos-services`, `chronos-sandbox`) + T4-smoke (`m0_acceptance`, `concurrency_stress`, `e2e_connectivity`, `analytics_tools`).
2. `delete(mcp,services): remove 22 deprecated alias handlers + legacy shims (REC-C5-C5.3.2)`
   - `server.rs`: 22 alias `#[tool] fn` removed; `ALL_TOOL_NAMES` becomes the 41-entry v2 list.
   - `diff.rs`: legacy-error literal comment removed (already partially in working tree).
   - `events_read.rs`: pagination comment (already partially in working tree).
   - `chronos-mcp/tests/sessions_tools.rs`: retarget the `forensic_memory_audit` test.
   - New `chronos-mcp/tests/alias_deletion.rs` asserts the alias RPCs return method-not-found.
   - Tier gates: T0 + T2 + T4-smoke + T3-no-sandbox (per AGENTS.md §2 A-lite).

No version bump (REC-C5 closes without a release tag — REC-C5 is a gate, not
a shipped deliverable; the next `v0.x.y` tag will land with REC-C7).

## ADR Candidates

None. The wrapper-first order is a sequencing choice, not an architectural
decision. ALL_TOOL_NAMES length is a test invariant, not an ADR.

## Out of scope

- The 29 not-yet-converged working tools (probe_*, browser_probe_*,
  counterexample_*, sessions CRUD, mutation_lens, causal_slice,
  compare_sessions). Per the REC-C5 proposal, they stay.
- ChronosAgent / daemon-level wire changes. The MCP wire is the only surface
  affected by REC-C5.
