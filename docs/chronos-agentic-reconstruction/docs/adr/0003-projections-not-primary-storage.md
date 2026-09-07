# ADR-0003 — Graphs and indexes are replayable projections

**Status:** Accepted

## Context

Persisting execution graphs as a second truth complicates schema evolution.

## Decision

Call/state/causality/performance graphs and indexes are versioned projections rebuilt from ExecutionLog + optional checkpoints.

## Consequences

- no graph database is required initially;
- changing an algorithm does not invalidate raw evidence;
- historical sessions can be reanalyzed with new projection versions.
