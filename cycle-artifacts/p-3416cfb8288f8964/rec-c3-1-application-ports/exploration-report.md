# Exploration — rec-c3-1-application-ports

> A-full cycle opened to design the application ports that close
> REC-C3 (Hexagonal boundary closure). Today the orchestrator only
> drove the explore → specify path. Implementation is paused at the
> phase boundary for a fresh session to continue.

## Why this cycle exists

The REC-C3 gate closes when `chronos-domain` declares a small,
domain-relevant set of ports and `chronos-services` depends only on
those ports — never on concrete adapters (`chronos-native`,
`chronos-ebpf`, `chronos-browser`, `chronos-store`). Per
`docs/chronos-agentic-reconstruction/docs/roadmap/CONVERGENCE_BACKLOG.md`,
C3.1 introduces these ports:

- `ProbeFactory` / `ProbeRegistry`
- `ExecutionLogProvider`
- `SessionRepository`
- `NotificationSink`
- `TelemetryReceiver`
- `ArtifactStore` / address-symbolization port (if still needed)

Per `docs/chronos-agentic-reconstruction/docs/reconstruction/CONVERGENCE_PLAN.md`
the work also requires:

- Move concrete construction to composition roots.
- Remove `chronos-services -> concrete adapters` dependencies.
- Remove `chronos-store -> chronos-native` dependency.
- Address normalization as a port/service.

`docs/chronos-agentic-reconstruction/docs/architecture/TARGET_ARCHITECTURE.md`
documents the higher-level port set:

```
trait ProbeSource { ... }
trait ProbeController { ... }
trait ExecutionLog { ... }
trait ProjectionStore { ... }
trait RuntimeMetadataProvider { ... }
trait StackInspector { ... }
trait ValueObserver { ... }
trait StateObserver { ... }
trait ReplayBackend { ... }
trait TelemetryReceiver { ... }
```

Not every trait in TARGET_ARCHITECTURE.md is in scope for C3.1 — many
are owned by REC-C4. C3.1 keeps the C3.1 list (6 ports) as its
explicit scope.

## What's already in the codebase (research summary)

### Port-shaped abstractions that already exist

1. **`NotificationSink` in `crates/chronos-domain/src/ports/notification.rs`** —
   177 lines. Declares `NotificationRequest`, `NotificationSink` trait,
   `NotificationDeliveryError`, `NullNotificationSink`. The trait is
   synchronous; adapters bridge sync → async internally. Hexagonal-by-
   design; nothing to change in C3.1 for this port.

2. **`ExecutionLogBackend` trait in `crates/chronos-log/src/backend.rs`** —
   the storage-side port. Methods: `append`, `record_gap`,
   `read_after`, `read_from_seq`, `tail_seq`, `append_many`. Wrapped
   in `ExecutionLog<B: ExecutionLogBackend + ?Sized>` (the `?Sized`
   bound means it already supports `dyn`).

3. **`ProbeBackend` trait in `crates/chronos-domain/src/adapter.rs`** —
   re-exported at crate root. The probe-side port, but it lives in
   `adapter.rs` rather than `ports/` (organizational smell, not a
   functional defect).

4. **`Capability` + `CapabilityUnavailable` in `crates/chronos-domain/src/capability.rs`** —
   capability contracts that backends advertise. REC-C4 territory
   but exists today.

### Concrete dep violations (HEX-002)

The following sites in `chronos-services` and `chronos-native` bind
to concrete implementations rather than traits:

| File | Concrete type | Used for |
|---|---|---|
| `crates/chronos-services/src/session_log.rs:56,135,170` | `Arc<SegmentedExecutionLog>` | session handle storage |
| `crates/chronos-services/src/events_log_read.rs:684` | `&Arc<SegmentedExecutionLog>` | test helper |
| `crates/chronos-capture/src/observation_log.rs:54,61` | `Arc<ExecutionLog<B>>` | typed |
| `crates/chronos-capture/src/session_feed.rs:75` | `Arc<ExecutionLog<SegmentedLogBackend>>` | concrete backend |
| `crates/chronos-native/src/probe_backend.rs` (5 sites) | `Arc<SegmentedExecutionLog>` | native owns log handles |
| `crates/chronos-mcp/src/server.rs:138` | `Arc<SessionExecutionLogRegistry>` | composes concrete |

C3.1 does NOT yet invert all of these. That is C3.3. C3.1's job is
to make sure the *port shape* exists; C3.3 will then route services
through it.

### Ports that do NOT exist yet

- **`ProbeFactory` / `ProbeRegistry`** — only the `ProbeBackend`
  trait exists. There is no factory (which picks a backend by
  capability) or registry (which tracks lifetime). Today the
  composition root wires native/ebpf/browser directly into a
  service. This is the HEX-002 surface for probe construction.

- **`SessionRepository`** — `SessionExecutionLogRegistry` lives in
  `chronos-services` (a concrete registry), not in domain. There is
  no domain-level "give me the session with id X" port. Today's
  registry is the dep-violating concrete.

- **`TelemetryReceiver`** — `TARGET_ARCHITECTURE.md` lists it; no
  Rust surface exists. Today's telemetry is direct
  `tracing::instrument` calls.

- **`ArtifactStore` / symbolization port** — addresses and symbols
  flow through `chronos-domain::SymbolId` and `InvocationId`, but
  there is no domain-level "resolve an address to a symbol" port.
  Native owns the resolver today (REC-C3.4 territory: store → native
  edge removal). Symbolization is *not yet* in C3.1's scope unless
  the call sites demand it; recommended: defer to C3.4.

## Proposed port shapes (preliminary, for review)

The shapes below are the orchestrator's draft for the design.md
phase. They are NOT yet final.

### 1. `ProbeFactory` and `ProbeRegistry`

```rust
// crates/chronos-domain/src/ports/probe.rs (proposed)

pub trait ProbeFactory: Send + Sync {
    /// Pick the best available backend for `requirements` and
    /// produce a `ProbeHandle` (the lifetime-owned handle the
    /// application gets to drive the probe). Returns
    /// `CapabilityUnavailable` if no backend can satisfy
    /// `requirements`.
    fn create(
        &self,
        session_id: SessionId,
        config: &CaptureConfig,
        requirements: &[Capability],
    ) -> Result<Box<dyn ProbeController>, CapabilityUnavailable>;
}

pub trait ProbeRegistry: Send + Sync {
    fn attach(&self, session_id: SessionId, controller: Box<dyn ProbeController>);
    fn detach(&self, session_id: SessionId) -> Result<(), TraceError>;
    fn list_active(&self) -> Vec<SessionId>;
}
```

Lives in domain. Concrete factories (`NativeProbeFactory`,
`EbpfProbeFactory`, `BrowserProbeFactory`) live in adapter crates
and are wired at the composition root.

### 2. `ExecutionLogProvider`

Already covered by `ExecutionLogBackend` + `ExecutionLog<B>`. C3.1's
job here is to **re-home the trait** in
`crates/chronos-domain/src/ports/` (currently in
`crates/chronos-log/src/backend.rs`) OR to declare a domain-side
re-export that services depend on. Re-homing is invasive; the
lighter-touch alternative is a re-export at `chronos_domain::ports`.
Recommended: **re-export**, not re-home (lower churn).

### 3. `SessionRepository`

```rust
// crates/chronos-domain/src/ports/session.rs (proposed)

pub trait SessionRepository: Send + Sync {
    fn find(&self, id: &SessionId) -> Result<Option<SessionHandle>, TraceError>;
    fn list(&self) -> Vec<SessionId>;
}

pub struct SessionHandle {
    pub session_id: SessionId,
    pub log: Arc<dyn ExecutionLogProvider>, // re-export of #2
    pub probe: Option<Box<dyn ProbeController>>,
    pub notifier: Arc<dyn NotificationSink>,
}
```

Domain owns the trait. Concrete
`InMemorySessionRepository` / `FilesystemSessionRepository` live in
the composition root.

### 4. `TelemetryReceiver`

```rust
// crates/chronos-domain/src/ports/telemetry.rs (proposed)

pub trait TelemetryReceiver: Send + Sync {
    fn metric(&self, name: &str, value: i64, attrs: &[(&str, &str)]);
    fn event(&self, name: &str, attrs: &[(&str, &str)]);
}
```

Adapters: `OtelReceiver`, `NoopReceiver`. The composition root picks
one. Today's `tracing::instrument` calls stay (they're instrumentation
of the domain itself), but `tracing-subscriber` exporting to OTLP
becomes an adapter.

### 5. `NotificationSink` — already done (HEX-001 next)

No design work needed. C3.1 verifies the existing port satisfies
the spec and adds a hex-boundary test (`HEX-001: domain tests
compile without webhook`).

### 6. `ArtifactStore` / symbolization port — DEFERRED to C3.4

`docs/chronos-agentic-reconstruction/docs/roadmap/CONVERGENCE_BACKLOG.md`
puts symbolization in C3.4 ("Extract address normalization /
symbolization behind a port owned by domain/application semantics").
C3.1 records this deferment but does not implement.

## Coupling analysis (why C3.1 ≠ full inversion)

The full C3 gate spans C3.1 + C3.2 + C3.3 + C3.4 + C3.5. Today:

- **C3.1** (this cycle): declare the ports. No inversion.
- **C3.2** (separate cycle): move webhook adapter out of domain.
- **C3.3** (separate cycle): invert services → concrete.
- **C3.4** (separate cycle): symbolization port.
- **C3.5** (separate cycle): dependency graph gate.

C3.1 can be **opened today** and **partially closed** by declaring
the ports. Inversion is C3.3's job. This split lets each cycle stay
small.

## Scope decision (proposal for `phase.specify.complete.a-full`)

C3.1 (this cycle) delivers:

1. `crates/chronos-domain/src/ports/probe.rs` — `ProbeFactory` +
   `ProbeRegistry` traits.
2. `crates/chronos-domain/src/ports/session.rs` — `SessionRepository`
   trait + `SessionHandle` struct.
3. `crates/chronos-domain/src/ports/telemetry.rs` — `TelemetryReceiver`
   trait.
4. `crates/chronos-domain/src/ports/execution_log.rs` — re-export
   shim for `ExecutionLogProvider` (= `ExecutionLogBackend` re-exposed
   as a port).
5. `crates/chronos-domain/src/ports/mod.rs` — wire all four new
   modules.
6. New unit tests under `crates/chronos-domain/tests/ports/` that
   compile-check the port contracts and exercise `NullNotificationSink`.
7. Update `reconstruction-contracts.toml` HEX-001 to `verified`
   (since the boundary is sealed at compile time).
8. README/docs note: ports owned by domain; concrete factories live
   in adapters + composition root.

C3.1 does NOT:

- Invert services → concrete adapters (C3.3).
- Move webhook out of domain (C3.2).
- Implement symbolization (C3.4).
- Touch the dependency graph (C3.5).

## Out-of-scope, follow-on cycles

- **REC-C3.2** — Extract webhook infrastructure (move HTTP/retry/spawn
  out of domain).
- **REC-C3.3** — Invert services → concrete adapter dependencies
  (route `chronos-services` through the new ports).
- **REC-C3.4** — Remove store → native (symbolization port).
- **REC-C3.5** — Dependency graph gate to zero (update
  `known_dependency_violations`).

## Tier

A-full (per `phase.specify.complete.a-full`).

## Today's session outcome

This exploration report is the deliverable for today. The cycle is
paused at the build phase boundary. Tomorrow's fresh session can:

1. Read this report.
2. Continue with the design phase (`design.md`).
3. Drive the build phase (write the 4 port modules).
4. Verify (T0 + per-crate lib tests).
5. Release (tag + push).
6. Archive (vault manifest).

The cycle resume command for tomorrow:

```bash
sddk cycle resume --cycle p-3416cfb8288f8964/rec-c3-1-application-ports \
  --root /var/mnt/DiscoChino2-fast/Proyectos/rust/chronos --scope .
```