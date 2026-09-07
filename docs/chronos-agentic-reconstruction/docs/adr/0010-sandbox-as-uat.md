# ADR-0010 — Preserve sandbox as executable acceptance specification

**Status:** Accepted

## Context

The historical cleanup proposal planned to delete the sandbox. The current repository contains substantial boundary, concurrency, analytics, forensic and MCP integration coverage.

## Decision

Keep `chronos-sandbox`. Evolve it toward `chronos-uat` semantics without a disruptive rename in the first milestone.

## Consequences

Each roadmap milestone adds real bug fixtures and agent-level acceptance assertions.
