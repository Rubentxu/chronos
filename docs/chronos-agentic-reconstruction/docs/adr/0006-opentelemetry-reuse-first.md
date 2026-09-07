# ADR-0006 — Reuse OpenTelemetry and OBI before custom generic instrumentation

**Status:** Accepted

## Decision

Use existing OTel spans/events/context and standard zero-code instrumentation whenever sufficient. Use OBI for supported generic Linux protocol/application telemetry.

Chronos eBPF focuses on debugging-specific gaps.

## Consequences

- less duplicated instrumentation code;
- distributed trace context becomes correlation input;
- OTel span identity is correlated with, not equated to, Chronos invocation identity;
- upstream version/capability changes remain behind adapters.
