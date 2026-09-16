# Chronos Verification Lab

Status: PROPOSED / SANDBOX-S0
Owner gate: post-REC-C0, introduced progressively from REC-C1
Non-blocking for REC-C0 closure

## Purpose

Chronos should progressively become its own execution-intelligence test harness. The goal is not to replace unit/integration tests or coverage, but to add a reproducible execution laboratory for real runtime verification.

The core dogfooding model is **Chronos-on-Chronos**:

```text
Chronos Observer (A)
        |
        | observes
        v
+-----------------------------+
| Clean Execution Environment |
|                             |
|  Chronos Subject (B)        |
|          |                  |
|          v                  |
|      test fixture           |
|                             |
+-----------------------------+
```

Chronos B verifies semantic/runtime contracts from the inside. Chronos A independently observes Chronos B from the outside. This gives two evidence planes:

```text
Inside evidence   -> semantic correctness
Outside evidence  -> runtime/process/resource behaviour
```

The Verification Lab must grow only when a roadmap milestone needs it. It is not a standalone platform initiative.

## Why this exists

Several Chronos capabilities are difficult to validate reliably on arbitrary developer or CI hosts:

- `ptrace`
- eBPF / uprobes
- signals and process lifecycle
- child-process behaviour
- kernel/security configuration
- crash recovery
- runtime resource cleanup
- timing-sensitive observation

Coverage tools also alter execution characteristics. Coverage remains a separate quality signal; the Verification Lab exists to prove behaviour in clean execution environments.

## Design principles

1. **Clean before clever** — every scenario starts from a known environment.
2. **Capabilities are explicit** — never discover privileged functionality only by failing an operation.
3. **Evidence remains truthful** — unsupported/partial environments must not produce false PASS.
4. **One scenario, multiple backends** — scenario semantics must not depend on Podman/QEMU-specific APIs.
5. **No production dependency on test infrastructure** — Bubblewrap/Podman/QEMU are driven adapters around test/runtime orchestration.
6. **Local-first** — developers can run the useful subset locally without a cloud service.
7. **Progressive isolation** — use the cheapest environment that satisfies the scenario.
8. **No hidden host coupling** — filesystem, environment variables, network and capabilities are declared.
9. **Chronos-on-Chronos state isolation** — observer and subject never share stores, logs or session namespaces.
10. **The lab must serve the roadmap** — no infrastructure work without a concrete verification consumer.

## Execution environment levels

### PROCESS — Bubblewrap

Best for fast deterministic scenarios where a shared kernel is acceptable.

Typical controls:

- clean `$HOME`
- private `/tmp`
- readonly workspace/input
- writable artifacts directory
- isolated PID/IPC/user/mount namespaces where supported
- network disabled by default
- explicit environment allow-list

Expected uses:

- MCP/API behaviour
- ExecutionLog/replay
- state/query semantics
- crash/error handling
- process trees not requiring privileged tracing
- property/invariant tests

### SYSTEM — Podman

Best for reproducible userspace and package/runtime composition.

Provides:

- pinned base image
- pinned toolchain/dependencies
- clean filesystem/users
- explicit Linux capabilities
- disposable volumes
- reproducible CI/local execution

Use special profiles rather than global privilege, for example:

```text
container-default
container-ptrace
container-ebpf-candidate
```

Do not use `--privileged` as the general solution.

### KERNEL — QEMU/KVM

Best for tests whose contract depends materially on kernel behaviour.

Required for strong verification of:

- ptrace edge cases
- eBPF/uprobe lifecycle
- perf/kernel facilities
- sysctl/security configuration
- kernel-version compatibility

Desired model:

```text
golden image
    -> ephemeral clone/snapshot
    -> boot known kernel/userspace
    -> inject Chronos build
    -> execute scenario
    -> collect evidence
    -> destroy
```

Firecracker or other microVM technology may be evaluated later only if VM throughput becomes a measured bottleneck.

### HOST — explicit diagnostic profile

Host execution remains useful for local diagnostics, but is never considered a reproducible acceptance environment unless the scenario explicitly declares the required host fingerprint/capabilities.

## Environment abstraction

The domain/application code must depend on a small port, not directly on Bubblewrap/Podman/QEMU.

Conceptual shape:

```rust
pub trait ExecutionEnvironment {
    fn capabilities(&self) -> EnvironmentCapabilities;
    fn prepare(&self, scenario: &Scenario) -> Result<PreparedEnvironment>;
    fn execute(&self, request: ExecutionRequest) -> Result<ExecutionHandle>;
    fn collect(&self, handle: &ExecutionHandle) -> Result<EvidenceBundle>;
    fn destroy(&self, handle: ExecutionHandle) -> Result<()>;
}
```

Adapters:

```text
ExecutionEnvironment port
        ^
        |
+-------+--------+---------+
|                |         |
Bubblewrap      Podman    QEMU/KVM
```

The exact Rust API is intentionally deferred until SANDBOX-S0 characterization proves the minimum useful contract.

## Environment capabilities

Capabilities should be typed and observable before scheduling a scenario.

Candidate model:

```text
ptrace
process_control
ebpf
uprobes
perf_events
namespaces
cgroup_v2
network_isolation
immutable_root
kernel_control
snapshot_restore
```

Scheduling rule:

```text
Scenario requirements
        +
Environment capabilities
        -> deterministic placement decision
```

A missing capability yields `Unsupported` / `CapabilityUnavailable`, never a silent skip or fake PASS.

## Experiment model

A Verification Lab scenario should be describable independently of its environment backend.

Example:

```yaml
name: events-read-gap-recovery

requires:
  - process_control

environment:
  class: system

subject:
  binary: chronos-mcp

observer:
  mode: chronos

fixture:
  binary: fixture-event-flood

faults:
  - pause_consumer:
      at_event: 5000
  - overflow:
      records: 10000

assert:
  - event_seq_monotonic
  - independent_consumers
  - gap_is_explicit
  - completeness_never_complete_after_gap
  - no_process_leak
```

The YAML is illustrative, not yet a public schema.

## Chronos-on-Chronos identities

Observer and subject must have distinct identities and stores.

Candidate identities:

```text
ExperimentId
SubjectInstanceId
ObserverInstanceId
ObservationRole { Subject, Observer }
```

Isolation invariant:

```text
Observer A:
  store A
  ExecutionLog A
  SessionId namespace A

Subject B:
  store B
  ExecutionLog B
  SessionId namespace B
```

No shared redb file, no shared execution-log directory and no implicit global session namespace.

## Initial dogfooding scenarios

### DOG-001 — Session lifecycle

Prove:

```text
session_start
-> log created
-> fixture executes
-> session_stop
-> no child process leak
-> no probe/resource leak
```

Target adoption: REC-C1.

### DOG-002 — ExecutionLog / cursor integrity

Exercise:

```text
10,000 events
reader A pauses
reader B continues
A resumes
subject restarts
```

Prove:

- EventSeq monotonicity
- independent consumers
- exact resume
- checkpoint/replay
- explicit gap semantics when loss is forced

Target adoption: REC-C1.

### DOG-003 — Crash recovery / truthful tails

Force abrupt termination of the subject.

Prove:

- recoverable ExecutionLog
- incomplete/sealed-tail semantics are truthful
- session does not become falsely Complete

Target adoption: REC-C2 or immediately after REC-C1 durable lifecycle is available.

### DOG-004 — Probe lifecycle

Exercise real attach/event/detach in a controlled kernel-capable environment.

Prove no residual:

- traced process relationship
- uprobe attachment
- child process
- temporary runtime resource

Target adoption: M4 adaptive instrumentation / privileged verification.

### DOG-005 — No Silent Lies

Deliberately remove or lose evidence.

Prove the result becomes one of:

```text
Partial
GapDetected
Unsupported
Unknown
```

and never an unjustified `Complete`/PASS.

This should become a flagship acceptance scenario for the reconstruction program.

## SANDBOX-S0 characterization spike

SANDBOX-S0 is an investigation milestone, not a production platform milestone.

### Goals

Characterize Bubblewrap, Podman and QEMU/KVM against real Chronos needs.

### Required experiments

1. **S0-PROCESS** — clean userspace/process scenario with Bubblewrap.
2. **S0-SYSTEM** — same scenario in a pinned Podman image.
3. **S0-KERNEL** — one ptrace/uprobe-capable scenario in an ephemeral QEMU/KVM guest.
4. Compare startup time, failure diagnostics, cleanup, reproducibility and capability detection.
5. Demonstrate artifact/evidence collection independent of backend.

### Deliverables

- measured comparison notes
- capability matrix
- proposed minimum `ExecutionEnvironment` port
- scenario/result schema draft
- local developer command(s)
- CI strategy
- decision ADR accepting/rejecting each backend for specific classes

### Exit criteria

SANDBOX-S0 is complete only when:

- the same logical scenario can execute on at least two environment backends;
- unsupported capabilities are reported explicitly;
- clean-start and cleanup behaviour is demonstrated;
- evidence/artifacts are preserved outside the ephemeral environment;
- the chosen backend mapping is justified by measurements, not preference.

## Relationship to coverage

Coverage and runtime verification answer different questions.

```text
Coverage
  -> which source paths were exercised?

Verification Lab
  -> did the real runtime contract hold in a controlled system?
```

Do not make privileged/runtime acceptance depend on a coverage tracer. Coverage-engine compatibility should be solved in the coverage workflow independently.

## Roadmap integration

The lab is introduced incrementally:

```text
REC-C0.5-E
  coverage-engine compatibility only

SANDBOX-S0
  environment characterization; non-blocking for REC-C0

REC-C1
  DOG-001 + DOG-002 as ExecutionLog/events_read UAT

REC-C2
  DOG-003 while deleting legacy truth paths

M4
  KERNEL profile + DOG-004 for ptrace/eBPF/adaptive instrumentation

Cross-cutting
  DOG-005 No Silent Lies regression scenario
```

No future milestone should require all environment classes when a cheaper class can prove the contract.

## Non-goals

SANDBOX-S0 does not include:

- a cloud execution service;
- a generic CI platform;
- Kubernetes orchestration;
- Firecracker adoption without measured need;
- arbitrary LLM-generated sandbox policy;
- replacing Cargo tests;
- replacing coverage;
- moving production domain logic into sandbox code.

## Risks

### Sandbox becomes a platform project

Mitigation: every implementation slice must name the roadmap/UAT scenario consuming it.

### Containers create false kernel reproducibility

Mitigation: classify kernel-dependent scenarios and execute them in the KERNEL profile.

### Excessive privilege

Mitigation: capability-specific profiles; no blanket privileged mode as default.

### Observer affects subject

Mitigation: record perturbation/provenance and compare observer-off/observer-on where the contract is timing-sensitive.

### Chronos-on-Chronos causal confusion

Mitigation: explicit role/instance/experiment identity and physically separate stores/logs.

## Success definition

The Verification Lab succeeds when Chronos can answer not only:

> "Do our tests pass?"

but also:

> "In a known clean system, what actually happened inside Chronos while it observed another program, and can an independent Chronos instance prove that behaviour?"
