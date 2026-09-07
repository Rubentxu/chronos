# Technology baseline and research notes

Research snapshot: September 2026. Revalidate versions when implementing.

## OpenTelemetry eBPF Instrumentation (OBI)

Use as preferred upstream source for generic zero-code Linux application/protocol telemetry when capabilities fit.

Current official docs describe broad Linux language/runtime support including Go and Rust, application/network/protocol telemetry and no source change, while explicitly noting that eBPF does not replace language/manual instrumentation for application-specific semantics.

Chronos implication: **integrate, do not clone OBI's generic scope**.

Reference: https://opentelemetry.io/docs/zero-code/obi/

## Go zero-code and Auto SDK

Go Auto SDK correlates manual OTel spans with eBPF auto-instrumented spans under shared trace context.

Chronos implication: ingest/correlate this context and avoid duplicate operation spans.

Reference: https://opentelemetry.io/docs/zero-code/go/autosdk/

## Go compile-time instrumentation

Official docs describe `otelc` compile-time instrumentation that injects lightweight hooks during build and can cover third-party dependencies without source edits.

Chronos implication: best first testbed for `InstrumentationSpec -> deterministic debug build`.

Reference: https://opentelemetry.io/docs/zero-code/go/compile-time/

## Rust OpenTelemetry

Current official status lists traces, metrics and logs as Beta.

Chronos implication: reuse it when present; do not require it as universal Rust debugging substrate.

Reference: https://opentelemetry.io/docs/languages/rust/

## Rust XRay

Rust Unstable Book documents `-Z instrument-xray`, generating XRay NOP sleds. It is nightly/unstable and requires a suitable runtime.

Chronos implication: high-value debug-build spike, not early stable dependency.

Reference: https://doc.rust-lang.org/beta/unstable-book/compiler-flags/instrument-xray.html

## LLVM XRay

Compiler-inserted instrumentation points + runtime patch/unpatch + trace tooling. FDR uses fixed-size circular buffers.

Chronos implication: reuse its activation model rather than inventing function tracing for instrumentable native builds.

Reference: https://llvm.org/docs/XRay.html

## Rust USDT

The Rust `usdt` ecosystem exposes userland statically-defined probes observable from Linux tracing tools such as bpftrace.

Chronos implication: promising semantic debug-build probe mechanism using stable semantic IDs instead of raw offsets.

Reference: https://github.com/oxidecomputer/usdt

## Python sys.monitoring

Python >=3.12 provides execution events, tool IDs and configurable activation.

Chronos implication: prefer to legacy `sys.settrace` on supported versions.

Reference: https://docs.python.org/3/library/sys.monitoring.html

## Java JFR

`RecordingStream` exposes events from the running JVM.

Chronos implication: low-intrusion runtime evidence before deep JDWP interaction.

Reference: https://docs.oracle.com/en/java/javase/25/docs/api/jdk.jfr/jdk/jfr/consumer/RecordingStream.html

## Linux BPF ring buffer

Kernel ring buffer is MPSC and addresses ordering of sequential events across CPUs.

Chronos implication: useful producer transport, but not the durable multi-consumer ExecutionLog.

Reference: https://www.kernel.org/doc/html/next/bpf/ringbuf.html
