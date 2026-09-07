# Chronos documentation

This directory is the architectural source for the Chronos reconstruction.

## Reading order

1. `reconstruction/00_RECONSTRUCTION_OVERVIEW.md`
2. `architecture/TARGET_ARCHITECTURE.md`
3. `specs/EXECUTION_LOG.md`
4. `specs/ADAPTIVE_INSTRUMENTATION.md`
5. `specs/EVIDENCE_AND_TRUST.md`
6. `specs/AGENT_API_V2.md`
7. `roadmap/ROADMAP.md`
8. `roadmap/MILESTONE_ACCEPTANCE.md`
9. `testing/UAT_STRATEGY.md`
10. `migration/CURRENT_TO_TARGET.md`

## Rule of authority

When documents conflict, use this order:

1. Accepted ADR
2. Current specification
3. Current roadmap/milestone acceptance
4. Historical proposal

The files under `docs/propuestas/` are historical inputs, not the final architecture.

## Baseline

Prepared against repository `main` at commit:

`c76b1096dfc25de666a32bbb56992d4190e5aee3`

Before implementing a milestone, reconcile the document against the then-current branch and record material architectural changes as ADRs.
