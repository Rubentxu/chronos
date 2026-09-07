# ADR-0005 — Adaptive instrumentation ladder

**Status:** Accepted

## Decision

Instrumentation escalates from existing telemetry to zero-code/runtime mechanisms, targeted passive probes, debug-build semantic probes and finally deep replay/debug backends.

## Rationale

Capturing everything is expensive and can change timing. The agent requests missing evidence, not maximal tracing.

## Consequences

Probe planning and capability discovery become first-class application concerns.
