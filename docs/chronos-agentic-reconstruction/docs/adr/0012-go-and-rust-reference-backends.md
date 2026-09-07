# ADR-0012 — Go and Rust are the near-term reference backends

**Status:** Accepted

## Decision

After core evidence contracts, prove the adaptive model in Go and Rust before broadening equally to every language.

Go validates OBI + Auto SDK + compile-time instrumentation.

Rust validates OBI + native eBPF + existing tracing/OTel + XRay/USDT/debug-build research.

## Consequences

Python/JVM/browser remain supported/evolved, but new deep features are not required to land in all languages simultaneously.
