# ADR-0008 — Breakpoints are internal escalation mechanisms

**Status:** Accepted

## Decision

Do not expose breakpoint/step-over workflows as primary Agent API.

A backend may internally suspend a process or use hardware/software breakpoints when that is the safest/cheapest mechanism for a specific fact.

## Consequences

Existing breakpoint code is not automatically deleted. Remove only when no accepted backend requires it.
