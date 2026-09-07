# Chronos Execution Explorer

## Product decision

Do not build a VS Code/GDB clone. Visualize what generic trace tools do not show well.

## Views

### Live
Session status, target/runtime, active evidence sources, event rate, gaps, perturbation, active properties and instrumentation plan.

### Execution
High-level OTel operation context where available, invocation timeline/flame hierarchy and incomplete invocations.

### Causality
Graph centered on selected violation/state/exception. Do not render the whole execution graph by default.

### Mutation Lens

```text
seq | time | before -> after | producer invocation | source | external trace/span
```

### Hypotheses / Properties
Hypothesis state, missing evidence, probes added, property violations and completeness.

### Compare
Known-good vs failing, aligned operations, first meaningful divergence, unmatched/gap regions and changed state transitions.

### Evidence inspector
Observed/Derived/Inferred/Hypothesis, provenance, event sequence, backend, perturbation, derivation version and gaps.

## Generic observability

Avoid duplicating mature OTel trace/service-map interfaces. Support export/linking where practical.

## Scale

Use server-side aggregation, cursor pagination, range queries, virtualized lists and progressive graph expansion.
