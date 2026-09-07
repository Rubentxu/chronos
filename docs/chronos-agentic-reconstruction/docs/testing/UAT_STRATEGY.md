# UAT and real-test strategy

## Philosophy

Chronos is a debugging product. A unit test asserting JSON shape is not enough proof.

Each capability needs:

1. **domain/unit** — deterministic invariants;
2. **integration** — adapter/store/query contracts;
3. **real UAT** — executable program with a known bug whose cause must be found.

## Preserve the sandbox

`chronos-sandbox` currently contains substantial integration coverage (analytics, boundary conditions, concurrency stress, event tools, forensic tools, memory and execution depth). Do not delete it.

Near-term:

- add `programs/bugs/` catalog;
- tag tests by capability/milestone;
- separate privileged/toolchain-specific tests;
- evolve terminology toward UAT without a rename-only migration.

## UAT fixture metadata

```yaml
id:
language:
bug_class:
known_root_cause:
trigger:
expected_observed_facts:
expected_derived_facts:
forbidden_claims:
required_capabilities:
max_allowed_gaps:
verification_patch:
```

## Golden assertions

Assert evidence, not prose.

Good example:

```text
PropertyViolation.property_id == order_total_non_negative
transition.before == 59
transition.after == -35
producer.symbol == Promotion.Calculate
completeness == Complete
```

LLM explanation tests are separate from deterministic architecture gates.

## CI matrix

### Unprivileged
Domain, ExecutionLog, projections, store, synthetic adapter, MCP schema, replay, properties.

### Linux privileged UAT
E.g. eBPF, uprobes, OBI, ptrace, scheduler/network fixtures.

### Toolchain-specific
Go compile-time instrumentation, Rust nightly XRay, rr, JVM/JFR, Python sys.monitoring.

A skipped privileged test is not considered a passing milestone gate.

## Performance/perturbation

Record wall-clock, CPU, memory, event count, gaps and bug reproduction rate per UAT workload. Do not publish universal overhead claims without measurements.

## Reliability fault injection

Inject adapter crash, ring overflow, interrupted store write, malformed event, out-of-order source timestamp, duplicate upstream IDs and detach failure.

Expected result is explicit incomplete/gap state, never silent success.

## Agent-in-the-loop UAT

After deterministic capabilities are stable:

```text
given repository + failing test
agent may use Chronos v2 tools
agent identifies root cause
agent proposes patch
sandbox executes patch
Chronos verifies behaviour
```

Score evidence correctness, instrumentation escalations, retained context volume, root-cause identification and correction verification. LLM success is not the sole architecture gate.
