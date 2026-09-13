# Verify Report — m9-72

| Campo | Valor |
|---|---|
| Cycle | `m9-72-read-path-table-error-classification` |
| Path | B-direct |
| Base SHA | `09e21107ca0707c0cd40d631eddcdb39cb3669ba` |
| Head SHA (pre-artifacts) | `845be8d8cadaa0ee94c60117927a45f9293c719a` |
| Diff digest | `sha256:9ae416b1e59e605a390a0a162f1b5a9173e611b790e203d63fb8e3aa6a111995` |
| Verified at | 2026-09-13T15:05:00Z |
| Working tree | clean before artifact writes |
| Verdict | **passed** |

## Summary

Four read paths in `chronos-store` answered *every* `open_table` failure on a
read transaction as "nothing stored": `cas::get` → `Ok(None)`, `cas::contains` →
`Ok(false)`, and both reads inside `storage::load_session` → `SessionNotFound`.
`redb` reports a database that exists but was never written as
`TableError::TableDoesNotExist`, and that is the only benign variant. Every other
variant — including `TableTypeMismatch` from a store written by a different
binary, and any variant redb adds later behind its `#[non_exhaustive]` enum — is
a storage fault.

The collapse had an observable cost, which is why this went from the cosmetic
"three read paths disagree" note recorded by m9-71 to a real cycle:
`SessionStore::load_session` consumes `ContentStore::get` in a loop
(`if let Some(evt) = self.cas.get(h)? { events.push(evt) }`), so a fault answered
with `Ok(None)` **silently dropped events** and returned a plausible, truncated
session instead of failing.

All four sites now share one classifier (`table_error`): absent means "nothing
stored yet", anything else propagates as `StoreError::Database`.

## Subject

- Closes `FIND-M9-71-LOAD-SESSION-TABLE-ERROR-COLLAPSE` (rule `code.error_classification_collapse`, low).
- Recon correction: the finding recorded **two** sites in `storage.rs`; the real count is **four** (`cas.rs` was not mentioned at all).
- New tests: 3 behavioural + 4 unit (classifier), all using a genuine fault.
- Net diff: 4 source files (1 new) + `AGENTS.md`, +103/−4.

## Files Inventory

| File | Kind | Change |
|---|---|---|
| `crates/chronos-store/src/table_error.rs` | source | new — `classify_read_table_error`, `TableOpenFailure::{Absent, Fault}`, `or_not_found`, `session_not_found`, 4 unit tests |
| `crates/chronos-store/src/lib.rs` | source | registers `mod table_error;` |
| `crates/chronos-store/src/cas.rs` | source | `get` and `contains` classify instead of matching `Err(_)`; fault test asserting both |
| `crates/chronos-store/src/storage.rs` | source | both `load_session` reads classify; two fault tests (META, EVENTS) |
| `AGENTS.md` | docs | T3 command (table in §2 and quick reference in §7) gains `--exclude chronos-e2e`; §6.5 gains the `test_ptrace_capture` hang row. See "Incidental fix" below |
| `cycle-artifacts/p-3416cfb8288f8964/m9-72-read-path-table-error-classification/apply-checkpoint.json` | artifact | new |
| `cycle-artifacts/p-3416cfb8288f8964/m9-72-read-path-table-error-classification/verify-report.md` | artifact | new (this file) |
| `cycle-artifacts/p-3416cfb8288f8964/m9-72-read-path-table-error-classification/verify-findings.json` | artifact | new |
| `cycle-artifacts/p-3416cfb8288f8964/m9-72-read-path-table-error-classification/release-report.md` | artifact | new |
| `cycle-artifacts/p-3416cfb8288f8964/m9-72-read-path-table-error-classification/merge-receipt.md` | artifact | new |
| `cycle-artifacts/p-3416cfb8288f8964/m9-72-read-path-table-error-classification/release-receipt.md` | artifact | new |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-72-read-path-table-error-classification/change-entry.md` | vault | new |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-72-read-path-table-error-classification/archive-manifest.md` | vault | new |
| `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` | vault | m9-72 row, Total cycles 71 → 72 |
| `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` | vault | `FIND-M9-71-LOAD-SESSION-TABLE-ERROR-COLLAPSE` terminated, two new deferred findings, Last archive bumped |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-02-events-side-table/archive-manifest.md` | vault | vault-index SHAs regenerated |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-70-mcp-store-isolation/archive-manifest.md` | vault | vault-index SHAs regenerated |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-71-services-list-store-contract/archive-manifest.md` | vault | vault-index SHAs regenerated |

## Falsification evidence

The fault in every test is genuine, not mocked: the table is created with an
incompatible `TableDefinition` signature, so **redb itself** fails the later
`open_table` with `TableTypeMismatch` — the same failure a store written by a
different binary would produce. `TableDoesNotExist` cannot be produced for a
table that was created, which is exactly why the old `Err(_)` arms were
invisible to the suite.

Each of the four sites was reverted **one at a time**, with the corresponding
test observed to fail, then restored:

| Site reverted | Test that fails | Observed failure |
|---|---|---|
| `cas::get` (`Err(_) => Ok(None)`) | `cas::tests::test_get_propagates_storage_fault_instead_of_reporting_missing_content` | `get` returns `Ok(None)` for a fault — no error surfaced |
| `cas::contains` (`Err(_) => Ok(false)`) | same test | `contains` returns `Ok(false)` for a fault |
| `storage::load_session` SESSION_META (`Err(_) => SessionNotFound`) | `test_load_session_propagates_storage_fault_instead_of_session_not_found` | reported as a missing session |
| `storage::load_session` SESSION_EVENTS (`Err(_) => SessionNotFound`) | `test_load_session_propagates_event_table_storage_fault` | reported as a missing session |

4/4 confirmed. The EVENTS test needed care: reaching the second `open_table`
requires a database whose `sessions` table opens normally and holds a row while
`session_events` is broken, so the metadata row is written with the correct
signature and only the event table is injected with a foreign one.

## Gates

| Tier | Command | Result |
|---|---|---|
| T0 | `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings` | pass |
| T2 | `cargo test -p chronos-store --lib` | **69** passed / 0 failed (62 → 69: 4 classifier + 3 behavioural) |
| T2 | `cargo test -p chronos-services --lib` | 263 passed / 0 failed |
| T2 | `cargo test -p chronos-mcp --tests` | 77 lib + 49 integration passed / 0 failed |
| T3 | `cargo test --workspace --lib --tests --exclude chronos-sandbox --exclude chronos-e2e --exclude chronos-native --no-fail-fast` | pass (see Incidental fix) |
| T3 | `cargo test -p chronos-native --lib -- --test-threads=1` | pass — `chronos-native` hangs in parallel, documented pre-existing |
| T4-smoke | `session_persistence` + `counterexample_tools` + `e2e_connectivity`, `--test-threads=1` | see "T4-smoke" below |
| Drift | `./scripts/check_vault_drift.sh` (CC#1..CC#55) | pass |
| CC smoke | `./scripts/smoke_test_ccs.sh` | 5/5 pass |
| CC#12 | `main_sha == head_sha == remote_tag_peel` | see release report |

## T4-smoke

Required, not optional: the diff is reachable through the MCP `session_load` /
`session_save` tool surface, and `load_session` is the function whose two reads
were changed. Subset chosen per AGENTS.md §2: `session_persistence` (the only
suite that round-trips sessions through the server), `e2e_connectivity` (proves
the server still starts and drains), `counterexample_tools` (the other consumer
of the store's read paths).

**The subset is flaky, and the flake is not this cycle's.** Full detail in
`verify-findings.json` under
`FIND-M9-72-SANDBOX-SHARED-STORE-SAVE-TIMEOUT`; the short version, with
evidence:

- `session_persistence::test_save_session_multiple_times` fails at line 128
  (`save1`) with `TimeoutError("method=tools/call", 30s)` — a client-side RPC
  timeout, not a store error. Observed in 2 of 4 consecutive runs of the
  identical three-binary command, and never in isolation.
- In the same runs `counterexample_tools::ce12` logs
  `opening session store at .../chronos-ce12-*.redb: Database error: Database
  already open. Cannot acquire lock.` — the same lock-contention symptom.
- Mechanism: `McpTestClient::start()` → `start_path()` → `factory::start()`
  inherits the caller's environment, so every sandbox client resolves
  `default_db_path()` = `$HOME/.local/share/chronos/sessions.redb`, one 86 MB
  file shared by all 35 suites. Shutdown is best-effort
  (`let _ = process.shutdown().await`), so a server can outlive its test while
  holding the redb write lock.
- Attribution: the harness and the failing test are byte-identical to `main` in
  this cycle (`git diff --name-only` lists only `crates/chronos-store`), the
  failing call is `session_save` — a write path — while the diff changes only
  read-path table classification, and the identical failure was **observed on
  `main`** with a build that excludes this cycle's code. Measured rather than
  argued: an interleaved A/B against `main` plus same-binary back-to-back runs
  (tables below).

A/B, identical three-binary command, alternating builds so the shared database
grows symmetrically for both configurations:

| Run | Build | Exit | Result |
|---|---|---|---|
| MINE-1 | cycle branch `845be8d` | **101** | `session_persistence` failed, `TimeoutError("method=tools/call", 30s)` |
| MAIN-1 | `main` `09e2110` | 0 | 17 passed |
| MINE-2 | cycle branch | 0 | 17 passed |
| MAIN-2 | `main` | 0 | 17 passed |

Then the single failing suite, three consecutive runs, **same binary** in each
group, no rebuild between runs within a group:

| Run | Build | Exit | Wall | Failure |
|---|---|---|---|---|
| BB-1 | cycle branch | 0 | 60 s | — |
| BB-2 | cycle branch (same binary as BB-1) | **101** | 76 s | line **133** (`save2`), `TimeoutError("method=tools/call", 30s)` |
| BB-3 | cycle branch (same binary) | 0 | 80 s | — |
| MAINBB-1 | `main` `09e2110` | 0 | 59 s | — |
| MAINBB-2 | `main` (same binary as MAINBB-1) | 0 | 64 s | — |
| MAINBB-3 | `main` (same binary) | **101** | 80 s | line **133** (`save2`), `TimeoutError("method=tools/call", 30s)` |

The last row is the exoneration: **the identical failure reproduces on `main`**,
with a build that does not contain this cycle's code at all. And BB-2 versus
BB-1/BB-3 shows the same binary producing both outcomes, so the outcome does not
track the build. The failing line moves between 128 (`save1`) and 133 (`save2`),
so the trigger is elapsed time, not data: `session_save` serializes ~2500
compressed events, occasionally exceeding the fixed 30 s client timeout, and the
failing runs are consistently the slower ones (76-80 s of wall time against
55-64 s for the passes).

Two claims were checked and dropped during this investigation rather than
carried into the record:

- *"A leaked `chronos-mcp` process holds the lock."* The initial symptom looked
  like a redb lock collision (`ce12` logs `Database already open. Cannot acquire
  lock.`), and `pgrep -fc chronos-mcp` reported 1-2 survivors after each run. That
  count was the pattern matching the *script's own command line*. With a specific
  pattern (`pgrep -fc 'cargo-targets/debug/chronos-mcp'`) the count is **0 before
  and after every run**, so there is no leak and no held lock. `ce12`'s message is
  its own test-level race on its scratch db, not the cause of the timeout.
- *"The flake is config-dependent."* Ruled out by MINE-2 and by BB-1/BB-3.

What remains is a shared-store, load-sensitive 30 s timeout, recorded as
`FIND-M9-72-SANDBOX-SHARED-STORE-SAVE-TIMEOUT` (medium).

## Incidental fix (AGENTS.md T3 command)

Running the documented T3 command cost this cycle ~11 minutes of no output: `cargo
test --workspace --lib --tests --exclude chronos-sandbox` pulls in
`chronos-e2e`, whose `test_ptrace_capture` needs ptrace permission this
environment does not grant, and it hangs rather than failing. AGENTS.md §1
already classifies `chronos-e2e` as bucket D, "explicit opt-in, needs root +
ptrace kernel", so the command contradicted the taxonomy two sections above it.

Fixed in this cycle: the T3 row and the quick reference now exclude
`chronos-e2e`, and §6.5 records the hang. Verified by re-running the corrected
command, which terminates, and separately by `chronos-native`'s own documented
parallel hang (that crate needs `--test-threads=1`, also pre-existing and
unchanged). The two-line change is in this cycle rather than deferred because the
bad command stalls every cycle that follows the manual, and the fix is verifiable
by the retry itself.

## Residual risk

- The behavioural tests inject a foreign-signature table. That is a real redb
  failure but not the only possible one; a future variant is covered by the
  wildcard arm and its unit test, not by end-to-end coverage.
- `load_session`'s read of the *metadata* row still uses `?`-propagation for a
  `bincode` decode failure, unchanged by this cycle: a corrupt metadata row is
  reported as a decode error, which is the pre-existing behaviour and out of
  scope here.
