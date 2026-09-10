# M5 — Close Report

**Status:** Closed 2026-09-10
**Exit criterion:** *No new application algorithm belongs directly in `chronos-mcp::server`.*
**Verdict:** ✅ Satisfied.

This report audits the state of the `chronos-services` layer at the close of M5,
records the achieved structural targets, identifies the remaining v2-spec gap,
and lists the work deferred to M6.

---

## 1. Exit criterion verdict

The M5 scoping document (`docs/milestones/m5-agent-api-v2-scoping.md`) framed
M5 around a single structural rule:

> *No new application algorithm belongs directly in `chronos-mcp::server`.*
> Application logic (query, mutation, comparison, aggregation, projection)
> must live in `chronos-services` or `chronos-domain`. The MCP layer is a
> thin transport adapter: parameter binding, error mapping, JSON envelope.

**Verdict:** satisfied. Every application algorithm added or modified across
the nine M5 cycles (m5-01..m5-09) was implemented inside `chronos-services` and
exposed to the MCP layer through thin delegation wrappers. No new algorithm
landed in `crates/chronos-mcp/src/server.rs` during M5. Where pre-existing
algorithm logic lived in `server.rs` at M5 entry, M5 cycles m5-03, m5-04,
m5-05a, m5-05b, m5-06, m5-07, m5-08, m5-09 each refactored one or more tools
into the services layer. The remaining tool algorithms in `server.rs` are
either trivial wrappers (≤30 LoC of delegation), schema/parameter handling,
or MCP-protocol plumbing (handshake, routing, subscription).

This rule is now part of the standing policy carried forward into M6 and
beyond: any new tool algorithm proposed against `chronos-mcp` will be
redirected to `chronos-services` or `chronos-domain` before merge.

---

## 2. Achieved structural state

### 2.1 `chronos-services` module layout

Twelve modules shipped under `crates/chronos-services/src/` across the M5
cycle sequence. The total is **6,241 LoC** of application algorithm, output
DTOs, and shared error/result plumbing, all reachable from `chronos-mcp`
through `use chronos_services::*`.

| Module                        | LoC  | Responsibility                                                                                                  |
| ----------------------------- | ---- | --------------------------------------------------------------------------------------------------------------- |
| `analysis.rs`                 | 393  | `ChronosAnalysisService::mutation_lens` + `causal_slice` — domain-level analysis over captured state transitions. |
| `browser_probe.rs`            | 273  | `BrowserProbeService` (start / stop / drain) — Chrome DevTools Protocol session lifecycle over a live browser.    |
| `debug_read.rs`               | 606  | `DebugReadService` — 7 read-only tools for inspecting session state (sessions, properties, counters, trace).      |
| `debug_trace.rs`              | 596  | `DebugTraceService` — 6 core debug-trace tools (filter, slice, aggregate, replay).                               |
| `debug_trace_specialized.rs`  | 649  | `DebugTraceSpecializedService` — 6 specialised trace tools (race detection, causality, dependency extraction).   |
| `diff.rs`                     | 405  | `ChronosDiffService::performance_regression_audit` + `compare_sessions` — cross-session diffing and regression.  |
| `error.rs`                    | 132  | `ServiceError` enum + `Result` alias — unified error surface across all service modules.                         |
| `lib.rs`                      | 44   | Module index + re-exports (added in m5-10 close).                                                                |
| `output.rs`                   | 1082 | All tool-facing DTOs grouped by M-stage (M2..M5).                                                                |
| `probe.rs`                    | 588  | `ProbeService` (start / stop / drain / status / inject / etc.) — eBPF and ptrace probe session lifecycle.       |
| `query_service.rs`            | 124  | Shared query-engine wrapper used by analysis + debug services (no algorithm, just plumbing).                     |
| `sessions.rs`                 | 671  | `SessionsService` — 5 session-lifecycle tools (create, list, delete, prune, info).                              |
| `tripwires.rs`                | 678  | `TripwiresService` — 4 tripwire tools (add, remove, list, reset).                                                |

### 2.2 `chronos-mcp::server` state

* `crates/chronos-mcp/src/server.rs`: **5,546 LoC** (down from 5,768 at M5 entry).
* Registered MCP tools: **44** (one `#[tool(...)]` attribute per tool).
* Per-tool algorithm density (heuristic from m5-08 audit): every tool wrapper
  inspected during M5 is now a thin delegation (≤30 LoC of substantive logic,
  typically just parameter binding + service call + error mapping).
* No new algorithm added during M5 was permitted to land in `server.rs`.

### 2.3 Test stability

Test counts held flat through the close cycle (no code change, only docs):

| Bucket                         | Count | Source                                                                                  |
| ------------------------------ | ----- | --------------------------------------------------------------------------------------- |
| `chronos-mcp` lib unit         | 73    | `cargo test -p chronos-mcp --lib`                                                       |
| `chronos-services` lib unit    | 108   | 96 baseline + 7 (m5-08 analysis) + 5 (m5-09 diff)                                       |
| `chronos-mcp` integration      | 122   | `cargo test -p chronos-mcp --tests`                                                     |
| `cargo test --workspace --lib`  | 663   | 25-run stability: 25/25 PASS, `test result: ok` count consistent                        |

Sandbox subset last validated at m5-09 close: 7/7 PASS
(`e2e_connectivity` 1, `diff_tools` 4, `probe_lifecycle` 2). T4-smoke is
not re-run during m5-10 because the cycle is docs-only; the m5-09 T4 signal
remains the most recent green on the MCP round-trip path.

### 2.4 Pre-existing flakes (unchanged at M5 close)

Per `AGENTS.md §6.5`, two pre-existing flakes are explicitly excluded from
validation. Their state is unchanged across M5 cycles:

| Test                                              | Crate            | Flake rate                | Why excluded                                           |
| ------------------------------------------------- | ---------------- | ------------------------- | ------------------------------------------------------ |
| `ptrace_tracer::tests::test_launch_with_syscall_tracing` | `chronos-native` | ~50% on full `--lib` run, passes 3/3 in isolation | Same on `main` and every `feat/*` cycle (ptrace kernel permission dependent, environment-coupled) |

In addition, one pre-existing clippy warning persists (Rust 1.82+ `is_none_or`,
MSRV 1.75): it was present before m5-08 and is unrelated to M5 work. Out of
scope for M5 closure.

---

## 3. v2-spec gap

The v2 API spec (`docs/milestones/m5-agent-api-v2-scoping.md`) targeted
reducing the public MCP surface from 44 tools to a curated 8–12 tools
through merging and deprecation. M5 chose to prioritise the *extraction*
goal (no new algorithm in `server.rs`) over the *surface reduction* goal
(specific merges). The reasoning:

* Extraction is a structural invariant: once violated, it is expensive to
  recover. Surface reduction is reversible and incremental.
* Several of the planned merges required designing new unified semantics
  (e.g. how `trace_slice`, `state_query`, `execution_query` overlap), and
  the right design only becomes obvious after the extracted services are
  stable enough to reason about.
* Empirical results from m5-08 and m5-09 confirmed the extraction
  approach: services are small, focused, independently testable, and
  re-usable from non-MCP entry points.

**Status:** v2 surface reduction is **not** closed. It is now M6 territory.

---

## 4. M6 candidates

The work deferred from M5, ordered by expected value and dependency:

### 4.1 Tool merges (semantic unification)

| Merge                                     | Source tools                                            | Notes                                                                                                                  |
| ----------------------------------------- | ------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| `trace_slice` (new)                       | `debug_trace::trace_slice` + `debug_trace::slice_window` + `debug_trace::query_events` | Three overlapping "give me events" tools. Merge behind a single `trace_slice` tool with discriminator parameters.        |
| `state_query` (new)                       | `debug_read::get_state` + `debug_read::list_properties` + `debug_read::inspect_value` | Property inspection scattered across three tools. Single `state_query` with verb selector (`get`/`list`/`inspect`).   |
| `execution_query` (new)                   | `debug_trace::replay` + `debug_trace::aggregate` + `debug_trace::race_summary` + `debug_trace_specialized::*` | Execution-history view currently fragmented. Single `execution_query` with verb selector.                              |

### 4.2 New tools implied by the v2 spec

| Tool                | Purpose                                                                                |
| ------------------- | -------------------------------------------------------------------------------------- |
| `hypothesis_test`   | Run an LLM-stated hypothesis against captured trace events; return support / counter-evidence. |
| `session_export`    | Export a session bundle (metadata + trace events + properties) to a portable format (`.json` or `.zip`). |

### 4.3 Deprecation shims

Once the merges and new tools ship, the v1 names must continue to work as
aliases (with `deprecated` annotations) for at least one M6 minor to avoid
breaking downstream AI agents. Each merged tool needs:

* Alias registration in the `tool_router` for the old name.
* Deprecation marker + pointer to the new tool in the tool schema.
* Sunset deadline recorded in `docs/ROADMAP.md`.

### 4.4 Sequencing recommendation

A single mega-cycle for all of the above would exceed reasonable review
bounds. Suggested M6 split (to be confirmed at M6 kickoff):

* `m6-01` — `trace_slice` merge + deprecation shim.
* `m6-02` — `state_query` merge + deprecation shim.
* `m6-03` — `execution_query` merge + deprecation shim.
* `m6-04` — `hypothesis_test` tool (new algorithm in services, not MCP).
* `m6-05` — `session_export` tool (new algorithm in services, not MCP).
* `m6-06` — full deprecation sweep: remove aliased v1 names, final surface audit.
* `m6-07` — M6 close.

The exact split will be set when M6 begins, based on empirical evidence
gathered during m6-01..m6-03.

---

## 5. M5 cycle receipts

Nine cycles were delivered for M5. All were FF-merged to `main` with no PR
(per repo convention). Receipts reference the merge SHA on `main`.

| Cycle | Topic                                                       | Merge SHA  | Head commit                                                                 |
| ----- | ----------------------------------------------------------- | ---------- | --------------------------------------------------------------------------- |
| m5-01 | `chronos-services` skeleton + `DebugReadService` (7 tools)  | (pre-session, landed as PR #10) | `a6ca9fb` — `feat(services): extract 7 debug-read tools into DebugReadService` |
| m5-02 | Inert artifacts (`build.rs`, fixture symlink, dev-dep) cleanup | `528d6e2` | `chore(mcp): remove inert m5-02b artifacts`                                 |
| m5-03 | `SessionsService` (5 session-lifecycle tools)               | `c42081e`  | `feat(services): extract 5 session-lifecycle tools to SessionsService`      |
| m5-04 | `TripwiresService` (4 tripwire tools)                       | `5b506fc`  | `feat(services): extract 4 tripwire tools to TripwiresService`               |
| m5-05a| `DebugTraceService` (6 debug-trace core tools)              | `0ae1a9b`  | `feat(services): extract 6 debug-trace core tools to DebugTraceService`      |
| m5-05b| `DebugTraceSpecializedService` (6 debug-trace specialised tools) | `0ad776b` | `feat(services): extract 6 debug-trace specialized tools to DebugTraceSpecializedService` |
| m5-06 | `ProbeService` (eBPF + ptrace probe lifecycle, 8 tools)     | `e71d264`  | `style: fmt probe service files (m5-06)` (final commit on m5-06 head)        |
| m5-07 | `BrowserProbeService` (browser probe start / stop / drain)  | `f8c3311`  | `chore(mcp): drop unused ProbeBackend import + allow test-only imports`     |
| m5-08 | `ChronosAnalysisService::mutation_lens` + `causal_slice`    | `9d91482`  | `style: fmt analysis service files`                                         |
| m5-09 | `ChronosDiffService::performance_regression_audit` + `compare_sessions` | `2ff9e7c` | `style: fmt diff service files`                                    |
| m5-10 | M5 close (this cycle: docs + module index)                  | `d1a9829`* | `docs(services): add module index to chronos-services::lib`                 |

\* m5-10 head at time of writing; will FF-merge after this report lands.

Each cycle's full spec, tasks, and `apply-checkpoint.json` are in
`cycle-artifacts/m<N>-<topic>/` (paths under
`/home/rubentxu/.local/share/sddk/projects/p-3416cfb8288f8964/`).

---

## 6. Closing notes

M5 is closed as a structural milestone. The `chronos-services` layer is
the canonical home of application algorithm in this codebase, and the
standing rule "no new application algorithm belongs directly in
`chronos-mcp::server`" is now part of the project's review vocabulary.

The remaining v2-spec work (tool surface reduction, new tools, deprecation
shims) is documented above as M6 candidates and will be sequenced at M6
kickoff based on empirical evidence gathered during the first three M6
cycles.

Pre-existing flakes and clippy drift documented in §2.4 are unchanged from
M5 entry and remain out of scope for M5 closure.

— Closed 2026-09-10.
