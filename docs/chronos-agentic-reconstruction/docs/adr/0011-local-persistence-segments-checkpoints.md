# ADR-0011 — Local persistence uses ExecutionLog segments and projection checkpoints

**Status:** Accepted

## Decision

Keep local persistence, but orient it around immutable compressed event segments, metadata and versioned projection checkpoints.

Use CAS selectively for high-value immutable artifacts rather than assuming per-event deduplication.

## Consequences

Historical executions become debugging memory and can be reanalyzed after Chronos improves.
