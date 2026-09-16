# Portable Runtime — Adoption Gates

Status: FUTURE

The portable runtime direction must not become active implementation work until the following gates are met.

## Gate A — SANDBOX-S0 evidence

Required:

- at least two environment backends demonstrated;
- explicit capability negotiation;
- portable evidence bundle;
- reliable cleanup;
- measured backend tradeoffs.

## Gate B — Chronos dogfooding

Required:

- DOG-001 and DOG-002 produce useful evidence beyond ordinary tests;
- observer/subject state isolation proven;
- execution placement does not change evidence semantics.

## Gate C — External project consumer

Required before PORTABLE-P1:

- one external project successfully uses the execution runtime;
- measurable reduction in setup friction or reproducibility problems;
- host/runtime workspace mapping proven.

## Gate D — Remote need

Required before PORTABLE-P2:

- a concrete workflow requires remote execution;
- dedicated security ADR approved;
- local-first usage remains supported.
