# SANDBOX-S0 — Verification Lab Roadmap Slice

Status: PLANNED / NON-BLOCKING FOR REC-C0
Primary spec: `CHRONOS_VERIFICATION_LAB.md`
ADR: `ADR-0012-chronos-verification-lab.md`
UAT: `CHRONOS_VERIFICATION_LAB_UAT.md`
Future extension: `PORTABLE_EXECUTION_RUNTIME.md`

## Position in reconstruction

```text
REC-C0
  Truth baseline
      |
      +--> REC-C0.5-E Coverage engine compatibility
      |
      `--> SANDBOX-S0 characterization (may be prepared in parallel,
           but does not block REC-C0 closure)

REC-C1
  ExecutionLog/events_read cutover
      +--> DOG-001
      `--> DOG-002

REC-C2
  Delete legacy truth paths
      `--> DOG-003

M4
  Adaptive instrumentation / privileged runtime
      `--> KERNEL profile + DOG-004

Cross-cutting
  DOG-005 No Silent Lies
```

## S0.1 — Environment characterization

Investigate and measure:

- Bubblewrap for PROCESS-class isolation;
- Podman for SYSTEM-class reproducibility;
- QEMU/KVM for KERNEL-class verification.

Deliver:

- startup/teardown measurements;
- capability matrix;
- privilege requirements;
- failure diagnostics;
- artifact collection notes;
- recommendation by scenario class.

No production abstraction is accepted before this evidence exists.

## S0.2 — Scenario/result contract

Define the smallest backend-neutral structures needed to express:

- scenario identity;
- required capabilities;
- workspace/fixture inputs;
- environment class preference;
- execution request;
- execution result;
- evidence bundle;
- cleanup result.

Keep backend-specific configuration under adapter-owned extensions/provenance.

## S0.3 — ExecutionEnvironment port

Introduce the minimum port proven by S0.1/S0.2.

Constraints:

- no Bubblewrap/Podman/QEMU types in domain/application contracts;
- capability negotiation explicit;
- no silent escalation;
- cleanup first-class;
- evidence collection independent of environment lifetime.

## S0.4 — PROCESS adapter

Implement the cheapest useful backend first, expected Bubblewrap if characterization confirms it.

Acceptance: UAT-S0-01 + UAT-S0-03.

## S0.5 — SYSTEM adapter

Implement Podman-backed reproducible userspace.

Acceptance: UAT-S0-02 + UAT-S0-05 against PROCESS where the scenario is compatible.

## S0.6 — Chronos-on-Chronos bootstrap

Run an Observer and Subject with separate state.

Acceptance foundation:

- unique Experiment/Observer/Subject identities or equivalent provenance;
- separate stores/logs;
- subject lifecycle visible externally;
- evidence bundle survives environment teardown.

This unlocks DOG-001/DOG-002 adoption in REC-C1.

## S0.7 — KERNEL spike

Create one ephemeral QEMU/KVM execution path for a real ptrace/uprobe scenario.

Acceptance: UAT-S0-04.

Do not optimize boot throughput in S0.

## S0.8 — Adoption review

Before expanding the lab, answer:

1. Which environment class actually delivers value to current milestones?
2. Which abstractions were premature or unnecessary?
3. Is QEMU/KVM operational cost justified by M4 needs?
4. Does Chronos-on-Chronos expose useful evidence unavailable from normal tests?
5. Can external-project execution reuse the same seam without changing evidence semantics?

At this gate decide whether to activate `PORTABLE-P0` feasibility work.

## Success metrics

SANDBOX-S0 is successful when:

- at least two backends run one equivalent logical scenario;
- capability mismatch is explicit;
- environment cleanup is verified;
- evidence is portable outside the sandbox;
- DOG-001/002 can be implemented without backend coupling;
- no existing REC-C0 acceptance gate is weakened;
- no generic sandbox platform work exists without a named roadmap consumer.

## Explicit non-blocking rule

SANDBOX-S0 documentation and experimentation may be added during REC-C0, but REC-C0 closure continues to depend only on its declared baseline gates. Failure or incompleteness of SANDBOX-S0 must not move the REC-C0 finish line.
