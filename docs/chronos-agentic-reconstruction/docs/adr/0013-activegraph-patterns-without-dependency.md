# ADR-0013 — Adopt event-sourcing patterns inspired by ActiveGraph without depending on ActiveGraph

**Status:** Accepted

## Context

ActiveGraph demonstrates useful patterns around authoritative event logs, replay, replaceable projections and isolated event sinks. Chronos has a different performance/runtime problem and is implemented in Rust close to execution/kernel boundaries.

## Decision

Adopt the patterns:

- authoritative event log;
- replay;
- projection rebuild;
- analysis forks;
- isolated consumers.

Do not adopt:

- ActiveGraph runtime dependency;
- generic graph/behaviour runtime;
- graph DB as source of truth;
- Packs abstraction.

## Consequences

Chronos gains local persistent debugging memory without coupling its runtime to a Python agent framework.
