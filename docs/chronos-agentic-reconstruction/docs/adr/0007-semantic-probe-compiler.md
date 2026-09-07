# ADR-0007 — Semantic Probe Compiler

**Status:** Accepted as architectural direction; implementation staged by language

## Decision

The LLM may propose an `InstrumentationSpec` describing evidence it needs. A deterministic language/backend-specific component resolves symbols/types and creates executable instrumentation.

## Rejected

- arbitrary LLM-generated eBPF bytecode;
- unvalidated memory offsets;
- permanent debug edits in the product branch by default.

## Consequences

Instrumentation manifests become reproducible artifacts and compiler/runtime validation is part of trust.
