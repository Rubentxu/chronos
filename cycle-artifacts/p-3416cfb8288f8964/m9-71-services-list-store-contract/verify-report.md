# Verify Report — m9-71

| Campo | Valor |
|---|---|
| Cycle | `m9-71-services-list-store-contract` |
| Path | B-direct |
| Base SHA | `e9b6277170cb8059f5cd0340a3f66a5144558faf` |
| Head SHA (pre-artifacts) | `8bbd08ccf8f48611f098776528efa6500cc4cc4e` |
| Diff digest | `sha256:9ff4b678f5b4ebeb8d2a281c1b80d2d387ed809d62daccdbbfe424115125ca63` |
| Verified at | 2026-09-13T12:38:00Z |
| Working tree | clean before artifact writes |
| Verdict | **passed** |

## Summary

`SessionsService::list_sessions` recovered from a missing `sessions` table by
substring-matching the store's rendered error text. m9-70 moved that decision
into `chronos-store` (a virgin database now returns `Ok(vec![])`), which left
the workaround unreachable dead code that could also swallow a genuine store
error whose message contained "not exist". This cycle deletes it: the service
propagates the store error verbatim and documents the empty-store case as a
store contract.

There is **no observable behavior change** after m9-70. The cycle's value is
the promotion of `list_sessions_empty` from a vacuous test (it passed with and
without the store fix) into a load-bearing guard (it now fails the instant the
store regresses), demonstrated in both directions before the patch landed.

## Subject

- Closes `FIND-M9-70-SERVICES-TABLE-STRING-MATCH` (rule `code.error_text_matching`, low).
- Defers `FIND-M9-71-LOAD-SESSION-TABLE-ERROR-COLLAPSE` (rule `code.error_classification_collapse`, low).
- New tests: 0. Existing test strengthened + documented: 1 (`list_sessions_empty`).
- Net diff: 1 source file, +22/−15.

## Files Inventory

| File | Kind | Change |
|---|---|---|
| `crates/chronos-services/src/sessions.rs` | source | `list_sessions` loses the `contains("does not exist")` branch and propagates the store error; doc comment rewritten to state the store contract; `list_sessions_empty` documented as the load-bearing guard |
| `cycle-artifacts/p-3416cfb8288f8964/m9-71-services-list-store-contract/apply-checkpoint.json` | artifact | new |
| `cycle-artifacts/p-3416cfb8288f8964/m9-71-services-list-store-contract/verify-report.md` | artifact | new (this file) |
| `cycle-artifacts/p-3416cfb8288f8964/m9-71-services-list-store-contract/verify-findings.json` | artifact | new |
| `cycle-artifacts/p-3416cfb8288f8964/m9-71-services-list-store-contract/release-report.md` | artifact | new |
| `cycle-artifacts/p-3416cfb8288f8964/m9-71-services-list-store-contract/merge-receipt.md` | artifact | new |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-71-services-list-store-contract/change-entry.md` | vault | new (post-release commit) |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-71-services-list-store-contract/archive-manifest.md` | vault | new (post-release commit) |
| `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` | vault | m9-71 row, Total cycles 70→71 |
| `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` | vault | FIND-M9-70-SERVICES-TABLE-STRING-MATCH terminated; FIND-M9-71-LOAD-SESSION-TABLE-ERROR-COLLAPSE deferred |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-02-events-side-table/archive-manifest.md` | vault | cycles/index.md + terms/index.md SHAs regenerated |

## Drift Evidence (pre-cycle baseline)

`cargo test -p chronos-services --lib list_sessions_empty` with the m9-70
store-side absent-table handling reverted and the m9-71 workaround removed:

```
test sessions::tests::list_sessions_empty ... FAILED
panicked at crates/chronos-services/src/sessions.rs:465:65:
called `Result::unwrap()` on an `Err` value: ListFailed("Database error: Table 'sessions' does not exist")
test result: FAILED. 0 passed; 1 failed; 262 filtered out
```

Vacuity baseline — same store revert, workaround still present (pre-m9-71
`list_sessions`), identical test:

```
test sessions::tests::list_sessions_empty ... ok
test result: ok. 1 passed; 0 failed; 262 filtered out
```

The same test therefore proved nothing before this cycle: it passed whether or
not the store honoured the contract it was supposed to be checking.

## Drift Evidence (post-cycle state)

`list_sessions` has exactly one failure mode left:

```rust
let sessions = ctx
    .store
    .list_sessions()
    .map_err(|e| ServiceError::ListFailed(e.to_string()))?;
```

`grep -rn 'contains("does not exist")'` over `crates/` returns no matches; the
only string-matching error handler in the workspace is gone.

## Falsification of the new guards

No new test was added, so the falsification target is the strengthened one.
Reverting the m9-70 store-side handling (the only thing keeping
`list_sessions_empty` honest) makes it FAIL, as shown above; restoring it makes
it pass. The guard is real and its dependency direction is explicit: the
service now *depends* on the store contract instead of papering over it.

## Gates

| Gate | Command | Result |
|---|---|---|
| T0 fmt | `cargo fmt --all -- --check` | pass |
| T0 clippy | `cargo clippy --workspace --all-targets -- -D warnings` | pass (0 warnings) |
| T2 | `cargo test -p chronos-services -p chronos-store --lib --no-fail-fast` | pass (263 + 62; 0 failed) |
| T2 | `cargo test -p chronos-mcp --tests --no-fail-fast` | pass (77 lib + 49 integration; 0 failed) |
| T4-smoke | `cargo test -p chronos-sandbox --test session_persistence --test e2e_connectivity -- --test-threads=1` | pass (5 passed; 0 failed; 62s incl. build) |
| Drift | `./scripts/check_vault_drift.sh` | pass (47 python CCs clean, 7 bash CCs clean) |
| CC smoke | `./scripts/smoke_test_ccs.sh` | pass (5 run, 0 failures) |

T4-smoke is required because the touched function is reachable through the
`session_list` MCP tool, even though this diff is a services-layer
simplification with no behavioral delta.

## Cross-checks

- CC#1..CC#55: pass, no drift. Vault files changed by this cycle are
  `cycles/index.md` and `terms/index.md`, and CC#4 validates those SHAs in
  **every** archive-manifest that lists them, so both m9-02's and m9-70's
  artifact indexes were regenerated. The first post-release sweep failed CC#4
  on m9-70's stale rows, which is how that requirement was discovered; the
  resulting O(n^2) ritual is recorded as
  `FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION`.
- No new CC (47 python + 7 bash unchanged) — the smoke test's expected counts
  need no update.
- CC#12: `main_sha == head_sha == remote_tag_peel` recorded in the
  apply-checkpoint after release (`v0.7.73` peel verified on origin).
- `FIND-M9-70-SERVICES-TABLE-STRING-MATCH` appears exactly once in
  `terms/index.md`, in the Terminated terms table.

## Pre-existing observations (not caused by this cycle)

- `SessionStore::load_session` classifies the absent-table case with `Err(_) =>
  SessionNotFound` in both read paths, unlike its two siblings in the same file.
  Pre-existing; recorded as `FIND-M9-71-LOAD-SESSION-TABLE-ERROR-COLLAPSE`.
- The m9-70 archive manifest records the T4-smoke `session_persistence` count as
  8; the file contains 4 tests (`test_save_and_load_session_roundtrip`,
  `test_save_session_multiple_times`, `test_list_sessions_after_save`,
  `test_load_nonexistent_session`). The m9-70 figure appears to have been
  double-counted. Recorded here rather than rewriting a frozen archive.
- The sandbox warm-up-ordering flake candidate for this cycle slot
  (`test_session_start_via_v2_then_session_stop_via_v2`) did **not** reproduce:
  5/5 passes in isolation (~16-17s each), plus a run with an unopenable
  `CHRONOS_DB_PATH` simulating the in-memory fallback. No cycle spent on it; the
  observation may be stale or environment-specific.

## Notes

The change is deliberately one function wide. Widening it (for example also
normalizing `load_session`'s error classification) would have mixed a
behaviorally inert cleanup with a real policy decision about error taxonomy, so
that decision is deferred as its own finding.

## History

| Timestamp | Event |
|---|---|
| 2026-09-13T12:30Z | Recon: single `contains("does not exist")` site in `crates/`; `list_sessions_empty` identified as vacuous by inspection of the pre-m9-70 path |
| 2026-09-13T12:30Z | Workaround removed; store-side revert reproduces the expected FAILED |
| 2026-09-13T12:30Z | Vacuity proved: pre-m9-71 + store revert = PASS |
| 2026-09-13T12:31Z | T0 pass; T2 pass (263 / 62 / 77 + 49); T4-smoke 5/5 |
| 2026-09-13T12:35Z | Drift sweep pass; CC smoke 5/5 |
