# Reconstruction overview

## Executive decision

Chronos should stop positioning itself primarily as a **time-travel debugger exposed through MCP** and become an **execution-intelligence runtime for coding agents**.

A traditional debugger exposes machine/human primitives:

```text
set breakpoint -> continue -> pause -> read locals -> step
```

Chronos should expose investigation primitives:

```text
observe -> read evidence -> query execution -> define property
-> detect violation -> slice causality -> compare behaviour
-> request deeper evidence -> verify correction
```

Breakpoints, ptrace stops, watchpoints and replay engines may remain internal when they are the cheapest mechanism for a required fact. They are not the public contract.

## Why reconstruct instead of extend

The current codebase contains useful subsystems, but important concepts are in transition:

- two event transport models coexist;
- consumers can destructively drain shared buffers;
- snapshots are not consistently cumulative;
- raw and semantic buffers can diverge;
- event names are duplicated as strings;
- the MCP server owns substantial application logic;
- language adapter contracts expose unsupported operations by default;
- tripwires are not yet connected end-to-end to the live event path;
- eBPF probe lifecycle is not modeled as session-owned infrastructure;
- some analysis paths can silently return plausible but wrong answers when evidence is absent.

Adding more tools on top of those contracts would increase later migration cost.

## Reconstruction thesis

1. Make evidence trustworthy.
2. Introduce one append-only ExecutionLog.
3. Give every consumer an independent cursor.
4. Turn call/state/causality views into replayable projections.
5. Introduce invocation identity.
6. Add runtime properties and mutation evidence.
7. Add adaptive instrumentation.
8. Reuse OpenTelemetry/zero-code/runtime instrumentation before inventing language-specific machinery.
9. Simplify the agent API.
10. Add UI and advanced differential reasoning only after the vertical slice is proven.

## Keep and strengthen

- `chronos-domain` for stable domain contracts;
- `chronos-query` concepts, evolved toward incremental/replayable projections;
- `chronos-index` separated from transport;
- `chronos-store`, reoriented toward log segments/checkpoints/artifacts;
- `chronos-ebpf`, narrowed to debugging-specific passive probes and external eBPF integration;
- `chronos-native` as deep/fallback backend;
- language adapters where they contain reusable runtime knowledge;
- MCP as a driving adapter;
- persistent sessions;
- comparison/diff concepts;
- `chronos-sandbox` as an acceptance-test asset.

## Keep internally, not as primary UX

- ptrace;
- breakpoints;
- watchpoints;
- expression evaluation;
- stack inspection;
- rr/GDB-style deep debugging when integrated.

## Reject as target architecture

### Ring buffer as authoritative storage

A ring may exist at a producer boundary, but it cannot be the truth if eviction is silent or consumers steal records.

### Universal semantic eBPF

eBPF is excellent at kernel/system/native boundaries. It is not a universal semantic decoder for arbitrary Python/JVM/Rust/Go objects. Runtime-native or compile-time evidence is preferred when more trustworthy.

### "Hash means O(1) semantic divergence"

Hashes accelerate equality checks. They do not solve alignment across loops, async work, scheduling differences or semantically equivalent traces.

### Delete the sandbox

The sandbox already contains expensive integration knowledge. It becomes the executable acceptance specification.

## Product statement

> **Chronos gives coding agents eyes inside a running program and enough provenance to distinguish observed facts from derived or hypothesized explanations.**

## Non-goals for reconstruction

- replace every production observability stack;
- build a general graph database;
- implement a universal debugger protocol;
- create a full IDE;
- implement deterministic record/replay from scratch;
- capture every variable of every function;
- support every language with identical depth;
- let an LLM execute arbitrary code inside eBPF.

## Emergent-architecture rule

Each milestone may change implementation details while preserving accepted ADRs, public evidence semantics and UAT gates. If a spike invalidates an ADR assumption, supersede it explicitly.
