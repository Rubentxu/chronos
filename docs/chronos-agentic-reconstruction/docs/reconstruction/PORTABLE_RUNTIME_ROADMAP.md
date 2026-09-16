# Portable Execution Runtime — Future Roadmap

Status: FUTURE
Prerequisite: SANDBOX-S0 + real Chronos-on-Chronos adoption

## PORTABLE-P0 — External Project Feasibility

Goal: prove the execution abstraction works for a project outside the Chronos repository.

Deliverables:

- external sample project;
- workspace materialization contract;
- runtime capability negotiation;
- evidence return without guest-path leakage;
- runtime provenance;
- startup/overhead measurement.

Exit: external project can be executed and observed through the same semantic Chronos API used locally.

## PORTABLE-P1 — Cross-OS Local Runtime

Goal: thin host client delegates to a local Linux execution runtime.

Targets:

- Windows -> local Linux VM/runtime;
- macOS -> local Linux VM/runtime;
- Linux -> native/container/VM selectable.

Exit:

- host does not need local ptrace/eBPF tooling;
- same Agent API semantics;
- workspace/artifact mapping deterministic;
- local-first and offline operation retained.

## PORTABLE-P2 — Optional Remote Runtime

Goal: extend the already-proven execution plane over a network when real use cases require it.

Potential capabilities:

- authenticated worker connection;
- capability negotiation;
- live events;
- durable resume/reconnect;
- cancellation;
- content-addressed workspace/artifact transfer.

Requires a dedicated security ADR before implementation.

## Rule

Do not implement P1/P2 to solve hypothetical portability. Each stage is unlocked only by measured friction or a real project consumer.
