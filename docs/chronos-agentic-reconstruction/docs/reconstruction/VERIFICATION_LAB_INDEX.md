# Verification Lab Documentation Index

The Chronos Verification Lab documentation is split by concern so the proposal can evolve without becoming a single monolithic design document.

- `CHRONOS_VERIFICATION_LAB.md` — product/architecture proposal and Chronos-on-Chronos model.
- `ADR-0012-chronos-verification-lab.md` — architectural decision and alternatives.
- `CHRONOS_VERIFICATION_LAB_UAT.md` — measurable UAT/acceptance scenarios.
- `SANDBOX_S0_ROADMAP.md` — incremental implementation/adoption slice.
- `PORTABLE_EXECUTION_RUNTIME.md` — future optional execution mode for external projects and cross-OS hosts.

## Scope rule

These documents do **not** add a new REC-C0 closure dependency.

Current intended adoption is:

```text
SANDBOX-S0 -> characterize execution environments
REC-C1     -> DOG-001 / DOG-002
REC-C2     -> DOG-003
M4         -> KERNEL / DOG-004
cross-cut  -> DOG-005
future     -> PORTABLE-P0/P1/P2
```

The portable execution direction is deliberately downstream of successful Verification Lab dogfooding.
