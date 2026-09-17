# REC-C1.5 closure — implementation receipt

**Cycle**: p-3416cfb8288f8964/rec-c1-5-closure
**Branch**: `feat/rec-c1.5-closure` (rebased onto `17367f2d`, merged --no-ff into `main`)
**Path**: A-lite
**Tag**: `rec-c1.5-closure` (peel matches `main` HEAD)
**Final SHA**: `5756168de6df3adcf0200a3e82ecce8214bafdbe` (a.k.a. `f86cb2bc`)
**Merge commit**: `a1a79c80122d3e8a859f44bb29f043f5422e1324`

## Step ledger (plan ↔ commit ↔ evidence)

| Step | Plan item | Commit | Evidence |
|---|---|---|---|
| 3 | Canonical ExecutionLog root resolver | `2c4da9ba` feat + `ee376e10` test | 4 unit tests in `chronos-log/src/location.rs` |
| 4 | MCP startup wiring (try_new boots the registry) | `94bd092a` feat | bootstrap_readiness test; typed `ChronosServerInitError::ExecutionLogBootstrap` |
| 5 | `delete_session` durable wiring | `6644bdbb`, `849184ee`, `1bb32f05`, `e2cd57a5` | restart_uat UAT-R4 (delete + restart → not found) |
| 5b | Clean-stop seal | `38e02be4`, `d3ea101b`, `1b7d0e1f`, `3acb3040` | restart_uat UAT-R3 (`tail_sealed=true`) |
| 6 | UAT-R1 unclean restart | `e75b5f6a` | r1_unclean_restart_reproduces_same_events_page |
| 7 | UAT-R2 identical-stale cursor | `23023651` | r2_stale_cursor_is_identical_before_and_after_restart |
| 8 | UAT-R3 sealed persistence | `1b7d0e1f`, `3acb3040` | r3_clean_session_stop_seals_and_restart_bootstraps_it |
| 9 | Readiness invariant (try_new) | `a2aee5b9` | ChronosServer::execution_log_registry accessor + bootstrap_readiness |
| 10 | Full regression | `1a1ec68a` fmt+clippy | T0/T1/T3/T4-smoke all green; one in-cycle fix (`59e6ac9f`) for CC#56 |
| 11 | Ledger closure | `a29baa0e`, `f86cb2bc` | apply-checkpoint + verify-findings + regen CC#4 SHA cascade; merge + tag; vault-drift-sweep PASS |

In-cycle correction: `59e6ac9f fix(rec-c1.5): sandbox tests pass env vars via child process (CC#56)` —
closes CC#56 in this same cycle by adding
`McpTestClient::start_with_db_and_exec_log_root` and migrating the four
restart_uat scenarios away from `std::env::set_var`.

## Test evidence

| Test bucket | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| T1 lib+tests (workspace, no sandbox/e2e/native) | **1262 / 0 failed** |
| `cargo test -p chronos-native --lib --tests -- --test-threads=1` | **115 / 0 failed** |
| `cargo test -p chronos-mcp --test bootstrap_readiness` | **1 / 0 failed** |
| T4-smoke: `restart_uat + e2e_connectivity + analytics_tools` | **9 / 0 failed** |
| `python3 scripts/regen_manifest_index_shas.py --check` | clean (101 manifests) |
| `bash scripts/check_vault_drift.sh` | **PASS (48 python CCs clean, 7 bash CCs clean)** |
| `sddk cycle verify-references --cycle rec-c1-5-closure` | PASSED (0 dangling) |
| `sddk ledger verify` | OK (event_count 284, last hash stable) |

## What this cycle changes from a user-visible perspective

1. **Restart no longer loses sessions.** `events_read` on a session whose
   process died survives a binary restart with byte-identical events.
   If the process was cleanly stopped, the manifest also carries
   `tail_state=Sealed`.

2. **Delete is now durable.** `delete_session` removes the
   `ExecutionLog` directory on disk; the next bootstrap does not
   resurrect it.

3. **No caller bootstrap needed.** `ChronosServer::try_new()` is
   ready for `events_read` immediately on construction. The
   `bootstrap_readiness` test fails (registry `len() == 0`) if anyone
   removes the bootstrap from `try_new` — that's the regression guard.

4. **Vault drift CC#56 closed for CHRONOS_EXECUTION_LOG_DIR too.**
   Sandbox suites pass the env var to the child MCP rather than
   mutating the test binary's process state.

## What this cycle does NOT change (out of scope)

- C1.6 (delete before probe stop, retention policy fields on wire).
- S0 sandbox suites.
- `reopen_existing` / `BootstrapPlan` / `delete_durable_execution_log`
  internals — these are off-limits to this cycle per the spec; the
  plan surfaces what already existed.

## Failure-modes observed and resolved mid-cycle

1. **CHRONOS_MCP_PATH pointed at a stale binary.** The user's
   previous session exported `CHRONOS_MCP_PATH=./target/debug/chronos-mcp`
   which on this dev machine points at the wrong directory (this repo
   has `CARGO_TARGET_DIR` overridden). Fixed by exporting the real
   one. Documented in the apply-checkpoint smoke subset note.

2. **Regression-test guard verified the R5 invariant is real.** With
   `bootstrap_execution_logs` commented out of `try_new`, the readiness
   test fails with `registry.len() == 0` vs `2` expected.
