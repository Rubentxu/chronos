# Chronos — Execution Intelligence for Coding Agents

> **Status:** reconstruction roadmap. The current implementation remains an experimental capture/query MCP server while the architecture converges on the model documented under `docs/`.

Chronos is an **execution-intelligence runtime for coding agents**. Its purpose is not to reproduce the human IDE debugger experience. Its purpose is to let an agent answer, with explicit evidence:

- What happened?
- What state changed?
- Which concrete invocation caused the change?
- What was the first meaningful divergence from a known-good execution?
- What evidence is missing?
- Which additional observation is the cheapest way to obtain it?
- Did a proposed correction actually remove the failing behaviour without introducing an unexpected regression?

The long-term interaction is not:

```text
breakpoint -> stop -> inspect -> step -> inspect -> guess
```

but:

```text
run -> observe -> detect anomaly -> slice causal evidence
    -> hypothesis -> add cheapest missing observation
    -> rerun -> prove/disprove -> patch -> verify
```

## Product principles

1. **Agent-first, not IDE-first.** Breakpoints and stepping may remain internal escalation mechanisms, but they are not the public mental model.
2. **No silent lies.** Missing, dropped, unsupported, partial and heuristic evidence are represented explicitly.
3. **ExecutionLog is the source of truth.** Events are append-only and receive a monotonic sequence. Consumers read with independent cursors.
4. **Graphs and indexes are projections.** Call, state, causality and performance views can be rebuilt from the log.
5. **Reuse observability before adding probes.** Existing OpenTelemetry, zero-code agents and runtime-native telemetry are evidence sources.
6. **Adaptive instrumentation.** Chronos starts cheap and increases resolution only where a hypothesis requires it.
7. **Semantic probes are temporary by default.** Debug instrumentation is generated in an isolated debug build/worktree or attached dynamically; product source stays clean.
8. **Evidence beats explanation.** An LLM may hypothesize; Chronos preserves provenance for the facts used to confirm or reject it.
9. **Emergent architecture.** Backends are capability-driven; a language is not forced into a universal tracing mechanism.
10. **Real bugs gate progress.** Every milestone proves value in the sandbox/UAT corpus, not only unit tests.

## Target architecture

```text
                           Coding Agent
                               |
                        Investigation API
                               |
                   Hypothesis / Probe Planner
                               |
        +----------------------+----------------------+
        |                      |                      |
  Existing semantics      Passive/zero-code      Deep escalation
  OTel/JFR/runtime        OBI/eBPF/uprobe         ptrace/rr/etc.
        |                      |                      |
        +----------------------+----------------------+
                               |
                         ExecutionLog
                     append-only + durable
                               |
        +----------------------+----------------------+
        |                      |                      |
 Invocation projection   State projection      Causality projection
        |                      |                      |
        +----------------------+----------------------+
                               |
                         Evidence Engine
                               |
                 MCP / CLI / future HTTP+WS UI
```

## Near-term language strategy

### Go — reference backend

- reuse existing OpenTelemetry spans when present;
- OpenTelemetry eBPF/OBI for zero-code coarse observation;
- Go Auto SDK to correlate manual spans with eBPF-derived spans;
- Go compile-time instrumentation (`otelc`) for debug builds without source edits;
- Chronos-specific semantic probes only for evidence OTel cannot express.

### Rust — co-priority backend

- reuse existing `tracing`/OpenTelemetry when present;
- OBI for generic zero-code protocol/network observation;
- eBPF uprobes/tracepoints for passive native evidence;
- LLVM XRay through Rust's unstable `-Z instrument-xray` as a spike/debug-build path;
- USDT or temporary source-overlay probes for high-value semantic events;
- ptrace/rr only for deep escalation.

Python (`sys.monitoring`), JVM (JFR first), JavaScript/Node, browser/WASM and other languages follow after the core contracts are proven.

## Start here

- [Reconstruction overview](docs/reconstruction/00_RECONSTRUCTION_OVERVIEW.md)
- [Target architecture](docs/architecture/TARGET_ARCHITECTURE.md)
- [ExecutionLog specification](docs/specs/EXECUTION_LOG.md)
- [Adaptive instrumentation](docs/specs/ADAPTIVE_INSTRUMENTATION.md)
- [Evidence and trust](docs/specs/EVIDENCE_AND_TRUST.md)
- [Agent API v2](docs/specs/AGENT_API_V2.md)
- [Roadmap](docs/roadmap/ROADMAP.md)
- [Milestone acceptance](docs/roadmap/MILESTONE_ACCEPTANCE.md)
- [Issue-ready implementation backlog](docs/roadmap/IMPLEMENTATION_BACKLOG.md)
- [UAT strategy](docs/testing/UAT_STRATEGY.md)
- [Sandbox bug corpus](docs/testing/SANDBOX_BUG_CORPUS.md)
- [Migration](docs/migration/CURRENT_TO_TARGET.md)
- [ADR index](docs/adr/README.md)

## Current implementation

The current repository already contains valuable building blocks: a multi-crate Rust workspace, native/eBPF/runtime/browser adapters, domain trace and semantic events, query/index/store crates, persistent sessions, MCP tools and a substantial sandbox/integration harness.

The reconstruction is deliberately **convergent**: preserve useful code, correct contracts and failure semantics, then grow the new vertical slice. Avoid a flag-day rewrite.

## Development gates

A change is not complete because it compiles. A milestone closes only when its required deterministic tests and real UAT scenarios pass.

## Documentation status

The historical documents in `docs/propuestas/` influenced this reconstruction but contain decisions now superseded. See [Legacy documentation disposition](docs/migration/LEGACY_DOCS_DISPOSITION.md).

## License
MIT
