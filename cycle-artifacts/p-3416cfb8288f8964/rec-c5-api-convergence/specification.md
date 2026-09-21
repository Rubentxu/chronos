# Specification: REC-C5 C5.3 — Delete 22 deprecated alias handlers

## Goal

REC-C5 closes with **41 MCP tools exposed** (12 canonical v2 + 29
not-yet-converged). The 22 deprecated alias handlers are deleted. Sandbox
tests keep working because their McpTestClient wrappers (which still
carry the v1 names) now forward to v2 dispatchers internally (C5.3.1).

## Scope

Two sub-cycles, both on `feat/rec-c5-api-convergence`:

- **C5.3.1** — Rewrite the 22 v1 wrappers in
  `chronos-sandbox/src/client/tools.rs` to call v2 dispatchers. No public
  API change.
- **C5.3.2** — Delete the 22 `#[tool(name = "<v1_alias>")]` handlers and
  the corresponding `ALL_TOOL_NAMES` rows in
  `crates/chronos-mcp/src/server.rs`. Drop the legacy shims in
  `crates/chronos-services/src/diff.rs` and
  `crates/chronos-services/src/events_read.rs`. Add a new
  `chronos-mcp/tests/alias_deletion.rs` test asserting every deleted alias
  returns `method_not_found` over raw RPC.

## Acceptance criteria

### C5.3.1

| ID | Criterion | Verification |
|---|---|---|
| AC-31-1 | All 22 wrappers in `chronos-sandbox/src/client/tools.rs` no longer call `call_tool("<v1_alias>", …)` for any of the 22 deprecated names. | `grep -E 'call_tool\("(query_events|get_event\|get_call_stack\|get_execution_summary\|debug_call_graph\|debug_detect_races\|debug_expand_hotspot\|debug_get_saliency_scores\|debug_find_variable_origin\|debug_find_crash\|inspect_causality\|forensic_memory_audit\|state_diff\|evaluate_expression\|debug_get_memory\|debug_get_registers\|debug_analyze_memory\|tripwire_create\|tripwire_list\|tripwire_delete\|tripwire_query\|probe_inject)"' chronos-sandbox/src/client/tools.rs` returns zero matches. |
| AC-31-2 | Public API of `McpTestClient` is byte-for-byte identical (method names, parameter lists, return types). | `cargo doc -p chronos-sandbox --no-deps` produces no warnings; `git diff` shows only the 22 wrapper bodies changed. |
| AC-31-3 | `toolset_sync_check` test in `chronos-mcp` still passes (C5.3.1 does NOT touch server.rs). | `cargo test -p chronos-mcp --lib -- toolset_sync_check` exits 0. |
| AC-31-4 | T0 lint passes. | `cargo fmt --all -- --check` + `cargo clippy -p chronos-sandbox --all-targets -- -D warnings` exit 0. |
| AC-31-5 | T2 (sandbox lib) passes. | `cargo test -p chronos-sandbox --lib --no-fail-fast` reports `0 failed`. |
| AC-31-6 | T4-smoke subset passes. | `cargo build --bin chronos-mcp` then `cargo test -p chronos-sandbox --test m0_acceptance --test concurrency_stress --test e2e_connectivity --test analytics_tools -- --test-threads=1` reports `0 failed` for all 4 files (subset chosen per AGENTS.md §2 A-lite: covers start, drain, stop, analytics; m0_acceptance and concurrency_stress were the C5.2 migration targets). |

### C5.3.2

| ID | Criterion | Verification |
|---|---|---|
| AC-32-1 | 22 alias `#[tool] fn` handlers deleted from `crates/chronos-mcp/src/server.rs`. | `grep -E '#\[tool\(name = "(query_events\|get_event\|get_call_stack\|get_execution_summary\|debug_call_graph\|debug_detect_races\|debug_expand_hotspot\|debug_get_saliency_scores\|debug_find_variable_origin\|debug_find_crash\|inspect_causality\|forensic_memory_audit\|state_diff\|evaluate_expression\|debug_get_memory\|debug_get_registers\|debug_analyze_memory\|tripwire_create\|tripwire_list\|tripwire_delete\|tripwire_query\|probe_inject)"' crates/chronos-mcp/src/server.rs` returns zero matches. |
| AC-32-2 | `ALL_TOOL_NAMES` const contains exactly 41 entries (12 v2 canonical + 29 not-yet-converged). | `const_array_length(ALL_TOOL_NAMES) == 41` evaluated by a compile-time assertion in `toolset_sync_check`; `cargo test -p chronos-mcp --lib -- toolset_sync_check` passes. |
| AC-32-3 | Legacy shim comment in `crates/chronos-services/src/diff.rs:246` removed. | Diff line count -2 (the literal-error reconstruct hint comment block). |
| AC-32-4 | Pagination comment in `crates/chronos-services/src/events_read.rs:28` updated. | The `Offset/limit compatibility` paragraph is replaced by `## Pagination`. |
| AC-32-5 | New `chronos-mcp/tests/alias_deletion.rs` test asserts each of the 22 alias names returns a `method_not_found`-class error when invoked via raw RPC. | Test runs in isolation and as part of T3-no-sandbox; passes. |
| AC-32-6 | `chrono s-mcp/tests/sessions_tools.rs:6637` reference to `server.forensic_memory_audit(…)` is deleted or rewritten. | `cargo check -p chronos-mcp --all-targets` clean. |
| AC-32-7 | T0, T2, T4-smoke, T3-no-sandbox all pass. | As listed in AGENTS.md §2 A-lite: `cargo fmt --check` + `cargo clippy --workspace --all-targets -- -D warnings` + `cargo test --workspace --lib --tests --exclude chronos-sandbox --exclude chronos-e2e --exclude chronos-native --no-fail-fast` + the 4-file T4-smoke subset. |

## Migration table (C5.3.1)

Source: `docs/specs/AGENT_API_V2.md` (C5.1 sunset policy, commit f9fa14b1).

| Wrapper (C5.3.1)              | v2 tool          | v2 discriminator                                  | Response shape: extract fields from v2 envelope and build v1 struct |
|-------------------------------|------------------|---------------------------------------------------|--------------------------------------------------------------------|
| `query_events`                | `events_read`    | `mode=Query`                                      | `events[]: Vec<TraceEvent>` (other v2 fields ignored)              |
| `get_event`                   | `events_read`    | `mode=ById`                                       | Full JSON value passthrough (returns `serde_json::Value`)          |
| `get_call_stack`              | `execution_query`| `kind=CallStack`, `event_id`                      | `frames: Vec<StackFrame>` (other v2 fields ignored)                |
| `get_execution_summary`       | `execution_query`| `kind=ExecutionSummary`                           | Construct `ExecutionSummaryResponse` from v2 fields                |
| `debug_call_graph`            | `execution_query`| `kind=CallGraph`, `max_depth`                     | Construct `CallGraphResponse`                                      |
| `debug_detect_races`          | `execution_query`| `kind=RaceDetect`, `threshold_ns`                 | Construct `DebugDetectRacesResponse`                               |
| `debug_expand_hotspot`        | `execution_query`| `kind=Hotspot`, `top_n`                           | Construct `DebugExpandHotspotResponse`                             |
| `debug_get_saliency_scores`   | `execution_query`| `kind=Saliency`, `saliency_limit`                 | Construct `DebugGetSaliencyScoresResponse`                        |
| `debug_find_crash`            | `trace_slice`    | `slice_kind=crash`                                | Construct `DebugFindCrashResponse`                                 |
| `debug_find_variable_origin`  | `trace_slice`    | `slice_kind=variable_origin`, `variable_name`     | Construct `DebugFindVariableOriginResponse`                        |
| `inspect_causality`           | `trace_slice`    | `slice_kind=causality`, `address`                 | Construct `InspectCausalityResponse`                               |
| `forensic_memory_audit`       | `trace_slice`    | `slice_kind=memory_audit`, `address`, `limit`     | Construct `ForensicMemoryAuditResponse`                            |
| `state_diff`                  | `state_query`    | `kind=register_diff`, `timestamp_a/b`             | Construct `StateDiffResponse`                                      |
| `evaluate_expression`         | `state_query`    | `kind=expression_eval`, `event_id`, `expression`  | Construct `EvaluateExpressionResponse`                             |
| `debug_get_memory`            | `state_query`    | `kind=memory_read`, `address`, `timestamp_ns`     | Construct `DebugGetMemoryResponse`                                 |
| `debug_get_registers`         | `state_query`    | `kind=register_snapshot`, `event_id`              | Construct `DebugGetRegistersResponse`                              |
| `debug_analyze_memory`        | `state_query`    | `kind=memory_analysis`, `start/end_address`, `start/end_ts` | Construct `DebugAnalyzeMemoryResponse`                     |
| `tripwire_create`             | `observe`        | `verb=create`, `condition.kind=tripwire`, …       | Returns success-only; no v1-shape extraction                       |
| `tripwire_list`               | `observe`        | `verb=list`                                       | Construct `TripwireListResponse` from `result.active_tripwires[]`, `result.fired_events[]` |
| `tripwire_delete`             | `observe`        | `verb=delete`, `subscription_id`                  | Construct `TripwireDeleteResponse` from `result.status`, `result.remaining_active` |
| `tripwire_query`              | `observe`        | `verb=query`                                      | Construct `TripwireQueryResponse` from `result.active_tripwires[]`  |
| `probe_inject`                | `observe`        | `verb=create`, `condition.kind=uprobe`, `scope=session` | Returns success-only; no v1-shape extraction                 |

## Out of scope

- The 29 not-yet-converged tools (probe_*, browser_probe_*, counterexample_*,
  sessions CRUD, mutation_lens, causal_slice, compare_sessions,
  performance_regression_audit, etc.) are NOT deprecated aliases. They stay.
- The `MINIMAL_TOOL_NAMES` constant references `state_diff` and `probe_inject`
  even though those are aliases. **Fix**: C5.3.2 replaces them with `state_query`
  (kind=register_diff) and `observe` (verb=create, kind=uprobe).
- Tag/version bump. REC-C5 is a gate, not a release.

## Risks & mitigations

| Risk | Mitigation |
|---|---|
| The v2 envelope uses `#[serde(flatten)]` so the response shape adds a top-level `kind`/`slice_kind` discriminator that the v1 response struct doesn't expect. | Use `serde_json::Value` round-trip then extract expected fields by name. This is the same pattern C5.2 used for `m0_acceptance.rs`. |
| Some v1 fields don't have a direct v2 mapping (e.g. `QueryEventsResponse::next_offset` vs. v2 cursor). | Wrapper returns the v1 struct populated only with the fields the v2 envelope actually provides; the optional `next_offset: Option<usize>` becomes `None` (or computed from `next_cursor`). Document in the apply-receipt. |
| `MINIMAL_TOOL_NAMES` references aliases. | Fix in C5.3.2 (replace `state_diff` with `state_query`, `probe_inject` with `observe`); acceptance criterion in AC-32-N/A. |
| ptrace environmental flake in sandbox (FIND-C5.2-1, FIND-C5.2-2). | Pre-existing flakes; not introduced by C5.3. Carry-forward finding recorded. |
