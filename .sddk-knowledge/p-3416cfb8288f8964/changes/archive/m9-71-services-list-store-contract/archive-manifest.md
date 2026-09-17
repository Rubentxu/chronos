# Archive Manifest — m9-71-services-list-store-contract

## Summary

m9-71 closes `FIND-M9-70-SERVICES-TABLE-STRING-MATCH`, the deferral recorded by m9-70. `SessionsService::list_sessions` had tolerated a missing `sessions` table by substring-matching the store's rendered error text; m9-70 moved that decision into `chronos-store` (a virgin database now returns `Ok(vec![])`), leaving the workaround unreachable dead code that could still swallow a genuine store error whose message contained "not exist". The service now propagates the store error verbatim and the empty-store case is documented as a store contract. There is no observable behavior change after m9-70: the cycle's actual product is that `list_sessions_empty` stops being vacuous, proved by running it in both directions before the patch landed. One B-direct source commit plus one artifacts commit landed as `f8abe7b` on `feat/m9-71-services-list-store-contract`. Tag `v0.7.73`.

## Cycle

| Campo | Valor |
|---|---|
| Cycle | m9-71-services-list-store-contract |
| Base SHA | `e9b6277170cb8059f5cd0340a3f66a5144558faf` |
| Head SHA | `f8abe7b91b70a7efb2633248d2e8aaea08b3c98a` |
| Path | B-direct |
| Date | 2026-09-13T12:45Z |
| Branch | `feat/m9-71-services-list-store-contract` |
| Tag | `v0.7.73` |
| Tag peel SHA | `f8abe7b91b70a7efb2633248d2e8aaea08b3c98a` |
| Peel match | `f8abe7b91b70a7efb2633248d2e8aaea08b3c98a` |
| Status | CLOSED |

## Evidence bindings

- **`apply-checkpoint.json`**: `status: CLOSED`, `verify_status: passed`, `release_status: released`, `archive_status: archived`, `findings_closed: [FIND-M9-70-SERVICES-TABLE-STRING-MATCH]`
- **`verify-findings.json`**: 1 finding closed (low `code.error_text_matching`), 1 deferred (low `code.error_classification_collapse`), verdict `passed`
- **`verify-report.md`**: Subject, Files Inventory, Drift Evidence (pre/post-cycle), Falsification, Gates, Cross-checks, Pre-existing observations, Notes, History
- **`merge-receipt.md`**: `Base SHA | e9b6277…`, `Head SHA | 8bbd08c`
- **`release-receipt.md`**: `Remote tag | v0.7.73`, `Peel match | f8abe7b…` (true)

## Falsification evidence

This cycle adds no test; the strengthened `list_sessions_empty` was run in both directions before the patch was committed:

| Configuration | Observed result |
|---|---|
| workaround removed (m9-71) + m9-70 store fix reverted | **FAILED** — `ListFailed("Database error: Table 'sessions' does not exist")` at `sessions.rs:465` |
| workaround present (pre-m9-71) + m9-70 store fix reverted | **PASSED** — 1 passed; 0 failed |

The second row is the substantive claim of this cycle: before m9-71 the test could not distinguish a store honouring the empty-store contract from one that does not, so it provided no coverage of the behaviour it appears to cover.

## Tangential modifications

1 source file + 6 artifacts, +22/−15 in the logic commit:

| File | Net change |
|---|---|
| `crates/chronos-services/src/sessions.rs` | +22, −15 (branch deleted; contract documented at the call site; guard documented) |
| `cycle-artifacts/…/m9-71-services-list-store-contract/` | 6 new files (apply-checkpoint, verify-report, verify-findings, release-report, merge-receipt, release-receipt) |

Two commits:
- `8bbd08c` — `m9-71: drop services-side error-text matching for the absent sessions table`
- `f8abe7b` — `feat(m9-71): cycle artifacts (apply-checkpoint, verify, release-report)` (tag `v0.7.73`)

## Cross-checks

- CC#1..CC#55: pass (no drift). Only vault files changed by this cycle are `cycles/index.md` and `terms/index.md`; the artifact-index rows for those two paths are regenerated in **every** archive-manifest that lists them (m9-02's and m9-70's), which is what CC#4 checks. This O(n^2) ritual is recorded as `FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION`.
- No new CC: 47 python + 7 bash CCs unchanged, so `scripts/smoke_test_ccs.sh` expected counts need no update (5 tests run / 0 failures).
- T0: `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings` pass (0 warnings)
- T2: `cargo test -p chronos-services -p chronos-store --lib` → 263 + 62; `cargo test -p chronos-mcp --tests` → 77 lib + 49 integration
- T4-smoke: `session_persistence` 4 passed (55.61s) + `e2e_connectivity` 1 passed (6.24s) = 5 passed / 0 failed
- CC#12: `main_sha == head_sha == remote_tag_peel == f8abe7b91b70a7efb2633248d2e8aaea08b3c98a`

## Follow-ups (deferred)

- **FIND-M9-71-LOAD-SESSION-TABLE-ERROR-COLLAPSE** (low): `SessionStore::load_session` collapses every `open_table` failure into `SessionNotFound` via `Err(_)`, unlike its two siblings in the same file.
- **Sandbox test warm-up ordering**: not reproducible this cycle (5/5 in isolation, plus an unopenable-`CHRONOS_DB_PATH` run). Preserved for re-characterization if it reappears.
- **5+19 not-merged branches triage**: preserved from m9-65 (human review needed).
- **m9-70 archive manifest T4-smoke count**: its `session_persistence` figure (8) exceeds the file's 4 tests; noted and left frozen.

## Artifact index


| Kind | Path | SHA-256 |
|---|---|---|
| archive-manifest (this file) | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-71-services-list-store-contract/archive-manifest.md` | `0000000000000000000000000000000000000000000000000000000000000000` |
| source (services sessions) | `crates/chronos-services/src/sessions.rs` | `185d6df37e6a7db21ef9945d0beea43b8ea2ab43c08279ab87e1d8defa9aabd5` |
| apply-checkpoint | `cycle-artifacts/p-3416cfb8288f8964/m9-71-services-list-store-contract/apply-checkpoint.json` | `cc5bb8dddb0d4ab50013f024f2267816c5293e4c230cf1b804708077e46b838b` |
| verify-report | `cycle-artifacts/p-3416cfb8288f8964/m9-71-services-list-store-contract/verify-report.md` | `7c71a52040000a94cc8377134ce861c54eb5abdfeacf499a8b73e651a27cfb7b` |
| verify-findings | `cycle-artifacts/p-3416cfb8288f8964/m9-71-services-list-store-contract/verify-findings.json` | `f4dff9ccff6892cf56f16951a2f6ca083f378ff1716c55f2f2e36657544b57d3` |
| release-report | `cycle-artifacts/p-3416cfb8288f8964/m9-71-services-list-store-contract/release-report.md` | `b532010f348b5552b2b4d57f3104e152ecbd7158c5e88f76ff9bf4185add7fd6` |
| release-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-71-services-list-store-contract/release-receipt.md` | `5328e9ef4a76b1f48de59833052b2a65c9c6653c34aa1aa7f5188c85b7b97227` |
| merge-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-71-services-list-store-contract/merge-receipt.md` | `9e92a663e685bed2f31cf8b64a3ae3b8ad53107164702ac60209121a738c3717` |
| vault index (cycles) | `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` | `75b99dbffbed9bb4eddbc9213cd748ad6994a3c84ddcbeacc47cce749d99df99` |
| vault index (terms) | `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` | `03221130e4cfca65f7b145f2907767c3c01a4e27a966af20dfec38249774de8d` |
