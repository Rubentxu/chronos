# Chronos Execution Contracts (SANDBOX-S0.2)

Status: **experimental normative contract**. Owner: SANDBOX-S0.
Promotion gate: none yet — this is crystallised from four placements (host,
bubblewrap, Podman, QEMU/KVM) that already run one scenario contract.

Namespace: `chronos.execution.scenario/v1`, `chronos.execution.result/v1`.
Renamed from `sddk.sandbox.*` before any consumer existed, so there is no
external migration to perform.

## 1. Six contracts

| Contract | Answers |
|---|---|
| `ExecutionScenario` | what should execute |
| `ExecutionWorkspace` | what enters, where mutable work lives, what exits |
| `EnvironmentCapabilities` | what this placement has ACTUALLY demonstrated |
| `ExecutionProvenance` | exactly what environment executed it |
| `ExecutionOutcome` | what happened |
| `ExecutionLifecycle` | prepare → execute → teardown |

### 1.1 ExecutionScenario

```json
{
  "schema": "chronos.execution.scenario/v1",
  "scenario_id": "child-process-lifecycle",
  "description": "parent spawns child, child writes marker, both exit",
  "steps": [
    {
      "id": "parent",
      "run": ["sh", "-c", "..."],
      "expect_stdout_contains": ["..."],
      "expect_exit": 0,
      "timeout_s": 60
    }
  ],
  "cleanup_required": ["$S0_MARKER"],
  "timeout_s": 300
}
```

The scenario is placement-agnostic: `$S0_*` variables are expanded by the
backend that materialises it (host paths, container paths, guest paths), so the
same text runs everywhere without knowing where it runs.

### 1.2 ExecutionWorkspace

```text
source    (input artifact)     staged in by the backend
work      (ephemeral)          backend-managed
artifacts (output destination) copied out by the backend
tmp       (ephemeral)          backend-managed
```

The workspace is **declarative**. It is not "a bind mount": Podman stages in via
a tar stream and copies out via a tar stream precisely because bind mounts
required relabelling host directories, and a future remote worker has no local
filesystem to bind at all.

### 1.3 EnvironmentCapabilities

```text
capability:
    supported | unsupported | unknown
```

Every entry carries `provenance` (a probe id) or `reason`. Nothing is `true` by
default: a capability that was not measured is `unknown`, never `supported`.

### 1.4 ExecutionProvenance

Observed/resolved facts only:

```text
placement            (host | bwrap | podman | qemu)
adapter_version
environment_identity
source/input digest
runtime/image identity   (e.g. podman image id, immutable)
kernel identity          (e.g. host-derived kernel + sha256)
accelerator              (kvm | unsupported)
scenario schema
result schema
```

Podman reports an immutable image id. QEMU reports `host-derived kernel +
sha256` honestly: it boots the host's kernel image, which is
`KERNEL / host-derived guest`, **not** cross-host kernel reproducibility. A
pinned kernel artifact belongs to S0.7+.

### 1.5 ExecutionOutcome

```text
result:
    pass | fail | unsupported

observation:
    observed(value + method) | unknown | unsupported

artifacts   (transported, not interpreted)
errors      (explicit, never folded into a successful result)
```

### 1.6 ExecutionLifecycle

```text
prepare  -> PreparedExecution   (materialise, resolve identity, fail early)
execute  -> ExecutionOutcome    (no registry/image resolution here)
destroy  -> TeardownReport      (verified: leftovers reported, not assumed)
```

`execute()` must not be able to contact a registry: Podman runs with
`--pull=never` and by the immutable image id resolved in `prepare()`.

## 2. Ports

```rust
trait ExecutionEnvironment {
    fn capabilities(&self) -> EnvironmentCapabilities;

    fn prepare(&self, spec: &ExecutionSpec)
        -> Result<PreparedExecution, EnvironmentError>;

    fn execute(&self, prepared: &PreparedExecution, scenario: &ExecutionScenario)
        -> Result<ExecutionOutcome, EnvironmentError>;

    fn destroy(&self, prepared: PreparedExecution)
        -> Result<TeardownReport, EnvironmentError>;
}

// Separate responsibility: the environment transports artifacts, it does not
// decide what they mean.
trait EvidenceCollector {
    fn interpret(&self, outcome: &ExecutionOutcome) -> Result<EvidenceBundle, EvidenceError>;
}
```

**`collect()` is removed from `ExecutionEnvironment`.** Four placements showed
that every adapter ended up hand-rolling its own artifact capture; turning
stdout/logs/probe output into evidence is interpretation, and interpretation is
not placement. A remote runtime will need the same split.

Not a production trait yet: the spike is Python and the contract is
**normative**, not a mandatory framework.

## 3. Invariants

```text
unknown        != false
unsupported    != fail
not measured   != unsupported
cleanup unverified != cleanup success
```

- An unavailable environment yields `result: unsupported` (explicit), never a
  silent degradation to the host.
- A broken environment yields `result: fail` with the container/guest error, and
  no output from another placement may leak into it.
- A metric that cannot be attributed is `unknown`, never `0`
  (`remaining_processes` on host).
- A capability that was not probed is `unknown`, never `supported`.
- Missing local assets (image, kernel, KVM) are reported in `prepare()`, not
  discovered mid-run.
- No silent accelerator fallback: without KVM the environment is `unsupported`,
  not TCG.

## 4. Negative cases (the confusions these contracts prevent)

| Case | Wrong answer | Correct answer |
|---|---|---|
| image absent locally | pull it, or reuse a stale cache | `unsupported` at prepare |
| image retagged between prepare and execute | run the new tag | run the resolved immutable id |
| KVM missing | fall back to TCG | `unsupported` |
| container cannot start | run on the host | `fail` with the container error |
| leftover process/asset | assume clean | reported in `destroy()` |
| stdout not captured | report `observed 0` | `unknown` |
| capability assumed | `supported` by default | `unknown` until probed |
| cleanup skipped | `pass` | cleanup result reported separately |

Cache correctness is part of this: the QEMU cache key is
`sha256(schema_version + image_immutable_id + init_script_sha256 + arch)` with a
validated `manifest.json`, so `image A built → image becomes B → cache A reused`
is a MISS, not a heuristic reuse.

## 5. What this contract deliberately does not cover

- Retention, reopen, restart, stale cursors (that is REC-C1.5's domain on the
  product side).
- Privileged probes inside the guest (ptrace/eBPF/uprobe) — S0.7.
- Remote/portable execution — PORTABLE-P0, only after the S0.8 adoption review.
