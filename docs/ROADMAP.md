# Chronos roadmap

> **Status:** reconstruction phase. See `docs/chronos-agentic-reconstruction/`
> for the full target architecture, milestones M0–M11, and implementation backlog.

## Active Milestones

> Currently no milestone is `Status: in_progress`. The next milestone
> (M6) is documented in
> `docs/chronos-agentic-reconstruction/docs/roadmap/ROADMAP.md` (OpenTelemetry
> correlation and export) and the v2-spec surface reduction candidates
> listed below. Per the cycle serialization lock, the next milestone will
> be promoted to `Status: in_progress` only after the previous one
> releases and archives.

### M6 candidates (deferred from M5)

The following work was identified during M5 closure
(`docs/milestones/m5-close-report.md` §4) and is sequenced for M6:

1. **`trace_slice` merge** — collapse `debug_trace::trace_slice` +
   `debug_trace::slice_window` + `debug_trace::query_events` into a
   single tool with a discriminator parameter.
2. **`state_query` merge** — collapse `debug_read::get_state` +
   `debug_read::list_properties` + `debug_read::inspect_value` into a
   single tool with a verb selector.
3. **`execution_query` merge** — collapse `debug_trace::replay` +
   `debug_trace::aggregate` + `debug_trace::race_summary` +
   `debug_trace_specialized::*` into a single tool with a verb selector.
4. **`hypothesis_test` tool** — net-new capability: run an LLM-stated
   hypothesis against captured trace events; return support /
   counter-evidence.
5. **`session_export` tool** — net-new capability: export a session
   bundle (metadata + trace events + properties) to a portable format
   (`.json` or `.zip`).
6. **Deprecation shims** — once the merges and new tools ship, the v1
   names must continue to work as aliases (with `deprecated` annotations)
   for at least one M6 minor.

The exact M6 cycle split (e.g. one cycle per merge vs. one cycle per
deliverable) will be set at M6 kickoff based on empirical evidence from
the first three cycles.

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
    candidates above).

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
