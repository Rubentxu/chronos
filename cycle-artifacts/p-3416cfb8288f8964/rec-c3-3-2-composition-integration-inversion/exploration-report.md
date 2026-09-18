# REC-C3.3.2 recon report — composition/integration inversion

**Cycle**: `p-3416cfb8288f8964/rec-c3-3-2-composition-integration-inversion`
**Path**: A-min (default per operator; composition inversion is bounded scope)
**Phase**: Recon
**Updated**: 2026-09-18
**Base SHA**: `93a80a5c` (REC-C3.3.1 close, on local main; GitHub is stale at `147c1019`)
**Scope**: composition root + native bridge removal + eBPF capability port + retention port shape decision

## Purpose

C3.3.1 created the seam. C3.3.2 inverts the application layer:
- composition (concrete adapter construction) moves to `chronos-mcp::composition`;
- the native execution-log bridge is killed (its escape hatch is the debt that this cycle attacks);
- eBPF construction leaves services (capability port, not port-of-the-whole-adapter);
- retention is decided as a semantic capability (with shape, even if implementation lands in C3.3.3).

C3.3.2 does NOT eliminate the four edges (`services → store`, `services → native`, `services → ebpf`, `services → browser`); that lands in C3.3.3. C3.3.2 eliminates the **construction and wiring** of those adapters from `chronos-services` and prepares the integration paths that today force escape hatches.

## Inventory at HEAD (`93a80a5c`)

### Constructors — concrete adapter types

| Constructor | Total sites | Production services | Test/Fixture | mcp-server prod | mcp-server test |
|---|---|---|---|---|---|
| `BrowserAdapter::new` | 20 | 1 (browser_probe.rs:147) | 14 (chronos-browser + services) | 0 | 5 (server.rs tests) |
| `NativeProbeBackend::new` | 14 | 3 (probe.rs:270, 388, 933) | 9 (chronos-native tests) | 0 | 2 (server.rs tests) |
| `EbpfAdapter::new` | 7 | 1 (probe.rs:750) | 5 (chronos-ebpf + tests) | 0 | 0 |
| `SessionStore::try_open` | 2 | 0 | 1 (chronos-store test) | 1 (server.rs:1777) | 0 |
| `SessionStore::in_memory` | 41 | 0 | 28 (services tests, store tests, cli, mcp tests) | 2 (server.rs:1789, 2091) | — |
| `SegmentedExecutionLogProvider::new` | 6 | 4 (session_log.rs factories:239, 268, 293, 739(test)) | — | 0 | 0 |
| `InMemoryExecutionLogProvider::new` | 6 | 4 (session_log.rs tests:752, 839, 850, plus chronos-log:428) | — | 0 | 0 |

### Bridge + native integration sites

| Symbol | Sites | Classification |
|---|---|---|
| `legacy_segmented_backend_for_native_bridge` | 1 def (session_log.rs:496) + 4 prod callers (probe.rs:278, 396, 641, 935) + 1 test (session_log.rs:905) + 1 doc comment (probe.rs:969) | **NATIVE-BRIDGE (kill)** |
| `attach_execution_log` | 1 def (probe_backend.rs:302, takes `Arc<SegmentedExecutionLog>`) + 3 prod callers (probe.rs:276, 394, 933) | **NATIVE-BRIDGE (refactor signature)** |
| `read_log_with_stats` | 1 def (probe_backend.rs:1583, takes `&SegmentedExecutionLog`) + 1 prod caller (probe.rs:640) + 2 internal callers (probe_backend.rs:366, 1195, 1204) | **NATIVE-BRIDGE (refactor or privatize)** |
| `persist_events_to_execution_log` | 1 def (probe_backend.rs:97) + 1 caller (m2 test only) | **TEST-FIXTURE** (not in scope) |

### Retention / maintenance sites in services

| Symbol | Sites | Classification |
|---|---|---|
| `retain_up_to` | 1 wrapper (session_log.rs:455) + 1 test caller (events_log_read.rs:1022) | **RETENTION (port candidate)** |
| `compact_up_to` | 1 wrapper (session_log.rs:437) + 1 test caller (projection.rs:353) | **MAINTENANCE (stays wrapper-gated)** |
| `flush()` | 33 sites, all on `log.flush()` wrapper, not direct | **MAINTENANCE (stays wrapper-gated)** |

### Concrete imports in services (production code)

| Edge | Files | Severity |
|---|---|---|
| `services → chronos_native` | probe.rs:20 (`NativeProbeBackend`) | **COMPOSITION** (out of services) |
| `services → chronos_browser` | browser_probe.rs:14 (`BrowserAdapter`) | **COMPOSITION** (out of services) |
| `services → chronos_ebpf` | probe.rs:53 (`EbpfAdapter`) + probe.rs:750 (`EbpfAdapter::new()`) | **COMPOSITION** (out of services) |
| `services → chronos_store` | 8 production files (session_explain, session_export, diff, output, sessions, session_compare, session_lifecycle, counterexample) + 1 ce_services_tests (tests only) | **STORE edge (C3.3.3 scope)** |
| `services → chronos_log` (concretes) | session_log.rs:70 (SegmentedExecutionLog, SegmentedExecutionLogProvider, ProviderKind gating) | **COMPOSITION (factories)** |

## Classification key

```text
COMPOSITION       = construction of a concrete adapter at this site
                    MUST move to chronos-mcp::composition or be delegated
                    to a factory the composition root injects.
CANONICAL-PORT    = reads/writes canonical evidence; uses Arc<dyn ExecutionLogProvider>;
                    correct as-is, no change in this cycle.
NATIVE-BRIDGE     = leak from the bridge transitional escape hatch
                    (legacy_segmented_backend_for_native_bridge).
                    MUST be eliminated in this cycle.
RETENTION         = semantic operation masquerading as maintenance.
                    Decide shape this cycle; implementation may live in C3.3.3.
MAINTENANCE       = physical/storage concern, stays wrapper-gated by ProviderKind.
                    No change in this cycle.
TEST-FIXTURE      = test only, not subject to composition ratchets.
STORE edge        = services → chronos-store; defer to C3.3.3.
```

## Decisions (operator-approved frame)

### 1. Composition root lives in `chronos-mcp::composition`

A single module file `crates/chronos-mcp/src/composition.rs`. NO new
crate yet (no second bootstrap consumer; HTTP/gRPC would justify a
crate, but it's speculative now).

Shape (sketch — actual structure decided during apply):

```rust
// Factory functions take RuntimeConfig and return wired concrete
// adapters or `Arc<dyn Port>` for the application layer.
pub fn compose_execution_log(
    config: &RuntimeConfig,
) -> Result<Arc<dyn ExecutionLogProvider>, CompositionError>;

pub fn compose_session_store(
    config: &RuntimeConfig,
) -> Result<Arc<dyn SessionRepository>, CompositionError>; // C3.3.3

pub fn compose_native_probe(
    config: &RuntimeConfig,
    log: Arc<dyn ExecutionLogProvider>,
) -> Result<Arc<dyn ProbeBackend>, CompositionError>;

pub fn compose_ebpf_capability(
    config: &RuntimeConfig,
) -> Result<Option<Arc<dyn UprobeInjector>>, CompositionError>;

pub fn compose_browser_probe(
    config: &RuntimeConfig,
) -> Result<Arc<dyn BrowserProbeBackend>, CompositionError>; // C3.3.3
```

`ChronosServer::new()` (and `try_new` test fixture factory) becomes a
thin wrapper that calls these factories and assembles the wired graph.

### 2. Native bridge elimination

This requires changing signatures inside `chronos-native` (the public
crate), not just in `chronos-services`:

- `NativeProbeBackend::attach_execution_log(self, log: Arc<dyn ExecutionLogProvider>) -> Self`
- `chronos_native::read_log_with_stats(log: &dyn ExecutionLogProvider, since, limit) -> ...`
- The internal `accept_raw` is updated to take `&dyn ExecutionLogProvider` and use `append` + `session_id()` from the port.
- The `flush()` call inside `persist_events_to_execution_log` (test-only helper) is removed or internalised; native owns its flush policy as part of `accept_raw` semantics.

`read_log_with_stats` keeps its name; it's a helper inside native for
native's own use. The services path does NOT call it anymore — services
reads evidence via `SessionExecutionLog.read_from_seq` (port).

Tests inside `chronos-native/tests/` may continue to use
`read_log_with_stats` directly because they are testing the helper
itself; that's TEST-FIXTURE classification.

### 3. eBPF capability port

Today's call site is `probe.rs:750`:

```rust
match chronos_ebpf::EbpfAdapter::new() {
    Ok(adapter) => {
        let adapter = Arc::new(adapter);
        match adapter.attach_uprobe(pid, &input.binary_path, &input.symbol_name) {
            ...
        }
    }
}
```

Shape (in `chronos_domain::ports::probe`):

```rust
pub trait UprobeInjector: Send + Sync {
    fn inject_uprobe(
        &self,
        pid: nix::unistd::Pid,
        binary_path: &str,
        symbol_name: &str,
    ) -> Result<UprobeHandle, UprobeError>;
}
```

The composition root wires an `EbpfAdapter` adapter (or a no-op for
non-Linux / unsupported environments) into a `Box<dyn UprobeInjector>`.
Services consume the trait. The concrete `EbpfAdapter` keeps living
inside `chronos-ebpf` (no need to move it); only the construction
moves to composition.

### 4. Retention capability port

Today's wrapper:

```rust
pub fn retain_up_to(&self, cutoff: EventSeq)
    -> Result<chronos_log::CompactionOutcome, ServiceError>
```

This is **semantic** — it changes what counts as evidence. The operator
flagged it must NOT masquerade as maintenance. Shape (decided in this
cycle, implementation may land in C3.3.3 if too wide):

```rust
// chronos_domain::ports::retention
pub trait ExecutionLogRetention: Send + Sync {
    /// Trim evidence strictly before `retained_from`. After this call,
    /// `provider.retained_from()` returns at least `retained_from`,
    /// and consumers asking for older evidence get a typed
    /// `EvidenceUnavailableDueToRetention`.
    fn retain_from(
        &self,
        retained_from: EventSeq,
    ) -> Result<RetentionResult, ExecutionLogError>;
}
```

Naming respects the semantic (we are declaring "the evidence boundary
moves HERE", not "trim records before X"). `retain_up_to` had
inclusive/exclusive ambiguity; `retain_from` declares what survives,
not what disappears.

The wrapper:

```rust
impl SessionExecutionLog {
    pub fn retain_from(&self, retained_from: EventSeq)
        -> Result<RetentionResult, ServiceError>;
}
```

Where `retention: Arc<dyn ExecutionLogRetention>` is held alongside
`log: Arc<dyn ExecutionLogProvider>`, both populated at composition.

### 5. Maintenance stays wrapper-gated (no change)

`flush`, `compact_up_to`, `maybe_compact`, `compaction_metrics` stay
on the wrapper, gated by `ProviderKind`. They are physical concerns;
the port stays minimal. NO change in C3.3.2.

## Sub-cycle shape

```text
REC-C3.3.2 — composition/integration inversion

A. composition root real                     (C3.3.2.1)
   chronos-mcp::composition
   chronos-server::new() / try_new() delega
   BrowserAdapter::new, NativeProbeBackend::new, EbpfAdapter::new,
   SessionStore::try_open, SegmentedExecutionLogProvider::new
   fuera de services y de mcp::server (delegación por factories)

B. native execution-log bridge elimination  (C3.3.2.2)
   attach_execution_log toma Arc<dyn ExecutionLogProvider>
   read_log_with_stats interno a native; services NO lo llama
   legacy_segmented_backend_for_native_bridge borrado
   accept_raw usa el port (append + session_id)

C. eBPF capability port                      (C3.3.2.3)
   chronos_domain::ports::probe::UprobeInjector
   services consume el port; chronos_ebpf implementa;
   composition root inyecta Option<Arc<dyn UprobeInjector>>

D. retention capability port                (C3.3.2.4)
   chronos_domain::ports::retention::ExecutionLogRetention
   shape decidido; impl puede vivir en C3.3.3
   retain_up_to del wrapper renombrado a retain_from (semántico)

E. ratchets + gates                          (C3.3.2.5)
   grep gates mecánicos en T0
   cierra C33-DEBT-NATIVE-LOG-BRIDGE-01
   deja C33-DEBT-RETENTION-01 OPEN si la impl no cabe en este ciclo
```

## What C3.3.2 does NOT do (per operator)

- Eliminate the full `services → store` / `services → browser` / `services → native` edges. (C3.3.3.)
- Touch `chronos-store → chronos-native`. (REC-C3.4.)
- Begin `TraceAdapter` capability split. (C4.)
- Add `flush`/`compaction` to the `ExecutionLogProvider` port. (Forbidden by operator.)
- Add a `chronos-composition` crate. (Speculative; one bootstrap consumer today.)

## Carry-forward from C3.3.1 (state at C3.3.2 open)

- C31-DEBT-01 CLOSED — port real + 2 adapters + productive consumer.
- C31-DEBT-02 CLOSED — SessionId single owner.
- C31-DEBT-03 OPEN — full inversion (C3.3.3 owner).
- C33-DEBT-RETENTION-01 OPEN — retention policy bypasses port (C3.3.2/C3.3.3 owner).
- C33-DEBT-NATIVE-LOG-BRIDGE-01 OPEN — native bridge (C3.3.2 owner; THIS cycle).
- HEX-002 GAP — closes only after C3.3.3.

C3.3.2 aims to close C33-DEBT-NATIVE-LOG-BRIDGE-01, register
C33-DEBT-RETENTION-01 with shape (port declared; impl possibly
deferred), and prepare the composition root that C3.3.3 will need
to finish the inversion.
