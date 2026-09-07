# ADR-0004 — No Silent Lies

**Status:** Accepted

## Decision

Chronos represents missing, dropped, unsupported, incomplete and heuristic evidence explicitly.

## Consequences

- overflow creates a `Gap`;
- invalid filters fail validation;
- nonexistent targets return `NotFound`;
- heuristic race detection cannot be named a confirmed race;
- agent responses include provenance/completeness when material.
