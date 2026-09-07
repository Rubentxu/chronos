# ADR-0001 — Agent-first execution intelligence

**Status:** Accepted

## Context

Current product language centers on time-travel debugging and concepts inherited from human IDE workflows. Coding agents do not benefit from interactive stepping as a primary interface.

## Decision

Position Chronos as an **execution-intelligence runtime for coding agents**. Public abstractions focus on observation, evidence, properties, causality, divergence and verification.

## Consequences

- debugger mechanisms remain possible backends;
- feature priority changes from IDE parity to evidence quality;
- README/product language changes;
- UAT measures bug-finding utility rather than number of debugger controls.
