# Release Report — m9-73-sandbox-client-store-isolation

## Path

B-direct

## Subject

Closes `FIND-M9-72-SANDBOX-SHARED-STORE-SAVE-TIMEOUT`, the deferral m9-72 recorded
when its required T4-smoke subset turned out to be flaky. The deferral named one
writer of `CHRONOS_DB_PATH`; recon found three, and the two extra ones were
hiding behind the first:

1. `McpTestClient::start()` → `start_path()` → `factory::start()` spawned the
   server with the caller's environment, so every sandbox client in every one of
   the 35 suites resolved the developer's
   `$HOME/.local/share/chronos/sessions.redb` — one 86 MB store.
2. `session_edge_cases::test_load_session_persists_across_client_instances`
   called `remove_var` / `set_var` / `set_var` / `remove_var`, mutating the
   environment of the **whole test binary**, so tests on other threads inherited
   whatever it published.
3. `counterexample_tools::ce12` called `set_var` under the comment "Set
   `CHRONOS_DB_PATH` so the CLI replay can find the same store" — false, because
   the client is started with `start_with_db_path` and `replay_bundle` passes
   `--db <path>` explicitly.

`start_path` now allocates a private store per client (`allocate_store_dir()`:
PID + process-local `AtomicU64` + nanosecond timestamp), hands it to the child
through `db_env()`, and removes the directory in `impl Drop` after taking the
process handle so the child is reaped first. `start_with_db_path` remains the
explicit opt-in for two clients that must share one store, records `db_dir: None`
so it never deletes a caller-owned path, and drops its redundant
`std::env::vars()` base. `start()` and `start_with_db_path` now share one binary
resolution ladder (`resolve_mcp_path()`). `spawn_with_env` removes
`CHRONOS_DB_PATH` from the inherited environment before applying `extra_env`, so
the child's store is only ever what the caller supplied. Both global-mutation
sites are gone.

The cycle's own test suite was rewritten once, because falsification rejected it:
the first version compared the paths the *clients recorded*, which is the
allocator's bookkeeping, and its "B cannot see A's session" assertion was
satisfied by `open_default_store` silently falling back to an in-memory store.
The suite now requires the recorded store file to exist on disk and be
non-empty, which is what proves the server opened it.

## Files changed

| Group | Count | Change |
|---|---|---|
| `chronos-sandbox/src/client/tools.rs` | 1 | +118/−39: private store allocation, `resolve_mcp_path`, `db_path()`, `impl Drop` |
| `chronos-sandbox/src/client/process.rs` | 1 | `spawn_with_env` removes the inherited `CHRONOS_DB_PATH` |
| `chronos-sandbox/tests/client_store_isolation.rs` | 1 | new: 3 tests + ambient-decoy check (212 lines) |
| `chronos-sandbox/tests/session_edge_cases.rs` | 1 | SE1 uses `start_with_db_path`; un-ignored (its `#[ignore]` existed for the mutation this cycle removed) |
| `chronos-sandbox/tests/counterexample_tools.rs` | 1 | ce12 drops `set_var`/`remove_var` |
| `AGENTS.md` | 1 | §6.5 gains the two pre-existing `session_edge_cases` failures with their measured attribution |
| `scripts/smoke_test_ccs.sh` | 1 | expected CC counts 47 → 48 python |
| `.sddk-knowledge/…/maintenance/vault-drift-sweep.md` | 1 | CC#56 documented |
| prior archive manifests | 2 | `smoke_test_ccs.sh` / `vault-drift-sweep.md` SHA rows regenerated (m9-67, m9-68) |
| `cycle-artifacts/…/m9-73-sandbox-client-store-isolation/` | 6 | new: apply-checkpoint, verify-report, verify-findings, release-report, merge-receipt, release-receipt |

## Cross-checks

- CC#1..CC#56: pass, no drift. Python CC count 47 → 48 (CC#56); bash unchanged at 7.
- CC#48 + CC#54: smoke asserts the PASS message counts; updated to 48 python, smoke 5/5.
- CC#4: this cycle edits `cycles/index.md`, `terms/index.md`, `scripts/smoke_test_ccs.sh`
  and `maintenance/vault-drift-sweep.md`, so the content-addressed rows in prior
  manifests went stale; regenerated to a fixpoint (second pass: 0 rows updated).
  The nine older manifests (m9-01, m9-03..m9-10) whose only "stale" row is the
  self-referential `archive-manifest (this file)` row were left untouched — that
  row cannot be correct by construction, CC#4 skips it by design, and rewriting it
  is unrelated churn. Recorded as `FIND-M9-73-CC4-REGEN-RITUAL-NOT-IN-REPO`.
- CC#12: `main_sha == head_sha == remote_tag_peel` (verified after the push).
- CC#24 / CC#31: this cycle's `verify-report.md` carries `## Cross-checks`; the
  archive manifest carries `Base SHA` and `Head SHA`.
- CC#56 (new): validated in both directions — 6 hits against the pre-fix tree
  (`counterexample_tools.rs:599,688`; `session_edge_cases.rs:41,44,94,176`),
  empty against the current tree.

## Self-tests performed

| Gate | Result |
|---|---|
| T0 `cargo fmt --all -- --check` | pass |
| T0 `cargo clippy --workspace --all-targets -- -D warnings` | pass, 0 warnings |
| T2 `cargo test -p chronos-sandbox --lib` | 3 passed |
| T4 `client_store_isolation` | 3 passed (36.15 s) |
| T4 `session_persistence` | 4 passed (56.56 s) |
| T4 `session_edge_cases` | 4 passed, 2 failed (109.44 s) — both failures pre-existing on `main` |
| T4 `counterexample_tools` | 12 passed (66.79 s) |
| T4 `e2e_connectivity` | 1 passed (5.64 s) |
| drift sweep | PASS (48 python + 7 bash CCs) |
| CC smoke test | 5 tests / 0 failures |

T3 was not re-run: no crate outside `chronos-sandbox` changed, and the sandbox
crate is excluded from T3 by construction.

## T4-smoke: flake tested, not tolerated

Falsification matrix (each revert applied, observed, restored; both restored
sources byte-identical to their pre-falsification copies and the suite re-run
green):

| # | Configuration | Observed |
|---|---|---|
| 1 | First draft of the suite, `start_path` guard removed | 3/3 **passed** — vacuous, as described above |
| 2 | Rewritten suite, guard removed | **2 of 3 failed**: "server A never opened the store the client recorded … fell back to an in-memory store"; explicit-path test still passed (it does not use `start_path`) |
| 3 | `CHRONOS_DB_PATH` exported to a scratch decoy, fix in place | 3/3 passed, decoy never created |
| 4 | Decoy + both guards removed | FAILED on the ambient check; `decoy.redb` created on disk |
| 5 | CC#56 against the pre-fix tree | 6 hits, exactly the removed writers |
| 6 | Interleaved A/B, `session_persistence`, alternating builds, 3 rounds | cycle branch 50/50/53 s, 3/3 pass · `main` 56/53/**60 s FAILED** (`session_persistence.rs:128`, `TimeoutError("method=tools/call", 30s)`) |
| 7 | Interleaved A/B, `session_edge_cases`, alternating builds, 3 rounds | cycle branch 3/2/2 failures · `main` 2/2/2, same two tests both sides |
| 8 | SE1 with `--ignored --exact`, twice | 1 passed / 0 failed both times (18.3 s each) |

Observation 6 tests the flake claim on the axis it was made: the failure landed
on the unmodified build in the same alternating sequence, on its slowest run.
Observation 7 is the counter-check that keeps the cycle honest — the suite it
modified is red, but it is red on `main` too.

## Pre-existing observations

- **`session_edge_cases` is red in this environment on both builds.** Two tests
  fail on every run: `test_compare_sessions_crash_vs_normal` and
  `test_performance_regression_audit_different_workloads`, both with
  `save_session failed: TimeoutError("method=tools/call", 30s)`. Attribution:
  interleaved A/B (both sides, 3 rounds each, same tests), reproduction on
  `main`, and a timeout experiment — raising the client timeout to 180 s still
  fails after 186 s, so the save genuinely does not complete.
- **Mechanism, found while characterising that failure:**
  `FIND-M9-73-CAS-PUT-ONE-WRITE-TRANSACTION-PER-EVENT` (high). The crash session
  holds 34,905 events (the test prints the count), and `ContentStore::put`
  (`cas.rs:55`) opens a redb write transaction per event — with redb's default
  immediate durability and no `set_durability` anywhere in the crate, each commit
  is a durability barrier. `SessionStore::save_session` calls it in a loop, so the
  cost is linear at roughly 4.8 ms per event: ≈168 s for 35k events against >186 s
  observed, and a few seconds for the ~2,500-event saves `session_persistence`
  uses, which is why that suite sits just under a 30 s client timeout when
  nothing contends. Proposed fix: `ContentStore::put_many(&[TraceEvent])` in one
  transaction, with `save_session` batching through it.
- **`session_edge_cases` also had a test ignored for a stale reason.** SE1 was
  `#[ignore]`d because parallel execution could interfere through the
  process-global `CHRONOS_DB_PATH`; that mechanism no longer exists, and the test
  passes 2/2 when run, so it now runs by default.

## History

- m9-72 found the flake, characterised it as a shared-store timing accident with
  interleaved A/B evidence, and deferred the harness change as out of scope for a
  store-policy cycle.
- m9-73 does the harness change, and the measurement that closed the loop
  (observation 6) is the same experiment m9-72 used to defer it — now showing the
  failure on `main` and none on the cycle branch.
- m9-73's falsification then pushed past the harness: requiring the recorded store
  to exist on disk exposed the silent in-memory fallback, and characterising the
  surviving `session_edge_cases` failure exposed the per-event write transaction.

## Future work

- `FIND-M9-73-CAS-PUT-ONE-WRITE-TRANSACTION-PER-EVENT` (high): batch the CAS
  writes; that is the fix that makes large sessions saveable, and it would turn
  the two red `session_edge_cases` tests green.
- `FIND-M9-73-SILENT-IN-MEMORY-FALLBACK-MASKS-STORE-OPEN-FAILURE` (medium):
  decide the policy for a configured store that cannot be opened instead of
  silently degrading to memory.
- `FIND-M9-73-CC4-REGEN-RITUAL-NOT-IN-REPO` (low): land the manifest-SHA
  regeneration as a repo script with the skip-self-row rule.
- `FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION` (low) and
  `FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION` (low): unchanged.
