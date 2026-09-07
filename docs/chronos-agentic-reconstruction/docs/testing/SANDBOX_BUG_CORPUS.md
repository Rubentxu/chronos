# Sandbox real bug corpus

Build a small, intentional bug corpus whose ground truth is known.

## Rust fixtures

### RUST-STATE-001 — negative order total
Loyalty multiplier applied incorrectly. Exercises invocation identity, mutation, property violation, causal slice and fix verification.

### RUST-EXC-002 — swallowed error
Error becomes default value and later corrupts state. Exercises error flow and causal evidence.

### RUST-TIMING-003 — instrumentation-sensitive race
Bug reproduction changes under heavy instrumentation. Exercises perturbation detection/fallback.

### RUST-RESOURCE-004 — leaked resource
Exercises acquire/release projection.

## Go fixtures

### GO-DISCOUNT-001 — wrong discount
HTTP service with manual OTel span plus failing business calculation.

Progression:
1. existing OTel shows request/service operation;
2. OBI/zero-code supplies coarse evidence;
3. compile-time instrumentation captures `Promotion.Calculate` args/return;
4. Chronos semantic hook captures `Order.Total` transition;
5. property violation isolates root cause.

### GO-FLAKY-002 — goroutine ordering
Repeated pass/fail runs for future happens-before comparison.

### GO-THIRDPARTY-003 — dependency behaviour
Proves compile-time instrumentation can observe selected dependency behaviour without editing dependency source.

## Distributed

### DIST-TRACE-001
`gateway -> payment`, with existing OTel context and a payment-state bug. Local mutation must correlate to external trace/span.

## Python

### PY-MON-001
Python >=3.12 branch/exception behaviour observed through `sys.monitoring`.

## JVM

### JVM-JFR-001
JFR runtime evidence plus OTel operation, without relying on IDE-style JDWP stepping.

## Concurrency validation

### CONC-LOCKED-001
Same address touched under lock. Must not be reported as confirmed race.

### CONC-RACE-002
Unsynchronized conflicting accesses. Initially suspicious; confirmed only after happens-before support.

## Scale

### SCALE-LOG-001
Millions of records validating segments, cursors, checkpoints, replay, pagination and explicit overflow.

## Rule

Every fixture has failing trigger, ground-truth cause, verification patch and deterministic Chronos-evidence assertions.
