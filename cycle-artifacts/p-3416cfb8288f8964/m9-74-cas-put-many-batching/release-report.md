# Release Report — m9-74-cas-put-many-batching

## Path

B-direct

## Subject

Closes the high finding m9-73 deferred, the symptom row that came with it, and a
medium finding that only became visible once the first one was fixed.

**1. `FIND-M9-73-CAS-PUT-ONE-WRITE-TRANSACTION-PER-EVENT` (high).**
`ContentStore::put` opened a redb write transaction, ran `ensure_table`, checked
the dedup row, inserted one row, and committed. redb defaults to immediate
durability and `set_durability` appears nowhere in `chronos-store`, so each
`commit()` is a `sync_data` barrier, and `SessionStore::save_session` called
`put` in a loop: one fsync per event. m9-73 measured the consequence — a
34,905-event crash session did not complete in 186 s, linear at ≈4.8 ms/event —
and proposed `put_many`.

The fix separates the three things that were tangled in one method:

- `ContentStore::encode(event) -> (ContentHash, Vec<u8>)` does the serialize,
  compress and BLAKE3 work and touches no I/O, so a batch is fully encoded
  before a transaction opens and an encoding failure stores nothing.
- `ContentStore::insert_batch(&[(ContentHash, Vec<u8>)])` opens one write
  transaction, keeps the per-event dedup check, and commits once. Dedup is
  unchanged in kind: a redb write transaction reads its own inserts, so
  duplicates *inside* a batch collapse exactly like duplicates against the
  existing store.
- `ContentStore::put_many(&[TraceEvent]) -> Vec<ContentHash>` encodes the whole
  slice, returns one hash per event in input order, and returns early for an
  empty batch without opening a transaction. `put` is now `encode` +
  `insert_batch`, with its signature, semantics and hash intact, and
  `save_session` writes its events through `put_many`, so a session costs one
  transaction for its events plus one for its metadata regardless of volume.

Evidence is not a wall-clock timing but a counted invariant.
`crates/chronos-store/src/test_support.rs` (new, `#[cfg(test)]`) provides
`CountingBackend`, which wraps redb's real `FileBackend` and counts `sync_data`
calls. It is file-backed on purpose: an in-memory redb never syncs, so the
assertion would pass for the wrong reason. On top of it, four tests in `cas.rs`
(order, per-event hash equality with `put`, dedup within and across batches, the
empty batch must not create the `cas` table) and one in `storage.rs` (a
1,000-event `save_session` must issue at most four barriers and still
round-trip). The barrier tests carry a control: a loop of 50 `put` calls must
report at least 50 barriers, so a counting backend that silently stops counting
cannot make the assertion pass.

**2. `FIND-M9-73-SESSION-EDGE-CASES-HEAVY-SAVE-NEVER-COMPLETES` (medium).**
The observable symptom, recorded separately by m9-73 because it could not then be
proven to be the same defect. It is: with the batching in place and the client
timeout untouched at 30 s, `session_edge_cases` goes from
`4 passed / 2 failed` (never completing, 186 s+ with a 180 s timeout) to
`6 passed / 0 failed in 65.1 s`. The two `AGENTS.md` §6.5 rows were removed
rather than reworded, with a note recording where they went, because a fixed
defect parked in the flake table is a defect waiting to be re-diagnosed.

**3. `FIND-M9-74-V1-SHIMS-RETURN-V2-ENVELOPE` (medium).** Found by running the
acceptance suite once #1 was fixed, which is the entire reason this cycle is not
a one-line diff.

With the crash save no longer timing out, both comparison tests failed with
`compare_sessions failed: RpcError("missing field 'session_a_id'")` and
`performance_regression_audit failed: …`. m7-03 (`947e73b`) converted those two
v1 tools into shims that route through `dispatch_session_compare`, and changed
what they return along the way: before it, the tool bodies built the flat v1
JSON by hand (`session_a_id`/`only_in_a_count`/… and
`baseline_session_id`/`regressions`/…); after it, they serialize the tagged
`SessionCompareOutput` enum with a `provenance` field. The names, the
parameters and the descriptions all stayed v1 — "DEPRECATED v1 shim … the v1
parameter names are preserved" — and `SessionCompareOutput`'s own doc comment
states the intended behaviour that the code violated: *"MCP shims drop the
`provenance` field so existing v1 callers see the same JSON they did before
m7-03."*

Nothing pinned it. The four `chronos-mcp` tests for these tools assert on the
`Debug` text of the content block and look for keys (`similarity_pct`,
`common_count`, `summary`) that also appear inside the nested envelope; the
sandbox client, the only caller that parses the payload as JSON, could not get
that far until this cycle; and the CLI does not call these tools. So a v1 client
received `{kind, result, provenance}` where the documented contract says a flat
object, and got `missing field 'session_a_id'`.

The fix is a wire-shape parameter rather than a second dispatcher: new
`SessionCompareWire { V2Envelope, V1Flat }` with a `to_value` that either
serializes the envelope or unwraps it to the flat
`CompareSessionsResult` / `PerformanceRegressionAuditResult` that
`chronos-services::output` kept for exactly this purpose; the dispatcher takes
the shape as an argument; `session_compare` passes `V2Envelope`, both shims pass
`V1Flat`; both descriptions now promise the response shape as well as the
parameter names. A new test parses the actual tool payloads and pins the split
in both directions. Grep confirms these two tools are the only ones in
`chronos-mcp` whose description contains "DEPRECATED v1 shim", so the class is
closed rather than sampled.

Ordering matters and is worth recording: #3 was reachable *only* after #1, since
the suite had to save a 35k-event session before it could compare anything. A
cycle that had stopped at "the timeout is fixed, the remaining failures
pre-exist" would have shipped a green-looking release with a broken v1 API
surface.

## Files changed

| File | Change |
|---|---|
| `crates/chronos-store/src/cas.rs` | +210/−23: `encode`, `insert_batch`, `put_many`; `put` refactored onto them; four tests |
| `crates/chronos-store/src/storage.rs` | +65/−7: `save_session` goes through `put_many` (with a note on the CAS-then-metadata ordering); one barrier-count test |
| `crates/chronos-store/src/lib.rs` | +2: `#[cfg(test)] mod test_support` |
| `crates/chronos-store/src/test_support.rs` | new, 70 lines: `CountingBackend` + `counting_file_db` |
| `crates/chronos-mcp/src/server.rs` | +173/−8: `SessionCompareWire`, dispatcher parameter, both shims `V1Flat`, descriptions, new shape test |
| `AGENTS.md` | +16/−3: §6.5 `chronos-native` row rewritten with the measured parallel behaviour, the two `session_edge_cases` rows removed, the T3 two-halves recipe |

Six files, +536/−41. Three commits on
`feat/m9-74-cas-put-many-batching`:

- `bc3b912` — `perf(store): commit a session's CAS writes in one transaction (m9-74)`
- `e3fa0fc` — `fix(mcp): restore the flat v1 response shape of the two deprecated shims (m9-74)`
- `ac539db` — `docs(agents): T3 runs in two halves here; drop the two session_edge_cases rows (m9-74)`

## Cross-checks

- **CC#1..CC#56** pass: 48 python + 7 bash. No new check this cycle, so
  `scripts/smoke_test_ccs.sh`'s expected counts are unchanged; the smoke test
  itself is 5/5.
- **CC#4**: `cycles/index.md` and `terms/index.md` changed, and the m9-74
  archive manifest is new, so every prior manifest whose artifact index lists
  those files had its SHA rows regenerated to a fixpoint (second pass reported
  zero updates). Unrelated churn in the nine pre-m9-11 manifests whose only
  stale row is the self-referential one was reverted, as in m9-73.
- **CC#12**: `main_sha == head_sha == remote_tag_peel` over `v0.7.76`.
- **CC#24 / CC#31**: this report and the archive manifest both carry
  `## Cross-checks`; the manifest carries `Base SHA` and `Head SHA`.
- **CC#36 / CC#55**: the verify report carries the `Path` row and the
  `## Files Inventory` section.
- **T0**: `cargo fmt --all -- --check` clean; `cargo clippy --workspace
  --all-targets -- -D warnings` clean (0 warnings).
- **T2**: `-p chronos-store --lib` 74 passed; `-p chronos-mcp --lib` 78 passed.
- **T3**: workspace minus `chronos-sandbox` minus `chronos-e2e` all green (37
  test binaries reported `ok`), plus `-p chronos-native --lib --test-threads=1`
  101 passed.
- **T4-smoke**: `session_edge_cases` 6/6 (65.1 s), `session_persistence` 4/4
  (27.8 s), `multi_session` 6/6 (51.3 s), `e2e_connectivity` 1/1 (5.6 s), all
  `--test-threads=1` with `CHRONOS_MCP_PATH` exported.

## Self-tests performed

| Revert | Observed result |
|---|---|
| `put_many` back to the per-event `put` loop | `test_put_many_commits_once_per_batch` **FAILED**: a 500-event batch issued one barrier per event, while the 50-`put` control loop reported ≥50 as designed |
| Both shims back to `SessionCompareWire::V2Envelope` | the new shape test **FAILED** at `assert_eq!(v["session_a_id"], json!(sid_a))` — `null` where the v1 contract has a string |

Both reverts were restored and compared byte-for-byte with their backups (`cmp`
byte-identical) before the final green runs: `-p chronos-store --lib` 74 passed,
`-p chronos-mcp --lib` 78 passed.

A green run proves nothing on its own here, because the oracle is the barrier
count and the parsed payload, not "the suite is green": the falsification table
above is the actual evidence, and the accepted state is the one where both
reverts fail and both restorations pass.

## T4-smoke subset and why these suites

The change is on the session save path and on two tool response shapes, so the
subset is the suite that saves a large probe session and then calls both tools
(`session_edge_cases`), the suites that save repeatedly and compare across
clients (`session_persistence`, `multi_session`), and the server-boot check
(`e2e_connectivity`). 17 tests, 0 failures, all serial with the binary path
exported. `session_edge_cases` is the acceptance test for finding #1: it is the
only place a real 35k-event crash capture is saved and then compared.

## Pre-existing observations (measured, not waved through)

Two anomalies showed up while running T3. Neither involves this cycle's diff, and
both were characterised rather than assumed:

1. `chronos-e2e`'s `test_ptrace_capture` never finishes: no output for 30 min,
   0% CPU, blocked on a `futex`. The crate does not mention `chronos_store`,
   `SessionStore` or `ContentStore` (grep: zero hits) — it drives
   `chronos_native::capture_runner` directly. §1 of `AGENTS.md` already
   classifies `chronos-e2e` as bucket D, "explicit opt-in, needs root + ptrace
   kernel", and §6.5 already documented this hang (commit `6ee95ea`).
2. `chronos-native --lib` in parallel: `test_launch_captures_events` and
   `test_launch_true_and_wait` fail, and another test blocks in `waitpid` for
   17 min — still blocking after `--skip test_launch_with_syscall_tracing`, so
   it is the class and not one test. Serially the same binary is 101 passed in
   13 s, and the originally-flaky test alone is 1 passed in 1.32 s. §6.5's old
   row called this a 50% flake; it now records the real shape (two failures plus
   a hang in parallel) and the recipe that works.

## History

- `FIND-M9-73-CAS-PUT-ONE-WRITE-TRANSACTION-PER-EVENT` was recorded by m9-73 with
  the mechanism identified (per-event write transaction + immediate durability),
  the cost measured (≈4.8 ms/event, >186 s for a 35k-event session) and the fix
  proposed (`put_many`). This cycle implemented it as proposed.
- `FIND-M9-73-SESSION-EDGE-CASES-HEAVY-SAVE-NEVER-COMPLETES` was the symptom row
  m9-73 added to `AGENTS.md` §6.5 to stop future cycles chasing it.
- `FIND-M9-74-V1-SHIMS-RETURN-V2-ENVELOPE` originates in m7-03 (`947e73b`),
  which rerouted the v1 tools through the v2 dispatcher. It survived four cycles
  of sandbox work because no test parsed those payloads and the only client that
  does was blocked one step earlier.

## Future work

- **FIND-M9-73-SILENT-IN-MEMORY-FALLBACK-MASKS-STORE-OPEN-FAILURE** (medium,
  still open): `chronos-mcp::open_default_store` degrades to an in-memory store
  when the configured store cannot be opened, so a locked store yields
  successful saves and empty listings instead of an error. It is a policy
  decision in `chronos-mcp`, not a harness one.
- **FIND-M9-74-NATIVE-PTRACE-TESTS-NEED-SERIAL** (low, new): the `chronos-native`
  lib suite cannot run in parallel here. The real remedy is structural (a
  test-level lock, a separate binary, or an explicit opt-in target) so the
  default `cargo test` does not lose 17 minutes to a silent hang; documenting the
  recipe is what this cycle could do without restructuring a crate it does not
  otherwise touch.
- **FIND-M9-73-CC4-REGEN-RITUAL-NOT-IN-REPO** (low, still open, third
  confirmation): land `scripts/regen_manifest_index_shas.py` with the
  skip-self-row rule and document it next to CC#4.
- **FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION** and
  **FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION** (low, unchanged).
- The next session-save-shaped finding to look for: `put_many` now holds redb's
  single write lock for a whole session, so a *concurrent* writer waits longer
  than before. Nothing measured is a problem (the previous behaviour interleaved
  fsyncs from the same lock), but a multi-prober workload that writes two large
  sessions at once is the case worth measuring if throughput complaints appear.
