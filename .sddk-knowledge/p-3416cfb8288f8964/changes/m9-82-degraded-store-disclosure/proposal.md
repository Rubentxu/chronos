# Proposal: m9-82 MCP tools disclose degraded (in-memory) store mode

> **Cycle**: `p-3416cfb8288f8964/m9-82-degraded-store-disclosure`
> **Status**: proposal-complete; spec/apply/release/archive pending
> **Date**: 2026-09-14
> **Path**: A-min
> **Tier**: T2
> **Carry-forward FIND**: FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE (m9-75)

## Intent

When a `chronos-mcp` server starts in degraded (in-memory, ephemeral)
mode because the on-disk store could not be opened and the operator
opted in via `CHRONOS_ALLOW_IN_MEMORY_FALLBACK=1`, the in-memory mode
is currently invisible to MCP tool callers. The store logs an error,
the binary returns 0 on startup, and tool responses are byte-for-byte
identical to those of a healthy server.

This cycle adds a server-level `degraded: bool` flag that is exposed
in the tool response envelope, so an MCP client that explicitly opts
into the fallback can confirm the runtime is degraded (or detect a
silent opt-in by a third party).

## Scope

- **`crates/chronos-store/src/storage.rs`**: add `pub fn is_persistent(&self) -> bool`
  to `SessionStore`. Returns `true` when the underlying redb backend is
  a file-backed `Database`; `false` for the `InMemoryBackend`. Mechanism:
  add a `kind: StoreKind` field on `SessionStore` (Persistent | InMemory),
  set by both constructors, exposed through this accessor.
- **`crates/chronos-mcp/src/server.rs`**: add `degraded: bool` field to
  `ChronosServer` (line ~1446 `from_store`); set in `from_store` based
  on the store kind. Add a `degraded` field to the JSON envelope
  emitted by the tools that return session-persistence results
  (`save_session`, `list_sessions`, `load_session`, `delete_session`,
  `drop_session`).
- **Tests**: 2 unit tests in `server.rs` and 1 in `storage.rs`. Pattern
  follows the existing `test_open_store_at_fails_closed_instead_of_degrading_silently`
  shape.

## Acceptance

- `SessionStore::is_persistent()` returns `true` for `try_open(path)`,
  `false` for `in_memory()`.
- `ChronosServer::degraded` is `false` for a server constructed from a
  `try_open(path)` store and `true` for one constructed from
  `in_memory()` (or after `try_open_default_store` falls back via the
  opt-in path).
- `save_session` / `list_sessions` / `load_session` tool responses include
  `degraded: <bool>` at the top level. Existing fields are unchanged.
  (Spec correction m9-82 mid-impl: the proposal draft named the tools
  `session_save`/`session_list`/`session_load`. The actual MCP tool
  names registered in `server.rs` are `save_session`, `list_sessions`,
  `load_session`, and the contract extends to `delete_session` and
  `drop_session` for symmetry — all five talk to the persistent store.)
- `cargo fmt --all -- --check` exits 0.
- `cargo clippy --workspace --all-targets -- -D warnings` exits 0.
- `cargo test -p chronos-store -p chronos-mcp --lib --no-fail-fast`
  passes with no regressions relative to `a0f72c2`.

## Out of scope

- Surfacing `degraded` through the `chronos-cli` (it does not currently
  use the MCP wire shape).
- Adding a dedicated "session info" tool. The cycle piggy-backs on
  existing tool responses.
- Changing the binary's exit-status policy (already correct per m9-75).
- New tests for the `chronos-sandbox` integration suite (no public
  behavior changes — the test surface is JSON body shape, which
  sandbox tests don't currently assert on).

## Carry-forward

- Closes: **FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE**.
- New: none.

## Risks

- **Wire-shape change.** The new `degraded` field is additive at the
  top level of three tool responses. Existing clients that ignore
  unknown fields keep working. Clients that strictly validate the
  response shape will need to allow the new field; this is a one-line
  update for the project itself (the only known strict validator is
  `chronos-sandbox/tests/*`, none of which assert on these shapes).
- **Tracking a runtime state through the constructor.** The
  `degraded` field is set once at construction. There is no path that
  flips it after `try_new()` returns — confirmed by reading `from_store`
  (the only constructor) and the existing `ChronosServer` field set.
- **Reversible.** `git revert` of the merge commit restores the file in
  one command.
