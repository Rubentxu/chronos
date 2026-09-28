# REC-C3-hexagonal-closure · Etapa B.4

## Scope

Introduce the `LifecycleStore` port in `chronos-domain` and rewire
`SessionLifecycleService` away from `chronos_store::SessionStore` for the
write-side operations it actually needs (`save_session_meta`,
`delete_session`).

## Commit

`daabd880` — refactor(services): decouple SessionLifecycleService from
concrete SessionStore via LifecycleStore port (REC-C3.5-B.4)

## Design decision: extend SessionReader, don't fork

`LifecycleStore: SessionReader` — write-side methods ride on top of the
read port, mirroring how the audit's §3.2-A2 framing described
"lifecycle" as a strictly narrower contract than the full store. Two
write methods, both borrowed:

```rust
pub trait LifecycleStore: SessionReader {
    fn save_session(
        &self,
        meta: &SessionMetadata,
        events: &[TraceEvent],
    ) -> Result<(), LifecycleStoreError>;
    fn delete_session(&self, id: &str) -> Result<(), LifecycleStoreError>;
}
```

Why a separate port (not fold into `SessionReader`):

- B.1+B.2+B.3 consumers (diff / compare / explain) are read-only; adding
  write methods would force them to drag dummy `save` implementations
  through their context types.
- The audit's smell was "services → store", not "any service → any port".
  Keeping the boundary narrow preserves the dependency-inversion benefit
  per consumer.

Why not three ports (one per write op): same reasoning as B.1 — both
write ops have identical failure semantics, both are owned by the same
service, splitting them would produce degenerate one-method traits.

## Wiring

- `chronos-domain/src/ports/lifecycle_store.rs`: port trait +
  `LifecycleStoreError` enum (`InvalidId`/`SaveFailed`/`DeleteFailed`) +
  `InMemoryLifecycleStore` (3 unit tests, all green).
- `chronos-store/src/lifecycle_store_adapter.rs`:
  `SessionStoreBackedLifecycleStore` that delegates to `SessionStore`.
  Adapter's `save_session` discards the `Vec<String>` hash return that
  `SessionStore::save_session` produces — lifecycle callers don't care
  about per-event hashes, only the metadata write matters. 4 unit tests.
- `chronos-services/src/session_lifecycle.rs`:
  - `SessionLifecycleContext.store: &SessionStore` →
    `Arc<dyn LifecycleStore>`.
  - `load`, `capabilities`, `mark_sealed` signatures take
    `&dyn LifecycleStore` (no Arc in the hot path).
  - `start` dispatcher does `&*ctx.store` to deref the Arc to the trait
    object (matches the `&*ctx.reader` pattern used by B.1 services).
  - Tests rewired with `empty_arc_store()` + `make_adapter(...)`
    helpers; `build_minimal_ctx` now returns
    `SessionLifecycleContext<'static>` constructed from an owned
    `Arc<SessionStore>`.
  - Fixed incidental `meta` → `&meta` borrow bug discovered while
    rewiring `mark_sealed` (the port method takes a reference).
- `chronos-mcp/src/server.rs`: added
  `lifecycle_store: Arc<dyn LifecycleStore>` field next to `reader`.
  Built once in `from_store` and `with_toolset` via the adapter. The
  four tool wrappers that build `SessionLifecycleContext` now clone
  the Arc instead of taking `&self.store`.

## Verification

- `cargo check --workspace --all-targets --exclude chronos-sandbox
  --exclude chronos-e2e`: clean (only the pre-existing
  `unused import: SessionReaderError` warning in chronos-domain
  remains, scope: minor cleanup, deferred to B.4.5 polish).
- `cargo test -p chronos-domain --lib`: 166 passed.
- `cargo test -p chronos-store --lib`: 84 passed (includes the 3 new
  lifecycle_store tests + 4 new adapter tests).
- `cargo test -p chronos-services --lib`: 391 passed
  (session_lifecycle's 16 tests all green after the Arc/borrow
  rewiring).
- `cargo test -p chronos-mcp --lib`: 109 passed.
- `cargo build --bin chronos-mcp`: clean. Binary SHA256:
  `3d02e02344a646f5d1d4a1da987fa47456050334b69f859a5acad37eec0ad9f7`
  (mtime 2026-09-20 13:01:19 +0200).
- T4 smoke: `chronos-sandbox e2e_connectivity` (1 test, 13.81 s) +
  `analytics_tools` (4 tests, 74.72 s) all green. No regressions.

## Decoupling check

```text
$ grep -rn "use chronos_store::SessionStore" \
    crates/chronos-services/src/
crates/chronos-services/src/diff.rs:258:    use chronos_store::SessionStore;
```

Only hit is inside `#[cfg(test)]`. `session_lifecycle`,
`session_compare`, `session_explain` are now port-only in non-test
code.

## Out of scope (deferred to Etapa B')

The 5 `store: &self.store` lines still inside `CounterexampleContext`
builders in `chronos-mcp/src/server.rs` (lines 6535–6694) consume the
`counterexample_storage` module directly. The `CounterexampleRepository`
port introduced by Tren B is not yet wired into `CounterexampleContext`.
Etapa B' closes that loop.
