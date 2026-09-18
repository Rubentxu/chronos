# REC-C3.3.1 exploration-report — identity + storage seam

**Cycle:** `p-3416cfb8288f8964/rec-c3-3-1-identity-storage-seam`
**Path:** A-min
**Phase:** Explore
**Updated:** 2026-09-18
**Scope:** identity (`SessionId` single owner) + storage seam (`ExecutionLogProvider` real, object-safe, not alias of `ExecutionLogBackend`). Composition root placement confirmed (chronos-mcp::composition).

## Subject

- Base: `147c10195f2da43b20b99b6d2d77f1c17c124242` (REC-C3.3.0 close, on origin/main).
- Prior: ADR-0015 (`~/.sddk-knowledge/p-3416cfb8288f8964/adrs/0015-identity-storage-seam.md`) frozen.
- Prior: REC-C3.3.0 exploration-report.md (4 edges inventoried, composition-root options reviewed).

## Scope statement

```text
C3.3.1 identity + storage seam:
  D1. chronos_domain::SessionId is the single owner of session identity.
      chronos_log::record::SessionId is deleted; chronos-log re-exports
      the canonical type via `pub use`.
  D2. ExecutionLogProvider is the minimal object-safe port the
      application layer needs; it is NOT an alias for ExecutionLogBackend.
  D3. EventSeq, ExecutionKind, ExecutionPayload, NewExecutionRecord,
      TailState, SealedTail move to domain (the bare minimum the port
      needs). Cursor state, gap types, segmented storage stay in chronos-log.
  D4. Composition root lives in chronos-mcp::composition, NOT
      chronos_services::composition.
  D5. C31-DEBT-01 + C31-DEBT-02 close when a productive services
      consumer of Arc<dyn ExecutionLogProvider> exists and the wire
      roundtrip preservation test passes.
```

Out of scope (explicit):

- Migrating every services consumer of chronos_log types. Only
  `SessionExecutionLog` migrates to `Arc<dyn ExecutionLogProvider>` in
  this cycle. Other call-sites (`observe`, `probe`, `events_log_read`,
  `execution_log_bootstrap`) stay on `chronos_log` types and migrate
  in REC-C3.3.3.
- `chronos-store → chronos-native` (REC-C3.4).
- Closing C31-DEBT-03 (REC-C3.3.3).
- HEX-002 closure (REC-C3.3.5).

## Inventory of SessionId call-sites

### Sites that use the type (require attention, mostly already work via re-export)

```text
crates/chronos-services/src/execution_log_bootstrap.rs:35:use chronos_log::SessionId;
crates/chronos-services/src/events_log_read.rs:1017:use chronos_log::SessionId as LogSessionId;
```

After C3.3.1 these `use` statements still resolve correctly because
`chronos_log::SessionId` becomes a re-export of
`chronos_domain::session_id::SessionId`. The `as LogSessionId` alias
is no longer semantically meaningful (both types are now the same
type) — left as a vestigial alias for the cycle, removed in C3.3.3
when `events_log_read` migrates.

### Sites that call constructors

```text
crates/chronos-services/src/observe.rs        6 sites (lines 358, 433, 637, 643, 1219, 1408)
crates/chronos-services/src/probe.rs           6 sites (lines 268, 382, 909, 973, 980, 998)
crates/chronos-services/src/events_log_read.rs 2 sites (lines 1277, 1314)
crates/chronos-native/tests/m1_03_execution_log_migration.rs 4 sites
crates/chronos-native/tests/m2_function_frame_capture.rs       2 sites
```

All constructor call-sites (`SessionId::new(...)`) compile unchanged
because `SessionId::new` already exists in
`chronos_domain::session_id::SessionId` and the re-export preserves
the path.

### Sites that touch the inner field directly (must change)

```text
crates/chronos-log/src/segment.rs:397  r.session_id.0.len()
crates/chronos-log/src/segment.rs:398  r.session_id.0.as_bytes()
```

These two lines are the **only** call sites that access `SessionId.0`
directly. Updated to use `r.session_id.as_str().len()` and
`r.session_id.as_str().as_bytes()` (ADR-0015 D1.2). The change is
mechanical and contained.

## Inventory of types lifted to domain

| Type | Current location | Lifted location | Derives needed |
|---|---|---|---|
| `SessionId` | `chronos_domain::session_id` (canonical) + `chronos_log::record` (duplicate) | `chronos_domain::session_id` (single) | `Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default` |
| `EventSeq` | `chronos_log::seq` | `chronos_domain::seq` (new) | `Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize` (already derives all) |
| `ExecutionKind` | `chronos_log::record` | `chronos_domain::evidence` (new) | `Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize` (already derives all) |
| `ExecutionPayload` | `chronos_log::record` | `chronos_domain::evidence` (new) | `Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize` (already derives all) |
| `NewExecutionRecord` | `chronos_log::backend` | `chronos_domain::evidence` (new) | `Debug, Clone, Default, PartialEq, Eq` (already derives all) |
| `TailState` | (not yet in repo) | `chronos_domain::ports::execution_log` | `Debug, Clone, Copy, PartialEq, Eq` (new) |
| `SealedTail` | (not yet in repo) | `chronos_domain::ports::execution_log` | `Debug, Clone, PartialEq, Eq` (new) |
| `ExecutionLogError` | (alias for `LogError`) | `chronos_domain::ports::execution_log` | `Debug, thiserror::Error` (new, mapped from `LogError`) |

After the lifts:

- `chronos_log::EventSeq` becomes `pub use chronos_domain::seq::EventSeq;`
- `chronos_log::ExecutionKind` becomes `pub use chronos_domain::evidence::ExecutionKind;`
- `chronos_log::ExecutionPayload` becomes `pub use chronos_domain::evidence::ExecutionPayload;`
- `chronos_log::NewExecutionRecord` becomes `pub use chronos_domain::evidence::NewExecutionRecord;`
- `chronos_log::SessionId` becomes `pub use chronos_domain::session_id::SessionId;`

The `ExecutionLogBackend` trait itself stays in `chronos_log`
(unchanged, internal). A new `chronos_log::SegmentedAdapter`
(implementing `chronos_domain::ports::execution_log::ExecutionLogProvider`)
is added — that is the productive adapter that closes `C31-DEBT-01`.

## Composition root placement (confirmed in ADR-0015)

```text
chronos-mcp::composition   (NEW module)
    ├─ build_native_probe_backend()       (factored from server.rs:8897)
    ├─ build_browser_adapter()            (factored from server.rs:8620, 8694, 8740, 8792, 8843)
    ├─ build_session_store()              (factored from server.rs:1777, 1789, 2091)
    └─ wire_application()                 (returns dyn-typed graph for services)
```

The wiring function returns the wired graph; `server.rs` calls into
the module instead of constructing adapters inline. The adapters are
still constructed inside the composition module; `Arc<dyn ...>` of
the port types flows out.

## UAT recipe for C31-DEBT-02 (wire roundtrip preservation)

The `rec_c1_8_uat_c1_05_four_dim` suite
(`crates/chronos-log/tests/rec_c1_8_uat_c1_05_four_dim.rs`)
exercises `SessionId` serialization through JSON. The test must pass
**without modification** after the D1.1..D1.3 changes — that proves
`Serialize`/`Deserialize` are preserved on the canonical type.

```bash
cargo test -p chronos-log --test rec_c1_8_uat_c1_05_four_dim -- --test-threads=1
```

If this test passes unchanged, C31-DEBT-02 closes (the wire roundtrip
is preserved). If it fails, the cycle is blocked and ADR-0015 D1.1
needs revisiting.

## UAT recipe for C31-DEBT-01 (productive consumer)

A new test `rec_c3_3_1_execution_log_provider_swap` (lives in
`crates/chronos-services/tests/`) exercises the same use case
(`SessionExecutionLog::create` + `SessionExecutionLog::reopen_existing`
+ append + read_from_seq) against **two** implementations:

1. `chronos_log::SegmentedAdapter` — the real adapter.
2. `chronos_services::InMemoryExecutionLogProvider` — a fake
   constructed in-test (lives under `#[cfg(test)]`).

Both must pass the same assertions. That proves the port is usable
through `Arc<dyn ExecutionLogProvider>` (the productive consumer is
real) and the wire-level behavior is preserved across
implementations (the abstract surface is not coupled to one
backend).

## Acceptance criteria

1. ADR-0015 committed at `~/.sddk-knowledge/p-3416cfb8288f8964/adrs/0015-identity-storage-seam.md`.
2. `chronos_domain::session_id::SessionId` derives
   `Serialize, Deserialize, Default`.
3. `chronos_log::record::SessionId` deleted; `chronos_log::lib.rs`
   has `pub use chronos_domain::session_id::SessionId;`.
4. `chronos-log/src/segment.rs` no longer touches `session_id.0`
   directly (replaced by `as_str()`/`as_bytes()`).
5. `EventSeq`, `ExecutionKind`, `ExecutionPayload`, `NewExecutionRecord`,
   `TailState`, `SealedTail`, `ExecutionLogError` lifted to domain
   with re-exports from `chronos-log`.
6. `chronos_domain::ports::execution_log::ExecutionLogProvider` is
   the real minimal port (not a `type` alias), object-safe, with
   the methods listed in ADR-0015 D2.
7. `chronos_log::SegmentedAdapter` implements the port.
8. `chronos_services::session_log::SessionExecutionLog` holds
   `Arc<dyn ExecutionLogProvider>` after the change.
9. `cargo test -p chronos-log --test rec_c1_8_uat_c1_05_four_dim`
   passes unchanged (C31-DEBT-02 closure).
10. `cargo test -p chronos-services --test
    rec_c3_3_1_execution_log_provider_swap` passes for both adapter
    implementations (C31-DEBT-01 closure).
11. `python3 scripts/check_hex_boundary.py` reports
    `chronos-log::SessionId definition count = 0` (greppable
    structural gate, see ADR-0015 D5).
12. No Cargo.toml cycle introduced. `chronos_domain` still does not
    depend on `chronos_log` (verified with `cargo tree`).
