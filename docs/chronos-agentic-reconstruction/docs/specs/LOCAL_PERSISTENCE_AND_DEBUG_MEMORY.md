# Specification: Local persistence, replay and debugging memory

## Decision

Local persistence is a core product capability, not optional archival plumbing.

The authoritative persisted object is the **ExecutionLog**. Graphs/indexes/analysis are replayable projections and checkpoints.

This selectively adopts the useful event-sourcing ideas explored in ActiveGraph without taking ActiveGraph as a runtime dependency or introducing a graph database.

## Ideas adopted

- event log is authoritative;
- replay rebuilds state/projections;
- projections are disposable/versioned;
- consumers/sinks are isolated;
- analysis can fork from a common immutable execution history.

## Ideas not adopted

- ActiveGraph runtime dependency;
- generic behaviour engine in the deterministic core;
- Packs abstraction;
- mandatory graph DB;
- generic agent memory model.

## Local session layout

Conceptual, storage-engine-independent layout:

```text
.chronos/
  sessions/
    <session-id>/
      metadata
      event-segments/
      checkpoints/
      artifacts/
      analyses/
```

Actual redb layout can differ; this is the logical model.

## Analysis fork

Do not fork/replay the program when only analysis changes.

```text
base ExecutionLog (immutable)
  |
  +-- Analysis A: property-set-v1 + causal-v1
  +-- Analysis B: property-set-v2 + causal-v2
  +-- Analysis C: differential-algorithm-v3
```

Each analysis records projection/resolver/property versions.

## Debugging memory

Persistence enables behavioural history across commits/builds/tests:

```text
commit A / test_checkout / session 1
commit B / test_checkout / session 2
commit C / test_checkout / session 3
```

Chronos can later ask:

- when did behaviour first diverge?
- which function acquired a new mutation side effect?
- when did exception profile change?
- did a patch restore the known-good behavioural fingerprint?

## BehaviourFingerprint

A future projection can summarize a symbol/invocation family using evidence such as:

- branch/decision profile where observed;
- callees;
- exception profile;
- state fields mutated;
- I/O/resource effects;
- return-shape/value class where safe;
- latency distribution.

It is a heuristic regression aid, not proof of equivalence.

## Historical property evaluation

A property introduced today may be evaluated against an older session only if its required facts exist in that session.

Return `UnsupportedByRecordedEvidence` if not.

## Retention

Retention policy is session/artifact level and explicit. Do not tie application correctness to a fixed in-memory five-second ring.

Producer rings may be finite; persistent session history records gaps when retention boundaries lose evidence.

## Privacy/security

Captured values may contain secrets/PII. The persistence design must support:

- value-capture allow/deny policy;
- redaction/transformation before durable append where required;
- size limits;
- session deletion;
- artifact encryption as a future deployment option.
