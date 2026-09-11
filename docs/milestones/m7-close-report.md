# M7 close report

**Branch:** `main` at `m7-06` tag (FF-merge from `feat/m7-06-session-lifecycle-sandbox-smoke-merge`)
**Cycle:** M7 (v2-spec sub-cycle) — **CLOSED** 2026-09-11
**Precedence:** `docs/milestones/m6-close-report.md` §6; `docs/milestones/m7-events-read-scoping.md`
**Status:** COMPLETE — 2026-09-11

## §1 Executive summary

The M7 sub-cycle ships the v2 API surface for Chronos session introspection. Six cycles (m7-01 through m7-06) deliver:

- **7 v2 tool entrypoints**: `events_read`, `observe`, `session_compare`, `session_explain`, `session_start`, `session_stop`, `capabilities`.
- **20 algorithm modules** in `chronos-services` (was 18 pre-m7; +2: `events_read`, `session_lifecycle`).
- **20+ deprecated v1 shims** that route through the v2 dispatchers (preserved until 2027-09-11 sunset).
- **800+ unit tests + 357 integration tests + 3 sandbox smoke suites** (all green).

After M7, the M7 backlog reduces to:
- **m7-07+ follow-ups** (single-call `session_stop`, `attach` domain API, etc.) — deferred to m7+ cycles.
- **v1 sunset bookkeeping** — passive; no work until 2027-09-11.

The M8 backlog (handoff): live probe improvements (perf, attach API, multi-target), MCP server hardening, distributed tracing support.

## §2 Cycle log

| Cycle | Scope | Tags | LoC (delta) | Tests (delta) |
|---|---|---|---|---|
| **m7-01** | `events_read` v2 dispatcher + v1 shims for `query_events` + `get_event` | `m7-01-events-read-merge.0` | +290 (events_read.rs) | +11 unit |
| **m7-02** | `observe` v2 dispatcher + v1 shims for `tripwire_*` + `probe_inject` | `m7-02-observability-merge.0` | +715 (observe.rs) | +? unit |
| **m7-03** | `session_compare` + `session_explain` v2 dispatchers + v1 shims | `m7-03-session-compare-explain-merge.0` | +1100 (2 modules) | +19 unit |
| **m7-04** | **Foundation-only** — v2 DTOs + `SessionMetadata.tail_sealed` + `sealed_at` extension | `m7-04-session-start-stop-capabilities-foundation.0` | +239 (output.rs) + 10 (storage.rs) + 9 literal-site patches | 0 (no behaviour change) |
| **m7-05** | `session_start` + `session_stop` + `capabilities` v2 dispatcher + MCP wrappers + v1 `probe_start` shim | `m7-05-session-lifecycle-dispatchers.0` | +739 (session_lifecycle.rs) + 332 (server.rs) | +14 unit |
| **m7-06** | Sandbox smoke + MCP integration tests + M7 close | `m7-06-session-lifecycle-sandbox-smoke.0` (this cycle) | +130 (types.rs) + 145 (tools.rs) + ~280 (test extensions) | +9 sandbox smoke |

Each cycle FF-merged to `main` immediately. No PRs (per project convention).

## §3 v2 tool surface inventory

| Tool | Module | Version | Net-new? | v1 shims |
|---|---|---|---|---|
| `events_read` | `chronos-services::events_read` | m7-01 | yes | `query_events`, `get_event` |
| `observe` | `chronos-services::observe` | m7-02 | yes | `tripwire_create`, `tripwire_list`, `tripwire_delete`, `tripwire_query`, `probe_inject` |
| `session_compare` | `chronos-services::session_compare` | m7-03 | yes | `compare_sessions`, `performance_regression_audit` |
| `session_explain` | `chronos-services::session_explain` | m7-03 | yes | (none — net-new) |
| `session_start` | `chronos-services::session_lifecycle` | m7-05 | yes | `probe_start` (m7-05) |
| `session_stop` | `chronos-services::session_lifecycle` | m7-05 | yes | (none — `probe_stop` preserved) |
| `capabilities` | `chronos-services::session_lifecycle` | m7-05 | yes | (none — net-new) |

## §4 v1 deprecation bookkeeping

v1 tools preserved (not removed) until 2027-09-11 sunset. They either:
- Route through v2 dispatcher (preferred path; preserves JSON shape 1:1).
- Stay as-is with deprecation hint in `hint` field (e.g., `probe_stop`).

### Active v1 tools (deprecated)

| v1 tool | Status | Routes via |
|---|---|---|
| `query_events` | deprecated | `events_read` |
| `get_event` | deprecated | `events_read` |
| `tripwire_create` | deprecated | `observe{verb=create}` |
| `tripwire_list` | deprecated | `observe{verb=list}` |
| `tripwire_delete` | deprecated | `observe{verb=delete}` |
| `tripwire_query` | deprecated | `observe{verb=query}` |
| `probe_inject` | deprecated | `observe{verb=create, condition.kind=uprobe}` |
| `compare_sessions` | deprecated | `session_compare{kind=divergence}` |
| `performance_regression_audit` | deprecated | `session_compare{kind=regression}` |
| `probe_start` | deprecated | `session_start{action=spawn}` (m7-05 shim) |

### v1 tools not yet shimmed

| v1 tool | Status |
|---|---|
| `probe_stop` | preserved as-is (calls `ProbeService::stop` + `build_and_store_engine`); v2 `session_stop` is the canonical path. Followup m7-07 adds `events` to `SessionStopOutput` so the shim can single-call. |
| All other v1 | unaffected by M7 (e.g., `debug_*`, `query_*`, `state_*`, `trace_*`) |

## §5 Architectural decisions log

- **Async dispatcher first** — `session_start{action=spawn}` and `session_stop` are `async fn` (m7-05). m7-01..m7-04 dispatchers are sync. Future v2 dispatchers should default to async for consistency.
- **Context bundles carry live state** — `SessionLifecycleContext<'a>` bundles `&ProbeContext`, `&ObserveContext`, `&SessionStore`. The cost is unit-test complexity (must leak empty statics or test only the store-only paths). Payoff: dispatchers can call across services (e.g., `session_stop` chains to `observe{verb=list}`).
- **Engine version hardcoded** — `"chronos-0.1.0"` baked into `*Provenance` until `chronos_query::QueryEngine::engine_version()` exists. m7-02/03/05 all use this pattern.
- **`#[serde(default)]` for additive SessionMetadata changes** — v2 → v3 metadata bump is backward-compatible (old files load with `tail_sealed=false, sealed_at=None`).
- **`Unsupported(String)` reused** — no new `ServiceError` variants in m7-04/05; the existing `Unsupported` carries m7+ stub messages.
- **ProbeContext/ObserveContext for stop shim** — `session_stop` shim double-calls `ProbeService::stop` because the dispatcher discards the raw events needed for `build_and_store_engine`. m7-07 will add `events` to `SessionStopOutput` to fix.

## §6 Test pyramid state

- **Unit (chronos-services lib)**: 198/198 pass + 1 ignored (`chronos-native::ptrace_tracer::tests::test_launch_with_syscall_tracing` is a pre-existing flake — `AGENTS.md` §6.5).
- **Per-crate integration** (services + store + mcp): 357/357 pass.
- **Sandbox smoke** (T4): 3 suites chosen for m7-06 (`probe_lifecycle` + `session_lifecycle` + `program_scenarios`); ~12 new tests.
- **Bench**: unchanged (2 bench crates untouched).

Test pyramid remains A/B-direct: unit + integration = primary signal; sandbox = verification gate; perf benches = opt-in.

## §7 m8+ handoff

The m8 backlog (to be scoped in the next cycle):

- **`attach` domain API** — implement `chronos_domain::attach` (m7+ scope per m7-04 decision). Unblocks `session_start{action=attach}`.
- **Single-call `session_stop`** — add `events` + `language` to `SessionStopOutput` (m7-07). Removes the v1 shim's double-call.
- **Live probe performance** — currently single-process; m8 could explore multi-process fan-out.
- **Distributed tracing** — out of scope until m8+.
- **MCP server hardening** — observability, rate limiting, etc.

See `docs/chronos-agentic-reconstruction/docs/specs/AGENT_API_V2.md` for the v2 spec that motivated this sub-cycle.
