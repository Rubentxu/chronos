# REC-C1.5 closure — specification

**Cycle**: p-3416cfb8288f8964/rec-c1-5-closure
**Path**: A-lite
**Branch**: feat/rec-c1.5-closure (branched from `17367f2d`)
**Authoring note**: H-1 and H-2 from the user's review are already implemented in `17367f2d`. This specification covers steps 3..11 of the plan.

## Goal

Close REC-C1.5 with three mechanical deliverables (root resolver, startup wiring,
`delete_session` durable) and three real-process UAT scenarios that prove
restart integrity. After this cycle, restart + restart-mutation + retention
cannot make a session remember its run differently than it did before the
restart.

## Non-goals

- No new retention semantics, no new tail-state taxonomy.
- No changes to `reopen_existing`, `BootstrapPlan`, `BootstrapEntry`, `apply_bootstrap_plan`, or `delete_durable_execution_log`. They are already what C1.5.4-fix made them. **Do not touch them in this cycle.**
- No new tool surface; `drop_session` semantics unchanged.
- No C1.6 scope.

## Public API & behaviour — REQ-REC-C1.5-CLOSURE

### REQ-1 — Canonical ExecutionLog root resolver

The three call sites that touch the durable ExecutionLog directory tree
(session start, bootstrap, durable delete) MUST resolve the root through a
single source of truth. No `CHRONOS_EXECUTION_LOG_DIR` lookup may exist in
more than one place.

```rust
// new module: chronos_log::location (or chronos_services::location if log
// does not want the env coupling; either is acceptable as long as one
// resolver exists and is the only one)

/// The canonical location for durable ExecutionLog storage.
///
/// Resolves once, per process, from:
///   1. CHRONOS_EXECUTION_LOG_DIR (if set and valid),
///   2. a temp-dir fallback (consistent across the process).
///
/// Resolution is deterministic: two `resolve_execution_log_root()` calls
/// inside one process MUST return the same `PathBuf`.
pub fn resolve_execution_log_root() -> PathBuf;

/// The directory a given session's ExecutionLog lives in.
pub fn execution_log_dir(root: &Path, session: &SessionId) -> PathBuf;
```

**Required call-site migration**:

- `session_start` path that creates a session's ExecutionLog directory: use
  `execution_log_dir(root, &session_id)`.
- `bootstrap_execution_logs(root, …)`: root MUST come from
  `resolve_execution_log_root()`.
- `delete_durable_execution_log(…, root, …)`: root MUST come from
  `resolve_execution_log_root()`.

**Test seam**: tests can pass an explicit root. The default-root lookup is
replaced by a process-wide memoized resolver. Tests that want isolation MUST
still be able to override the root via a `with_root_for_testing(root, || …)`
helper, scoped to one closure and reverted on exit.

**Acceptance**: a test that constructs the root via env, then calls
`resolve_execution_log_root()` twice, asserts equality. A test that mutates
the env between calls asserts equality (memoization).

### REQ-2 — MCP startup wiring (bootstrap before READY)

`ChronosServer::try_new()` currently opens the `SessionStore`, constructs an
empty `SessionExecutionLogRegistry`, and returns. After this cycle, before
the server can be considered "ready" and before it accepts any
`events_read` call, the registry MUST be populated via
`bootstrap_execution_logs()`.

**Sequencing**:

```
process start
   ↓
resolve_execution_log_root()
   ↓
open SessionStore (existing)
   ↓
create empty ExecutionLogRegistry (existing)
   ↓
bootstrap_execution_logs(root, &registry)            ← NEW
   ↓
publish registry
   ↓
construct ChronosServer
   ↓
announce MCP ready
   ↓
accept events_read
```

**Error policy**:

- **Per-session failure** during bootstrap (e.g. corrupt manifest for
  session B): the existing `Unavailable(reason)` path applies. Server still
  becomes ready. events_read for B returns `ExecutionLogUnavailable`.
- **Root-level failure** (cannot stat the root directory, cannot read it,
  filesystem error, permission denied, invalid config):
  - Server MUST NOT become ready.
  - A typed `ChronosServerInitError` is returned from `try_new()`. It is
    not a `ServiceError` (the store open already had its own error
    `StoreOpenError`); this is a distinct, narrow enum:
      ```rust
      pub enum ChronosServerInitError {
          StoreOpen(StoreOpenError),
          ExecutionLogBootstrap { root: PathBuf, cause: String },
      }
      ```
  - No `warn!("bootstrap failed"); continue;` swallowing.

**Ready signal**: `try_new` returns `Ok(server)` only when the bootstrap step
succeeded (root-level) or completed with per-session `Unavailable`. There is
no separate "ready" flag; the `Result` itself is the signal.

**Acceptance test**: `try_new()` with a non-existent or unreadable root
returns `Err(ChronosServerInitError::ExecutionLogBootstrap { .. })` and the
server is never constructed. With a read/write root that contains valid
sessions, it returns `Ok(server)` and the registry already has those
sessions.

### REQ-3 — `delete_session` durable wiring

`SessionsService::delete_session` today deletes the row from `SessionStore`
and returns `Ok`. After this cycle, it MUST additionally call
`delete_durable_execution_log(&registry, root, session_id)` from
`chronos_services::execution_log_bootstrap`.

**Ordering**: `SessionStore` delete first, then durable ExecutionLog delete,
then memory cleanup. The order is deliberate:

- Store-first: the in-memory `engines` map and the `ExecutionLogRegistry`
  lose the session at the same conceptual moment. If the durable delete
  fails, the row is still gone from the store — the registry and the
  filesystem are the source of remaining truth.
- Durable second: uses pure discovery, never `build_bootstrap_plan`, and
  covers `report.duplicates`. Refuses to touch `unreadable`.

**Memory cleanup**: the existing `cleanup_session_memory` contract (the
caller already does it after `delete_session` returns `Ok`) remains.

**Partial-failure contract**:

- SessionStore delete fails → `Err(ServiceError::DeleteFailed(…))`, registry
  is unchanged, no directory removed.
- SessionStore delete succeeds, durable delete fails → `Err(ServiceError::
  DrainFailed(…))`. The handle is already out of the registry; the on-disk
  directory is still there. The tool MUST return `Ok(false)` with a
  structured `partial: true, failed_paths: [...]` payload, NOT a fake
  success.

  Wait — re-read the requirement. The user said:
  > "durable ExecutionLog delete falla → tool devuelve ERROR → nunca success falso"

  So: durable delete fails → `Err(ServiceError::DrainFailed(…))`. The
  caller of `SessionsService::delete_session` (the tool handler in
  `server.rs`) MUST surface this error to the MCP client, not convert it
  to a success.

- Durable delete succeeds → `Ok(DeleteResult { session_id, paths_removed:
  Vec<PathBuf> })`. Restart after success finds nothing in discovery.

**`drop_session`** stays as it is (memory only; restart rediscovers). The
asymmetry between `delete_session` (durable + memory) and `drop_session`
(memory only) is part of the contract and is documented at the tool level.

**Acceptance tests**:

- Happy path: delete a session, restart, discovery does not find it.
- Partial failure: delete with the durable delete path returning an error
  → tool returns `Err`, not `Ok`.
- Unaffected session: delete A while B is alive; B's manifest on disk is
  byte-identical before and after.
- Duplicate identity: delete `"foo"` when two paths claim it; both go.

### REQ-4 — Readiness invariant test

A test that asserts: `MCP server constructed ⇒ bootstrap already executed`.

Concretely, the test constructs a `ChronosServer` with a root containing a
known session, then immediately calls `events_read` on that session and
gets a populated page. It does NOT call `bootstrap_execution_logs` between
construction and the read. If a future refactor introduces background
bootstrap, this test fails.

### REQ-5 — UAT-R1: real-process resume

**Process A**:

1. Start a session, attach a probe.
2. Emit 10 000 events.
3. Call `events_read` with `cursor = cursor_start()` (None); capture first
   event E₀.
4. Call `events_read` with `cursor = advanced_to(3000)`; capture first
   event of that page as E_3k.
5. Flush.
6. Apply retention that leaves `retained_from ≤ 3000` (so cursor 3000 is
   still valid).
7. SIGKILL the process (no seal).

**Process B**:

1. Start (boots MCP, executes bootstrap).
2. Call `events_read(session_id, cursor = advanced_to(3000))`.

**Assertions**:

- The first event of the EventSeq returned by B equals E_3k.
- No duplicate event between A's page and B's page.
- No omission: the EventSeq at cursor 3000 in B equals the EventSeq at
   cursor 3000 in A.
- `SessionId` matches exactly.
- `completeness` claim is coherent (the page is either `Complete` or
   `Gap`, never `Unknown` after a successful sealed/unclean read).
- `tail_state` of the session in B is `Unclean` (process A was killed).

**Important**: this test runs in two real processes, not in a single
process that drops and recreates the registry. BOOT-2 already proved that
in-process pattern works; this proves the production wiring is equivalent.

### REQ-6 — UAT-R2: identical stale before/after restart

```text
session S
eventually cursor X
S.retained_from advances to R, R > X

before restart:
  events_read(S, cursor=X)
    => CursorStale {
         requested_next_seq: X,
         retained_from_seq:  R,
         .. (exact struct equality)
       }

kill process (no seal)

after restart:
  events_read(S, cursor=X)
    => SAME CursorStale struct, same X, same R
```

Both numbers verified, not just "some stale error".

### REQ-7 — UAT-R3: sealed persistence

```text
Process A:
  start session, 10k events, flush, seal_session, kill (clean exit)

Process B:
  bootstrap, tail_state of session == Sealed
  tail_seq identical to A
  retained_from identical to A
  events_read on session returns readable evidence
```

### REQ-8 — ById over retention

Already landed (REC-C1.5 acceptance item from C1.5.4):

> `retained_from > 0`, event not in retained range →
>   `EvidenceUnavailableDueToRetention { retained_from }`
> event IS in retained range → returned normally.

No new behaviour required. Verify the existing acceptance test still passes.

## Test strategy

- Unit tests in `chronos-services` for the root resolver and for
  `delete_session` partial-failure contract.
- Unit tests in `chronos-log` for the root resolver (if it lives in `chronos-log`).
- Integration tests in `chronos-mcp/tests/sessions_tools.rs` for the tool
  contract.
- Real-process UAT tests live in `chronos-sandbox/tests/` (or a new
  `chronos-services/tests/restart_uat.rs`) and use the existing
  `McpTestClient::start` helper.
- BOOT-*, TAIL-*, RET-*, REP-*, CONTROL sandbox tests MUST remain green
  (no regression).

## Sandbox test selection (T4-smoke)

Per AGENTS.md §2, this cycle is A-lite and touches server startup wiring,
so T4-smoke is mandatory. Subset:

- `e2e_connectivity` — confirms server starts.
- `analytics_tools` — broad coverage.
- `sessions_tools` — covers `delete_session` + `drop_session`.
- `program_scenarios` — exercises probe lifecycle on real binaries.
- `restart_uat` (NEW) — the three real-process UAT scenarios above.

## Out of scope

- Sandbox S0 (sandbox-s0 still parallel, not part of this cycle).
- C1.6.
- Any change to the `BootstrapPlan` shape, `apply_bootstrap_plan`, or
  `delete_durable_execution_log`. They are correct as of `17367f2d` and
  must not be touched in this cycle unless a UAT reveals a real bug (in
  which case file a separate change).