# Change: m9-74 CAS put_many batching

## Summary

Closes `FIND-M9-73-CAS-PUT-ONE-WRITE-TRANSACTION-PER-EVENT`, the high finding
m9-73 deferred with its mechanism identified and its fix proposed, plus the
symptom row that came with it and one more medium finding that the first fix
uncovered.

`ContentStore::put` opened a redb write transaction, inserted one row and
committed, and redb's default immediate durability makes every `commit()` a
`sync_data` barrier — `set_durability` appears nowhere in `chronos-store` — while
`SessionStore::save_session` called it in a loop. Saving a session therefore cost
one fsync per event: ~4.8 ms/event, so the 34,905-event crash capture in
`session_edge_cases` did not complete in 186 s against the sandbox client's 30 s
`tools/call` timeout. The fix splits the pure `encode` step (bincode + LZ4 +
BLAKE3) from the write so a batch is fully encoded before a transaction opens and
an encoding failure stores nothing, inserts a whole batch in one write
transaction with the per-event dedup check preserved (a redb write transaction
reads its own inserts, so intra-batch duplicates collapse too), and exposes
`put_many` for `save_session`; `put` is now `encode` + `insert_batch` and keeps
its signature, semantics and hash.

The evidence is a counted invariant, not a wall-clock timing: a new
`#[cfg(test)]` `CountingBackend` wraps redb's real `FileBackend` and counts
`sync_data` calls — file-backed on purpose, because an in-memory redb never syncs
and the assertion would pass for the wrong reason. A 500-event batch must stay
under four barriers, against a control loop of 50 `put` calls that must report at
least 50; a 1,000-event `save_session` must stay under four barriers and still
round-trip.

The interesting part of the cycle is what the first fix uncovered. With the save
no longer timing out, `session_edge_cases` reached the comparison step and both
tests failed with `missing field 'session_a_id'`. m7-03 (`947e73b`) had rerouted
the two deprecated v1 tools through the `session_compare` dispatcher and silently
changed their response shape from the flat v1 result to the tagged
`SessionCompareOutput` envelope, while keeping the v1 names, parameters and
descriptions — and while `SessionCompareOutput`'s own doc comment declares the
opposite intent ("MCP shims drop the `provenance` field so existing v1 callers see
the same JSON they did before m7-03"). Nothing pinned it: the four `chronos-mcp`
tests for those tools assert on `Debug` text that the nested shape also satisfies,
and the sandbox client, the only caller that parses the payload as JSON, was
blocked one step earlier. A new `SessionCompareWire { V2Envelope, V1Flat }`
parameterises the dispatcher, `session_compare` keeps the envelope, both shims
return the flat result the services crate kept for them, and a new test pins the
split in both directions by parsing the real tool payloads.

## Ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-74-cas-put-many-batching` |
| Path | B-direct |
| Status | CLOSED |
| Base SHA | `6317af899440178d30dc1d45807aed1851d0bb83` |
| Head SHA | `c2c0d3738d8295721eff30a12e7601af4010072f` |
| Tag | `v0.7.76` |

## Subject

- base_sha: `6317af899440178d30dc1d45807aed1851d0bb83`
- head_sha: `c2c0d3738d8295721eff30a12e7601af4010072f`
- source commits: `bc3b912` (store), `e3fa0fc` (mcp), `ac539db` (docs)
- cycle: m9-74
- branch: `feat/m9-74-cas-put-many-batching`
- date: `2026-09-13T16:51Z`
- tag: `v0.7.76`
- findings_closed: 3 (`FIND-M9-73-CAS-PUT-ONE-WRITE-TRANSACTION-PER-EVENT` high, `FIND-M9-73-SESSION-EDGE-CASES-HEAVY-SAVE-NEVER-COMPLETES` medium, `FIND-M9-74-V1-SHIMS-RETURN-V2-ENVELOPE` medium)
- findings_introduced (deferred): 1 (`FIND-M9-74-NATIVE-PTRACE-TESTS-NEED-SERIAL` low), plus 4 inherited low/medium rows re-listed
- new tests: 6 (4 in `cas.rs`, 1 in `storage.rs`, 1 in `server.rs`) + `test_support.rs`

## Files changed

- (modified) `crates/chronos-store/src/cas.rs` — `encode`, `insert_batch`, `put_many`; `put` refactored onto them; the four tests
- (modified) `crates/chronos-store/src/storage.rs` — `save_session` goes through `put_many`, with a note on the CAS-then-metadata ordering; the barrier-count test
- (modified) `crates/chronos-store/src/lib.rs` — registers `test_support`
- (new) `crates/chronos-store/src/test_support.rs` — `CountingBackend` + `counting_file_db`
- (modified) `crates/chronos-mcp/src/server.rs` — `SessionCompareWire`, the dispatcher parameter, both shims `V1Flat`, both descriptions, the new shape test
- (modified) `AGENTS.md` — §6.5 `chronos-native` row rewritten with the measured parallel behaviour, the two `session_edge_cases` rows removed, the T3 two-halves recipe recorded
- (new) `cycle-artifacts/p-3416cfb8288f8964/m9-74-cas-put-many-batching/*`
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-74-cas-put-many-batching/change-entry.md` (this file)
- (new) `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-74-cas-put-many-batching/archive-manifest.md`
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` (m9-74 row, Total cycles 73→74)
- (modified) `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` (two m9-73 rows and one m9-74 row terminated; the m9-73 `TBD_TAG` filled in; one new deferred row)

## Cross-checks

CC#1..CC#56: pass (48 python + 7 bash, counts unchanged — no new check this cycle). T0 fmt + clippy (`-D warnings`) clean workspace-wide. T2: `-p chronos-store --lib` 74 passed, `-p chronos-mcp --lib` 78 passed. T3: workspace minus `chronos-sandbox` minus `chronos-e2e` all green, plus `-p chronos-native --lib --test-threads=1` 101 passed. T4-smoke: `session_edge_cases` 6/6 (65.1 s), `session_persistence` 4/4, `multi_session` 6/6, `e2e_connectivity` 1/1, all serial with `CHRONOS_MCP_PATH` exported. CC#4: the regeneration loop reached a fixpoint (second pass 0 updates) and unrelated churn in the nine pre-m9-11 manifests was reverted. CC#12: `main_sha == head_sha == remote_tag_peel`.

## Falsification evidence

Two independent reverts, each observed to fail and then restored (both restored
files byte-compared with their backups):

| Reverted | Observed result |
|---|---|
| `put_many` back to the per-event `put` loop | `test_put_many_commits_once_per_batch` FAILED — the 500-event batch issued one barrier per event against the `CountingBackend`, while the 50-`put` control loop correctly reported ≥50 |
| Both v1 shims back to `SessionCompareWire::V2Envelope` | `test_v1_shims_return_flat_result_while_v2_returns_envelope` FAILED at `assert_eq!(v["session_a_id"], json!(sid_a))` — `null` where the v1 contract has a string, i.e. exactly the `missing field 'session_a_id'` the sandbox client reported |

A third observation is not a revert but the reason the cycle has three findings:
running the acceptance suite with the CAS fix alone in place showed
`session_edge_cases` passing the save and failing both comparison tests, which is
how the v1 shim drift surfaced.

## Attribution of the two T3 anomalies

Neither involves this cycle's diff, and both were measured rather than assumed:

- `chronos-e2e`'s `test_ptrace_capture` never finishes (30 min, 0% CPU, `futex`
  wait). The crate does not reference `chronos_store`, `SessionStore` or
  `ContentStore` at all (grep: zero hits); it drives `chronos_native::capture_runner`.
  §1 of `AGENTS.md` already classifies it as bucket D, opt-in only, and §6.5
  already documented the hang (`6ee95ea`).
- `chronos-native --lib` in parallel fails two ptrace tests and blocks a third in
  `waitpid` for 17 min, still blocking after skipping the documented flaky test;
  serially the same suite is 101 passed in 13 s. §6.5's row understated it as a
  50% flake and was rewritten with the real shape and the working recipe.

## Follow-ups (deferred)

- **FIND-M9-74-NATIVE-PTRACE-TESTS-NEED-SERIAL** (low, new): make the ptrace tests
  serial structurally (test-level lock, separate binary, or an explicit opt-in
  target) so the default `cargo test` cannot hang for 17 minutes.
- **FIND-M9-73-SILENT-IN-MEMORY-FALLBACK-MASKS-STORE-OPEN-FAILURE** (medium, still
  open): `open_default_store` degrades to an in-memory store when the configured
  store cannot be opened; a policy decision for `chronos-mcp`.
- **FIND-M9-73-CC4-REGEN-RITUAL-NOT-IN-REPO** (low, third confirmation): land
  `scripts/regen_manifest_index_shas.py` with the skip-self-row rule.
- **FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION** (low, unchanged).
- **FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION** (low, unchanged).
- **Sandbox warm-up ordering** (preserved): `test_session_start_via_v2_then_session_stop_via_v2`
  still not reproducible.
- **5+19 not-merged branches triage** (preserved from m9-65): human review needed.
