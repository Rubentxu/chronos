# Design: REC-C1.5 closure

**Cycle**: p-3416cfb8288f8964/rec-c1-5-closure
**Path**: A-lite
**Branch**: feat/rec-c1.5-closure (from `17367f2d`)

## Technical Approach

Three mechanical deliverables + three real-process UAT scenarios. The
deliverables are:

1. **Canonical ExecutionLog root resolver** in `chronos_log::location`,
   used by the three call sites that touch durable ExecutionLog state.
2. **MCP startup wiring**: `ChronosServer::try_new()` runs the bootstrap
   before returning `Ok`; root-level bootstrap failure surfaces as
   `ChronosServerInitError::ExecutionLogBootstrap`.
3. **`delete_session` durable wiring**: `SessionsService::delete_session`
   calls `delete_durable_execution_log` after the store delete. Partial
   failures surface as `Err`, never as `Ok`.

Plus three real-process UAT scenarios (`restart_uat.rs`):
UAT-R1 (resume valid), UAT-R2 (identical stale), UAT-R3 (sealed).

## Architecture Decisions

### Decision: Where `resolve_execution_log_root()` lives

**Choice**: `chronos_log::location` (a new module under
`crates/chronos-log/src/location.rs`).
**Alternatives considered**:
- In `chronos_services` — rejected: it must be usable from
  `chronos_log::SegmentedExecutionLog::create`, which is the session-start
  path. Putting the resolver in `chronos_services` would create a cycle
  (services depends on log, log would depend on services).
- In a top-level `crate::location` of `chronos-mcp` — rejected: the
  resolver is also used by `delete_session` and bootstrap, both of which
  live in `chronos_services`. The MCP layer is a consumer.
**Rationale**: log already owns the durable storage abstraction. The
resolver belongs with the storage primitives.

### Decision: How the resolver exposes override-for-tests

**Choice**: a process-wide `OnceLock<PathBuf>` initialised lazily by
`resolve_execution_log_root()`, plus a `set_execution_log_root_for_testing`
helper gated behind `#[cfg(test)]` and re-exported under
`#[cfg(test)] mod test_root`.
**Alternatives considered**:
- Thread-local — rejected: bootstrap runs on one thread, MCP tool
  handlers on another. Tests would see two different roots.
- Env var re-read on every call — rejected: env mutation between calls
  would make the resolver non-deterministic and re-introduce the
  "start writes A, restart reads B" bug class.
**Rationale**: a single `OnceLock` makes the resolver idempotent and
memorable across threads. The `cfg(test)` setter is the single seam tests
use; once set it is sticky until the process exits.

### Decision: Where `ChronosServerInitError` lives

**Choice**: `crates/chronos-mcp/src/init_error.rs` (new file).
**Alternatives considered**:
- Extend `StoreOpenError` — rejected: a bootstrap failure is not a store
  open failure. Conflating them would leak the bootstrap path into the
  store error type.
- Reuse `ServiceError` — rejected: per user instruction ("preferiría un
  pequeño `ChronosServerInitError` antes que meter un `ServiceError`
  convertido a string"). The startup barrier must be a typed, narrow
  enum.
**Rationale**: matches the existing pattern (`StoreOpenError` already
lives in this crate).

### Decision: Order of operations in `delete_session`

**Choice**: store-delete → durable-delete → memory cleanup, returning
`Err` if either store-delete or durable-delete fails.
**Alternatives considered**:
- Durable-delete first, store-delete second — rejected: store is the
  index of "what sessions exist" and the in-memory `engines` map mirrors
  it. If durable-delete fails, we want the caller to retry; the session
  is still in the store, so re-trying will hit the same state.
- Memory cleanup before durable-delete — rejected: if the durable delete
  fails, the in-memory map is gone but the on-disk directory remains,
  leaving a non-discoverable session that bootstrap cannot revive.
**Rationale**: matches user's "store first, durable second, memory last"
sequencing; surfaces partial failures as errors.

### Decision: Real-process UAT harness

**Choice**: new file `crates/chronos-services/tests/restart_uat.rs` (a
test that spawns two `chronos-mcp` subprocesses via the existing
`chronos_sandbox::client::tools::McpTestClient::start`).
**Alternatives considered**:
- Single-process with `drop(registry); new_registry(); bootstrap()` —
  rejected: user explicitly said this pattern (BOOT-2) is insufficient.
- Shell-driven harness via a `justfile` target — rejected: not
  reproducible in CI; needs a hand-maintained test sequence.
**Rationale**: stays inside `cargo test`, uses the existing
`McpTestClient` infrastructure that already knows how to spawn the
server and resolve `CHRONOS_MCP_PATH`.

## Data Flow

### Startup

```
process start
   ↓
CHRONOS_EXECUTION_LOG_DIR (env, optional)
         ↓
resolve_execution_log_root() → PathBuf   (memorized OnceLock)
         ↓
execution_log_dir(root, session_id) → PathBuf   (per-session subdir)
         ↓
ChronosServer::try_new()
   ├── SessionStore::open (existing)
   ├── SessionExecutionLogRegistry::new (existing)
   ├── bootstrap_execution_logs(root, &registry)
   │      ├── discover_execution_logs(root) (chronos_log::discovery)
   │      ├── build_bootstrap_plan(root)
   │      │      for each candidate:
   │      │         reopen_existing → Available{log} | Unavailable{reason}
   │      └── apply_bootstrap_plan(&registry, &plan)
   │             pure publish, no IO
   └── construct ChronosServer
   ↓
events_read calls now hit the populated registry
```

### delete_session

```
tool call: delete_session(session_id)
   ↓
SessionsService::delete_session(ctx, session_id)
   ├── store.delete_session(session_id)
   │     └── Err(ServiceError::DeleteFailed) → return Err
   ├── delete_durable_execution_log(&registry, root, session_id)
   │     ├── registry.remove(session_id)
   │     ├── discover_execution_logs(root)   ← pure discovery, no reopen
   │     ├── collect paths from report.logs + report.duplicates
   │     └── remove_dir_all(paths)
   │     └── Err(ServiceError::DrainFailed) → return Err
   ├── cleanup_session_memory   (existing helper, after Ok from delete)
   └── return Ok(DeleteResult { session_id, paths_removed })
```

## File changes

| File | Change |
|---|---|
| `crates/chronos-log/src/location.rs` (new) | `resolve_execution_log_root`, `execution_log_dir`, `set_execution_log_root_for_testing` |
| `crates/chronos-log/src/lib.rs` | re-export `location` |
| `crates/chronos-log/src/segmented.rs` | session-create path uses `location::execution_log_dir` |
| `crates/chronos-services/src/execution_log_bootstrap.rs` | bootstrap and delete take `root` from `resolve_execution_log_root()` |
| `crates/chronos-mcp/src/init_error.rs` (new) | `ChronosServerInitError` enum |
| `crates/chronos-mcp/src/server.rs` | `try_new` runs bootstrap; `delete_session` tool calls durable-delete helper |
| `crates/chronos-services/src/sessions.rs` | `delete_session` calls `delete_durable_execution_log` after store delete |
| `crates/chronos-services/tests/restart_uat.rs` (new) | three real-process UAT scenarios |

## Risks

- The `OnceLock` resolver introduces a process-wide test seam. Tests that
  forget to use `set_execution_log_root_for_testing` may leak state
  between test functions. Mitigation: each UAT test creates a fresh
  `tempdir`, sets it as the root, runs the test, and re-checks the
  helper clears via process exit (per-test tempdir guarantees no
  cross-test pollution).
- Touching `server.rs::try_new` will run the bootstrap on every test that
  uses `try_new`, including unit tests. Mitigation: most unit tests use
  `with_toolset` (test-only); the new bootstrap path is exercised by
  integration tests with a controlled root.
- The real-process UAT may be slow. Mitigation: 10k events is realistic
  but not enormous; assert on first event + EventSeq equality, not full
  byte-equality of 10k records.

## ADR candidates

None. None of the decisions meet all three ADR criteria (hard to reverse,
surprising, real trade-off). The resolver placement, the
`OnceLock` test strategy, and the delete order are all conventional.