# Specification: Adaptive instrumentation

## Objective

Obtain the smallest amount of additional evidence required to prove/disprove a hypothesis while minimizing perturbation and maintenance cost.

## Instrumentation ladder

### L0 — Existing evidence

Reuse what the application already emits: OpenTelemetry, Rust `tracing`, runtime telemetry and correlated logs.

### L1 — Standard zero-code/runtime instrumentation

Use maintained ecosystem mechanisms:

- OpenTelemetry OBI/eBPF for supported generic application/network/protocol visibility;
- Java/.NET/Node/Python zero-code agents where applicable;
- Go zero-code instrumentation + Auto SDK;
- JFR;
- Python `sys.monitoring`.

### L2 — Chronos passive probes

Targeted uprobes/uretprobes, kernel tracepoints, syscall/scheduler/network events and symbol-level native observation.

### L3 — Compile-time/debug-build instrumentation

Create an isolated debug binary without changing the product branch.

Go:

- prefer `otelc` for supported instrumentation;
- extend with Chronos typed hook packages only when needed.

Rust:

- spike `-Z instrument-xray` on nightly for function entry/exit;
- evaluate USDT for semantic markers;
- use temporary source overlay/probe compiler only when required state cannot be exposed otherwise.

### L4 — Semantic state probes

Selected arguments, returns, state transitions, branch decisions and invariant inputs. Never instrument all fields/functions by default.

### L5 — Deep replay/debug backend

ptrace/rr/GDB-like mechanisms only when exact historical memory/register state or deterministic replay is required.

## InstrumentationPlan

The LLM proposes *what information is missing*. A deterministic planner/compiler resolves *how to obtain it*.

```yaml
goal: "prove why Order.total becomes negative"
budget:
  max_perturbation: moderate
  max_retained_events: 500000
observe:
  - function: Promotion.Calculate
    capture: [args, return]
  - state:
      type: Order
      field: Total
      transition: true
properties:
  - "Order.Total >= 0"
```

The plan is validated against discovered symbols, types and capabilities.

## Capability discovery

Report capabilities such as:

```text
existing_otel otel_context obi_available ebpf_available symbols_available
debug_info_available runtime_agent_available compile_time_instrumentation_available
xray_available usdt_available rr_available
```

Do not infer capability from language name alone.

## OpenTelemetry reuse rule

If OTel already exposes an operation sufficiently, do not add a duplicate Chronos function span. Correlate deeper state evidence with external `trace_id`/`span_id`.

OTel is the distributed semantic skeleton; Chronos adds local execution evidence.

## OBI scope

Use OBI for generic protocol/application observability it already implements. Do not rebuild generic HTTP/gRPC/database/context plumbing in `chronos-ebpf` without a measured unmet requirement.

Reserve custom eBPF for investigation-specific capabilities.

## Debug isolation

```text
repo (unchanged)
 -> temporary worktree/build dir
 -> instrumentation plan
 -> debug compilation/agent configuration
 -> execute
 -> persist evidence
 -> cleanup
```

The user's product working tree remains clean.

## Perturbation comparison

For timing/concurrency-sensitive bugs compare baseline vs instrumented externally relevant behaviour. If a bug disappears under deeper instrumentation, report `PossibleHeisenbug` and fall back to lighter/deep-replay strategies.

## Safety

- no arbitrary LLM-generated eBPF bytecode;
- no unvalidated memory offsets;
- no unbounded payload capture;
- configurable redaction;
- compiler/runtime validates symbols and types;
- persist instrumentation manifests for reproducibility.
