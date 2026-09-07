# Specification: Evidence, trust and "No Silent Lies"

## Goal

Agents can produce convincing explanations from incomplete data. Uncertainty must therefore be part of the domain model.

## Evidence classes

### Observed
Captured directly: uprobe hit, syscall, OTel span, JFR event, explicit state probe.

### Derived
Calculated deterministically: duration, explicit parent/child edge, call count, property evaluation.

### Inferred
Algorithmic conclusion with assumptions: likely causal predecessor, heuristic alignment across traces.

### Hypothesis
Explanation proposed for testing. Never silently promote it to an observed fact.

## Completeness

```text
Complete
Partial
GapDetected
Unsupported
Unknown
```

## Provenance

At minimum:

```text
source kind
backend name/version
probe ID
session
seq/range
instrumentation plan
perturbation level
projection algorithm/version
```

Suggested source kinds:

```text
OpenTelemetryManual OpenTelemetryZeroCode OpenTelemetryOBI
RuntimeNative ChronosSemanticProbe EbpfKernel EbpfUprobe
Ptrace ReplayBackend SyntheticTest
```

## Perturbation

```text
NoneKnown Low Moderate High Unknown
```

Do not encode universal overhead claims. Measure against UAT workloads.

## Correctness rules

### Missing event
Do not infer "did not happen" unless observation completeness makes absence meaningful.

### Unknown filter
Reject invalid typed filters. Do not silently drop an unknown string and broaden the query.

### Missing target event
Return `NotFound`, never default to thread 1 or another plausible event.

### Race analysis
Until happens-before evidence exists, label time-window/address matches `SuspiciousConcurrentAccess`, not confirmed race.

### Differential execution
Return alignment method, unmatched ranges, first divergence according to that alignment and confidence if heuristic. Hash inequality alone is not semantic divergence proof.

## Verification bundle

A fix-verification result should include:

```text
original failure reproduced?
property status before/after
counterexample/input
unexpected behavioural differences
new exceptions
gaps
test results
instrumentation perturbation assessment
```

Chronos verifies behaviour. The LLM authors the patch.
