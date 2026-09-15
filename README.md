# Chronos — Execution Intelligence for Coding Agents

Chronos is an agent-first runtime for observing, querying and verifying what a program actually did. The long-term goal is not to reproduce a human debugger UI: it is to give coding agents trustworthy execution evidence, causal context, runtime properties and adaptive instrumentation.

> **Reconstruction status (2026-09-15): convergence in progress.**
> Chronos already has substantial ExecutionLog, invocation-identity, property/Mutation-Lens, query and MCP foundations, but the repository still contains legacy/parallel evidence paths and unfinished adaptive-instrumentation contracts. Official reconstruction M6+ is blocked until the mandatory `REC-C0..REC-C7` convergence gates close.

Current truth and plan:

- `docs/ROADMAP.md`
- `docs/chronos-agentic-reconstruction/docs/reconstruction/CONVERGENCE_PLAN.md`
- `docs/chronos-agentic-reconstruction/docs/roadmap/MILESTONE_ACCEPTANCE.md`
- `reconstruction-contracts.toml` — machine-readable requirement status

## Product principle

**No Silent Lies.** Agent-visible evidence must distinguish known, unknown, unsupported, incomplete, dropped/gapped, estimated and heuristic results. An empty/default success must never hide missing evidence.

## What exists today

### Evidence and replay foundations

- `chronos-log`: append-only ExecutionLog foundation with monotonic `EventSeq`, segmented persistence, durable independent consumer cursors, explicit gaps and compaction.
- invocation identity foundation with `SymbolId` / `InvocationId` and parent relationships.
- query/index/store layers for historical execution analysis.
- runtime property, state-transition / Mutation-Lens and conservative causal-slice foundations.

### Capture/adapters

- native C/C++/Rust tracing through ptrace-based components;
- eBPF components through aya;
- Python, Java, Go and browser adapters at varying capability depth;
- Go currently includes Delve-based debugging support.

**Capability depth is not uniform across languages.** The reconstruction roadmap requires a verified capability matrix and real UAT before a backend is presented as feature-equivalent to another.

### Agent API

- MCP server with legacy tools plus newer v2 dispatcher/services;
- `chronos-services` contains extracted application use cases;
- canonical v2 API convergence and legacy deletion remain active reconstruction work.

## Current architectural convergence

The active `REC-C*` sequence is:

1. **REC-C0** — restore the truth baseline, CI and machine compliance gates.
2. **REC-C1** — make ExecutionLog authoritative and make `events_read` cursor/gap/completeness semantics truthful.
3. **REC-C2** — delete legacy EventBus/query/tripwire delivery paths and dual truth.
4. **REC-C3** — close hexagonal dependency boundaries.
5. **REC-C4** — reduce SOLID/connascence debt and split broad adapter interfaces by capabilities.
6. **REC-C5** — converge on the small canonical Agent API and delete compatibility shims.
7. **REC-C6** — finish promised M1–M4 residuals, especially Go/Rust adaptive instrumentation.
8. **REC-C7** — strict reconstruction close gate; only then unblock official M6.

See the convergence plan for detailed deletion criteria and UAT.

## Target architecture

```text
Driving adapters
  MCP / CLI / tests / future HTTP
          |
          v
Application services
  Session / Probe / Observe / Property / Query / Diff
          |
          v
Domain + ports
  Evidence identities, properties, capability contracts
          ^
          |
Driven adapters
  ExecutionLog/storage / native / eBPF / browser / OTLP / notifications
```

`ExecutionLog` is the target authoritative evidence stream. Query engines and other indexes are projections, not sources of truth.

## Build

Prerequisites:

- Rust stable (see workspace toolchain/Cargo configuration);
- Linux capabilities/permissions as required by ptrace/eBPF tests;
- runtime-specific tools only for the adapter being exercised (for example Delve for Go integration paths).

```bash
cargo build -p chronos-mcp --release
```

Run MCP server:

```bash
./target/release/chronos-mcp --stdio
```

## Verification

Default workspace verification:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -- --test-threads=1
```

Reconstruction architecture/spec gate:

```bash
python3 scripts/check_architecture_contracts.py
```

The active convergence CI additionally compiles all features:

```bash
cargo check --workspace --all-targets --all-features
```

REC-C7 strict close will require:

```bash
python3 scripts/check_architecture_contracts.py --strict-no-gaps
```

plus all mandatory sandbox UATs.

## Roadmap after convergence

Once REC-C7 closes, the official reconstruction sequence resumes with:

- **M6** — OpenTelemetry ingestion/correlation/export;
- **M7** — semantic differential execution;
- **M8** — counterexample shrinking/test intelligence completion;
- **M9** — happens-before concurrency intelligence;
- **M10** — Execution Explorer;
- **M11** — additional language depth.

## Development rule

New features must not bypass the canonical evidence path, add new legacy architecture, or claim a capability without executable UAT evidence. Architecture decisions that change semantics must update ADR/spec, `reconstruction-contracts.toml` and acceptance tests in the same cycle.

## License

MIT
