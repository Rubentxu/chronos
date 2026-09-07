# ADR-0002 — ExecutionLog is the source of truth

**Status:** Accepted

## Context

Destructive drains and parallel raw/semantic buffers can cause loss and disagreement between consumers.

## Decision

Introduce one append-only, cursor-readable `ExecutionLog` as authoritative session history.

## Consequences

- every record gets `EventSeq`;
- consumers use independent cursors;
- raw evidence is never discarded because semantic resolution failed;
- snapshot becomes a projection/checkpoint boundary, not a shared drain.
