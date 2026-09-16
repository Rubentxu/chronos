# Portable Execution Runtime — Future UAT Sketch

Status: FUTURE / NOT ACTIVE
Depends on: SANDBOX-S0 + DOG-001/DOG-002 evidence

This file records future acceptance expectations so the portable runtime direction remains concrete without becoming current implementation scope.

## PORTABLE-P0 — External project feasibility

Given a sample project outside the Chronos repository:

1. materialize the project into a SYSTEM-class execution runtime;
2. launch the requested command under Chronos observation;
3. return ExecutionLog/evidence/artifacts to the host;
4. prove no guest absolute paths are required to interpret the result;
5. prove runtime provenance records project revision/digest and environment identity;
6. compare result semantics with native/local execution where both are supported.

## PORTABLE-P1 — Cross-OS client

From a non-Linux host:

```text
Chronos client
   -> local Linux runtime
   -> project execution
   -> evidence returned
```

Acceptance intent:

- no local ptrace/eBPF tool installation required on host;
- workspace mapping deterministic;
- cancellation works;
- errors identify whether they are host/control/runtime/subject failures;
- Agent API semantics match Linux native mode;
- local runtime can be destroyed/recreated without losing exported evidence.

## PORTABLE-P2 — Optional remote worker

Future only.

Acceptance intent:

- authenticated runtime connection;
- capability negotiation;
- explicit project/artifact transfer;
- live event stream or durable resume;
- cancellation/reconnect;
- execution-placement details remain provenance rather than semantic API forks.

## Invariant

Moving execution from native -> sandbox -> container -> VM -> remote worker must not change the meaning of Chronos evidence. If placement changes semantics, the abstraction is not ready for portable execution.
