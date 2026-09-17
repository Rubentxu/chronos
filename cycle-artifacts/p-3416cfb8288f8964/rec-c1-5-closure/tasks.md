# REC-C1.5 closure — implementation plan (tasks)

**Cycle**: p-3416cfb8288f8964/rec-c1-5-closure
**Path**: A-lite
**Branch**: feat/rec-c1.5-closure (from `17367f2d`)

## Scope

Steps 3..11 from the user-approved plan. Steps 1 (H-1) and 2 (H-2) are
already implemented in `17367f2d`.

## Decision needed before apply: No
## Chained PRs recommended: No
## Chain strategy: pending
## 400-line budget risk: Low (each step is one crate or one helper module)

## Tasks

### Step 3 — Canonical ExecutionLog root resolver

- [ ] 3.1 Create `crates/chronos-log/src/location.rs` with
      `resolve_execution_log_root()` (OnceLock-backed, memoized) and
      `execution_log_dir(root, session)`.
- [ ] 3.2 Re-export from `crates/chronos-log/src/lib.rs`.
- [ ] 3.3 Add `set_execution_log_root_for_testing` gated `#[cfg(test)]`
      under `pub mod test_root`.
- [ ] 3.4 Migrate the session-create path in
      `crates/chronos-log/src/segmented.rs` (`SessionExecutionLog::create`)
      to call `location::execution_log_dir`.
- [ ] 3.5 Migrate `bootstrap_execution_logs` and
      `delete_durable_execution_log` in
      `crates/chronos-services/src/execution_log_bootstrap.rs` to read the
      root via `location::resolve_execution_log_root()`.
- [ ] 3.6 Unit tests: env round-trip, two-call equality, override seam,
      default-fallback determinism.

### Step 4 — MCP startup wiring

- [ ] 4.1 Create `crates/chronos-mcp/src/init_error.rs` with
      `ChronosServerInitError { StoreOpen(StoreOpenError),
      ExecutionLogBootstrap { root, cause } }` and Display impl.
- [ ] 4.2 Modify `ChronosServer::try_new` in `crates/chronos-mcp/src/server.rs`:
      after `from_store`, call `bootstrap_execution_logs(root, &registry)`.
      On `Err`, return
      `Err(ChronosServerInitError::ExecutionLogBootstrap { root, cause })`.
- [ ] 4.3 Update `new()` (infallible wrapper) to map the init error to
      `panic!`.
- [ ] 4.4 Update `with_toolset(test-only)` to NOT bootstrap (tests inject
      their own registry state).
- [ ] 4.5 Integration test: `try_new()` with an unreadable root returns
      the typed error and never constructs the server.
- [ ] 4.6 Integration test: `try_new()` with a valid root returns `Ok` and
      the registry is pre-populated.

### Step 5 — `delete_session` durable wiring

- [ ] 5.1 Modify `SessionsService::delete_session` in
      `crates/chronos-services/src/sessions.rs` to call
      `delete_durable_execution_log` after the store delete.
- [ ] 5.2 Plumb the root and registry through `SessionsContext` (or pass
      them explicitly via a new field). Today `SessionsContext` doesn't
      carry the registry; add it.
- [ ] 5.3 Extend `DeleteResult` to include `paths_removed: Vec<PathBuf>`.
- [ ] 5.4 Update the MCP tool handler in `server.rs` (the
      `delete_session` tool) to surface partial failures: store-delete
      fail or durable-delete fail → tool returns `Err`. No fake success.
- [ ] 5.5 Tests:
      - happy path (delete + restart finds nothing)
      - partial failure (durable delete returns Err → tool Err)
      - unaffected session (B's manifest byte-identical before/after)
      - duplicate identity (both paths removed)

### Step 6 — UAT-R1 real-process resume

- [ ] 6.1 Create `crates/chronos-services/tests/restart_uat.rs`.
- [ ] 6.2 Implement Process A: spawn `chronos-mcp`, start session,
      emit 10k events via `probe_start` + driver, read first event
      E_3k, flush, set retention keeping C=3000, kill.
- [ ] 6.3 Implement Process B: spawn a fresh `chronos-mcp`, read the
      same cursor, assert EventSeq equality with Process A's page.
- [ ] 6.4 Assertions: first event == E_3k, no duplicates, no omissions,
      SessionId matches, tail_state == Unclean.

### Step 7 — UAT-R2 identical stale

- [ ] 7.1 In `restart_uat.rs`: scenario that sets up a session with
      `retained_from > cursor.X`, reads the cursor and captures the
      exact `CursorStale { requested_next_seq: X, retained_from_seq: R }`.
- [ ] 7.2 Restart, read again with the same cursor, capture the same
      struct.
- [ ] 7.3 Assert both numbers match exactly.

### Step 8 — UAT-R3 sealed persistence

- [ ] 8.1 In `restart_uat.rs`: scenario that emits 10k events, flush,
      seal the session, kill cleanly.
- [ ] 8.2 Restart, verify tail_state == Sealed, tail_seq matches,
      retained_from matches, evidence readable.

### Step 9 — Readiness invariant test

- [ ] 9.1 In `crates/chronos-mcp/tests/sessions_tools.rs` or new file:
      construct `ChronosServer::try_new()` with a known session in a
      tempdir root, immediately call `events_read` on that session,
      assert a populated page WITHOUT any explicit bootstrap call.

### Step 10 — Full regression

- [ ] 10.1 `cargo fmt --all -- --check`
- [ ] 10.2 `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] 10.3 `cargo test --workspace --lib --no-fail-fast`
- [ ] 10.4 `cargo test -p chronos-services -p chronos-mcp -p chronos-log --tests --no-fail-fast`
- [ ] 10.5 `cargo build --bin chronos-mcp`
- [ ] 10.6 Sandbox T4-smoke:
      `cargo test -p chronos-sandbox --test e2e_connectivity
       --test analytics_tools --test sessions_tools
       --test program_scenarios --test restart_uat -- --test-threads=1`
- [ ] 10.7 Sandbox regression for BOOT-*/TAIL-*/RET-*/REP-*/CONTROL tests.

### Step 11 — Ledger closure

- [ ] 11.1 Write `implementation-receipt.md` summarizing each step,
      each commit, each test result.
- [ ] 11.2 Run `bash scripts/check_vault_drift.sh`.
- [ ] 11.3 Run `python3 scripts/regen_manifest_index_shas.py --check`.
- [ ] 11.4 Run `sddk cycle verify-references` and `sddk ledger verify`.
- [ ] 11.5 `git checkout main && git merge --no-ff feat/rec-c1.5-closure`
      (after rebasing onto `design/rec-c1-truth-cutover` to inherit
      C1.5.4-fix; or merging both as separate PRs — to be decided in
      apply).
- [ ] 11.6 Push, tag, archive.

## Commit strategy

Each numbered step becomes a single commit on `feat/rec-c1.5-closure`
(reviewable work unit). Within step 3 and 5, sub-tasks may produce
multiple commits if they touch different files; the goal is that each
commit compiles and passes `cargo fmt --check` + `cargo clippy
--workspace --all-targets -- -D warnings`.

## Out of scope (explicit)

- Steps 1 and 2 from the user-approved plan (already in `17367f2d`).
- C1.6.
- Sandbox S0.
- Changes to `reopen_existing`, `BootstrapPlan`, `apply_bootstrap_plan`,
  or `delete_durable_execution_log`. Touching any of these in this cycle
  is forbidden unless a UAT reveals a real bug (then file a separate
  change).