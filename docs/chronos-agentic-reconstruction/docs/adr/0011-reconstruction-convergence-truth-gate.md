# ADR-0011 — Reconstruction Convergence Truth Gate

Status: Accepted
Date: 2026-09-15

## Context

Chronos now contains substantial reconstruction work, but the repository still mixes new and legacy execution paths. Several public contracts can currently claim stronger semantics than the implementation proves. Examples include cursor/completeness behavior above a non-authoritative query projection, optional dual-write between EventBus and ExecutionLog, infrastructure dependencies crossing inward into domain/application layers, and capability discovery through broad interfaces with unsupported defaults.

The failure mode is not simply technical debt. For an agentic debugger, false confidence is worse than an explicit unsupported result.

## Decision

Before official reconstruction milestone M6 or later feature work is promoted to active, Chronos SHALL complete REC-C0 through REC-C7 from `reconstruction/CONVERGENCE_PLAN.md`.

The following are architectural invariants:

1. `ExecutionLog` is the authoritative evidence stream for every agentic session.
2. Agent-visible reads are non-destructive and cursor on authoritative `EventSeq` positions.
3. Gaps, incompleteness, unsupported capabilities, heuristics and uncertainty are explicit.
4. Domain/application policy depends on ports, never concrete transport/storage/debugger infrastructure.
5. Backend capabilities are represented through small ports and capability descriptors, not a broad interface whose normal behavior is `UnsupportedOperation`.
6. Legacy architecture may only shrink. New use of legacy symbols or forbidden dependency edges is a CI failure.
7. A specification requirement may be marked `verified` only with executable evidence: implementation path, UAT identifier and reproducible verification command.

## Consequences

### Positive

- roadmap closure means behavioral closure, not document/file existence;
- future agents can inspect a machine-readable contract ledger;
- architectural direction becomes mechanically enforceable;
- legacy deletion becomes measurable;
- false-success API behavior becomes a release blocker.

### Negative

- feature velocity is intentionally reduced until convergence closes;
- temporary adapters/shims require explicit waivers and deletion criteria;
- some current milestones previously described as closed will have residual obligations tracked by REC-C6.

## Supersession / relationship

This ADR does not supersede ADR-0002 (`ExecutionLog` source of truth), ADR-0004 (`No Silent Lies`) or ADR-0009 (thin MCP adapter). It makes their enforcement mandatory and adds a convergence gate around them.

## Verification

- `python3 scripts/check_architecture_contracts.py`
- `reconstruction-contracts.toml` has no malformed `verified` entries;
- architecture workflow passes;
- REC-C7 UAT bundle is green.
