# Chronos Verification Lab — UAT and Acceptance Matrix

Status: PROPOSED
Related: `CHRONOS_VERIFICATION_LAB.md`, `ADR-0012-chronos-verification-lab.md`

## Purpose

Define measurable acceptance criteria for SANDBOX-S0 and the first Chronos-on-Chronos dogfooding scenarios. These UATs supplement, not replace, Cargo tests and coverage.

## SANDBOX-S0 acceptance

### UAT-S0-01 — Clean process environment

Given a PROCESS-class backend, when a deterministic Chronos fixture runs, then:

- `$HOME` is synthetic/clean;
- `/tmp` is isolated where supported;
- network state matches the scenario declaration;
- undeclared environment variables are not visible;
- writable artifacts leave the ephemeral environment through the evidence bundle;
- cleanup leaves no subject child process.

Expected initial backend: Bubblewrap.

### UAT-S0-02 — Reproducible userspace

Given a SYSTEM-class environment, run the same scenario twice from a clean image.

Require:

- same declared OS/image identity;
- same Chronos binary digest;
- same fixture digest;
- same declared capabilities;
- same semantic result;
- no dependency on host `$HOME`, installed Chrome, Cargo target layout or arbitrary host packages.

Expected initial backend: Podman.

### UAT-S0-03 — Capability refusal

Request a scenario requiring a capability absent from the selected backend.

Require an explicit result equivalent to:

```text
CapabilityUnavailable(<capability>)
```

Forbidden outcomes:

- silent skip;
- PASS without execution;
- fallback to a stronger/privileged mode without policy approval.

### UAT-S0-04 — Kernel-controlled runtime

Boot an ephemeral KERNEL-class environment and execute one real ptrace/uprobe-capable scenario.

Record:

- guest image/kernel identity;
- capabilities/sysctls relevant to the scenario;
- Chronos/fixture digests;
- attach/event/detach evidence;
- cleanup evidence.

Expected initial backend: QEMU/KVM.

### UAT-S0-05 — Backend-independent result

Execute one logical scenario on two compatible environment backends.

The orchestrator-facing result must preserve the same logical schema for:

- scenario ID;
- environment identity;
- subject identity;
- exit/result status;
- evidence/artifact references;
- capability/provenance report.

Backend-specific details may appear only in provenance/diagnostics.

## Chronos-on-Chronos UAT

### DOG-001 — Session lifecycle

Observer A observes Subject B while B runs a deterministic fixture.

Prove:

1. A and B use separate stores and ExecutionLogs.
2. B starts a session.
3. B executes the fixture.
4. B stops the session.
5. A observes B's runtime/process lifecycle.
6. No subject child process remains.
7. No test probe/resource remains attached.

Target milestone: REC-C1.

### DOG-002 — ExecutionLog / cursor integrity

Subject B receives 10,000 ordered events.

Sequence:

1. consumer A reads an initial prefix;
2. consumer B reads independently;
3. A pauses;
4. production continues;
5. A resumes from authoritative EventSeq;
6. B is unaffected;
7. force retention/loss where supported;
8. gap becomes explicit;
9. incomplete evidence never reports Complete;
10. restart/resume preserves the durable contract required by REC-C1.

Observer A independently records subject lifecycle/runtime behaviour.

Target milestone: REC-C1.

### DOG-003 — Crash recovery

Force abrupt Subject B termination while evidence is being produced.

Require:

- ExecutionLog recovery according to its durable contract;
- incomplete tail/session represented truthfully;
- no fabricated Complete state;
- cleanup/restart behaviour observable by A.

Target milestone: REC-C2/post-REC-C1 lifecycle hardening.

### DOG-004 — Probe lifecycle

In a KERNEL-class environment:

1. advertise required capability;
2. attach real probe;
3. observe real event;
4. keep subject healthy;
5. detach;
6. prove no probe/process/resource residue.

Target milestone: M4.

### DOG-005 — No Silent Lies

Introduce controlled evidence loss or unsupported observation.

Accept only truthful outcomes such as:

```text
Partial
GapDetected
Unsupported
Unknown
```

Reject an unjustified `Complete` or PASS.

This is a cross-cutting reconstruction acceptance scenario.

## Evidence bundle minimum

Each Verification Lab run should eventually emit a backend-independent bundle containing at least:

```text
experiment_id
scenario_id
subject identity/digest
observer identity/digest (when present)
environment class/backend
OS/kernel/image identity where applicable
capability report
start/end timestamps
result classification
ExecutionLog/session references
stdout/stderr/diagnostics references
cleanup result
perturbation/provenance metadata
```

The exact serialization is deferred to SANDBOX-S0.

## Negative acceptance

The lab must fail verification when:

- a required capability is absent but the scenario reports PASS;
- subject and observer share a store/log unexpectedly;
- the ephemeral environment leaves undeclared resources behind;
- result parsing depends on backend-specific path layout;
- host state changes the semantic result without being declared in provenance;
- an environment backend silently escalates privileges.

## Future portability acceptance (not SANDBOX-S0)

The same execution abstraction may later support Chronos as an optional portable runtime for external projects.

Future acceptance target:

```text
Windows/macOS/other host
        |
        | thin Chronos client/control plane
        v
Linux Execution Runtime
  (container / VM / remote worker)
        |
        v
subject project + Chronos probes
```

A host should not need to install ptrace/eBPF/kernel tooling locally when it selects this mode.

This future mode must preserve:

- explicit capability negotiation;
- artifact/evidence portability;
- path/workspace mapping;
- deterministic runtime identity;
- no semantic difference in Agent API results merely because execution is delegated;
- local-first operation when a local runtime backend is available;
- remote execution as an optional extension, not a hard dependency.

This portability mode is intentionally deferred until the Verification Lab has proven the execution abstraction with real roadmap UATs.
