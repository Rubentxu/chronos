# Chronos roadmap

> **Status:** reconstruction phase. See `docs/chronos-agentic-reconstruction/`
> for the full target architecture, milestones M0–M11, and implementation backlog.

## Active Milestones

- **m0-truth-first-foundation** — Status: in_progress
  - Cycle: `vault/cycles/m0-truth-first-foundation/` (XDG state)
  - Reconstruction roadmap: `docs/chronos-agentic-reconstruction/docs/roadmap/ROADMAP.md`
  - DoD: spec contracts C1–C7 in `vault/cycles/m0-truth-first-foundation/spec.md`
  - Branch: `feat/m0-truth-first-foundation`
  - Scope: docs merge + M0 issue decomposition + sandbox stub + coverage CI activation.
  - Out of scope: any `crates/` code change; any MCP tool surface change.

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
