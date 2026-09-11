# Chronos roadmap

> **Status:** reconstruction phase. See `docs/chronos-agentic-reconstruction/`
> for the full target architecture, milestones M0–M11, and implementation backlog.

## Active Milestones

> Currently no milestone is `Status: in_progress`. The next v2-spec sub-cycle
> (M7) candidates are listed below. The reconstruction-roadmap M6
> (OpenTelemetry correlation + export) is a separate, still-pending
> milestone documented in
> `docs/chronos-agentic-reconstruction/docs/roadmap/ROADMAP.md`. Per the
> cycle serialization lock, the next milestone will be promoted to
> `Status: in_progress` only after the previous one releases and archives.

### M7 candidates (v2-spec sub-cycle, deferred from M6)

The following work was identified during M6 closure
(`docs/milestones/m6-close-report.md` §6) and is sequenced for M7:

1. **`events_read` merge** — collapse `query_events` + `get_event` + future
   cursor-aware reads into a single tool with cursor semantics (spec line
   15). Today the v1 tools are still active in `chronos-mcp::server`.
2. **`observe` merge** — collapse the 4 `tripwire_*` tools + `probe_inject`
   behind a typed subscription model (spec lines 28–42). Tripwire +
   property + probe injection today is split across 5 v1 tools.
3. **`session_compare` + `session_explain`** — split out from the
   overloaded `compare_sessions` + `performance_regression_audit` tools
   (currently both live in `chronos-services::diff`).
4. **`session_start` + `session_stop` + `capabilities`** — session
   lifecycle v2 surface (spec lines 11–13). Today the flow is split
   across `probe_start` / `probe_stop` / `session_snapshot`.
5. **Deprecation sunset sweep** — remove the 15 v1 shims added in
   m6-01..m6-03 after the **2027-09-11** deadline (or extend the
   deadline if downstream AI agents still call them).

The exact M7 cycle split will be set at M7 kickoff based on empirical
evidence from the first three cycles.

> **Naming.** "M7" here refers to the v2-spec sub-cycle that follows the
> M6 sub-cycle. It is **not** the same as the reconstruction-roadmap M7
> (*Differential execution v2*) in
> `docs/chronos-agentic-reconstruction/docs/roadmap/ROADMAP.md` line 163.
> Each is a separate milestone with its own scope and cycle split.

### Closed milestones

- **m0-truth-first-foundation** — Status: closed
  - Reconstruction roadmap: `docs/chronos-agentic-reconstruction/docs/roadmap/ROADMAP.md`
  - Spec contracts C1–C7 were satisfied; the M0 issue decomposition
    (`vault/cycles/m0-truth-first-foundation/m0-01.md` .. `m0-10.md`) was
    consumed as part of milestone acceptance.
- **m5-agent-api-v2** — Status: closed 2026-09-10
  - Close report: `docs/milestones/m5-close-report.md`
  - Scoping: `docs/milestones/m5-agent-api-v2-scoping.md`
  - Exit criterion (*no new application algorithm belongs directly in
    `chronos-mcp::server`*) satisfied. 12 service modules in
    `chronos-services` (~6,241 LoC); `server.rs` at 5,546 LoC / 44 tool
    wrappers; 9 cycles shipped (m5-01..m5-09) plus this close cycle
    (m5-10).
  - v2-spec surface reduction (44 → 8–12 tools) deferred to M6 (see
    closed entry below).
- **m6-v2-spec-surface-reduction** — Status: closed 2026-09-11
  - Close report: `docs/milestones/m6-close-report.md`
  - Sub-cycle: v2-spec surface reduction (deferred from M5). Distinct
    from the reconstruction-roadmap M6 (OpenTelemetry export, pending).
  - Exit criterion (5 v2 dispatcher tools live, 15 v1 tools preserved as
    deprecated shims) satisfied. 18 service modules in `chronos-services`
    (9,503 LoC, +3,262 from M5 close); `server.rs` at 6,041 LoC / 49 tool
    wrappers (+5 v2 dispatcher tools); 5 cycles shipped
    (m6-01..m6-05) plus this close cycle (m6-06).
  - Remaining 7 v2 spec tools (events_read, observe, session_compare,
    session_explain, session_start, session_stop, capabilities) deferred
    to M7 (see candidates above).

## M0 backlog (decomposed)

See `vault/cycles/m0-truth-first-foundation/m0-01.md` .. `m0-10.md` (XDG state).
Each ticket links to a UAT gate from the reconstruction milestone acceptance
(`docs/chronos-agentic-reconstruction/docs/roadmap/MILESTONE_ACCEPTANCE.md`).

## Future milestones (M1–M11)

See `docs/chronos-agentic-reconstruction/docs/roadmap/ROADMAP.md` for the
ordered milestone list. Each milestone becomes one or more SDDK cycles with
its own proposal, spec, design, tasks, apply, verify, release, and archive.

## Cycle serialization lock

Per A-full workflow step 0.2, only one milestone is `Status: in_progress` at
any time. The cycle that owns it must complete (release + archive) or be
explicitly marked blocked/abandoned before another milestone starts.
