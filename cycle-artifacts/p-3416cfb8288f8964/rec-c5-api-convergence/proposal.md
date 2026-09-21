# Proposal — REC-C5: Agent API convergence

Path: A-lite. Baseline: origin/main @ 9db697a5.

## Intent

Shrink the MCP tool surface to the canonical v2 agent API (AGENT_API_V2.md,
12 tools), sunset and then delete the 22 deprecated aliases, and remove the
legacy shims behind them. Contracts: API-001 (partial), API-002 (partial).

## Evidence (from explore, session sabertooth, 2026-09-20)

- 63 tools registered in server.rs ALL_TOOL_NAMES (server.rs:521-580),
  kept in lockstep by the toolset_sync_check test.
- v2 canonical (12): session_start, session_stop, capabilities, observe,
  events_read, execution_query, state_query, hypothesis_test, trace_slice,
  session_compare, session_explain, session_export.
- Deprecated aliases (22) marked "Deprecated.": query_events, get_event,
  get_call_stack, get_execution_summary, state_diff, debug_call_graph,
  debug_find_variable_origin, debug_find_crash, debug_detect_races,
  inspect_causality, debug_expand_hotspot, debug_get_saliency_scores,
  evaluate_expression, debug_get_memory, debug_get_registers,
  debug_analyze_memory, forensic_memory_audit, tripwire_create/list/delete/
  query, probe_inject.
- Sandbox test consumers of deprecated names: 3 files (m0_acceptance.rs,
  concurrency_stress.rs, +1). v2 callers: 10 files.
- Services shims: events_read.rs:33 (query_events offset→cursor),
  diff.rs:246 (legacy error string), output.rs:983-1004 (legacy
  browser_probe_drain shape).
- RISK: some aliases (get_event, get_call_stack) lack explicit v2-replacement
  mapping in descriptions — must be resolved per alias before deletion.

## Slices

- C5.1 Sunset markers: add explicit v2-replacement mapping to every
  deprecated alias description; document sunset policy in AGENT_API_V2.md;
  update API-001/API-002 notes. ~4 files. Tier: T0+T2.
- C5.2 Shrink: migrate the 3 sandbox test files off deprecated names onto v2
  equivalents; verify toolset_sync_check. ~5-8 files. Tier: T0+T2+T4-smoke
  (m0_acceptance, concurrency_stress, e2e_connectivity).
- C5.3 Delete: remove the 22 deprecated alias tool handlers from server.rs,
  drop the events_read/diff/output legacy shims, keep MCP wrappers thin.
  Large server.rs diff. Tier: T0+T2+T4-smoke (analytics_tools,
  e2e_connectivity, m0_acceptance, concurrency_stress).

## Non-goals

- The ~29 not-yet-converged working tools (probe_*, browser_probe_*,
  counterexample_*, sessions CRUD, mutation_lens, causal_slice,
  compare_sessions) are NOT deprecated aliases; they stay.

## Gates

Per AGENTS.md tiers; sandbox runs with CHRONOS_MCP_PATH set, binary rebuilt
first, --test-threads=1.
