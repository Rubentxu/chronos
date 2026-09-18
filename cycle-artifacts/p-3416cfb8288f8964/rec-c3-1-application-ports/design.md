# Design — rec-c3-1-application-ports

> A-full design for the application ports cycle. Final architecture
> decisions before implementation (which is paused to a future
> session).

## Architectural decisions

### AD-1 — Ports live in `chronos-domain/src/ports/`

All 6 ports (5 new + 1 verified) are declared in
`crates/chronos-domain/src/ports/`. The directory already hosts
`notification.rs`; this cycle adds `probe.rs`, `session.rs`,
`telemetry.rs`, and `execution_log.rs` (a re-export module).

**Rationale:** domain is the innermost ring; ports are its
declaration of intent. The composition root (in `chronos-services`
or `chronos-mcp`) wires concrete adapters. The boundary is sealed
by the compiler: if a port trait method is used by domain code, the
adapter cannot leak any HTTP/runtime types into that signature.

**Rejected alternatives:**
- `chronos-services/src/ports/` — wrong ring. Services depend on
  domain, not the other way around. Putting ports in services
  would force domain to depend on services' port module.
- `crates/ports/` (new top-level crate) — premature. The current
  workspace has 13; adding a 14th just for ports creates more
  cross-crate boundary churn than it solves. If/when a new
  infrastructure crate is needed, ports can move.

### AD-2 — `ProbeController` is a new lifetime-focused trait

The existing `ProbeBackend` trait in `chronos-domain/src/adapter.rs`
is *capability-focused* (what can this probe do?). The new
`ProbeController` is *lifetime-focused* (who owns the probe, when
does it stop, how do I detach?). The composition root wires one to
the other.

```rust
pub trait ProbeController: Send + Sync {
    fn session_id(&self) -> &SessionId;
    fn stop(&self) -> Result<(), TraceError>;
    fn detach(self: Box<Self>);
    fn backend(&self) -> &dyn ProbeBackend;
}
```

**Rationale:** the registry needs to attach/detach without knowing
the backend's capability surface. Splitting the trait keeps each
one narrow.

**Rejected alternatives:**
- Adding `attach`/`detach` to `ProbeBackend` — pollutes the
  capability surface with lifetime concerns. Today `ProbeBackend`
  doesn't have those methods; concrete adapters don't expose them
  via the trait.
- A single `Probe` trait that merges both — violates interface
  segregation. The REC-C4 SOLID gate explicitly rejects wide
  trait surfaces.

### AD-3 — `ExecutionLogProvider` is a re-export, not a new trait

`chronos-log/src/backend.rs::ExecutionLogBackend` already exists.
This cycle adds `chronos-domain/src/ports/execution_log.rs` that
re-exports it under the domain-level name `ExecutionLogProvider`:

```rust
pub use chronos_log::{
    ExecutionLog, ExecutionLogBackend as ExecutionLogProvider,
    NewExecutionRecord,
};
```

Plus a thin domain-side alias `pub type ExecutionLogProvider = B`
where `B: ExecutionLogBackend`. This is a rename, not a new trait.

**Rationale:** inventing a parallel trait would either duplicate
the surface (two traits to keep in sync) or be a thin wrapper
(marketing rename with no value). The re-export gives C3.1 the
domain-level naming the spec wants without code duplication.

**Rejected alternatives:**
- Move `ExecutionLogBackend` from `chronos-log` to `chronos-domain`.
  Would force `chronos-log` to depend on `chronos-domain`, which
  inverts the current dependency direction (today: log depends on
  domain for `InvocationId`/`SymbolId`). Cost: a Cargo.toml
  circular dependency workaround. Benefit: zero. Rejected.
- Wrap `ExecutionLogBackend` in a new `DomainExecutionLogProvider`
  trait that delegates each method. Adds an indirection layer for
  no semantic gain. Rejected.

### AD-4 — `SessionHandle` references ports, not concretes

```rust
pub struct SessionHandle {
    pub session_id: SessionId,
    pub log: Arc<dyn ExecutionLogProvider>,
    pub probe: Option<Box<dyn ProbeController>>,
    pub notifier: Arc<dyn NotificationSink>,
}
```

Each field is a port-shaped reference. The composition root
populates `SessionHandle` from concrete adapters. `chronos-services`
uses `SessionHandle` polymorphically — no concrete imports needed.

**Rationale:** the handle is the seam. If a field were a concrete
type, that would be the back door through which concrete adapters
leak into services. Ports at every field is the only way the
boundary holds.

### AD-5 — `NullProbeFactory`, `NullProbeRegistry`, `InMemorySessionRepository`, `NoopTelemetry` ship in the same commit

Each port ships with a no-op or in-memory implementation:

- `NullProbeFactory` returns `CapabilityUnavailable` for any
  requirement set.
- `NullProbeRegistry` is a no-op attach/detach with empty list.
- `InMemorySessionRepository` is a `HashMap<SessionId,
  SessionHandle>` with the documented API.
- `NoopTelemetry` discards all metrics/events.

These exist for two reasons: (1) domain unit tests can construct
handles without wiring real adapters; (2) the composition root can
default to no-op when an adapter is not configured.

**Rationale:** shipping a no-op with each declaration is standard
hexagonal practice. It keeps the domain tests self-contained and
gives the composition root a fallback.

### AD-6 — `ProbeController::detach` consumes `Box<Self>`

`fn detach(self: Box<Self>)`. This prevents calling detach twice
on the same boxed controller (the box is consumed). The
`ProbeRegistry::detach(&self, id)` method handles the actual
removal from the registry's internal map; the controller's `detach`
is a hook for the concrete adapter to release resources.

**Rationale:** lifetime correctness without `&mut self` on the
registry. The registry holds `Box<dyn ProbeController>`; when
removed, the boxed controller is dropped, which calls its
`detach` hook via `Drop`.

**Rejected alternative:** a `Drop` impl on the trait (Rust doesn't
allow `Drop` on trait objects without an explicit `Drop` trait).
A `detach` method that's idempotent on `&mut self` would require
double-call protection at every concrete site; worse.

### AD-7 — `cargo tree -p chronos-domain --depth 1` is the boundary check

After the ports are in place, `cargo tree -p chronos-domain
--depth 1` must show **no** `reqwest`, `hyper`, `tokio`,
`tokio-retry`, or any HTTP/async-runtime crate. The check is
scripted into `scripts/check_hex_boundary.py` (NEW file in this
cycle).

**Rationale:** the boundary claim is mechanical, not architectural.
If the dep graph proves it, the claim holds. If the dep graph
fails it, no amount of module-level discipline will save us.

### AD-8 — `reconstruction-contracts.toml` HEX-001 → verified

The cycle flips HEX-001 from `gap` to `verified`. The new evidence
list includes:

- The 6 port files (this cycle's deliverable).
- `scripts/check_hex_boundary.py` (the mechanical check from AD-7).
- The re-export pattern from AD-3.

The `uat` field is `[ "cargo tree -p chronos-domain --depth 1
shows no reqwest/hyper/tokio", "scripts/check_hex_boundary.py
PASS" ]`.

The `verify` field is `"cargo check -p chronos-domain --all-targets
&& cargo tree -p chronos-domain --depth 1 | scripts/check_hex_boundary.py"`.

**Rationale:** HEX-001 becomes verifiable by the framework itself,
not by an opinion. Future REC-* cycles can trust it.

## Module map (after this cycle)

```
crates/chronos-domain/
├── Cargo.toml
├── src/
│   ├── lib.rs              # re-exports ports::* at crate root
│   ├── adapter.rs          # ProbeBackend (capability surface, unchanged)
│   ├── ports/
│   │   ├── mod.rs          # mod + re-export wiring
│   │   ├── notification.rs # UNCHANGED (HEX-001 verified)
│   │   ├── probe.rs        # NEW: ProbeFactory, ProbeRegistry, ProbeController
│   │   ├── session.rs      # NEW: SessionRepository, SessionHandle
│   │   ├── telemetry.rs    # NEW: TelemetryReceiver, NoopTelemetry
│   │   └── execution_log.rs # NEW: re-export of ExecutionLogProvider
│   └── ...
├── tests/
│   └── ports/
│       ├── main.rs         # test harness
│       ├── probe.rs        # NullProbeFactory + NullProbeRegistry tests
│       ├── session.rs      # InMemorySessionRepository tests
│       ├── telemetry.rs    # NoopTelemetry tests
│       ├── execution_log.rs # dyn ExecutionLogProvider compile test
│       └── notification.rs # NullNotificationSink tests
```

## Risk assessment

| Risk | Likelihood | Mitigation |
|---|---|---|
| `dyn ExecutionLogProvider` doesn't compile (missing `?Sized`) | low | already verified in `chronos-log/src/backend.rs` |
| `Box<dyn ProbeController>` lifecycle bug | medium | unit tests cover attach/detach on NullProbeRegistry |
| `cargo tree -p chronos-domain` shows unexpected deps | medium | AD-7 adds a mechanical check that fails the cycle if violated |
| HEX-001 verified prematurely (C3.2 hasn't moved webhook) | low | C3.2 must re-open HEX-001 if any webhook code stays in domain |
| `ProbeController::detach` called from `Drop` + from registry | medium | test asserts single-call semantics |

## Out of scope (REC-C3.x)

- C3.2 — webhook extraction (next cycle after this one)
- C3.3 — services → concrete inversion
- C3.4 — symbolization port
- C3.5 — dep graph gate

## Acceptance

This cycle closes when all 10 REQs from `specification.md` are
met, plus:

- The 4 new port modules compile and pass tests.
- `cargo tree -p chronos-domain --depth 1` shows zero HTTP/async deps.
- `scripts/check_hex_boundary.py` exists and passes.
- `reconstruction-contracts.toml` HEX-001 → verified.
- `docs/ROADMAP.md` reflects REC-C3.1 in-progress.

Today's session pauses here. Tomorrow's session continues with
the build phase.