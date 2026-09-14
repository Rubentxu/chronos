# Exploration Report — m9-82-degraded-store-disclosure

> **Cycle**: `p-3416cfb8288f8964/m9-82-degraded-store-disclosure`
> **Status**: explore-complete; spec/apply/release/archive pending
> **Date explored**: 2026-09-14
> **Branch**: `feat/m9-82-degraded-store-disclosure` (from `a0f72c2` == `origin/main`)

## Subject

| Base | Head (current) | CWD | Verified at |
|---|---|---|---|
| `a0f72c2a7fe36eaeb9c772505dfe563f85f42773` | `a0f72c2a7fe36eaeb9c772505dfe563f85f42773` (no work yet) | `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos` | 2026-09-14T09:46Z |

## Problem statement (carry-forward FIND)

`.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` lists:

> **FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE** | m9-75 | With
> `CHRONOS_ALLOW_IN_MEMORY_FALLBACK=1` the degraded in-memory mode is
> logged but never surfaced in a tool response, so an opted-in client
> cannot tell from a `session_save` / `session_list` payload that
> nothing is persisted | unassigned | m9+

m9-75 (commit by `Chronos Maintainer`, merged into main as
`v0.7.77`) closed the **silent-degradation** half of this problem
(`chronos-sandbox/tests/store_open_failure.rs` covers the binary's exit
status: 2 when the store cannot be opened without opt-in, 0 when the
opt-in is set). The remaining half is **MCP tool payload transparency**:
the same `session_save` / `session_list` payloads are produced regardless
of whether the server is in degraded mode.

## Findings (recon)

### F1 — `SessionStore` does not know its own kind

`crates/chronos-store/src/storage.rs`:50-55 defines `SessionStore` with
two private fields (`db: Arc<redb::Database>`, `cas: ContentStore`).
The discriminator between persistent and in-memory is the redb backend:
`Database::create(path)` writes to disk;
`Builder::new().create_with_backend(InMemoryBackend::new())` does not.
Neither state is recorded in the struct, so callers cannot query it.

`SessionStore::try_open(path)` (line 84) creates a persistent store;
`SessionStore::in_memory()` (line 145) creates an in-memory one. Both
return `Self` with no flag.

### F2 — `open_store_at` knows the fallback path was taken, but throws the signal away

`crates/chronos-mcp/src/server.rs`:1392 `open_store_at(path, allow_in_memory_fallback)`
is the only function that has the discrimination. When `allow_in_memory_fallback=true`
and `SessionStore::try_open` fails, it logs at error level and returns
`SessionStore::in_memory()`. The caller (`try_open_default_store` at
line 1467, `from_store` at line 1446) wraps the result in
`ChronosServer.store: Arc<SessionStore>` without recording which path
was taken.

### F3 — `ChronosServer` has no degraded flag

`ChronosServer` (line ~1420) carries the store as
`store: Arc<SessionStore>`. Tool response generation accesses the store
through methods like `list_sessions()`, `load_session()`, `save_session()`,
none of which carry a "this store is in-memory and ephemeral" annotation.

### F4 — Tool responses are JSON envelopes, not free-form

Sample shape from `mcp::server.rs`: `session_save` returns
`serde_json::to_value(&out)`. Adding a `degraded: bool` field requires
either:
- A new field on each response type that callers care about, or
- A new envelope `info { degraded: bool }` field set at the server
  boundary.

The second option is the cleaner choice because:
1. It does not require touching every response type.
2. It mirrors the way MCP servers commonly add server-level metadata
   (`_meta` keys, etc.).
3. It is additive, not breaking.

### F5 — Test plan

The existing `test_open_store_at_fails_closed_instead_of_degrading_silently`
test (server.rs ~line 6260) already exercises both modes. The new
acceptance test:
1. Construct two `ChronosServer`s. One with a persistent store, one with
   `in_memory()`.
2. Call `session_save` and assert the response includes `degraded: false`
   on the persistent one and `degraded: true` on the in-memory one.
3. Call `session_list` and assert the same.

### F6 — Risk profile

- **Single-tool surface change.** Only the JSON envelope changes; the
  shadowed types are unchanged. Existing tests that decode the response
  bodies (and ignore the new field) keep working.
- **No new dependencies.** No new crate, no new module.
- **Reversible.** `git revert` restores the file in one command.

## Path decision

**A-min** — cross-crate (chronos-store + chronos-mcp + a small
chronos-services touch) but bounded. Per AGENTS.md tier table, this
needs T0 + T2 (the changed crates' tests).

| Gate | Command | Rationale |
|---|---|---|
| T0 | `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings` | Lint gate before tests |
| T2 | `cargo test -p chronos-store -p chronos-mcp --lib --no-fail-fast` | Re-run the changed-crate suites |

## Carry-forward

- Closes: **FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE**.
- No new FINDs introduced.

## Next roadmap candidate after this cycle

m9+ carry-forwards still unassigned after m9-82:

- **FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK** (new) — `sddk cycle evaluate-gate`
  CLI is unable to record admission events on this project (FOREIGN KEY
  constraint + duplicate event_id). Recommend a separate follow-up
  cycle.
- **CC#39 off-by-one** (pre-existing): `cycles/index.md` Total cycles
  says 83 but folder count is 80. Cosmetic. Recommend a follow-up cycle
  (probably B-direct: investigate why m9-78 has no folder, then bump
  Total to match or fix the gap).
- **cc-001-god-module** (m9-04): `counterexample_storage.rs` at 2,821
  lines; 5 distinct concerns. Larger refactor; A-lite.
- M7 candidates (deferred from M6 — see `docs/ROADMAP.md`): events_read
  merge, observe merge, session_compare+session_explain split,
  session_start/stop lifecycle, deprecation sunset sweep. Each is its
  own milestone.

## Metadata

- cycle_id: p-3416cfb8288f8964/m9-82-degraded-store-disclosure
- base_sha: a0f72c2a7fe36eaeb9c772505dfe563f85f42773
- head_sha (current): a0f72c2a7fe36eaeb9c772505dfe563f85f42773
- path: A-min
- tier: T2
