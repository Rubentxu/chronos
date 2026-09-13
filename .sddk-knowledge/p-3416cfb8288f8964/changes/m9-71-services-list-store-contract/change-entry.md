# Change: m9-71 services list store contract

## Summary

Closes `FIND-M9-70-SERVICES-TABLE-STRING-MATCH`, the deferral recorded by m9-70. `SessionsService::list_sessions` tolerated a missing `sessions` table by substring-matching the store's rendered error text (`err_str.contains("does not exist") || err_str.contains("not exist")`) and returning an empty listing on a match. After m9-70 taught `chronos-store` to answer a virgin database with `Ok(vec![])`, that branch was unreachable for its only intended case while still able to swallow a genuine store failure whose message happened to contain those words. The service now propagates the store error verbatim and documents the empty-store case as a store contract. There is no observable behavior change post-m9-70; the cycle's value is that `list_sessions_empty` stops being a vacuous test. It passed both with and without the store fix before, and now fails the instant the store regresses, which was demonstrated in both directions before the patch landed.

## Ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-71-services-list-store-contract` |
| Path | B-direct |
| Status | CLOSED |
| Base SHA | `e9b6277170cb8059f5cd0340a3f66a5144558faf` |
| Head SHA | `f8abe7b91b70a7efb2633248d2e8aaea08b3c98a` |
| Tag | `v0.7.73` |

## Subject

- base_sha: `e9b6277170cb8059f5cd0340a3f66a5144558faf`
- head_sha: `f8abe7b91b70a7efb2633248d2e8aaea08b3c98a`
- cycle: m9-71
- branch: `feat/m9-71-services-list-store-contract`
- date: 2026-09-13
- tag: `v0.7.73`
- findings_closed: 1 (FIND-M9-70-SERVICES-TABLE-STRING-MATCH)
- findings_introduced (deferred): 1 (FIND-M9-71-LOAD-SESSION-TABLE-ERROR-COLLAPSE)
- new tests: 0 (1 existing test strengthened: `list_sessions_empty`)

## Files changed

- (modified) `crates/chronos-services/src/sessions.rs` — `list_sessions` drops the `contains("does not exist")` branch and propagates the store error via `?`; doc comment rewritten to state the store's contract; `list_sessions_empty` documented as the load-bearing guard with its falsification procedure (+22/−15)
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-71-services-list-store-contract/apply-checkpoint.json`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-71-services-list-store-contract/verify-findings.json`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-71-services-list-store-contract/verify-report.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-71-services-list-store-contract/release-report.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-71-services-list-store-contract/merge-receipt.md`
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-71-services-list-store-contract/release-receipt.md`
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-71-services-list-store-contract/change-entry.md` (this file)
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-71-services-list-store-contract/archive-manifest.md`
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` (m9-71 row added, Total cycles 70→71)
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` (FIND-M9-70-SERVICES-TABLE-STRING-MATCH terminated; FIND-M9-71-LOAD-SESSION-TABLE-ERROR-COLLAPSE deferred; Last archive bumped)
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-02-events-side-table/archive-manifest.md` (cycles/index.md + terms/index.md SHAs regenerated)

## Cross-checks

CC#1..CC#55: pass (no drift; no new CC — 47 python + 7 bash unchanged, so the smoke test's expected counts need no update). T0 fmt + clippy (`-D warnings`) pass workspace-wide. T2: chronos-services 263, chronos-store 62, chronos-mcp 77 lib + 49 integration. T4-smoke: `session_persistence` 4 + `e2e_connectivity` 1 = 5 passed / 0 failed (required because `list_sessions` is reachable through the `session_list` tool). CC smoke 5/5 pass. CC#12: `main_sha == head_sha == remote_tag_peel == f8abe7b` (peel confirmed on origin).

## Falsification evidence

The cycle adds no test, so the falsification target is the strengthened one, and it was run in both directions before committing:

| Configuration | `list_sessions_empty` |
|---|---|
| workaround removed + m9-70 store fix reverted | **FAILED** — `ListFailed("Database error: Table 'sessions' does not exist")` at `sessions.rs:465` |
| workaround present (pre-m9-71) + same revert | **PASSED** — 1 passed; 0 failed |

The second row is the point: before this cycle the test could not distinguish a store that honours the empty-store contract from one that does not.

## Follow-ups (deferred)

- **FIND-M9-71-LOAD-SESSION-TABLE-ERROR-COLLAPSE** (low): `SessionStore::load_session` uses `Err(_) => SessionNotFound` in both read paths while its siblings match `redb::TableError::TableDoesNotExist` explicitly. Harmless today, but three read paths in one file disagree about the same concern.
- **Sandbox warm-up ordering** (preserved): `test_session_start_via_v2_then_session_stop_via_v2` was reported to fail alone and pass with the full file. Not reproducible this cycle: 5/5 passes in isolation, plus a run with an unopenable `CHRONOS_DB_PATH` simulating the in-memory fallback. Re-characterize only if it reappears.
- **5+19 not-merged branches triage** (preserved from m9-65): human review needed.
- **m9-70 archive manifest count**: its T4-smoke `session_persistence` figure (8) does not match the file's 4 tests; noted, frozen archive left as-is.
