# Archive Manifest — m9-70-mcp-store-isolation

## Summary

m9-70 closes `FIND-M9-69-MCP-STORE-ISOLATION`, the deferral recorded by m9-69. `ChronosServer::new()` opened the developer's real `$HOME/.local/share/chronos/sessions.redb` even under `cfg(test)`, and `SessionStore::list_sessions` hard-failed on the first record it could not deserialize, so `chronos-mcp`'s `server::tests::test_list_sessions_after_save` failed deterministically on a populated machine and passed on a clean one. Three fixes landed, each independently falsified: best-effort listing (skip unreadable records with a warning), absent-table-is-empty reads (which also fixes a real fresh-install bug where `session_list` errored on a brand-new database), and a hermetic `cfg(test)` store so the unit suite never reads or writes the developer's database. A third defect surfaced by the cycle's own new test (`FIND-M9-70-VIRGIN-STORE-READ-ERROR`) was closed in the same cycle. Two B-direct commits landed as `54e859f` on `feat/m9-70-mcp-store-isolation`. Tag `v0.7.72`.

## Cycle

| Campo | Valor |
|---|---|
| Cycle | m9-70-mcp-store-isolation |
| Base SHA | `682817b520010dfa15512fb461a9eeb679009708` |
| Head SHA | `54e859ffe43c09cc2217c8943233f27016bbe76b` |
| Path | B-direct |
| Date | 2026-09-13T12:30Z |
| Branch | `feat/m9-70-mcp-store-isolation` |
| Tag | `v0.7.72` |
| Tag peel SHA | `54e859ffe43c09cc2217c8943233f27016bbe76b` |
| Peel match | `54e859ffe43c09cc2217c8943233f27016bbe76b` |
| Status | CLOSED |

## Evidence bindings

- **`apply-checkpoint.json`**: `status: CLOSED`, `verify_status: passed`, `release_status: released`, `archive_status: archived`, `findings_closed: [FIND-M9-69-MCP-STORE-ISOLATION, FIND-M9-70-VIRGIN-STORE-READ-ERROR]`
- **`verify-findings.json`**: 2 findings closed (medium `test.store_env_coupling`, low `code.read_error_on_absent_table`), 1 deferred (low `code.error_text_matching`), verdict `passed`
- **`verify-report.md`**: Subject, Files Inventory, Drift Evidence (pre/post-cycle), Falsification of the new guards, Gates, Cross-checks, Notes, History
- **`merge-receipt.md`**: `Base SHA | 682817b…`, `Head SHA | 071b820`
- **`release-receipt.md`**: `Remote tag | v0.7.72`, `Peel match | 54e859f…` (true)

## Falsification evidence

Every new guard was reverted and its test observed to fail:

| Guard reverted | Observed result |
|---|---|
| `list_sessions` best-effort skip | `test_session_store_list_sessions_skips_unreadable_record` FAILED |
| absent-table handling (`list_sessions`) | `test_list_sessions_on_virgin_store_is_empty` FAILED |
| absent-table handling (`session_exists`) | `test_session_exists_on_virgin_store_is_false` FAILED |
| pre-fix `cfg(test)` store (reads `$HOME`) | FAILED: `fresh test server must start with an empty store, found 21 session(s): the test store is reading the developer's $HOME database` |

The first draft of the hermeticity test passed *before* the fix (a two-server comparison is masked by redb's exclusive lock, which forces the second server to an in-memory fallback); it was rewritten to assert the observable bug and then failed pre-fix as shown above.

## Tangential modifications

2 files changed across two commits (+183/−27 in the logic commit; 6 new artifact files):

| File | Net change |
|---|---|
| `crates/chronos-store/src/storage.rs` | +83, −10 (best-effort listing, absent-table handling in two read paths, 3 tests) |
| `crates/chronos-mcp/src/server.rs` | +100, −17 (`from_store` split, `cfg`-gated store impls, `PathBuf` import removed, 1 test) |
| `cycle-artifacts/…/m9-70-mcp-store-isolation/` | 6 new files (apply-checkpoint, verify-report, verify-findings, release-report, merge-receipt, release-receipt) |

Two commits:
- `071b820` — `m9-70: isolate chronos-mcp test store + make session listing resilient`
- `54e859f` — `feat(m9-70): cycle artifacts (apply-checkpoint, verify, release-report)` (tag `v0.7.72`)

## Cross-checks

- CC#1..CC#55: pass (no drift). Only vault files changed by this cycle are `cycles/index.md` and `terms/index.md`; m9-02's archive-manifest artifact-index SHAs are regenerated.
- No new CC: 47 python + 7 bash CCs unchanged, so `scripts/smoke_test_ccs.sh` expected counts need no update (5 tests run / 0 failures).
- T0: `cargo fmt --all -- --check` + `cargo clippy -p chronos-store -p chronos-mcp --all-targets -- -D warnings` pass (0 warnings)
- T2: `cargo test -p chronos-store -p chronos-mcp --tests` → store 62, mcp lib 77, 49 across 6 integration binaries; `cargo test -p chronos-services --lib` → 263 passed
- T4-smoke: `e2e_connectivity` 1 passed (5.55s) + `session_persistence` 8 passed (56.99s) + `session_lifecycle` 4 passed (54.90s) = 13 passed / 0 failed

## Follow-ups (deferred)

- **FIND-M9-70-SERVICES-TABLE-STRING-MATCH** (low): `chronos-services::sessions::list_sessions` still tolerates a missing table by substring-matching the error text; unreachable after the chronos-store fix. Decide delete vs typed match.
- **5+19 not-merged branches triage**: preserved from m9-65 (human review needed).
- **Sandbox test warm-up ordering**: `test_session_start_via_v2_then_session_stop_via_v2` fails alone, passes with the full file.

## Artifact index

| Kind | Path | SHA-256 |
|---|---|---|
| archive-manifest (this file) | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-70-mcp-store-isolation/archive-manifest.md` | `0000000000000000000000000000000000000000000000000000000000000000` |
| source (session store) | `crates/chronos-store/src/storage.rs` | `7971d253211974da6e3f8c268b3c399684493550569f31d50888a26a66673427` |
| source (mcp server) | `crates/chronos-mcp/src/server.rs` | `c15aa346088f42bed26256a6f8ee12c33873320ae39ae68abc7902f7ab8a045b` |
| apply-checkpoint | `cycle-artifacts/p-3416cfb8288f8964/m9-70-mcp-store-isolation/apply-checkpoint.json` | `619c7367dc623d47ff703fade38c0d042e02b776c8fb0c46f378d4bf3226ab0d` |
| verify-report | `cycle-artifacts/p-3416cfb8288f8964/m9-70-mcp-store-isolation/verify-report.md` | `ded260435052b8cb7a3737a0462fa0e9498a97f0990d66324a428e7ca1a57403` |
| verify-findings | `cycle-artifacts/p-3416cfb8288f8964/m9-70-mcp-store-isolation/verify-findings.json` | `8ea37a0a336953dd7cec411e85b13f60d4cb06b6ce10ca7b77abf3ea0f2b9ef4` |
| release-report | `cycle-artifacts/p-3416cfb8288f8964/m9-70-mcp-store-isolation/release-report.md` | `90ce13f216e0304ed976e58fa1b033bbc54822b0f47835bd7d1e0306fba5e79a` |
| release-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-70-mcp-store-isolation/release-receipt.md` | `aad24fa2ab687befb3a2acb52018b395de6935b259f4bdb392c821089a2799a4` |
| merge-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-70-mcp-store-isolation/merge-receipt.md` | `3df919a052069e0829ac84668c6ac95e5d3219fe83186ea02149820b2c0a494b` |
| vault index (cycles) | `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` | `427f1921b0002e84c5c436ad1e19451ea46ab262680e388669f9bb865e3d0099` |
| vault index (terms) | `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` | `8d262955b5079b561d8c5097dc14c48284031aeb746a152ad12575287967ac2c` |
