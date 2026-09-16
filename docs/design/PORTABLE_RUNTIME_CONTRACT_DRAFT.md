# Portable Runtime Contract Draft — placement, not semantics

Status: CONTRACT DRAFT ONLY. No implementation. Feeds REC-C3/C4 port split
and SANDBOX-S0.3 ExecutionEnvironment port.

## ExecutionEnvironment port

```rust
trait ExecutionEnvironment {
    /// Declared capabilities; negotiation is explicit, never silent.
    fn capabilities(&self) -> EnvironmentCapabilities;
    /// Build a prepared, clean environment from a spec (workspace/fixture
    /// inputs, required capabilities, preferred class).
    fn prepare(&self, spec: &ExecutionSpec) -> Result<PreparedEnvironment>;
    /// Run one request; handle is opaque to callers.
    fn execute(&self, request: ExecutionRequest) -> Result<ExecutionHandle>;
    /// Collect evidence independent of environment lifetime.
    fn collect(&self, handle: &ExecutionHandle) -> Result<EvidenceBundle>;
    /// First-class cleanup; idempotent; verified, not assumed.
    fn destroy(&self, handle: ExecutionHandle) -> Result<()>;
}
```

## Capability taxonomy (aligned with SANDBOX-S0 roadmap)

```text
PROCESS  : pid/fs/net isolation subset          -> Bubblewrap
SYSTEM   : reproducible userspace, pinned image -> Podman (rootless)
KERNEL   : ptrace/eBPF/uprobe in controlled VM  -> QEMU/KVM
REMOTE   : future; same port, same semantics    -> out of scope until P2
```

## What must NOT leak to the Agent API

- Backend identifiers/types (bwrap, podman, qemu) — placement detail.
- Host paths as identity; only logical workspace/fixture ids.
- Environment lifetime mechanics (boot/pull/cleanup latency handling).
- Privilege escalation choices; capabilities are negotiated, never assumed.
- Cleanup implementation (tmpdirs, containers, VMs).

## What MUST be in the Agent API

- Scenario identity, required capabilities, environment class preference.
- Execution result + completeness semantics (Complete/Partial/GapDetected/Unknown).
- EvidenceBundle portable across placements (DOG-005: same evidence semantics
  regardless of where the scenario ran).
- Explicit capability-mismatch error when required > declared.

## Invariants

1. No evidence-semantics change based on execution placement.
2. No silent escalation (requirement > capability = hard error).
3. Cleanup verified (destroy returns verified-clean, not best-effort).
4. No backend types in domain/application contracts (adapter-owned extensions).
