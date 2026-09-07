# Spike backlog

A spike has a question, experiment, measurable result and decision. "Prototype exists" is not an outcome.

## SP-001 — ExecutionLog backend

Can redb immutable segments sustain target append/read/replay with crash recovery?

Measure append throughput/latency, replay, memory, compression and recovery.

## SP-002 — OBI adapter

Can Chronos ingest/correlate OBI evidence without owning OBI internals? Test Go + Rust HTTP fixtures, privileges, evidence, gaps and context quality.

## SP-003 — Go compile-time instrumentation

Can `InstrumentationSpec` select one application function and one third-party function without source changes?

Success: clean tree, typed evidence, reproducible manifest, measured perturbation.

## SP-004 — Go Auto SDK correlation

Can manual OTel and eBPF auto-instrumentation preserve correct external context while Chronos adds deeper invocations?

## SP-005 — Rust XRay

Is Rust nightly XRay practical as Chronos debug-build backend?

Validate target platforms, runtime linking, function identity, entry/exit accuracy, optimized builds, overhead and dynamic activation.

Outcomes may be adopt experimental, restrict or reject with superseding ADR.

## SP-006 — Rust USDT semantic probes

Can a temporary debug build expose stable semantic probe IDs/payloads consumed through eBPF with low perturbation?

## SP-007 — Rust semantic source overlay

If XRay/USDT cannot expose typed state, can Chronos generate minimal temporary instrumentation in an isolated worktree safely?

Guardrails: compiler validated, minimal diff, no product-branch edits, manifests retained, cleanup exact.

## SP-008 — property DSL

What minimum operators solve first five UAT bugs without creating a general language?

## SP-009 — causal slicing

Which conservative dataflow/causal edges can be derived without pretending missing dependencies are known?

## SP-010 — session alignment

Evaluate alignment hierarchy:

1. external trace/span;
2. invocation topology/symbol;
3. request/task;
4. state/property anchors;
5. hierarchical segment hashes.

## SP-011 — rr integration

Can Chronos use rr as an escalation backend rather than implementing deterministic replay?

## SP-012 — GUI scale

Which aggregation/virtualization strategy handles millions of records without transferring/materializing everything?
