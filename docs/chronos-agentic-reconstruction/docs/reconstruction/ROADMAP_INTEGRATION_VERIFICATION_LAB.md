# Roadmap Integration — Chronos Verification Lab

Status: ACTIVE ROADMAP ADDENDUM

This addendum integrates the Verification Lab into the reconstruction roadmap without changing REC-C0 closure criteria.

## Placement

### REC-C0

No new closure requirement.

Allowed work:

- documentation;
- SANDBOX-S0 characterization notes;
- coverage-engine experiments when needed to close Coverage.

Forbidden interpretation:

> REC-C0 cannot close until SANDBOX-S0 is implemented.

That is explicitly false.

### REC-C1 — ExecutionLog / events_read truth cutover

Adopt the first Verification Lab dogfooding scenarios:

- DOG-001 Session lifecycle;
- DOG-002 ExecutionLog/cursor integrity.

REC-C1 should use these when they materially strengthen acceptance, while preserving the existing mandatory 10k/two-consumer/gap UAT.

Desired result:

```text
normal deterministic tests
        +
Chronos Subject semantic evidence
        +
Chronos Observer runtime evidence
```

### REC-C2 — Legacy truth path deletion

Adopt DOG-003 crash recovery/truthful tails to prove that deleting EventBus/fired-buffer/dual-write paths does not create false completeness or resource leakage.

### REC-C3/REC-C4

Use SANDBOX-S0 findings to shape ports/capabilities without coupling application/domain code to concrete sandbox engines.

In particular, capability-aware execution can inform the planned split of oversized debugger interfaces.

### M4 — Adaptive instrumentation

Activate KERNEL-class execution and DOG-004 for real ptrace/eBPF/uprobe lifecycle verification.

Do not claim M4 runtime support solely from mocked or host-accidental behaviour.

### Cross-cutting reconstruction gate

DOG-005 No Silent Lies should evolve into a cross-cutting regression scenario:

- deliberately partial evidence;
- deliberately unsupported capability;
- deliberate gap/loss;
- never unjustified Complete/PASS.

## Future product evolution

After SANDBOX-S0 and real REC-C1 dogfooding, evaluate PORTABLE-P0.

The future direction is:

```text
Chronos client/control plane
        |
        +-- native execution
        +-- process sandbox
        +-- container runtime
        +-- local Linux VM
        `-- remote runtime (future)
```

This would allow projects on Windows/macOS/other systems to use stronger Linux Chronos capabilities without installing the full tracing stack on the host.

The portable runtime must remain an execution-placement option, not a different semantic API.

## Roadmap constraints

1. No sandbox infrastructure without a named consuming UAT/milestone.
2. No blanket privileged container profile.
3. No remote/cloud runtime before local execution separation is proven.
4. No host-path identity in execution/evidence protocols.
5. No change to evidence semantics based on execution placement.
6. Coverage and Verification Lab remain independent quality surfaces.
7. REC-C0 finish line is not moved by this addendum.

## Proposed milestone identifiers

```text
SANDBOX-S0  characterization and minimum execution abstraction
DOG-001     Chronos session lifecycle dogfood
DOG-002     ExecutionLog/cursor dogfood
DOG-003     crash recovery dogfood
DOG-004     probe lifecycle dogfood
DOG-005     No Silent Lies dogfood
PORTABLE-P0 external-project execution feasibility
PORTABLE-P1 cross-OS client -> local Linux runtime
PORTABLE-P2 optional remote worker
```

These identifiers are separate from REC-C* and official M* milestones and must not be confused with reconstruction milestone completion.
