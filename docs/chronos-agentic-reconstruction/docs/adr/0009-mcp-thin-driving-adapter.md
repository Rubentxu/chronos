# ADR-0009 — MCP is a thin driving adapter

**Status:** Accepted

## Decision

Move session/probe/query/diff/property orchestration into application services. MCP owns schema validation, transport mapping and response formatting.

## Consequences

`chronos-mcp::server` shrinks progressively. New business workflows must not be implemented directly in MCP handlers.
