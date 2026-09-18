# Specification — rec-c3-1-application-ports

## Goal

Introduce the 6 application ports required by REC-C3.1 in
`chronos-domain/src/ports/`, declaring the contracts that
`chronos-services` and the composition root will route through.

## Non-goals

- **No inversion of services → concrete adapters.** That is REC-C3.3.
- **No extraction of webhook infrastructure.** That is REC-C3.2.
- **No symbolization port.** That is REC-C3.4.
- **No removal of `reqwest` from domain deps.** That is REC-C3.2.
- **No change to `chronos-services` callers.** This cycle adds
  ports in domain; nothing imports them yet.

## Requirements

### REQ-1 — `ProbeFactory` + `ProbeRegistry` ports

Create `crates/chronos-domain/src/ports/probe.rs` declaring:

```rust
pub trait ProbeFactory: Send + Sync {
    fn create(
        &self,
        session_id: SessionId,
        config: &CaptureConfig,
        requirements: &[Capability],
    ) -> Result<Box<dyn ProbeController>, CapabilityUnavailable>;
}

pub trait ProbeRegistry: Send + Sync {
    fn attach(&self, session_id: SessionId, controller: Box<dyn ProbeController>);
    fn detach(&self, session_id: &SessionId) -> Result<(), TraceError>;
    fn list_active(&self) -> Vec<SessionId>;
}
```

- `ProbeController` is a new abstract trait introduced here. It must
  cover the minimum surface that
  `crates/chronos-domain/src/adapter.rs::ProbeBackend` exposes today:
  attach, detach, stop, capture_frame (if applicable), event sink.
  Decision: model `ProbeController` as a distinct trait
  (lifetime-focused) that wraps the lower-level
  `ProbeBackend` (capability-focused). The composition root wires
  one to the other.

### REQ-2 — `ExecutionLogProvider` re-export port

Add `crates/chronos-domain/src/ports/execution_log.rs` declaring:

```rust
pub use chronos_log::{ExecutionLog, ExecutionLogBackend, NewExecutionRecord};
```

This is a *re-export*, not a new trait. The rationale: C3.1 needs a
domain-level name for the storage-side port. The `?Sized` bound on
`ExecutionLog<B>` (already present) means `dyn ExecutionLogBackend`
works without code change.

Tests under `crates/chronos-domain/tests/ports/execution_log.rs`:

- Compile-only test that `Arc<dyn ExecutionLogProvider>` compiles.
- Compile-only test that the re-export does not bring any
  HTTP/runtime dependency into domain's dep graph (asserted via
  `cargo tree -p chronos-domain --depth 1`).

### REQ-3 — `SessionRepository` port

Create `crates/chronos-domain/src/ports/session.rs` declaring:

```rust
pub trait SessionRepository: Send + Sync {
    fn find(&self, id: &SessionId) -> Result<Option<SessionHandle>, TraceError>;
    fn list(&self) -> Vec<SessionId>;
}

pub struct SessionHandle {
    pub session_id: SessionId,
    pub log: Arc<dyn ExecutionLogProvider>,
    pub probe: Option<Box<dyn ProbeController>>,
    pub notifier: Arc<dyn NotificationSink>,
}
```

`SessionHandle` references the ports from REQ-1, REQ-2, REQ-5. No
domain types are leaked.

### REQ-4 — `TelemetryReceiver` port

Create `crates/chronos-domain/src/ports/telemetry.rs` declaring:

```rust
pub trait TelemetryReceiver: Send + Sync {
    fn metric(&self, name: &str, value: i64, attrs: &[(&str, &str)]);
    fn event(&self, name: &str, attrs: &[(&str, &str)]);
}

pub struct NoopTelemetry;
impl TelemetryReceiver for NoopTelemetry { ... }
```

### REQ-5 — `NotificationSink` port — verified, no change

`crates/chronos-domain/src/ports/notification.rs` already declares
`NotificationSink` + `NullNotificationSink`. The cycle verifies:

- `chrono-domain` does NOT import `reqwest`, `hyper`, or any HTTP
  client. Asserted by `cargo tree -p chronos-domain`.
- `NullNotificationSink` is exported alongside `NotificationSink`.

### REQ-6 — `ArtifactStore` / symbolization port — DEFERRED

The cycle records the deferment but does NOT introduce the port.
`docs/chronos-agentic-reconstruction/docs/roadmap/CONVERGENCE_BACKLOG.md`
places this in C3.4. The spec section "Out of scope" carries the
note.

### REQ-7 — `ports/mod.rs` updated

`crates/chronos-domain/src/ports/mod.rs` wires the new modules:

```rust
mod execution_log;
mod notification;
mod probe;
mod session;
mod telemetry;

pub use execution_log::*;
pub use notification::*;
pub use probe::*;
pub use session::*;
pub use telemetry::*;
```

Re-exports at `chronos_domain::*` (in `lib.rs`) are updated.

### REQ-8 — Compile-test under `tests/ports/`

Add `crates/chronos-domain/tests/ports/main.rs` (or equivalent) with
unit tests for each port:

- `probe.rs` test: `NullProbeFactory` and `NullProbeRegistry` exist
  and have no-op implementations.
- `session.rs` test: `InMemorySessionRepository` is implementable
  (compile-check + trivial smoke).
- `telemetry.rs` test: `NoopTelemetry` satisfies the trait.
- `execution_log.rs` test: the path compiles; the dyn bound works.
- `notification.rs` test: `NullNotificationSink` is the documented
  default.

### REQ-9 — `reconstruction-contracts.toml` HEX-001 → verified

Flip HEX-001 ("Domain contains no HTTP/runtime infrastructure")
from `gap` to `verified`. Evidence: this cycle introduces the port
shape that lets the domain compile without runtime deps. The
HTTP-removal is REC-C3.2 (next); HEX-001 *verified* here means
"the port contract exists and the boundary is sealed by the
compiler", not "the adapter has been moved out".

Add explicit `uat` + `verify` fields per the contract schema.

### REQ-10 — `docs/ROADMAP.md` REC-C3 entry updated

Add a short note in the Active Milestone section: "REC-C3.1 in
progress at cycle `p-3416cfb8288f8964/rec-c3-1-application-ports`".

## Tier

A-full. T0 + T1 + T2 of `chronos-domain` + integration of touched
crates. No sandbox smoke (no MCP wire change).

## Out of scope (REC-C3.x follow-on cycles)

- **REC-C3.2** — webhook infrastructure extraction.
- **REC-C3.3** — invert services → concrete.
- **REC-C3.4** — symbolization port.
- **REC-C3.5** — dependency graph gate.

## Acceptance

The cycle closes when:

- All 6 REQs above are met.
- `cargo check --workspace --all-targets --all-features` clean.
- `cargo test -p chronos-domain --lib --tests --no-fail-fast` green.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo tree -p chronos-domain --depth 1` shows no `reqwest`, no
  `hyper`, no `tokio`.
- `reconstruction-contracts.toml --strict-legacy` still PASS.