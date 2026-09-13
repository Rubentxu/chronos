# Release Report — m9-71-services-list-store-contract

## Path

B-direct

## Subject

Closes `FIND-M9-70-SERVICES-TABLE-STRING-MATCH` (deferred by m9-70).
`SessionsService::list_sessions` tolerated a missing `sessions` table by
substring-matching the store's rendered error text
(`err_str.contains("does not exist") || err_str.contains("not exist")`). m9-70
made `chronos-store` return `Ok(vec![])` for a virgin database, so the branch
was unreachable for its only intended case, while remaining able to swallow a
genuine store failure whose message happened to contain those words. The service
now propagates the store error verbatim and its doc comment records that "no
sessions yet" is a store contract, not a caller concern.

No observable behavior change post-m9-70. The cycle converts
`list_sessions_empty` from a vacuous test into a load-bearing contract guard.

## Files changed

| Group | Count | Change |
|---|---|---|
| `crates/chronos-services/src/sessions.rs` | 1 | +22/−15: substring-match branch deleted; doc comment states the store contract; `list_sessions_empty` documented (why it is load-bearing, how it was falsified) |
| `cycle-artifacts/…/m9-71-services-list-store-contract/` | 5 | new: apply-checkpoint, verify-report, verify-findings, release-report, merge-receipt |

Total: 1 source file + 5 artifacts, +22/−15, 0 new tests (1 test strengthened).

## Cross-checks

| ID | Description | Status |
|---|---|---|
| T0 | `cargo fmt --all -- --check` | pass |
| T0 | `cargo clippy --workspace --all-targets -- -D warnings` | pass (0 warnings) |
| T2 | `cargo test -p chronos-services -p chronos-store --lib --no-fail-fast` | pass (263 + 62; 0 failed) |
| T2 | `cargo test -p chronos-mcp --tests --no-fail-fast` | pass (77 lib; 49 across integration binaries; 0 failed) |
| T4-smoke | `cargo test -p chronos-sandbox --test session_persistence --test e2e_connectivity -- --test-threads=1` | pass (5 passed; 0 failed; 62s incl. build) |
| CC#1..CC#55 | `./scripts/check_vault_drift.sh` | pass (47 python CCs clean, 7 bash CCs clean) |
| CC smoke | `./scripts/smoke_test_ccs.sh` | pass (5 run, 0 failures) |

## Self-tests performed

| Check | Command | Observation |
|---|---|---|
| Guard is real | revert m9-70 store absent-table handling, run `list_sessions_empty` | FAILED: `ListFailed("Database error: Table 'sessions' does not exist")` |
| Old test was vacuous | same store revert with the pre-m9-71 workaround restored | PASSED: 1 passed; 0 failed |
| No string matching remains | `grep -rn 'contains("does not exist")' crates/` | no matches |
| Vault term moved | `grep -n FIND-M9-70-SERVICES-TABLE-STRING-MATCH terms/index.md` | one hit, Terminated table |
| Tool surface intact | sandbox `session_persistence` (incl. `test_list_sessions_after_save`) | 4 passed |
| Server reachable | sandbox `e2e_connectivity` | 1 passed |

## Pre-existing observations

- `SessionStore::load_session` still collapses every `open_table` error into
  `SessionNotFound` (`Err(_)`), unlike `list_sessions` / `session_exists` which
  match `redb::TableError::TableDoesNotExist` explicitly. Deferred as
  `FIND-M9-71-LOAD-SESSION-TABLE-ERROR-COLLAPSE`.
- The m9-70 manifest's T4-smoke `session_persistence` count (8) does not match
  the file's 4 tests; the earlier figure looks double-counted. Noted, archive
  left frozen.
- Sandbox warm-up-ordering flake: not reproducible this cycle (5/5 in
  isolation).

## History

| Timestamp | Event |
|---|---|
| 2026-09-13T12:30Z | cycle opened on `feat/m9-71-services-list-store-contract` off `e9b6277` |
| 2026-09-13T12:30Z | workaround removed; falsified both directions |
| 2026-09-13T12:31Z | T0 / T2 / T4-smoke green |
| 2026-09-13T12:36Z | logic commit `8bbd08c` |
| 2026-09-13T12:38Z | artifacts commit; tag `v0.7.73` |

## Future work

- `FIND-M9-71-LOAD-SESSION-TABLE-ERROR-COLLAPSE` (low): normalize the absent-table
  classification across the three read paths in `chronos-store`.
- Sandbox warm-up ordering: re-characterize only if the flake reappears with a
  minimum reproduction.
- Triage of the 5 local + 19 remote unmerged branches (human review).
