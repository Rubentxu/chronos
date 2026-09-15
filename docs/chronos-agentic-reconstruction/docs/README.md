# Chronos reconstruction documentation

This directory is the architectural source of truth for the Chronos agentic reconstruction.

## Current phase

**REC-C0 — reconstruction convergence is active.**

The reconstruction has delivered substantial foundations, but official M6+ feature work is blocked until REC-C0..REC-C7 remove legacy truth paths, close architecture boundaries and prove the remaining specification contracts.

Machine-readable current compliance lives at repository root:

`/reconstruction-contracts.toml`

## Reading order

1. `reconstruction/00_RECONSTRUCTION_OVERVIEW.md`
2. `reconstruction/CONVERGENCE_PLAN.md`
3. `architecture/TARGET_ARCHITECTURE.md`
4. `adr/0011-reconstruction-convergence-truth-gate.md`
5. `specs/EXECUTION_LOG.md`
6. `specs/EVIDENCE_AND_TRUST.md`
7. `specs/ADAPTIVE_INSTRUMENTATION.md`
8. `specs/AGENT_API_V2.md`
9. `roadmap/ROADMAP.md`
10. `roadmap/MILESTONE_ACCEPTANCE.md`
11. `testing/ARCHITECTURE_FITNESS_FUNCTIONS.md`
12. `testing/SPEC_COMPLIANCE.md`
13. `testing/UAT_STRATEGY.md`
14. `migration/CURRENT_TO_TARGET.md`

## Rule of authority

When documents conflict, use this order:

1. Accepted ADR
2. Current specification
3. `reconstruction-contracts.toml` for present implementation/compliance status
4. Current roadmap/milestone acceptance
5. Historical proposal or close report

A close report records what a historical cycle claimed/delivered at that time. It does not override later source evidence or the current compliance ledger.

The files under `docs/propuestas/` are historical inputs, not final architecture.

## Verification rule

A requirement is not `verified` because a type, DTO, MCP tool, file or test exists. A verified requirement must have:

- concrete implementation/test evidence;
- a behavioral UAT identifier;
- a reproducible verification command.

Run:

```bash
python3 scripts/check_architecture_contracts.py
```

REC-C7 additionally requires:

```bash
python3 scripts/check_architecture_contracts.py --strict-no-gaps
cargo check --workspace --all-targets --all-features
cargo test --workspace -- --test-threads=1
```

## Baseline

Convergence plan prepared from a source audit of `main` on 2026-09-15, based on the repository state around commit `9cc44ce37224e8e35b069a160c140e0f0bf7e016` and subsequent branch-local convergence documentation.

Every implementation cycle must reconcile its assumptions against current `main`; material semantic changes require ADR/spec + ledger + UAT updates in the same cycle.
