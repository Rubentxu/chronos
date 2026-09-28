# REC-C3-hexagonal-closure · Etapa B.1+B.2+B.3

## Scope

Introduce the `SessionReader` port in `chronos-domain` and rewire three
services that previously depended on `chronos_store::SessionStore` directly
for read-only access.

## Commit

`c8c6bf60` — feat(rec-c3-hexagonal-closure): etapa B.1+B.2+B.3
SessionReader port for read-only services

## Design decision: one port, three consumers

The proposal sketched three separate ports (`DiffSource`, `ExplainSource`,
`CompareSource`). After reading the call sites, all three services have
**identical** load semantics — fetch `(SessionMetadata, Vec<TraceEvent>)`
by id, surface a `StoreError`-shaped failure with three canonical variants
(NotFound / InvalidId / LoadFailed). Splitting them would produce three
one-method traits with zero behavioral divergence.

Following the existing `SessionArchive` pattern (one port, multiple
consumers), I introduced a single `SessionReader` port and rewired all
three services through it. The write side stays separate (Etapa B.4)
because save/delete semantics differ per service.

This means the proposal's B.1/B.2/B.3 collapse into a single commit.
Documented in the commit message body and here for the audit trail.

## Changes

| File | Change |
|---|---|
| `crates/chronos-domain/src/ports/session_reader.rs` (new, 206 lines) | `SessionReader` trait + `SessionReaderError` enum + `InMemorySessionReader` + 4 unit tests |
| `crates/chronos-store/src/session_reader_adapter.rs` (new, 121 lines) | `SessionStoreBackedSessionReader` adapter + 3 unit tests |
| `crates/chronos-services/src/diff.rs` | `DiffContext.store` → `DiffContext.reader: Arc<dyn SessionReader>`; both `compare_sessions` and `performance_regression_audit` rewired; `map_load_error` now matches `SessionReaderError` |
| `crates/chronos-services/src/session_compare.rs` | `SessionCompareContext.store` → `.reader`; passes `reader.clone()` to `DiffContext` |
| `crates/chronos-services/src/session_explain.rs` | `SessionExplainContext.store` → `.reader`; `map_load_error` updated |
| `crates/chronos-mcp/src/server.rs` | `ChronosServer.reader: Arc<dyn SessionReader>` field; `from_store()` and `with_toolset()` build the adapter once; 4 tool wrappers use `Arc::clone(&self.reader)` |

## Decoupling verification

```text
$ grep -l "use chronos_store::SessionStore" crates/chronos-services/src/*.rs
crates/chronos-services/src/ce_services_tests.rs    (test helpers, not production)
crates/chronos-services/src/diff.rs                (only inside #[cfg(test)])
crates/chronos-services/src/sessions.rs            (Etapa B' target, Tren B slice C)
```

Production code in `diff.rs`, `session_compare.rs`, `session_explain.rs`
no longer imports `SessionStore`. The only `chronos_store` reference is
`TraceDiff` (a separate, stable BLAKE3 set-diff helper).

`counterexample.rs` still depends on `chronos_store::counterexample_storage`
directly — that's the Etapa B' / CounterexampleRepository port consumer
(Tren B introduced the port, not yet consumed). Separate scope.

## Evidence (pre-commit gate)

| Gate | Result |
|---|---|
| `cargo check --workspace --all-targets` | OK |
| `cargo fmt --all -- --check` | OK |
| `cargo clippy --workspace --all-targets -- -D warnings` | OK |
| `cargo test -p chronos-domain --lib session_reader` | 4/4 OK |
| `cargo test -p chronos-store --lib session_reader_adapter` | 3/3 OK |
| `cargo test -p chronos-services --lib` | 391/391 OK |
| `cargo test -p chronos-mcp --lib` | 109/109 OK |

## Audit-grounded decisions

  * **Single port, not three** — matches the existing `SessionArchive`
    pattern (one port shared across many consumers).
  * **`Arc<dyn SessionReader>`** instead of `&'a dyn SessionReader` —
    avoids lifetime gymnastics in test helpers and matches the existing
    `Arc<dyn SessionArchive>` convention.
  * **Adapter wraps the same in-memory `SessionStore`** for production;
    no new storage backend. Tests build an `Arc<SessionStore>` in the
    test, wrap it with the adapter, and hand the `Arc<dyn SessionReader>`
    to the service. Test fixtures stay simple (no new in-memory map to
    keep in sync with `SessionStore`'s own state).

## Binary artifacts

  * Path: `/var/home/rubentxu/cargo-targets/debug/chronos-mcp`
  * SHA256: `e70ff4af99dfd5e4b09d16842a3fb7d397bd3d1382fe5ff82d371508449f3a53`
  * mtime: 2026-09-20 12:51:20
  * Note: previous SHA `56b75707...` (Etapa A.3) is now superseded.

## State after B.1+B.2+B.3

  * 3 services (`diff`, `session_compare`, `session_explain`) consume
    `Arc<dyn SessionReader>` instead of `&SessionStore`.
  * `ChronosServer` builds the adapter once at construction.
  * Production code is decoupled from `chronos_store::SessionStore` for
    read paths.
  * Next: Etapa B.4 (`LifecycleStore` port for `session_lifecycle.rs` —
    save/delete semantics differ per service).
