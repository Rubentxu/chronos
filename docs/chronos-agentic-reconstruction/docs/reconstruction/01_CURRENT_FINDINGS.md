# Current architecture findings and technical debt baseline

Baseline: `c76b1096dfc25de666a32bbb56992d4190e5aee3`.

This document translates the source review into implementation priorities. Reconfirm line-level details before modifying code.

## P0 — correctness / trust blockers

### Destructive live reads

Current live probe draining consumes the whole available semantic buffer before applying paging parameters. A small requested page can therefore discard unseen evidence for that consumer.

Required outcome: cursor reads over ExecutionLog; no shared global drain.

### Snapshot replacement instead of cumulative refresh

Current session snapshot flow can build a query engine from the latest drained raw batch and replace the previous engine, creating a false impression that the whole session is indexed.

Required outcome: incremental projection cursor/checkpoint.

### eBPF probe ownership/lifecycle

The current MCP probe-injection path can construct an eBPF adapter locally while the adapter itself owns link objects that must stay alive to keep probes attached.

Required outcome: `ProbeHandle`/backend lifetime owned by `SessionService`/`ProbeRegistry` until explicit detach.

### Tripwire disconnected/incomplete state

Current tripwire management has label/fire-count/state inconsistencies and is not reliably wired into the live event append path.

Required outcome: tripwire is evaluated on canonical evidence flow and produces an auditable fired record.

### State-diff/register contradiction

Register events can be filtered as generic noise while state-diff logic depends on register evidence.

Required outcome: projection-specific evidence retention rules, not generic filtering that removes required facts.

### Invalid event filter broadening

Manual string mapping supports only part of the domain `EventType` set. Unknown strings can be silently dropped, potentially making a query broader than requested.

Required outcome: generated/typed schema + validation error.

### Missing event target can return plausible call stack

A query path can default to a thread when the target event is absent.

Required outcome: `NotFound`/`Unsupported`, never plausible fallback.

## P1 — architecture convergence

### Two adapter contracts

A canonical `ProbeBackend` coexists with a much broader capture `TraceAdapter` whose optional debugger capabilities often default to unsupported.

SOLID impact:

- ISP: backends depend on methods they do not meaningfully support;
- LSP: successful trait conformance does not mean operations are usable;
- OCP: new capability often means expanding the broad interface.

Target: capability ports and runtime capability negotiation.

### Two event-transport abstractions

An mpsc capture pipeline and a domain ring/bus coexist.

Target: internal transports are adapter implementation details; application boundary is ExecutionLog append.

### MCP server is an application-layer God object

`chronos-mcp::server` holds session, engines, stores, probes, tripwires and large tool-handler algorithms.

SOLID impact: SRP/DIP.

Target: thin transport adapter over application services.

### Store depends on native concern

Optional address-normalization/native dependency in persistence violates strict dependency direction.

Target: address normalization as domain/application port; store persists normalized metadata/artifacts without importing tracing infrastructure.

### Network/webhook concern in domain

Domain should not need HTTP infrastructure merely because a tripwire action can call a webhook.

Target: domain action intent + driven notification adapter.

## Connascence baseline

### Connascence of Name

- event names duplicated between enums and MCP string parsing;
- IDs encoded/parsed through string prefixes.

Mitigation: typed schemas and strong IDs.

### Connascence of Meaning

`timestamp_ns` semantics are not consistently aligned with the domain documentation.

Mitigation: explicit session-monotonic vs wall-clock fields.

### Connascence of Identity

Parallel raw/semantic buffers need implicit identity agreement.

Mitigation: one ExecutionRecord source + derived/enriched projections linked by stable sequence/record IDs.

### Connascence of Execution/Timing

Correctness depends on drain -> stop -> index ordering and backend background-thread completion.

Mitigation: explicit lifecycle state machine and acknowledgements.

### Connascence of Algorithm

Call stack reconstruction depends on balanced FunctionEntry/FunctionExit and aggregate call graph logic relies on stack simulation.

Mitigation: explicit `InvocationId`, incomplete invocations and backend evidence quality.

## Existing strengths to preserve

- workspace separation already provides useful seams;
- `chronos-query -> domain + index` direction is relatively clean;
- `chronos-index -> domain` is clean;
- semantic resolver pipeline is extensible in concept;
- browser latest refactor already moves toward agent-first capture and reduces duplicated adapter responsibility;
- sandbox tests encode significant integration behaviour;
- persistent sessions and comparison concepts are strong foundations.

## Product-level smells

### Tool count as a feature

A growing MCP tool count is not user value. It increases agent discovery/composition cost and string-coupled API surface.

### "Time travel" overclaim

Event reconstruction is valuable but should not be marketed as deterministic instruction replay unless a backend such as rr provides that evidence.

### Race detection naming

A time-window same-address heuristic is useful triage but not a proof of a data race.

### Call graph naming

The current aggregate caller/callee graph is useful analytics, but not a concrete dynamic execution graph because it lacks invocation identity, duration, async links and state causality.

## First reconstruction issue batch

Create implementation issues directly from M0:

1. non-destructive event pagination;
2. cumulative session projection refresh;
3. session-owned eBPF probe lifetime;
4. tripwire live-flow wiring + state correctness;
5. state-diff evidence preservation;
6. typed event filters;
7. explicit query NotFound semantics;
8. timestamp semantics tests;
9. rename/classify race heuristic;
10. establish M0 sandbox acceptance suite.
