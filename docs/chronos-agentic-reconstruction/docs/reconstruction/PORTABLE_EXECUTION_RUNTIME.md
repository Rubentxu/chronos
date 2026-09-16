# Portable Execution Runtime — Future Extension of the Verification Lab

Status: FUTURE / EXPLORATORY
Depends on: SANDBOX-S0 proven execution abstraction
Does not block: REC-C0, REC-C1, REC-C2

## Intent

The execution-environment abstraction created for the Chronos Verification Lab may later become an optional **portable execution mode** for external projects.

The goal is to make powerful Chronos capabilities available without requiring every developer host to install or support Linux-specific tracing stacks, eBPF tooling, ptrace configuration, kernel packages, or a complete Chronos runtime locally.

Conceptually:

```text
Developer host
Windows / macOS / Linux
        |
        | Chronos client / MCP / Agent API
        v
Execution Runtime
        |
        +-- local Bubblewrap
        +-- local Podman
        +-- local Linux VM
        +-- remote worker (future)
        |
        v
Project under observation
```

The host remains the control plane. Observation executes where the required capabilities exist.

## Product value

This could make Chronos usable in situations such as:

- Windows project with Linux-compatible build/test path;
- macOS developer using Chronos' Linux runtime capabilities without local kernel instrumentation;
- immutable developer distributions where installing tracing dependencies is undesirable;
- CI workers with prebuilt Chronos execution images;
- teams wanting a disposable clean debugging environment;
- agent workflows that need a reproducible runtime rather than the developer's mutable machine.

## Architectural direction

Chronos should keep a logical separation between:

```text
Control Plane
  session intent
  scenario definition
  capability requirements
  Agent API / MCP
  evidence/query UX

Execution Plane
  workspace materialization
  subject process
  probes/tracers
  ExecutionLog producer
  runtime dependencies
```

The initial Verification Lab may run both planes on one machine. A future portable runtime may separate them physically.

## Required properties

### Capability negotiation

The client asks for capabilities, not a concrete backend.

Example intent:

```text
requires:
  ptrace: true
  ebpf: false
  network: isolated
  clean_filesystem: true
```

The runtime chooses or rejects an execution backend deterministically.

### Workspace transport

Projects need an explicit materialization contract rather than assumptions about host paths.

Potential strategies to evaluate later:

- bind mount for local Linux/Podman;
- virtiofs/9p for local VM;
- content-addressed archive/sync for remote workers;
- Git checkout + declared dirty patch bundle.

Host absolute paths must not become protocol identity.

### Portable evidence

Execution results returned to the control plane should use stable logical identities and content-addressed artifacts where useful.

The control plane must not require direct access to guest paths to interpret:

- ExecutionLog records;
- scenario outcome;
- provenance;
- stdout/stderr artifacts;
- snapshots/checkpoints;
- probe metadata.

### Runtime identity

Every delegated run must record enough provenance to answer:

```text
Which Chronos version?
Which runtime image?
Which OS/kernel?
Which project revision/digest?
Which capabilities?
Which instrumentation plan?
```

### Local-first

Portable execution is optional.

If the host already supports the required capabilities, Chronos can run locally. A VM/container/runtime is selected when it improves reproducibility, portability or capability availability.

### No hidden cloud requirement

A remote worker may be added later, but Chronos must retain a fully local mode.

## Suggested future runtime profiles

```text
native
sandbox-process
sandbox-container
sandbox-vm
remote-runtime   # future
```

These are execution-placement choices, not different Agent APIs.

## User experience target

A future user-facing command might conceptually look like:

```text
chronos run --runtime auto -- cargo test
chronos run --runtime container -- pytest
chronos run --runtime vm --requires ptrace ./integration-suite
```

or through the Agent API:

```text
session_start(
  project = ...,
  execution = {
    placement = "auto",
    requires = ["ptrace"]
  }
)
```

Exact CLI/API design is deferred.

## Relationship to Chronos-on-Chronos

Chronos-on-Chronos is the proving ground for this architecture.

If an Observer can control and understand a Subject Chronos inside a disposable execution environment without relying on shared host state, the same seams become candidates for external project execution.

Therefore the order is deliberately:

```text
Verification Lab
    -> prove execution abstraction
    -> dogfood with Chronos-on-Chronos
    -> stabilize workspace/evidence/provenance contracts
    -> only then expose Portable Execution Runtime externally
```

## Non-goals for current roadmap

Do not implement yet:

- remote worker scheduler;
- cloud control plane;
- multi-tenant execution;
- generic container platform;
- cross-platform UI installer;
- arbitrary workload hosting.

These become justified only after local lab usage proves the abstractions.

## Future investigation gates

### PORTABLE-P0 — Feasibility

After SANDBOX-S0 and at least DOG-001/DOG-002:

- run a real external sample project inside a SYSTEM or KERNEL runtime;
- invoke it from outside the runtime;
- return evidence without guest-path leakage;
- measure setup/startup overhead;
- identify minimum host dependencies.

### PORTABLE-P1 — Cross-OS client spike

When control/execution separation is mature:

- Windows client -> local Linux VM/runtime;
- macOS client -> local Linux VM/runtime;
- same Agent API contract as native Linux;
- workspace and artifact mapping demonstrated.

### PORTABLE-P2 — Optional remote worker

Only if demanded by real workflows:

- authenticated transport;
- execution capability negotiation;
- artifact transfer;
- live event streaming;
- cancellation/reconnect;
- no change to semantic evidence contracts.

## Decision rule

This future product direction is accepted only if the Verification Lab demonstrates that execution placement can change without changing the meaning of Chronos evidence.
