# REC-C3.3.1 specification — identity + storage seam

**Cycle:** `p-3416cfb8288f8964/rec-c3-3-1-identity-storage-seam`
**Path:** A-min
**Phase:** Specify
**Updated:** 2026-09-18
**Scope:** D1..D5 from ADR-0015. Code-change cycle, not recon.

## Why this cycle exists

REC-C3.3.0 was recon + dependency map. REC-C3.3.1 takes the first
inversion step: the **identity seam** and the **storage seam**. The
hexagonal rule that motivates it:

```text
chronos-services
      ↓
chronos-domain::ports
      ↑
composition root
      ↓
native / ebpf / browser / store / webhook
```

Today `chronos_services::session_log::SessionExecutionLog` holds
`Arc<SegmentedExecutionLog>` directly. After C3.3.1 it holds
`Arc<dyn ExecutionLogProvider>`. The seam moves one layer inward —
from concrete backend to abstract port — but the cycle does not move
the composition root (that's C3.3.2). It **creates the abstract
port** so the composition root has something concrete to wire
through.

## Decisions frozen (from ADR-0015)

The four architectural decisions frozen by ADR-0015 are the binding
spec for this cycle:

1. **D1 — `SessionId` single owner.** `chronos_domain::session_id::SessionId`
   is the canonical type. `chronos_log::record::SessionId` is
   deleted. `chronos_log::lib.rs` re-exports the canonical type via
   `pub use chronos_domain::session_id::SessionId;`.

2. **D1.1 — derives promoted to domain.**
   `chronos_domain::session_id::SessionId` derives
   `Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default`.
   The field stays private; `as_str()`, `as_bytes()`, `into_inner()`
   are the accessors (D1.3).

3. **D1.2 — inner field access updated in segment.rs.**
   `crates/chronos-log/src/segment.rs:397-398` reads
   `r.session_id.0.len()` / `r.session_id.0.as_bytes()`. Replaced by
   `r.session_id.as_str().len()` / `r.session_id.as_str().as_bytes()`.
   `as_bytes()` is added to `chronos_domain::session_id::SessionId`.

4. **D2 — `ExecutionLogProvider` real, object-safe, minimal.**
   Declared in `chronos_domain::ports::execution_log` (replacing the
   placeholder `ExecutionLogProviderShape = Capability`). Shape:

   ```rust
   pub trait ExecutionLogProvider: Send + Sync {
       fn session_id(&self) -> &SessionId;
       fn append(&self, record: NewExecutionRecord)
           -> Result<EventSeq, ExecutionLogError>;
       fn read_from_seq(&self, from: EventSeq, limit: usize)
           -> Result<ExecutionLogPage, ExecutionLogError>;
       fn retained_from(&self) -> EventSeq;
       fn tail_state(&self) -> TailState;
       fn seal(&self) -> Result<SealedTail, ExecutionLogError>;
   }
   ```

   - Object-safe (no generic methods).
   - No `record_gap`, no `read_after(consumer, cursor)`,
     no `ConsumerCursor`, no `LogConsumerId`, no `ReadResult`. Those
     stay in `chronos_log` as backend implementation detail.
   - No `append_many(impl IntoIterator<...>)` — that breaks
     object-safety. Implementations that batch expose a non-trait
     method on the concrete adapter.

5. **D3 — types lifted to domain.** `EventSeq`,
   `ExecutionKind`, `ExecutionPayload`, `NewExecutionRecord`,
   `TailState`, `SealedTail`, `ExecutionLogError` move to
   `chronos_domain`. `chronos_log` re-exports each one
   (`pub use chronos_domain::…`). No alias in the other direction.

6. **D4 — composition root lives in `chronos-mcp::composition`.**
   The C3.3.0 option 3 (`chronos_services::composition`) is
   rejected. A new `chronos-mcp::composition` module factors the
   construction sites that today live in `server.rs` (5× Browser,
   2× Native, 3× Store). For C3.3.1 the module exists structurally
   and exports builder functions; the call sites in `server.rs` are
   not yet rewritten (that's C3.3.2's first move).

7. **D5 — carry-forward closures.** `C31-DEBT-01` closes when the
   productive consumer of `Arc<dyn ExecutionLogProvider>` exists
   AND a real adapter (`chronos_log::SegmentedAdapter`) implements
   the port AND a UAT runs the same use case against the real
   adapter and an in-memory fake. `C31-DEBT-02` closes when the
   `SessionId` definition is single AND the wire roundtrip
   preservation test passes (`rec_c1_8_uat_c1_05_four_dim`,
   unchanged).

## Requirements

### REQ-C33-01 — `chronos_domain::SessionId` derives promoted
- `chronos_domain::session_id::SessionId(String)` derives
  `Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default`.
- `pub fn as_bytes(&self) -> &[u8]` and `pub fn into_inner(self) -> String`
  added.
- Field stays private.

### REQ-C33-02 — `chronos_log::record::SessionId` deleted
- `pub struct SessionId(pub String)` removed from
  `crates/chronos-log/src/record.rs:18`.
- `crates/chronos-log/src/lib.rs` gains
  `pub use chronos_domain::session_id::SessionId;`.

### REQ-C33-03 — segment.rs uses accessors
- `crates/chronos-log/src/segment.rs:397-398` updated to use
  `as_str()`/`as_bytes()` instead of `r.session_id.0`.

### REQ-C33-04 — Semantic types lifted to domain
- `EventSeq` → `chronos_domain::seq::EventSeq`. Re-exported by
  `chronos_log`.
- `ExecutionKind` → `chronos_domain::evidence::ExecutionKind`.
- `ExecutionPayload` → `chronos_domain::evidence::ExecutionPayload`.
- `NewExecutionRecord` → `chronos_domain::evidence::NewExecutionRecord`.
- `TailState`, `SealedTail`, `ExecutionLogError` →
  `chronos_domain::ports::execution_log` (the port module).

### REQ-C33-05 — `ExecutionLogProvider` real port
- `chronos_domain::ports::execution_log::ExecutionLogProvider` is a
  trait (not a `type` alias), with the shape in ADR-0015 D2.
- The placeholder `ExecutionLogProviderShape = Capability` is
  removed.
- `NoopExecutionLogProvider` is replaced by
  `InMemoryExecutionLogProvider` (real, not no-op; lives in
  `chronos_domain` or `chronos_log` depending on what the UAT
  recipe wants — final location decided at apply).

### REQ-C33-06 — `chronos_log::SegmentedAdapter`
- A new type in `chronos_log` that wraps a `SegmentedExecutionLog`
  and implements `chronos_domain::ports::execution_log::ExecutionLogProvider`.
- Construction takes `Arc<SegmentedExecutionLog>` and a
  `SessionId`; maps backend errors to `ExecutionLogError`; exposes
  the non-trait batching method `append_many` for the
  `find_c1_7` + `rec_c1_5_3_tail_state` performance-sensitive
  callers (the trait method is `append`).

### REQ-C33-07 — `SessionExecutionLog` holds the port
- `crates/chronos-services/src/session_log.rs::SessionExecutionLog`
  holds `Arc<dyn ExecutionLogProvider>` instead of
  `Arc<SegmentedExecutionLog>`.
- The `SessionExecutionLogRegistry` accepts `Arc<dyn ExecutionLogProvider>`
  for the same reason.
- Construction sites in `services::probe::start_native_probe` and
  `services::session_log::reopen_existing` construct a
  `SegmentedAdapter` (via `chronos_log`); that adapter is what
  flows into the registry. The migration is **inside** the
  `SessionExecutionLog`/`SessionExecutionLogRegistry` boundary,
  not across the rest of services.

### REQ-C33-08 — `chronos-mcp::composition` module factored
- New module `crates/chronos-mcp/src/composition.rs` exposes:
  - `pub fn build_session_store() -> Result<Arc<SessionStore>, StoreError>`
  - `pub fn build_browser_adapter() -> Arc<BrowserAdapter>`
  - `pub fn build_native_probe_backend() -> NativeProbeBackend`
- `crates/chronos-mcp/src/server.rs` may keep using inline
  constructors (C3.3.2 rewrites them); for C3.3.1 the module exists
  and exports compile, but the call sites are not yet rewritten.
  The minimum is "structural existence + compile" — this lets the
  next cycle do mechanical rewrites without re-architecting.

### REQ-C33-09 — UAT: wire roundtrip preservation
- `cargo test -p chronos-log --test rec_c1_8_uat_c1_05_four_dim`
  passes **without modification**. The test exercises
  `Serialize`/`Deserialize` on `SessionId` and the four-dimension
  `ExecutionRecord` invariant.

### REQ-C33-10 — UAT: productive consumer with two adapters
- New test `crates/chronos-services/tests/rec_c3_3_1_execution_log_provider_swap.rs`
  exercises the same use case (`create` → `append` → `read_from_seq` →
  `seal`) against:
  1. `chronos_log::SegmentedAdapter` (real).
  2. An in-memory fake (constructed inline in the test).
- Both implementations must satisfy the same assertions.

## Architectural decisions NOT changed by this cycle

- `HEX-002` remains `gap`. Only one services consumer migrates this
  cycle; the rest migrate in C3.3.3.
- `C31-DEBT-03` remains open until C3.3.3.
- `chronos-store → chronos-native` (REC-C3.4) untouched.
- `chronos-mcp::server.rs` construction sites not yet rewritten
  (C3.3.2).
- No new `chronos-composition` crate (ADR-0015 D4).

## Acceptance criteria for THIS cycle

1. ADR-0015 committed at
   `~/.sddk-knowledge/p-3416cfb8288f8964/adrs/0015-identity-storage-seam.md`.
2. `chronos_domain::session_id::SessionId` derives
   `Serialize, Deserialize, Default` (REQ-C33-01).
3. `chronos_log::record::SessionId` deleted; re-export from
   `chronos_log::lib.rs` (REQ-C33-02).
4. `chronos-log/src/segment.rs:397-398` uses `as_str()` (REQ-C33-03).
5. `EventSeq`, `ExecutionKind`, `ExecutionPayload`, `NewExecutionRecord`,
   `TailState`, `SealedTail`, `ExecutionLogError` lifted to
   domain with `pub use` re-exports from `chronos_log`
   (REQ-C33-04).
6. `chronos_domain::ports::execution_log::ExecutionLogProvider`
   is the real minimal port; the placeholder
   `ExecutionLogProviderShape` removed (REQ-C33-05).
7. `chronos_log::SegmentedAdapter` implements the port (REQ-C33-06).
8. `chronos_services::session_log::SessionExecutionLog` holds
   `Arc<dyn ExecutionLogProvider>` (REQ-C33-07).
9. `chronos_mcp::composition` module exists with the three
   builder functions and compiles (REQ-C33-08).
10. `cargo test -p chronos-log --test rec_c1_8_uat_c1_05_four_dim`
    passes unchanged (REQ-C33-09, C31-DEBT-02 closure).
11. `cargo test -p chronos-services --test
    rec_c3_3_1_execution_log_provider_swap` passes for both
    adapter implementations (REQ-C33-10, C31-DEBT-01 closure).
12. `python3 scripts/check_hex_boundary.py` (or a sibling
    structural check) reports
    `chronos-log::SessionId definition count = 0`.
13. No Cargo.toml cycle introduced. `cargo tree -p chronos-domain`
    still does not show `chronos_log` as a transitive dep.

## Out-of-scope

- Migrating `observe.rs`, `probe.rs`, `events_log_read.rs`,
  `execution_log_bootstrap.rs` to consume the lifted domain types.
  They keep using `chronos_log` paths; C3.3.3 handles them.
- Closing `C31-DEBT-03` (REC-C3.3.3).
- Closing `HEX-002` (REC-C3.3.5).
- Rewriting `chronos_mcp::server` to call into the new
  composition module (REC-C3.3.2 first move; C3.3.1 only needs
  the module to compile).
- `chronos-store → chronos-native` (REC-C3.4).
- `chronos-composition` new crate (ADR-0015 D4 — only when a second
  entrypoint exists).
