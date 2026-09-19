# Archive Manifest — m9-72-read-path-table-error-classification

## Summary

m9-72 closes `FIND-M9-71-LOAD-SESSION-TABLE-ERROR-COLLAPSE`, the deferral recorded by m9-71 — after recon showed the deferral understated it twice. The real count is four read paths in `chronos-store`, not two (`ContentStore::get` → `Ok(None)`, `contains` → `Ok(false)`, and both `SessionStore::load_session` reads → `SessionNotFound`), and the collapse has an observable cost rather than being cosmetic: `load_session` consumes `cas::get` in a loop, so a fault answered with `Ok(None)` silently deleted events from the loaded session and still reported success. Only redb's `TableError::TableDoesNotExist` is benign; every other variant, including `TableTypeMismatch` from a store written by a different binary, is a storage fault. A new `table_error` module owns the distinction and all four sites route through it, which also brings `storage.rs` and `cas.rs` onto the policy `counterexample_storage.rs` already followed. One B-direct source commit plus one artifacts commit landed on `feat/m9-72-read-path-table-error-classification`. Tag `v0.7.74`.

## Cycle

| Campo | Valor |
|---|---|
| Cycle | m9-72-read-path-table-error-classification |
| Base SHA | `09e21107ca0707c0cd40d631eddcdb39cb3669ba` |
| Head SHA | `f3500a9071d528fd6256c5f65ef00c0ca300b903` |
| Path | B-direct |
| Date | 2026-09-13T15:20Z |
| Branch | `feat/m9-72-read-path-table-error-classification` |
| Tag | `v0.7.74` |
| Tag peel SHA | `f3500a9071d528fd6256c5f65ef00c0ca300b903` |
| Peel match | `f3500a9071d528fd6256c5f65ef00c0ca300b903` |
| Status | CLOSED |

## Evidence bindings

- **`apply-checkpoint.json`**: `status: CLOSED`, `verify_status: passed`, `release_status: released`, `archive_status: archived`, `findings_closed: [FIND-M9-71-LOAD-SESSION-TABLE-ERROR-COLLAPSE]`
- **`verify-findings.json`**: 1 finding closed (low `code.error_classification_collapse`), 2 deferred (medium `test.shared_database_state`, low `code.duplicated_policy`), verdict `passed`
- **`verify-report.md`**: Summary, Subject, Files Inventory, Falsification evidence, Gates, T4-smoke, Residual risk
- **`merge-receipt.md`**: `Base SHA | 09e2110…`, `Head SHA | 845be8d`
- **`release-receipt.md`**: `Remote tag | v0.7.74`, `Peel match | true`

## Falsification evidence

Seven new tests; the fault in each is genuine rather than simulated, because the table is created with an incompatible `TableDefinition` signature so **redb itself** returns `TableTypeMismatch` on the later read-path `open_table`. `TableDoesNotExist` cannot be manufactured for a table that was created, which is precisely why the old `Err(_)` arms were invisible to the suite. Each of the four call sites was reverted individually, its test observed to fail, then restored — 4/4:

| Configuration | Observed result |
|---|---|
| `cas::get` reverted to `Err(_) => Ok(None)` | **FAILED** — `test_get_propagates_storage_fault_instead_of_reporting_missing_content` sees `Ok(None)` for a fault |
| `cas::contains` reverted to `Err(_) => Ok(false)` | **FAILED** — same test, `contains` assertion |
| `load_session` SESSION_META reverted to `SessionNotFound` | **FAILED** — `test_load_session_propagates_storage_fault_instead_of_session_not_found` |
| `load_session` SESSION_EVENTS reverted to `SessionNotFound` | **FAILED** — `test_load_session_propagates_event_table_storage_fault` |

The EVENTS test is the subtle one: reaching the second `open_table` requires a database whose `sessions` table opens normally and holds a row while `session_events` is broken, so the metadata row is written with the correct signature and only the event table is injected with a foreign one.

## T4-smoke flake (characterised, attribution tested)

The required subset (`session_persistence` + `counterexample_tools` + `e2e_connectivity`) is unstable in this environment: `session_persistence::test_save_session_multiple_times` fails with `save_session failed: TimeoutError("method=tools/call", 30s)` — a fixed client-side RPC timeout applied to a write that serializes ~2500 compressed events into `$HOME/.local/share/chronos/sessions.redb`, one 86 MB store shared by all 35 sandbox suites because `McpTestClient::start()` inherits the caller's environment. Observed failure line varies (128 = first save, 133 = second save), so the trigger is duration, not data.

Attribution was measured, not argued:

1. **Config-controlled**: three consecutive runs of `session_persistence` alone against the *same* binary gave exit `0` (60s) / `101` (76s, timeout) / `0` (80s) — one binary, both outcomes.
2. **Interleaved A/B against `main`** (four runs of the three-binary command, alternating builds so the shared store grows symmetrically for both configurations): cycle branch `101`, `main` `0`, cycle branch `0`, `main` `0`.
3. **Reproduced on `main`**: three consecutive runs of `session_persistence` alone built from `main` (`09e2110`, no m9-72 code) gave `0` (59s) / `0` (64s) / `101` (80s) with the identical signature — `session_persistence.rs:133`, `save_session failed: TimeoutError("method=tools/call", 30s)`. The flake therefore exists without this cycle's code at all.
4. **Code review**: the diff changes four read-path classifications only; `session_save` is a write path (`cas.put` / `ensure_table` in a write transaction) and `SessionStore::open`, where a database-level error would surface, is untouched.
5. **Byte-identical scope**: `git diff --name-only main..HEAD` lists only `crates/chronos-store`; the harness and the failing test are unchanged by this cycle.

No leaked `chronos-mcp` process was observed after any run of these experiments (`pgrep -fc 'cargo-targets/debug/chronos-mcp'` = 0 before and after each), so the mechanism is load-sensitive duration against a shared store, not a held lock. Recorded as `FIND-M9-72-SANDBOX-SHARED-STORE-SAVE-TIMEOUT` (medium).

## Tangential modifications

4 source files (1 new) + 1 doc + 6 artifacts, +103/−4 in the logic commit (AGENTS.md is its own commit):

| File | Net change |
|---|---|
| `crates/chronos-store/src/table_error.rs` | new, ~140 lines (classifier + 4 unit tests) |
| `crates/chronos-store/src/lib.rs` | +1 (`mod table_error;`) |
| `crates/chronos-store/src/cas.rs` | 2 sites routed through the classifier + 1 behavioural test |
| `crates/chronos-store/src/storage.rs` | 2 sites + 2 behavioural tests (META, EVENTS) |
| `AGENTS.md` | T3 loses `chronos-e2e` (bucket D, hangs on ptrace); §6.5 records the hang |
| `cycle-artifacts/…/m9-72-read-path-table-error-classification/` | 6 new files |

Three commits:
- `845be8d` — `fix(store): propagate read-path storage faults instead of swallowing them (m9-72)`
- `6ee95ea` — `docs(agents): exclude chronos-e2e from the T3 command (it hangs on ptrace)`
- `f3500a9` — `feat(m9-72): cycle artifacts (apply-checkpoint, verify, release-report)` (tag `v0.7.74`)

## Cross-checks

- CC#1..CC#55: pass (no drift). Vault files changed by this cycle are `cycles/index.md` and `terms/index.md`; their artifact-index rows are regenerated in **every** archive-manifest that lists them (m9-02's, m9-70's and m9-71's — the set grew to three this cycle, which is `FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION` doing what it promised).
- No new CC: 47 python + 7 bash unchanged, so `scripts/smoke_test_ccs.sh` expected counts need no update (5 tests run / 0 failures).
- T0: `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings` pass (0 warnings)
- T2: `cargo test -p chronos-store --lib` → 69 (62 → 69); `-p chronos-services --lib` → 263; `-p chronos-mcp --tests` → 77 lib + 49 integration
- T3: `cargo test --workspace --lib --tests --exclude chronos-sandbox --exclude chronos-e2e --exclude chronos-native --no-fail-fast` passes, plus `-p chronos-native --lib -- --test-threads=1` (that crate hangs in parallel; pre-existing and documented). The `chronos-e2e` exclusion is this cycle's AGENTS.md fix: the old documented T3 command pulled in bucket D and hung.
- T4-smoke: flaky subset, characterised above; `session_persistence` 4/4 and `counterexample_tools` 12/12 and `e2e_connectivity` 1/1 were each observed green with this build
- CC#12: `main_sha == head_sha == remote_tag_peel`

## Follow-ups (deferred)

- **FIND-M9-72-SANDBOX-SHARED-STORE-SAVE-TIMEOUT** (medium): give each sandbox client a unique temporary `CHRONOS_DB_PATH`; `start_with_db_path` already exists and is used only by `ce12`. Also consider widening the 30s client timeout for `save_session`, or shrinking the fixture's event volume.
- **FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION** (low): four hand-rolled copies of the `table_error` policy remain in `counterexample_storage.rs`.
- **FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION** (low): unchanged; the affected set is now three manifests.
- **Sandbox warm-up ordering**: not reproducible this cycle (or the previous one).
- **5+19 not-merged branches triage**: preserved from m9-65 (human review needed).

## Artifact index

| Kind | Path | SHA-256 |
|---|---|---|
| archive-manifest (this file) | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-72-read-path-table-error-classification/archive-manifest.md` | `0000000000000000000000000000000000000000000000000000000000000000` |
| source (store table_error) | `crates/chronos-store/src/table_error.rs` | `e0ce9280d864cb660020d0fa5ad9ed7adb465ffefd50319965142076be5f1d0a` |
| source (store lib) | `crates/chronos-store/src/lib.rs` | `093934f02101a7f6404fcd3cebe7fce972861731a6657becaa8bba5eb6f687fa` |
| source (store cas) | `crates/chronos-store/src/cas.rs` | `a0f32a00c732bde1cd2e2be7744cb25be1c2e583aaa987593e7c80b31510660a` |
| source (store storage) | `crates/chronos-store/src/storage.rs` | `6c42c78f4ea8e5b73bb3d7ffd5a228396a12a1e6052c8fc6b8354c816469a113` |
| docs (agents manual) | `AGENTS.md` | `83d09aa421c0c0c06a8dc28d4a5ba27dbb66772f7a1310103f3c133969fe8924` |
| apply-checkpoint | `cycle-artifacts/p-3416cfb8288f8964/m9-72-read-path-table-error-classification/apply-checkpoint.json` | `0475383cd18fbccf83e647f29bf17ddfaeefab66300c489c96147a761f8daa28` |
| verify-report | `cycle-artifacts/p-3416cfb8288f8964/m9-72-read-path-table-error-classification/verify-report.md` | `e2f4694b3b6105b75b1d72b1af71db43e12cf8ab4ba1349b41f889319eec637b` |
| verify-findings | `cycle-artifacts/p-3416cfb8288f8964/m9-72-read-path-table-error-classification/verify-findings.json` | `4258d8ec331d27a0a2b062910908e2c0fe9e186d9019ea9b8dddea00c4fd226d` |
| release-report | `cycle-artifacts/p-3416cfb8288f8964/m9-72-read-path-table-error-classification/release-report.md` | `bc34b242176d6fd041a4105eff75a7e51c0eea92ccf50983240942f4c535303f` |
| release-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-72-read-path-table-error-classification/release-receipt.md` | `f00addd97fb963a2df18e88ae20fc5dec28f12361daf4a5db7e72de961a87d36` |
| merge-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-72-read-path-table-error-classification/merge-receipt.md` | `310fdbfa1108ecd039d0241eacef0804fab7d805ac50de984f8868d4a0f0531a` |
| change-entry | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-72-read-path-table-error-classification/change-entry.md` | `bd9b4be16a0db039dbfda4c28c0a6a434858c14512a93d94da6ade8c09301262` |
| vault index (cycles) | `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` | `2abe826f997082a6423ebc4f487782b92630a82ab695d9a3e698868e82313e4e` |
| vault index (terms) | `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` | `03221130e4cfca65f7b145f2907767c3c01a4e27a966af20dfec38249774de8d` |
