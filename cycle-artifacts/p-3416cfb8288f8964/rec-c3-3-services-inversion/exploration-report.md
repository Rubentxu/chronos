# REC-C3.3 exploration-report — C3.3.0 recon + dependency map

**Cycle:** `p-3416cfb8288f8964/rec-c3-3-services-inversion`
**Path:** A-min
**Phase:** Explore
**Updated:** 2026-09-18
**Scope:** recon + dependency map (C3.3.0 only). No code changes.

## Subject

- Base: `188f2e182df180f9f947ccb1f6ad87ca028216e6` (REC-C3.2 close, pushed to origin/main).
- Head: pre-cycle (this exploration).
- Prior close: `REC-C3.2-webhook-reconciliation` (CLOSED, B-direct, reconcile-only).

## Scope statement (single sub-cycle)

> C3.3.0 — recon + dependency map: inventariar exactamente los edges
> `chronos-services → chronos-{store,native,ebpf,browser}` y qué casos de
> uso los generan. No cambiar código todavía.

The architectural rule C3.3 enforces above every other work:

```text
chronos-services
      ↓
chronos-domain::ports
      ↑
composition root
      ↓
native / ebpf / browser / store / webhook
```

Never `chronos-services → concrete adapter`. Out of scope for this
recon: `chronos-store → chronos-native` (C3.4), and the three
C31-DEBT-01/02/03 findings are observed, not closed.

## Cargo.toml: `chronos-services` declared workspace dependencies

`crates/chronos-services/Cargo.toml` declares the following workspace
dependencies (excluding non-infra):

```text
chronos-query       (legitimate: query layer)
chronos-domain      (legitimate: domain layer)
chronos-store       (EDGE #1 — to be inverted)
chronos-index       (legitimate: index layer)
chronos-ebpf        (EDGE #2 — to be inverted)
chronos-native      (EDGE #3 — to be inverted)
chronos-log         (legitimate: log layer, owns ExecutionLog writer)
chronos-browser     (EDGE #4 — to be inverted)
```

External deps (legitimate infrastructure shared by all servers):
`thiserror`, `serde`, `serde_json`, `schemars`, `tokio`, `tracing`, `uuid`,
`proptest` (dev).

`cargo tree -p chronos-services --depth 1` confirms: only those
8 workspace crates + 6 externals; no transitives are unexpected.

## Cargo.toml: `chronos-mcp` declared workspace dependencies

`crates/chronos-mcp/Cargo.toml` declares the following:

```text
chronos-domain       (legitimate)
chronos-capture      (legitimate: capture adapter layer)
chronos-native       (EDGE A — same as services #3)
chronos-log          (legitimate)
chronos-index        (legitimate)
chronos-query        (legitimate)
chronos-python       (legitimate: language pack)
chronos-js           (legitimate: language pack)
chronos-store        (EDGE B — same as services #1)
chronos-ebpf         (EDGE C — same as services #2)
```

`chronos-mcp/src/server.rs` is the **composition root for the
infrastructure adapters** (it constructs `BrowserAdapter::new()`,
`NativeProbeBackend::new()`, and opens `SessionStore::try_open` /
`SessionStore::in_memory`). This is the **only place** today where
concrete adapters are instantiated. C3.3.2 will relocate that
construction to a dedicated composition-root module.

## Edge inventory

### Edge 1 — `services → chronos-store`

| File | Symbols imported | Use case |
|---|---|---|
| `crates/chronos-services/src/session_explain.rs` | `SessionStore`, `StoreError`, `SessionMetadata` | `SessionExplain` borrowed view of metadata; reads `load_session`. |
| `crates/chronos-services/src/session_export.rs` | `SessionMetadata` | Constructs a `SessionMetadata` to write OTel attributes. |
| `crates/chronos-services/src/diff.rs` | `SessionStore`, `StoreError`, `TraceDiff` | `TraceDiff::compare` requires `&SessionMetadata` pairs. |
| `crates/chronos-services/src/output.rs` | `SessionMetadata` | Output DTO types reuse `SessionMetadata` fields. |
| `crates/chronos-services/src/sessions.rs` | `SessionMetadata`, `SessionStore` | Core `SessionsService::save_session` / `list_sessions`. |
| `crates/chronos-services/src/session_lifecycle.rs` | `SessionMetadata`, `SessionStore` | (test-only `use`) Lifecycle invariants on saved sessions. |
| `crates/chronos-services/src/session_compare.rs` | `SessionMetadata`, `SessionStore` | (test-only `use`) Compare sessions. |
| `crates/chronos-services/src/counterexample.rs` | `counterexample_storage::{ExistencePredicateWire, HypothesisInputWire, bundle_events_or_legacy, …}`, `SessionStore` | Bundle event lookups + existence predicate wire format. |
| `crates/chronos-services/src/ce_services_tests.rs` | `SessionStore` (15 sites) | Test-only; will follow inversion. |

**Symbol surface used by services from chronos-store:**
- `SessionStore`: `save_session`, `load_session`, `list_sessions`, `delete_session`, `in_memory`, `load_counterexample_bundle`, `load_counterexample_bundle_events`.
- `SessionMetadata`: full struct read (`session_id`, `created_at`, `language`, `target`, `event_count`, `duration_ms`, `tail_sealed`, `sealed_at`).
- `StoreError`: `SessionNotFound(s)`, mapped to `ServiceError::SessionNotFound(s)`.
- `TraceDiff::compare` (8-arg signature; takes `&SessionMetadata` directly).
- `counterexample_storage::ExistencePredicateWire` / `HypothesisInputWire` / `bundle_events_or_legacy` / `bundle_events_count_or_legacy`.

### Edge 2 — `services → chronos-native`

| File | Symbols imported | Use case |
|---|---|---|
| `crates/chronos-services/src/probe.rs` | `chronos_native::probe_backend::NativeProbeBackend` (used as `pub backend: NativeProbeBackend` in `LiveProbeSession`) | Owns the native probe backend driving the ptrace loop. |
| `crates/chronos-services/src/probe.rs` (line 53, 733) | `chronos_ebpf::EbpfAdapter` | (See Edge 3.) |

Symbol surface: only `NativeProbeBackend` — used in `LiveProbeSession`
struct field. Construction site lives in MCP composition root
(`server.rs:8897`).

### Edge 3 — `services → chronos-ebpf`

| File | Symbols imported | Use case |
|---|---|---|
| `crates/chronos-services/src/probe.rs` | `chronos_ebpf::EbpfAdapter` (full path — no `use`) | Optional eBPF adapter owned by `LiveProbeSession`. |
| `crates/chronos-services/src/probe.rs` (line 733) | `chronos_ebpf::EbpfAdapter::new()` + `attach_uprobe()` | Construction + attach in `probe_inject`. |

Symbol surface: `EbpfAdapter::new()` and `EbpfAdapter::attach_uprobe(pid, path, symbol)`. No other methods. The construction site at line 733 must move to the composition root.

### Edge 4 — `services → chronos-browser`

| File | Symbols imported | Use case |
|---|---|---|
| `crates/chronos-services/src/browser_probe.rs` | `chronos_browser::BrowserAdapter` (used as `pub adapter: Arc<BrowserAdapter>` in `BrowserProbeSession`) | Owns the browser adapter driving the CDP session. |

Symbol surface: only `BrowserAdapter` — used in `BrowserProbeSession`
struct field. Construction site lives in MCP composition root
(`server.rs:8620`, `:8694`, `:8740`, `:8792`, `:8843`).

## Cross-crate observations

1. **MCP is the only composition root today.** Construction sites:
   - `chronos_browser::BrowserAdapter::new()` × 5 in `server.rs`.
   - `chronos_native::probe_backend::NativeProbeBackend::new()` × 2 in `server.rs`.
   - `chronos_store::SessionStore::try_open()` / `SessionStore::in_memory()` × 3 in `server.rs` (lines 1777, 1789, 2091).
2. **SessionMetadata is dual-purposed.** It is read as a domain concept
   (session id, timestamps, language, target) but its full struct is
   imported from `chronos-store`. Domain has `SessionHandle` and
   `SessionState` in `ports/session.rs` but no `SessionMetadata` port.
   `SessionMetadata` is the strongest single inversion candidate.
3. **TraceDiff is a free function** (`pub fn compare(...) -> DiffReport`)
   that takes `&SessionMetadata`. It is a **pure computation**, not
   stateful — its inputs are already domain-shaped. It should move to
   `chronos-domain` (or stay in `chronos-store` if the persistence-side
   optimization stays attached, but be reachable only via a domain port).
   This is C3.3.1 territory.
4. **counterexample_storage module is a wire-format adapter** that
   happens to live in `chronos-store`. Its wire enums
   (`ExistencePredicateWire`, `HypothesisInputWire`) are line-for-line
   mirrors of `chronos_services::output::ExistencePredicate` /
   `HypothesisInput`. The module's docstring explicitly notes this
   (`**Circular-dep disclosure**` in `counterexample_storage.rs:14-20`).
   Inversion moves the wire enums into `chronos-domain`; the redb
   persistence stays in `chronos-store`.
5. **ExecutionLog registry** (`chronos_services::session_log`) is
   already a port-shaped wrapper around `chronos_log::SegmentedExecutionLog`.
   It carries `(SessionId, dir: Option<PathBuf>, Arc<SegmentedExecutionLog>)`.
   The wire from native backend to `SessionExecutionLog` is C1.2a;
   inverting it further (so services holds an `Arc<dyn ExecutionLogProvider>`
   instead of `Arc<SegmentedExecutionLog>`) is the natural endpoint of
   C3.3.1 — see `ports/execution_log.rs::ExecutionLogProviderShape` (REC-C3.1).
6. **Tests in services are heavy:** `ce_services_tests.rs` has ~15
   `use chronos_store::SessionStore` sites. This file inverts in C3.3.3
   alongside the production code; it is not a separate effort.

## Use case map (where each edge is needed)

| Edge | Concrete use cases that justify the dep today | C3.3.x target port |
|---|---|---|
| `services → store` (`SessionMetadata`) | `list_sessions`, `save_session`, OTel attribute export, explain, compare | `ports/session.rs::SessionRepository` (already declared in C3.1 — needs widening to carry `SessionMetadata`) |
| `services → store` (`SessionStore::save/load`) | Persistence of session state | `SessionRepository::save / load` (same port) |
| `services → store` (`counterexample_storage`) | Counterexample bundle persistence | new port: `CounterexampleStore` (or extend `SessionRepository` with bundle methods) |
| `services → native` | Live probe ptrace loop | `ports/probe.rs::ProbeController` (declared) — backend field becomes `Arc<dyn ProbeBackend>` instead of `NativeProbeBackend` |
| `services → ebpf` | Optional uprobe injection per session | new port: `EbpfInjector` (or `ProbeController::inject_uprobe`) |
| `services → browser` | Live browser probe CDP session | `ports/probe.rs::ProbeController` (browser controller wraps `BrowserAdapter` — already declared) |

## Composition root decision (preview, finalized in C3.3.2)

Three plausible placements:

1. **`chronos-mcp::composition`** (current location, factored out):
   - Pros: today the only construction site; minimum file moves.
   - Cons: ties composition to the MCP server crate; future servers
     (gRPC, HTTP-API) would need to duplicate it.
2. **New `chronos-composition` crate:**
   - Pros: explicit boundary; one source of truth for wiring.
   - Cons: a new crate for a small concern; bumps the workspace graph.
3. **`chronos-services::composition` module + re-export from `chronos-mcp`:**
   - Pros: services is the natural consumer of the wired adapters; mcp
     is a thin driving layer.
   - Cons: services acquires infra at runtime, but the imports are
     function-scoped (composition root function takes infra deps and
     returns the wired application context).

**Recommendation (provisional): option 3.** The composition root lives
inside `chronos-services` (because services holds the application
context that the root builds) but the module exposes only builders that
take `dyn`-typed arguments and return the wired graph. `chronos-mcp`
calls those builders. No new crate. ADR will be written in C3.3.1.

## Out of scope (explicit)

- `chronos-store → chronos-native` (C3.4 territory; pre-existing
  edge).
- `chronos-services → chronos-log` (legitimate: log layer owns the
  ExecutionLog writer).
- `chronos-services → chronos-query/index` (legitimate: read layers).
- C31-DEBT-01 (`ExecutionLogProvider` placeholder is still placeholder).
  Will close when C3.3.1 lands the real `Arc<dyn ExecutionLogProvider>`
  consumer in services.
- C31-DEBT-02 (`SessionId` type divergence). Will close when C3.3.1
  decides ownership (likely: `chronos-domain::session_id::SessionId`
  becomes an alias of `chronos_log::SessionId` or vice versa, decided
  by ADR).
- C31-DEBT-03 (ports not consumed by services). Will close when C3.3.3
  finishes the inversion.
- C3.4 (store → native).
- C3.5 (zero-waivers gate).

## Carries forward from REC-C3.1 / REC-C3.2

- C31-DEBT-01/02/03 (REC-C3.1).
- SDDK-GOV-RELEASE-APPLY-PERMISSIONS (framework finding).
- HEX-002 (`gap`) — closure criterion for C3.3.
- HEX-C32-03 V5 (adapter → port direction, verified in C3.2; services
  → port is the C3.3 half).

## Findings produced by this recon (no code changed)

None. Recon-only cycle. C31-DEBT-01/02/03 are observed but not closed;
no new findings are filed in this sub-cycle.
