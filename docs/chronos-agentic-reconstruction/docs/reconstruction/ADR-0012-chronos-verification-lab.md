# ADR-0012 — Chronos Verification Lab and Chronos-on-Chronos Dogfooding

- Status: Proposed
- Decision scope: Reconstruction verification architecture
- Introduced by: SANDBOX-S0
- Blocks REC-C0: No
- Earliest adoption: REC-C1

## Context

Chronos increasingly depends on runtime behaviours that are difficult to verify reliably on arbitrary developer or CI hosts: ptrace, eBPF/uprobe lifecycle, process trees, signals, crash recovery, kernel/security configuration and timing-sensitive observation.

The existing `chronos-sandbox` is valuable and must be preserved, but it currently acts primarily as a Rust integration-test harness. Container packaging is also currently focused on running `chronos-mcp`, not on reproducible runtime experiments.

REC-C0 has also exposed an important distinction: coverage instrumentation and runtime verification are different concerns. A coverage tracer can perturb or conflict with ptrace/runtime tests even when the product behaviour itself is correct.

Chronos should therefore develop a controlled execution laboratory that can run real scenarios in clean environments and, where useful, use one Chronos instance to observe another.

## Decision

Adopt **Chronos Verification Lab** as a progressive verification architecture, not as a standalone platform project.

The laboratory will provide three primary execution classes:

1. **PROCESS** — Bubblewrap or equivalent namespace sandbox for fast clean-process scenarios.
2. **SYSTEM** — Podman for reproducible userspace/system scenarios.
3. **KERNEL** — QEMU/KVM ephemeral VM for kernel-dependent ptrace/eBPF/UAT.

Host execution remains available as an explicit diagnostic profile but is not considered reproducible acceptance evidence by default.

A small application port, provisionally named `ExecutionEnvironment`, will separate scenario orchestration from concrete sandbox technologies. Its exact API will be finalized only after SANDBOX-S0 characterization.

Chronos will additionally adopt **Chronos-on-Chronos** dogfooding where a Chronos Observer independently observes a Chronos Subject running a real fixture or scenario.

Observer and Subject MUST have physically separate stores, ExecutionLogs and session namespaces.

## Decision rules

### Cheapest sufficient environment

A scenario must execute in the cheapest environment class that can prove its contract:

```text
PROCESS -> SYSTEM -> KERNEL
```

Escalation happens only when a required capability is unavailable or when the contract materially depends on the stronger isolation/kernel control.

### Explicit capabilities

Execution environments advertise capabilities before scenario placement. Missing capabilities yield an explicit Unsupported/CapabilityUnavailable result rather than a silent skip or fake pass.

### Sandbox code remains outside the domain

Bubblewrap, Podman, QEMU/KVM and related process/runtime APIs are adapters. They must not leak into the domain model.

### Coverage is separate

Coverage tooling remains responsible for source coverage only. The Verification Lab is responsible for controlled runtime/UAT evidence. A runtime test is not reclassified as functional debt merely because a coverage engine perturbs it.

### Progressive adoption

SANDBOX-S0 is a characterization spike. Implementation grows only when a roadmap scenario consumes it.

## Initial roadmap consumers

- REC-C1: DOG-001 Session lifecycle, DOG-002 ExecutionLog/cursor integrity.
- REC-C2: DOG-003 crash recovery/truthful incomplete tails.
- M4: DOG-004 real probe lifecycle in kernel-capable environment.
- Cross-cutting: DOG-005 No Silent Lies.

## Consequences

### Positive

- reproducible runtime acceptance evidence;
- clearer separation of deterministic, privileged and kernel-dependent tests;
- less dependence on arbitrary host state;
- direct dogfooding of Chronos' own evidence model;
- improved capability-aware testing;
- a foundation for agent-driven experiments without making CI itself the product.

### Negative

- multiple environment backends increase operational complexity;
- VM-based UAT is slower than ordinary Cargo tests;
- Chronos-on-Chronos can perturb timing-sensitive workloads;
- scenario/evidence identity becomes more important.

## Risks and mitigations

### Platform-project creep

Every implementation slice must identify a concrete roadmap/UAT consumer. No generic orchestration features without a measured need.

### Excessive privilege

Use capability-specific Podman/VM profiles. Do not adopt blanket privileged containers as the default.

### Kernel false confidence in containers

Kernel-dependent contracts use the KERNEL execution class.

### Observer/subject causal ambiguity

Introduce explicit ExperimentId, ObserverInstanceId, SubjectInstanceId and ObservationRole if/when the model requires them. Keep stores and logs separate from the first dogfooding scenario.

### Perturbation

Record environment/provenance and compare observer-enabled/disabled execution when timing sensitivity is relevant.

## Alternatives considered

### Podman only

Rejected as the universal solution because containers share the host kernel and therefore cannot provide reproducible kernel semantics for all ptrace/eBPF cases.

### QEMU/KVM for everything

Rejected because it would make ordinary deterministic scenarios unnecessarily slow and expensive.

### Firecracker immediately

Deferred. Evaluate only if QEMU/KVM startup/throughput becomes a measured bottleneck.

### Keep host-only tests

Rejected as the target state because it preserves implicit coupling to developer/CI host state.

## Validation

ADR acceptance requires SANDBOX-S0 to produce:

- measured Bubblewrap/Podman/QEMU comparison;
- capability matrix;
- minimum port proposal;
- at least one logical scenario demonstrated on two backends;
- explicit unsupported-capability result;
- clean-start/cleanup evidence;
- backend-independent evidence bundle.

Until those results exist this ADR remains Proposed and implementation details are intentionally revisable.
