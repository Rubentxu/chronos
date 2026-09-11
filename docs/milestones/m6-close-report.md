# M6 (sub-cycle) — v2-spec surface reduction close report

**Status:** Closed 2026-09-11
**Sub-cycle:** v2-spec surface reduction (deferred from M5; **not** the
reconstruction-roadmap M6 = "OpenTelemetry correlation and export",
which remains pending).
**Exit criterion:** *The MCP v2 surface in `docs/chronos-agentic-reconstruction/docs/specs/AGENT_API_V2.md`
is implemented in `chronos-services` dispatchers, exposed via v2 MCP tools, and every merged v1 tool is preserved as a deprecated shim.*
**Verdict:** ✅ Satisfied.

This report audits the state of the `chronos-services` + `chronos-mcp` layers
at the close of the M6 sub-cycle, records the achieved structural targets,
identifies the remaining v2-spec gap, and lists the work deferred to M7+.

> **Naming clarification.** The cycle labels `m6-01..m6-05` used in commit
> footers and `docs/milestones/m6-*.md` refer to the **v2-spec surface
> reduction sub-cycle** (deferred from M5 per `docs/milestones/m5-close-report.md`
> §4). This is distinct from the **reconstruction-roadmap M6**
> (`docs/chronos-agentic-reconstruction/docs/roadmap/ROADMAP.md` line 147)
> which targets OpenTelemetry correlation + export and is a separate,
> still-pending milestone. M7+ in this report refers to the next v2-spec
> sub-cycle work, **not** the reconstruction-roadmap M7.

---

## 1. Exit criterion verdict

The m5 close report (§4.4) framed the M6 sub-cycle around five concrete
deliverables:

1. `trace_slice` merge (3 source tools → 1 v2 tool + deprecation shims)
2. `state_query` merge (3 source tools → 1 v2 tool + deprecation shims)
3. `execution_query` merge (4+ source tools → 1 v2 tool + deprecation shims)
4. `hypothesis_test` net-new tool
5. `session_export` net-new tool

**Verdict:** satisfied. Five cycles (m6-01..m6-05) shipped every deliverable
on the planned list. The MCP server now exposes five new v2 dispatcher
tools (`trace_slice`, `state_query`, `execution_query`, `hypothesis_test`,
`session_export`), every v1 tool that was merged behind a dispatcher now
routes through the dispatcher and is annotated as `Deprecated.` in its tool
description, and the net-new tools add the two capabilities the v2 spec
demanded but no v1 tool provided. The `chrono-native` flaky tests remain
documented and out of scope per `AGENTS.md §6.5`.

---

## 2. Achieved structural state

### 2.1 `chronos-services` module layout

Eighteen modules live under `crates/chronos-services/src/` at M6 close. The
total is **9,503 LoC** of application algorithm, output DTOs, and shared
error/result plumbing (up from 6,241 LoC at M5 close — net +3,262 LoC
across the five M6 cycles). All are reachable from `chronos-mcp` through
`use chronos_services::*`.

| Module                         | LoC   | M6 delta | Responsibility                                                                         |
| ------------------------------ | ----- | -------- | -------------------------------------------------------------------------------------- |
| `analysis.rs`                  | 396   | —        | `ChronosAnalysisService::mutation_lens` + `causal_slice` (M5)                          |
| `browser_probe.rs`             | 273   | —        | `BrowserProbeService` (start/stop/drain) (M5)                                          |
| `debug_read.rs`                | 606   | —        | `DebugReadService` (7 read-only tools) (M5) — **3 callers now route via state_query**  |
| `debug_trace.rs`               | 596   | —        | `DebugTraceService` (6 core debug-trace tools) (M5) — **4 callers now route via trace_slice** |
| `debug_trace_specialized.rs`   | 649   | —        | `DebugTraceSpecializedService` (6 specialized tools) (M5) — **6 callers now route via execution_query** |
| `diff.rs`                      | 405   | —        | `ChronosDiffService::performance_regression_audit` + `compare_sessions` (M5)           |
| `error.rs`                     | 147   | —        | `ServiceError` enum + `Result` alias                                                    |
| `execution_query.rs`           | 331   | **+331** | v2 dispatcher (m6-03) — call_stack / execution_summary / call_graph / race_detect / hotspot / saliency |
| `hypothesis_test.rs`           | 1023  | **+1023**| v2 net-new (m6-04) — Invariant / Existence / CallPath hypothesis evaluation              |
| `lib.rs`                       | 84    | —        | Module index + re-exports                                                               |
| `output.rs`                    | 1546  | —        | All tool-facing DTOs grouped by M-stage (grew with each M6 cycle)                      |
| `probe.rs`                     | 588   | —        | `ProbeService` (8 probe-lifecycle tools) (M5)                                           |
| `query_service.rs`             | 124   | —        | Shared query-engine wrapper                                                              |
| `session_export.rs`            | 681   | **+681** | v2 net-new (m6-05) — Json / OtlpJson export of session bundle                           |
| `sessions.rs`                  | 671   | —        | `SessionsService` (5 session-lifecycle tools) (M5)                                     |
| `state_query.rs`               | 328   | **+328** | v2 dispatcher (m6-02) — register_diff / memory_read / register_snapshot / memory_analysis / expression_eval |
| `trace_slice.rs`               | 377   | **+377** | v2 dispatcher (m6-01) — variable_origin / crash / causality / memory_audit            |
| `tripwires.rs`                 | 678   | —        | `TripwiresService` (4 tripwire tools) (M5)                                              |
| **Total**                      | **9,503** | **+3,262** |                                                                                          |

The five M6-dispatched service modules are written under a consistent
pattern: a single `ChronosXxxService::query` (or equivalent) dispatcher
that owns an internal context (`XxxContext<'a>` with `&'a Mutex<HashMap<String, QueryEngine>>`),
takes a typed `XxxKind` enum input, returns a `XxxOutput` envelope whose
variants are the individual v1 tool outputs renamed and re-exported.

### 2.2 `chronos-mcp::server` state

* `crates/chronos-mcp/src/server.rs`: **6,041 LoC** (up from 5,546 at M5
  close — +495 LoC, dominated by the 5 new v2 tool wrappers + the shim
  refactor of 15 v1 wrappers to route through the v2 dispatchers).
* Registered MCP tools: **49** (`#\[tool(...)]` attribute count, up from
  44 at M5 close).
* **5 new v2 dispatcher tools**: `trace_slice`, `state_query`,
  `execution_query`, `hypothesis_test`, `session_export`.
* **15 deprecated v1 shims** that route through the v2 dispatchers:
  * 4 trace shims (`debug_find_variable_origin`, `debug_find_crash`,
    `inspect_causality`, `forensic_memory_audit` → `trace_slice`)
  * 5 state shims (`state_diff`, `evaluate_expression`, `debug_get_memory`,
    `debug_get_registers`, `debug_analyze_memory` → `state_query`)
  * 6 execution shims (`get_call_stack`, `get_execution_summary`,
    `debug_call_graph`, `debug_detect_races`, `debug_expand_hotspot`,
    `debug_get_saliency_scores` → `execution_query`)
* Each shim's `#[tool]` `description` opens with the literal string
  *"Deprecated. Use `<v2>` with `<kind>=<value>` instead."* so MCP
  clients see the migration path before they call the tool. The shim
  bodies build the v2 `XxxInput` (populating variant-specific fields,
  leaving others `None`), call the dispatcher, unwrap the corresponding
  `XxxOutput` variant, and re-serialise with the v1 JSON shape (byte
  identical to the previous response). An exhaustive `Ok(_) =>
  unreachable!()` arm documents the variant invariant.

### 2.3 Per-crate test counts

| Crate             | lib unit at M6 close | Notes                                                           |
| ----------------- | -------------------- | --------------------------------------------------------------- |
| `chronos-services`| (counted below)      | New module unit tests shipped in m6-01..m6-05                   |
| `chronos-mcp`     | (counted below)      | Shim bodies are covered by existing dispatch tests              |
| `chronos-native`  | (per §6.5 flakes)    | 1 pre-existing flaky test excluded                              |
| workspace `--lib` | **741 passed / 0 failed / 4 ignored across 15 crates** (excluding `chronos-native`) | Run during m6-06 close: T0 PASS, T1 PASS |

**Per-crate breakdown from the m6-06 close `cargo test --workspace --lib --exclude chronos-native --no-fail-fast` run:**

| Crate                | Passed | Failed | Ignored |
| -------------------- | ------ | ------ | ------- |
| `chronos-services`   | 149    | 0      | 0       |
| `chronos-mcp`        | 73     | 0      | 0       |
| `chronos-domain`     | 156    | 0      | 0       |
| `chronos-store`      | 30     | 0      | 0       |
| `chronos-query`      | 77     | 0      | 0       |
| `chronos-log`        | 42     | 0      | 1       |
| `chronos-capture`    | 44     | 0      | 1       |
| `chronos-browser`    | 24     | 0      | 1       |
| `chronos-python`     | 30     | 0      | 1       |
| `chronos-java`       | 26     | 0      | 0       |
| `chronos-go`         | 13     | 0      | 0       |
| `chronos-js`         | 8      | 0      | 0       |
| `chronos-ebpf`       | 3      | 0      | 0       |
| `chronos-index`      | 48     | 0      | 0       |
| `chronos-e2e`        | 18     | 0      | 0       |
| **Total (excl. native)** | **741** | **0** | **4** |

(`chronos-native` excluded per §6.5; pre-existing ptrace flakes.)

### 2.4 Pre-existing flakes (unchanged at M6 close)

Per `AGENTS.md §6.5`, two pre-existing flakes remain excluded from
validation. Their state is unchanged across M5 + M6 cycles:

| Test                                              | Crate            | Flake rate                | Why excluded                                           |
| ------------------------------------------------- | ---------------- | ------------------------- | ------------------------------------------------------ |
| `ptrace_tracer::tests::test_launch_with_syscall_tracing` | `chronos-native` | ~50% on full `--lib` run, passes 3/3 in isolation | ptrace kernel permission dependent; same on `main` and every `feat/*` cycle |

The pre-existing clippy drift noted in the M5 close report (Rust 1.82+
`is_none_or`, MSRV 1.75) was resolved by the `chore(clippy)` commit
`7780df3` that landed in `feat/m6-01-trace-slice-merge`. No new clippy
warnings have been introduced by the M6 sub-cycle.

### 2.5 Sandbox subset

Sandbox tests were not re-run during `m6-06` because the cycle is
docs-only — no source code touched. The most recent green signal on the
MCP round-trip path is the m5-09 T4-smoke (7/7 PASS:
`e2e_connectivity`, `diff_tools`, `probe_lifecycle`); m6-01..m6-05 each
shipped their own cycle-level validation when the dispatcher modules
were introduced, and `chronos-sandbox` itself does not exercise the
deprecated shim layer because the public surface is the v2 names.

---

## 3. v2-spec gap

The v2 API spec (`docs/chronos-agentic-reconstruction/docs/specs/AGENT_API_V2.md`)
targets **8–12 public tools**, all with structured-typed parameters. At M6
close the **5 dispatcher tools from the M6 sub-cycle** are present in the
MCP surface:

| Spec tool         | Status at M6 close                                              |
| ----------------- | --------------------------------------------------------------- |
| `execution_query` | ✅ Shipped (m6-03)                                              |
| `state_query`     | ✅ Shipped (m6-02)                                              |
| `hypothesis_test` | ✅ Shipped (m6-04)                                              |
| `trace_slice`     | ✅ Shipped (m6-01)                                              |
| `session_export`  | ✅ Shipped (m6-05)                                              |
| `session_start`   | ❌ Deferred to M7+ (today's flow is `probe_start` + session-snapshot) |
| `session_stop`    | ❌ Deferred to M7+ (today's flow is `probe_stop` + `drop_session`) |
| `capabilities`    | ❌ Deferred to M7+                                               |
| `observe`         | ❌ Deferred to M7+ (current surface uses `tripwire_*` + `probe_inject`) |
| `events_read`     | ❌ Deferred to M7+ (current surface is `query_events` + `get_event`) |
| `session_compare` | ❌ Deferred to M7+ (current surface is `performance_regression_audit` + `compare_sessions`) |
| `session_explain` | ❌ Deferred to M7+                                               |

**Status:** 5 of 8–12 v2 spec tools shipped. The remaining 7 are listed as
M7+ candidates. The M6 sub-cycle's exit criterion ("ship the 5 tools on
the M6 candidate list") is satisfied; the wider "8–12 tools" target is the
next sub-cycle's goal.

---

## 4. Standing policy carried into M7+

* The structural rule from M5 ("no new application algorithm belongs
  directly in `chronos-mcp::server`") is reaffirmed. The 15 v1 → v2
  dispatcher shim refactors in m6-01..m6-03 demonstrate the inverse
  direction: any v1 tool that still owned algorithm in `server.rs` has
  been folded into a service dispatcher. Future cycle proposals that
  attempt to land algorithm in `server.rs` will be redirected to
  `chronos-services` or `chronos-domain`.
* The deprecation policy introduced in m6-01..m6-03 is reaffirmed: any
  v1 tool merged behind a v2 dispatcher remains callable with a
  `Deprecated.` annotation in its tool description. A separate
  deprecation-sweep cycle (the originally-planned `m6-06`) was *not*
  needed because each merge cycle shipped its own shim conversion in the
  same branch; the empirical evidence the sweep was scoped against was
  collected cycle-by-cycle.
* The shim sunset deadline is **2027-09-11 (one year from this close
  report)**. After that date, the deprecated v1 names may be removed in
  any M7+ cycle without further notice; downstream AI agents that still
  call the v1 names should migrate to the v2 dispatcher names before
  then.

---

## 5. M6 (sub-cycle) receipts

Five M6 sub-cycles delivered the work. All were FF-merged to `main` with
no PR (per repo convention). Receipts reference the merge SHA on `main`.

| Cycle    | Topic                                                                 | Merge SHA  | Head commit (key)                                                  |
| -------- | --------------------------------------------------------------------- | ---------- | ------------------------------------------------------------------ |
| m6-01    | `trace_slice` merge — 3 v1 trace tools → 1 v2 dispatcher + 4 shims    | `c6aa009`* | `refactor(mcp): convert 4 v1 trace tools to deprecated shims via trace_slice dispatcher (m6-01)` (`0d78ef3`) |
| m6-02    | `state_query` merge — 3 v1 state tools → 1 v2 dispatcher + 5 shims    | `0f96e26`* | `refactor(mcp): convert 5 v1 state tools to deprecated shims via state_query dispatcher (m6-02)` (`f78fb63`) |
| m6-03    | `execution_query` merge — 6 v1 execution tools → 1 v2 dispatcher + 6 shims | `ef6c12d`* | `refactor(mcp): convert 6 v1 execution tools to deprecated shims via execution_query dispatcher (m6-03)` (`ef6c12d`) |
| m6-04    | `hypothesis_test` net-new — Invariant / Existence / CallPath          | `da11c80`* | `feat(services): add HypothesisKind + HypothesisOutput DTOs (m6-04)` (`ae6b6a6`) + dispatcher + MCP wrapper + clippy cleanup |
| m6-05    | `session_export` net-new — Json / OtlpJson export of session bundle   | `da11c80`  | `feat(services): add ExportFormat + ExportBundle + ExportResult DTOs (m6-05)` (`4694eec`) + dispatcher + MCP wrapper |
| m6-06    | **M6 close (this cycle: docs + structural audit)**                    | (this cycle) | `docs(milestones): add M6 close report (v2-spec surface reduction)` |

\* final commit on the merge branch; the cycle's full commit list is in the
matching `docs/milestones/m6-*.md` spec file.

Each cycle's full spec, tasks, and `apply-checkpoint.json` are in
`cycle-artifacts/m<N>-<topic>/` (paths under
`/home/rubentxu/.local/share/sddk/projects/p-3416cfb8288f8964/`).

---

## 6. M7+ candidates

The remaining v2-spec work, ordered by expected value and dependency on
existing infrastructure:

| Cycle    | Topic                                                                                | Notes                                                                                          |
| -------- | ------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------- |
| m7-01    | `events_read` merge — `query_events` + `get_event` + future cursor-aware reads      | Replaces ad-hoc event queries with cursor-based, non-destructive reads (spec line 15)         |
| m7-02    | `observe` merge — `tripwire_*` (4 tools) + `probe_inject` behind a typed observer    | Unifies subscriptions + tripwires + properties under one subscription model (spec lines 28–42) |
| m7-03    | `session_compare` + `session_explain` — already partially in `diff.rs`               | Split out from `compare_sessions` (currently overloaded)                                       |
| m7-04    | `session_start` + `session_stop` + `capabilities` — session-lifecycle v2 surface     | Spec lines 11–13; today the flow is split across `probe_start` / `probe_stop` / `session_snapshot` |
| m7-05    | Deprecation sunset sweep — remove v1 shims after 2027-09-11 (or extend the deadline) | Cleanup; only run after the deadline passes or the deadline is extended                         |
| m7-06    | M7 close                                                                             | Docs + structural audit                                                                         |

The exact M7+ split will be set at M7 kickoff based on empirical evidence
gathered during m7-01..m7-03.

**Independent milestones in flight (not part of the v2-spec sub-cycle):**

* **Reconstruction-roadmap M6** — *OpenTelemetry correlation and export*
  (`docs/chronos-agentic-reconstruction/docs/roadmap/ROADMAP.md` line 147).
  Local OTLP ingestion + external trace/span context projection +
  provenance mapping + optional OTLP export. Note: `session_export`'s
  `OtlpJson` variant is a *partial* pre-implementation of this milestone's
  export half (it ships the JSON wire format), but the **correlation**
  half (linking captured events to incoming distributed traces) is not
  yet implemented.
* **Reconstruction-roadmap M7** — *Differential execution v2* (BehaviourFingerprint
  prototype, semantic divergence localization).

---

## 7. Closing notes

The M6 sub-cycle is closed as a structural milestone. The v2-spec
surface reduction target — 5 of the 8–12 v2 spec tools live and the
v1→v2 deprecation shim pattern is established and reproducible — is
satisfied. The remaining 7 v2 spec tools are documented above as M7+
candidates and will be sequenced at M7 kickoff.

Pre-existing flakes and clippy drift documented in §2.4 are unchanged
from M5/M6 entry and remain out of scope for M6 closure (the clippy
drift was actually *resolved* during M6, not regressed).

— Closed 2026-09-11.
