# Archive Manifest — m9-74-cas-put-many-batching

## Summary

m9-74 closes the high finding m9-73 deferred with its mechanism already measured,
the symptom row that came with it, and one more medium finding that the first fix
uncovered. `ContentStore::put` opened a redb write transaction, inserted one row
and committed, and redb's default immediate durability makes every `commit()` a
`sync_data` barrier — `set_durability` appears nowhere in `chronos-store` — while
`SessionStore::save_session` called it in a loop, so a save cost one fsync per
event (~4.8 ms/event, ≈168 s for the 34,905-event crash capture in
`session_edge_cases`, against the sandbox client's 30 s `tools/call` timeout).
The fix splits the pure `encode` step from the write so a batch is fully encoded
before a transaction opens (an encoding failure stores nothing), inserts a batch
in one write transaction with the per-event dedup check preserved, and exposes
`put_many` for `save_session`; `put` is now `encode` + `insert_batch` and keeps its
signature, semantics and hash. The evidence is a counted invariant rather than a
timing: a new `#[cfg(test)]` `CountingBackend` wraps redb's real `FileBackend`
(file-backed on purpose — an in-memory redb never syncs) and counts `sync_data`
calls, a 500-event batch must stay under four barriers against a 50-`put` control
loop that must report at least 50, and a 1,000-event `save_session` must stay
under four barriers and still round-trip. With that in place and the client
timeout untouched, `session_edge_cases` went from 4 passed / 2 failed to 6 passed
/ 0 failed in 65 s, so both `AGENTS.md` §6.5 rows were removed rather than
reworded. The third finding was found exactly there: `compare_sessions` and
`performance_regression_audit` had been returning the tagged `session_compare`
envelope since m7-03 (`947e73b`) instead of the flat v1 result their own names,
parameters and descriptions promise, and nothing pinned it because the four
`chronos-mcp` tests assert on `Debug` text the nested shape also satisfies while
the sandbox client was blocked one step earlier by the save timeout. Three
commits on `feat/m9-74-cas-put-many-batching`. Tag `v0.7.76`.

## Cycle

| Campo | Valor |
|---|---|
| Cycle | m9-74-cas-put-many-batching |
| Base SHA | `6317af899440178d30dc1d45807aed1851d0bb83` |
| Head SHA | `c2c0d3738d8295721eff30a12e7601af4010072f` |
| Path | B-direct |
| Date | `2026-09-13T16:51Z` |
| Branch | `feat/m9-74-cas-put-many-batching` |
| Tag | `v0.7.76` |
| Tag peel SHA | `c2c0d3738d8295721eff30a12e7601af4010072f` |
| Peel match | `c2c0d3738d8295721eff30a12e7601af4010072f` |
| Status | CLOSED |

## Evidence bindings

- **`apply-checkpoint.json`**: `status: CLOSED`, `verify_status: passed`, `release_status: released`, `archive_status: archived`, `findings_closed: [FIND-M9-73-CAS-PUT-ONE-WRITE-TRANSACTION-PER-EVENT, FIND-M9-73-SESSION-EDGE-CASES-HEAVY-SAVE-NEVER-COMPLETES, FIND-M9-74-V1-SHIMS-RETURN-V2-ENVELOPE]`
- **`verify-findings.json`**: 3 findings closed (high `perf.transaction_per_item`, medium `test.environmental_failure`, medium `api.deprecated_shim_shape_drift`), 5 deferred (low `test.environmental_failure` new, medium `code.silent_degradation`, low `process.undocumented_tooling`, low `code.duplicated_policy`, low `process.growing_regeneration_set`), verdict `passed`
- **`verify-report.md`**: Summary, Subject, Closed findings, Files inventory, Falsification evidence, Gates, Residual risk, Cross-checks
- **`merge-receipt.md`**: `Base SHA | 6317af8…`, `Head SHA | c2c0d3738d8295721eff30a12e7601af4010072f`
- **`release-receipt.md`**: `Remote tag | v0.7.76`, `Peel match | true`

## Falsification evidence

Two independent reverts, each applied, observed failing, and restored (both
restored files byte-compared with their backups, then the affected suite re-run
green):

| # | Reverted | Observed result |
|---|---|---|
| 1 | `put_many` back to the per-event `put` loop | `test_put_many_commits_once_per_batch` **FAILED** — the 500-event batch issued one barrier per event, while the 50-`put` control loop correctly reported ≥50 |
| 2 | Both v1 shims back to `SessionCompareWire::V2Envelope` | `test_v1_shims_return_flat_result_while_v2_returns_envelope` **FAILED** at `assert_eq!(v["session_a_id"], json!(sid_a))` — `null` where the v1 contract has a string, i.e. the `missing field 'session_a_id'` the sandbox client reported |

A third observation is not a revert but the reason the cycle has three findings:
running the acceptance suite with the CAS fix alone in place showed
`session_edge_cases` passing the save and failing both comparison tests. Before
that, at the start of the cycle, the same two tests were pinned as environmental
because they failed on `main` too, with `TimeoutError("method=tools/call", 30s)`
after a 186 s run.

## Tangential modifications

| File | Net change |
|---|---|
| `crates/chronos-store/src/cas.rs` | `encode`, `insert_batch`, `put_many`; `put` refactored onto them; four barrier tests |
| `crates/chronos-store/src/storage.rs` | `save_session` goes through `put_many`; the 1,000-event barrier test |
| `crates/chronos-store/src/lib.rs` | registers `test_support` |
| `crates/chronos-store/src/test_support.rs` | new, 70 lines, `CountingBackend` + `counting_file_db` |
| `crates/chronos-mcp/src/server.rs` | `SessionCompareWire`, the dispatcher parameter, both shims `V1Flat`, both descriptions, the new shape test |
| `AGENTS.md` | §6.5 `chronos-native` row rewritten with the measured parallel behaviour; the two `session_edge_cases` rows removed with a note; the T3 two-halves recipe recorded |
| `m9-02` / `m9-67` / `m9-68` / `m9-70` / `m9-71` / `m9-72` archive manifests | index SHA rows regenerated to a fixpoint |

Three commits:
- `bc3b912` — `perf(store): commit a session's CAS writes in one transaction (m9-74)`
- `e3fa0fc` — `fix(mcp): restore the flat v1 response shape of the two deprecated shims (m9-74)`
- `ac539db` — `docs(agents): T3 runs in two halves here; drop the two session_edge_cases rows (m9-74)`
- `c2c0d3738d8295721eff30a12e7601af4010072f` — `feat(m9-74): cycle artifacts (apply-checkpoint, verify, release-report)` (tag `v0.7.76`)

## Cross-checks

- CC#1..CC#56: pass (no drift). Python CC count unchanged at 48 and bash at 7; no new cross-check this cycle, so `scripts/smoke_test_ccs.sh` was untouched and still reports 5/5.
- CC#4: vault files changed by this cycle are `cycles/index.md`, `terms/index.md` and the new m9-74 manifest; the archive manifests that list those index SHAs had their rows regenerated to a fixpoint (second pass reported 0 updates). Unrelated churn in the nine pre-m9-11 manifests whose only stale row is the self-referential `archive-manifest (this file)` row was reverted, and the m9-67 / m9-68 self-rows were restored to their committed values.
- CC#24 / CC#31: `verify-report.md` carries `## Cross-checks` and a `Path` field; this manifest carries `Base SHA` and `Head SHA`.
- CC#12: `main_sha == head_sha == remote_tag_peel`.
- CC#36 / CC#55 / CC#39: `verify-report.md` has a `## Files Inventory` section and a `| Campo | Valor |` header with `Path` near the top; `verify-findings.json` carries `lens_summary` after `subject`; this manifest and `release-report.md` both carry `## Cross-checks`.
- T0: `cargo fmt --all -- --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean.
- T2: `cargo test -p chronos-store --lib` → 74 passed; `cargo test -p chronos-mcp --lib` → 78 passed.
- T3: `cargo test --workspace --lib --tests --exclude chronos-sandbox --exclude chronos-e2e --no-fail-fast` → 37 test binaries `ok`; `cargo test -p chronos-native --lib -- --test-threads=1` → 101 passed in 13 s.
- T4-smoke (`--test-threads=1`, `CHRONOS_MCP_PATH` exported): `session_edge_cases` 6/6 (65.1 s), `session_persistence` 4/4 (27.8 s), `multi_session` 6/6 (51.3 s), `e2e_connectivity` 1/1 (5.6 s).

## Follow-ups (deferred)

- **FIND-M9-74-NATIVE-PTRACE-TESTS-NEED-SERIAL** (low, new): the `chronos-native` lib suite cannot run in parallel here — two ptrace tests fail and a third blocks in `waitpid` for 17 min at 0% CPU; serial it is 101 passed in 13 s. Make the ptrace tests serial structurally (test-level lock, separate binary, or an explicit opt-in target).
- **FIND-M9-73-SILENT-IN-MEMORY-FALLBACK-MASKS-STORE-OPEN-FAILURE** (medium, still open): `chronos-mcp::open_default_store` silently degrades to an in-memory store when the configured store cannot be opened, so a locked store yields successful saves and empty listings; the remedy is a policy decision.
- **FIND-M9-73-CC4-REGEN-RITUAL-NOT-IN-REPO** (low, third confirmation): land the manifest-SHA regeneration as a repo script with the skip-self-row rule.
- **FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION** (low): unchanged.
- **FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION** (low): unchanged; the affected set grows by one manifest per cycle.
- **Sandbox warm-up ordering** (`test_session_start_via_v2_then_session_stop_via_v2`) and **5+19 not-merged branches triage**: preserved.

## Artifact index

| Kind | Path | SHA-256 |
|---|---|---|
| archive-manifest (this file) | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-74-cas-put-many-batching/archive-manifest.md` | `0000000000000000000000000000000000000000000000000000000000000000` |
| source (content store) | `crates/chronos-store/src/cas.rs` | `a0f32a00c732bde1cd2e2be7744cb25be1c2e583aaa987593e7c80b31510660a` |
| source (session storage) | `crates/chronos-store/src/storage.rs` | `6c42c78f4ea8e5b73bb3d7ffd5a228396a12a1e6052c8fc6b8354c816469a113` |
| source (store crate root) | `crates/chronos-store/src/lib.rs` | `093934f02101a7f6404fcd3cebe7fce972861731a6657becaa8bba5eb6f687fa` |
| test support (sync barrier counter) | `crates/chronos-store/src/test_support.rs` | `60f6d32eddead5721c5e6ebb6037f54f945562f88cc4c6c0038869d0a5b42680` |
| source (mcp server, tool wire shapes) | `crates/chronos-mcp/src/server.rs` | `7d689b1f0b450fd84359c08d052fa7c23b874aeb7dabdecf7070df80f3aef178` |
| docs (agents manual) | `AGENTS.md` | `83d09aa421c0c0c06a8dc28d4a5ba27dbb66772f7a1310103f3c133969fe8924` |
| apply-checkpoint | `cycle-artifacts/p-3416cfb8288f8964/m9-74-cas-put-many-batching/apply-checkpoint.json` | `dab499eb017af26c3bf2a2a696d7c2b3dfdb47502f58429e00ace325181610d9` |
| verify-report | `cycle-artifacts/p-3416cfb8288f8964/m9-74-cas-put-many-batching/verify-report.md` | `06e94982bb2c35c8e592760689758136fdd37b8d7f25b737ceb51d36ffceb6c0` |
| verify-findings | `cycle-artifacts/p-3416cfb8288f8964/m9-74-cas-put-many-batching/verify-findings.json` | `cf6aac480c21339b0c4aea42893e9a3b339ffffe01d4428ab57a2d3737ac44b0` |
| release-report | `cycle-artifacts/p-3416cfb8288f8964/m9-74-cas-put-many-batching/release-report.md` | `a361dc9312a12739c154e841dc3c4cb93eaeeda1054b1dff0cfc6c52b4aaee9a` |
| release-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-74-cas-put-many-batching/release-receipt.md` | `5de5a22a246d5cb8bb8f8b47260d09b767fd7250f38dcd6362432c36f4c55a53` |
| merge-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-74-cas-put-many-batching/merge-receipt.md` | `54c887e312366589c08ac19109959ff6d88367b553ab0c6a78535615c762ac76` |
| change-entry | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-74-cas-put-many-batching/change-entry.md` | `898af1d8a0e49a2fc63a79a5f4b0a1aa9fa60383c48b826d84dad41f06f2263d` |
| vault index (cycles) | `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` | `822284b2e06bd4ea027b9e158bbabab882e0f9a3de9ee9c64265e9d8701b9e6f` |
| vault index (terms) | `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` | `d370c78319548ace8a6d055ac1e988ff6fb6231b106f49d81e3c49e77244ea1c` |
