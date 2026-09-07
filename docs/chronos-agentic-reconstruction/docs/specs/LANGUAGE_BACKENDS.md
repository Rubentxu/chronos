# Specification: Language backend strategy

## Principle

Chronos exposes a stable investigation model and negotiates capabilities per runtime. It does not promise the same mechanism/depth everywhere.

## Go — first reference vertical

Priority:

1. existing manual OTel;
2. Auto SDK correlation where applicable;
3. OpenTelemetry eBPF/OBI zero-code;
4. OpenTelemetry Go compile-time instrumentation (`otelc`);
5. Chronos compile-time typed semantic hooks;
6. debugging-specific eBPF;
7. Delve/ptrace as deep fallback.

Let upstream OpenTelemetry own common net/http, gRPC, database and context-propagation instrumentation. Chronos focuses on state mutation, properties, causal evidence and verification.

Spikes: goroutine identity, channel send/receive, synchronization evidence, typed state transitions and exact-vs-instrumented behavioural comparison.

## Rust — co-priority

Priority:

1. existing `tracing`/OpenTelemetry;
2. OBI for generic Linux protocol/network spans;
3. native eBPF uprobes/tracepoints;
4. DWARF/symbol enrichment;
5. Rust nightly XRay spike via `-Z instrument-xray`;
6. USDT semantic debug probes;
7. temporary source-overlay semantic probes;
8. ptrace/rr escalation.

Rust OpenTelemetry is useful when present but is not a hard dependency for the debugger architecture.

XRay is a spike, not an early stable dependency: nightly, runtime linking and platform support must be validated.

## Python

Use `sys.monitoring` on Python >=3.12 as preferred runtime event API. Use older tracing only as compatibility fallback.

## JVM

Prefer existing OTel Java instrumentation + JFR/RecordingStream. Use targeted agent/JVMTI/JDWP only when deeper state requires it.

## Node/JavaScript

Prefer zero-code OTel/library instrumentation, then runtime/browser mechanisms.

## Browser/WASM

Keep CDP/WASM specialized adapter; converge output/loss semantics on ExecutionLog.

## C/C++

Existing OTel/OBI/eBPF, LLVM XRay for instrumentable debug builds, rr as deep replay. Evaluate deeper compiler instrumentation only against a real requirement.

## Adding a language

Require capability matrix, provenance mapping, perturbation profile, at least three real UAT scenarios, explicit unsupported evidence and lifecycle ownership.
