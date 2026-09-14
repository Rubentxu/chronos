# Spec: m9-82 MCP tools disclose degraded (in-memory) store mode

> Spec is the contract.

## Background

m9-75 closed the silent-degradation half of the in-memory-fallback
problem by:
- making `try_open_default_store` return a hard error when the on-disk
  store cannot be opened (no fallback by default);
- preserving the pre-m9-75 opt-in (`CHRONOS_ALLOW_IN_MEMORY_FALLBACK=1`)
  but explicitly and loudly (logged at error level).

The remaining half is **tool-payload transparency**: when the
opt-in is set and the fallback fires, the server's MCP tool responses
are byte-for-byte identical to those of a healthy server. An agent
that ran a long capture session and then received a successful-looking
`session_save` response has no way to know that nothing was actually
persisted to disk.

This cycle closes that gap.

## REQ-M9-82-01 — `SessionStore` exposes its kind

`chronos_store::SessionStore` SHALL expose `pub fn is_persistent(&self) -> bool`.
The accessor SHALL return:
- `true` for a `SessionStore` constructed via `SessionStore::open(p)` or
  `SessionStore::try_open(p)` (where `p` resolves to a file-backed
  `redb::Database`).
- `false` for a `SessionStore` constructed via `SessionStore::in_memory()`.

Mechanism: add a `kind: StoreKind` enum (`Persistent | InMemory`) field
on `SessionStore`, set by both constructors.

**Scenarios:**
- `session_store_is_persistent_after_try_open` — `grep -n 'pub fn is_persistent'
  crates/chronos-store/src/storage.rs` returns 1 match.
- `session_store_kind_set_in_open` — `grep -n 'kind: StoreKind::Persistent'`
  in storage.rs returns >=1 match.
- `session_store_kind_set_in_in_memory` — `grep -n 'kind: StoreKind::InMemory'`
  in storage.rs returns >=1 match.
- `session_store_is_persistent_unit_test` — `cargo test -p chronos-store
  --lib storage::tests::test_session_store_is_persistent` passes.

## REQ-M9-82-02 — `ChronosServer` carries a `degraded` flag

`ChronosServer` SHALL expose `pub fn is_degraded(&self) -> bool` that
returns `true` when the underlying store is in-memory and `false` when
it is persistent. The flag is computed once at construction and is
immutable for the lifetime of the server.

**Scenarios:**
- `server_carries_degraded_flag` — `grep -n 'pub fn is_degraded' crates/chronos-mcp/src/server.rs`
  returns 1 match.
- `server_degraded_set_in_try_open_default_store` — `grep -n 'degraded: is_in_memory'
  crates/chronos-mcp/src/server.rs` returns >=1 match.

## REQ-M9-82-03 — Tool responses include `degraded` at the top level

`session_save`, `session_list`, and `session_load` tool responses SHALL
include `degraded: <bool>` at the top level of the JSON envelope. The
flag SHALL match `ChronosServer::is_degraded()`.

**Scenarios:**
- `session_save_includes_degraded` — a unit test that calls
  `session_save` on a server with an in-memory store and asserts the
  response contains `degraded: true`.
- `session_list_includes_degraded` — same shape, on `session_list`.
- `session_save_includes_degraded_false_for_persistent` — same shape,
  on a server with a persistent store; asserts `degraded: false`.

## REQ-M9-82-04 — Wire-shape additivity

The new `degraded` field SHALL NOT change any existing field's shape,
type, or value. Tools that already serialize their payload via
`serde_json::to_value(&out)` continue to work; only the response
wrapper is extended.

**Scenarios:**
- `existing_session_save_tests_still_pass` — `cargo test -p chronos-mcp
  --lib --no-fail-fast` returns the same pass/fail counts as the cycle
  base.

## REQ-M9-82-05 — Lint and clippy clean

- `cargo fmt --all -- --check` exits 0.
- `cargo clippy --workspace --all-targets -- -D warnings` exits 0.

## Design notes

The change to `SessionStore` is minimal:

```rust
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum StoreKind {
    Persistent,
    InMemory,
}

pub struct SessionStore {
    db: Arc<redb::Database>,
    cas: ContentStore,
    kind: StoreKind,
}

impl SessionStore {
    pub fn is_persistent(&self) -> bool { self.kind == StoreKind::Persistent }
    // ... existing constructors set kind appropriately
}
```

The change to `ChronosServer`:

```rust
pub struct ChronosServer {
    // ... existing fields ...
    store: Arc<SessionStore>,
}

// existing: fn from_store(store: SessionStore) -> Self
fn from_store(store: SessionStore) -> Self {
    let degraded = !store.is_persistent();
    Self { /* ... */ store: Arc::new(store), degraded }
}

impl ChronosServer {
    pub fn is_degraded(&self) -> bool { self.degraded }
    // ...
}
```

The change to tool responses is at the envelope layer in each tool
function: after `serde_json::to_value(&out)`, add a top-level
`degraded: serde_json::Value::Bool(self.is_degraded())`. Because the
function returns `Result<CallToolResult, rmcp::ErrorData>`, the edit
sits in the success arm and is consistent across the three tools.
