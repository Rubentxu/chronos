# Chronos roadmap

> **Status:** reconstruction phase. See `docs/chronos-agentic-reconstruction/`
> for the full target architecture, milestones M0–M11, and implementation backlog.

## Active Milestones

> No milestone is currently `Status: in_progress`. All m0–m8 reconstruction
> sub-cycles are closed (m0, m5, m6, m7, m8) and the m9 vault-hygiene +
> m10-vault-ms-cleanup follow-ups are also closed. The next milestone on
> the reconstruction roadmap is **M6 (OpenTelemetry correlation + export)**,
> a separate, still-pending milestone documented in
> `docs/chronos-agentic-reconstruction/docs/roadmap/ROADMAP.md`. Per the
> cycle serialization lock, M6 will be promoted to `Status: in_progress`
> only after a future explicit kickoff cycle.

## Next milestone

The reconstruction roadmap's next pending milestone is **M6 (OpenTelemetry
correlation + export)**, documented in
`docs/chronos-agentic-reconstruction/docs/roadmap/ROADMAP.md`. No M7
candidates remain — all v2-spec work was closed in m6 and m7 sub-cycles
(see closed entries below).

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
- **m7-v2-spec-introspection** — Status: closed 2026-09-11
  - Close report: `docs/milestones/m7-close-report.md`
  - Sub-cycle: completes the v2-spec sub-cycle opened by M6. Ships the
    remaining 7 v2 dispatcher tools (events_read, observe, session_compare,
    session_explain, session_start, session_stop, capabilities) with 20+
    deprecated v1 shims preserved until 2027-09-11 sunset. 6 cycles shipped
    (m7-01..m7-06) plus this close cycle (m7-07).
  - Follow-ups (`session_start{action=attach}` stub, v1 sunset bookkeeping)
    deferred to m9+ cycles (closed by m9-77/m9-78/m9-79).
- **m8-counterexample-shrinking** — Status: closed 2026-09-11
  - Close report: `docs/milestones/m8-05-proptest-shrinking-pagination-events-tool-m8-close-merge.md`
  - Sub-cycle: counterexample shrinking foundation + real per-variant
    proptest shrinking + pagination + events tool + M8 close. 5 cycles
    shipped (m8-01..m8-05) plus this close cycle (m8-05 close).
- **m9-vault-hygiene** — Status: closed 2026-09-14
  - Handoff: `.sddk-knowledge/p-3416cfb8288f8964/handoff/m9-backlog-blocked-2026-09-12.md`
  - Sub-cycle: vault hygiene + drift remediation across 98 cycles. Closed
    13 cross-check classes (CC#1..CC#56), introduced smoke test harness
    (`scripts/smoke_test_ccs.sh`), brought all vault indexes to canonical
    schema. Net outcome: vault state canonical, 56 CCs documented, peel_match
    verified for all cycles.
- **m10-vault-ms-cleanup** — Status: closed 2026-09-15
  - Handoffs: `cycle-artifacts/p-3416cfb8288f8964/handoffs/HANDOFF-2026-09-15*.md`
  - Sub-cycle: vault hygiene follow-ups after m9-* close + capability
    discovery refinements (ms- prefix = "milestone slice"). 8 cycles
    shipped (m10-ms-cap-discovery, m10-ms-cap-discovery-followup,
    m10-ms-cap-discovery-followup-2, m10-vault-handoff-relocate,
    m10-vault-last-updated-backfill, m10-cc17-cc26-schema-fix,
    m10-cc30-cc34-cc35-cc36-cc41-cc43-schema-fix, m10-m9-legacy-schema-migration).
  - Plus 2 milestones closed (m10-ms-evt-typed, m10-ms-property-policy)
    and 1 housekeeping cycle (m10-stale-branch-cleanup-2).
  - All v0.7.103..v0.7.110 tags peel correctly. Carry-forward:
    `m10-spec-coverage-glue` (latent).
  - Note: the `m10-` prefix here is an internal cycle-artifact namespace
    distinct from the reconstruction-roadmap M10 (Execution Explorer).
    See `docs/chronos-agentic-reconstruction/docs/roadmap/ROADMAP.md` for
    the official M10 milestone, which remains pending.

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
