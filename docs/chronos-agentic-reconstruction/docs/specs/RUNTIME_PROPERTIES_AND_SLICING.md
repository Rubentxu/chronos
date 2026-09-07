# Specification: Runtime properties, Mutation Lens and causal slicing

## Runtime properties

```text
property order_total_non_negative {
    observe: Order.total
    when: after Order.apply_discount
    invariant: value >= 0
}
```

Temporal example:

```text
when Payment.authorize succeeds
eventually within request Payment.capture
never Payment.refund before Payment.capture
```

## Initial DSL

Keep deterministic and intentionally small:

```text
== != < <= > >= changed() unchanged() exists() contains() matches()
count() delta() before() after() eventually() never() until()
```

No arbitrary LLM code in probes.

## Mutation Lens

Prefer transitions:

```text
Order.total
100.00 -> -50.00
by: Discount.apply#inv-8281
source: discount.rs:118
inputs: discount=150.00
```

## PropertyViolation

Links property ID/version, triggering sequence, before/after values, invocation, external trace/span, causal predecessors, provenance and gaps.

## Causal slicing

Given violation/exception/wrong return/state write/divergence, walk backward through evidence-supported causal/dataflow edges and return the smallest useful set.

The first implementation should be conservative. Missing evidence must remain visible.

## Counterexample shrinking

Integrate rather than reimplement ecosystem generators such as proptest/Hypothesis:

```text
generate -> run -> property violation -> shrink input -> rerun -> slice
```

Persist minimal input plus minimal causal evidence.

## Historical evaluation

A new property can be evaluated against an old ExecutionLog only if required observations were captured. Otherwise return `UnsupportedByRecordedEvidence`, never PASS.
