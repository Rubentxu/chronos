# REC-C1.7 — Projection Authority + final REC-C1 acceptance (tasks)

**Cycle**: `p-3416cfb8288f8964/rec-c1-7-projection-authority-acceptance`
**Companion**: `proposal.md`, `design.md`

Tasks are ordered so each GREEN leaves the tree green. CHAR tests are
RED on `main` by construction — they go in before any code change,
then they flip GREEN when the projection lands.

## C1.7.0 — reconciliation (planning commit)

Single commit. No production code.

1. Update `docs/chronos-agentic-reconstruction/docs/reconstruction/ROADMAP_CONTROL_PLANE.md`:
   - STATUS: REC-C1.6 CLOSED; REC-C1.7 ACTIVE.
   - ACTIVE PRODUCT GATE: REC-C1.7 (projection authority + final REC-C1
     behavioral acceptance). Sub-gates C1.7.1..C1.7.7 listed.
   - DECLARED WINDOWS unchanged.
2. Update `reconstruction-contracts.toml`:
   - `active_gate = "REC-C1.7"`.
   - `updated = "2026-09-17"`.
3. Update `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md`:
   - Append the C1.7 row (`status: ACTIVE` in the live row, will be
     `CLOSED` on merge).
   - Bump `Total cycles` from 100 to 101.
   - The pre-existing CC#39 drift grows from 1 to 2. Do **not**
     touch CC#39's counting logic.
4. Drop this `tasks.md` next to the existing
   `proposal.md` / `design.md` / `apply-checkpoint.json`.

Commit: `chore(rec-c1.7): planning artifacts (C1.7.0)` on
`feat/rec-c1.7-projection-authority-acceptance`.

## C1.7.1 — RED: dual-truth characterization (commit 1 of code)

Add a new module `chronos-services::dual_truth_characterization`
(file `crates/chronos-services/src/dual_truth_characterization.rs`)
containing two tests that **fail on main**:

- `dual_truth_state_query_diverges_from_execution_log`:
  1. Open a `SessionExecutionLog` in a tempdir.
  2. Append records {A, B, C} via the canonical
     `NewExecutionRecord` + `serde_json::to_vec(&trace_event)`.
  3. Spin up the MCP server's `ChronosServer` with the
     `SessionExecutionLog` registered.
  4. Call `execution_query` via the wire. Capture the response.
  5. Assert the response **does not contain** a record C; the
     current engine was built from a drain that never saw C.

- `dual_truth_state_query_diverges_after_append`:
  1. Same setup.
  2. Stop a probe (which today drains the backend and stores an
     engine holding {A, B}).
  3. Append record C to the ExecutionLog.
  4. Call `execution_query` and `trace_slice`.
  5. Assert: response shows only {A, B}; record C is absent.

These two tests prove the user's "second tree of authority" diagnosis.
The cycle is not legitimate until they flip GREEN, so they live in
their own module and are referenced by name in the design.

Commit: `test(rec-c1.7): characterize dual-truth divergence (RED, flips GREEN in C1.7.2)`
on the cycle branch.

## C1.7.2 — GREEN: `chronos_services::projection` + builder (commit 2)

- New file `crates/chronos-services/src/projection.rs` containing
  the types and `build_engine` from `design.md`.
- `chronos-services/src/lib.rs`: add `pub mod projection;`.
- New tests in `chronos-services::projection::tests`:
  - `build_engine_empty_session_returns_empty_completeness`
  - `build_engine_full_history_returns_full_completeness`
  - `build_engine_truncated_history_returns_truncated_completeness_and_refuses_require_full_history`
  - `build_engine_decodes_via_shared_decode_helper` (corrupt JSON → `EvidenceDecodeFailed`)
  - `build_engine_filters_registers_and_unknown`

These five tests are unit-level and run under `cargo test -p
chronos-services --lib projection` — they go GREEN the moment
`projection.rs` compiles and the decode filter is correct.

Commit: `feat(rec-c1.7): chronos_services::projection::build_engine + 5 unit tests`
on the cycle branch.

## C1.7.3 — wire the projection into MCP and gate canonical operations (commit 3)

- `crates/chronos-mcp/src/server.rs`:
  - New field `projection_meta: Arc<Mutex<HashMap<String, ProjectionMeta>>>`
    next to `engines`.
  - Rewrite `build_and_store_engine` to take `&SessionExecutionLog`
    and call `chronos_services::projection::build_engine`. Insert
    `(engine, meta)` atomically under one lock acquisition.
  - Replace every caller of `build_and_store_engine` (10 callers
    in `server.rs` after tests) with one that takes a
    `&SessionExecutionLog` from the registry.
  - In the three canonical operations
    (`execution_query::ChronosExecutionQueryService::query`,
    `state_query::ChronosStateQueryService::query`,
    `trace_slice::ChronosTraceSliceService::slice`), read
    `projection_meta[session_id]` and call
    `chronos_services::projection::require_full_history` before
    touching the engine. Map
    `EvidenceUnavailableDueToRetention { retained_from }` to the
    same wire surface the existing variants produce.
  - Existing tests that synthesize raw events into the engine map
    are updated to use `SessionExecutionLog::create` +
    `append(NewExecutionRecord{...})` fixtures.

- `chronos-services::execution_query`, `state_query`,
  `trace_slice`:
  - Accept `&ProjectionMeta` alongside `&Mutex<HashMap<String,
    QueryEngine>>` (or a `Projection` wrapper that holds both).
  - Gate on `require_full_history`.

Commit: `feat(rec-c1.7): route canonical queries through projection + require_full_history`
on the cycle branch.

This commit flips the C1.7.1 RED tests GREEN.

## C1.7.4 — restart-equivalence sandbox UAT (commit 4)

New sandbox test file
`chronos-sandbox/tests/projection_restart_equivalence.rs`:

- `restart_preserves_execution_query`:
  1. Spawn MCP server (fixture from `chronos-sandbox::client::McpTestClient`).
  2. `session_start` to spawn a real C process.
  3. Wait for 1,000 records to be appended (use the
     `RawTraceProducer`-style fixture from the existing
     sandbox tests).
  4. Call `execution_query` (and `state_query` and
     `trace_slice`); capture the JSON.
  5. Stop the probe cleanly.
  6. **Restart** by closing the server, dropping the engines
     map, re-opening the session log via
     `SessionExecutionLog::reopen_existing`, and reconnecting.
  7. Re-call the three operations.
  8. Assert the post-restart response bodies are semantically
     equal to the pre-restart ones (same `event_id`s in same
     order; same `timestamp_ns`; same gap information; same
     completeness).

Commit: `test(rec-c1.7): restart-equivalence UAT for canonical operations`
on the cycle branch.

## C1.7.5 — public C1-01 UAT (commit 5)

New sandbox test file
`chronos-sandbox/tests/c1_01_two_consumers_real_wire.rs`:

- Spawn an MCP server, append 10,000 records.
- Consumer A reads 100, captures cursor, returns.
- Consumer B reads 100, captures cursor, returns.
- Producer advances by 1,000.
- A and B resume from their own cursors.
- Assert: A's reads are disjoint from B's; neither read steals
  the other's position; `events_read` cursors are non-destructive
  (re-reading with the same cursor returns the same records).
- Force a gap (use the C1.4 retention fixture): read crossing
  the gap returns `GapDetected`; never `Complete`.

Commit: `test(rec-c1.7): UAT-REC-C1-01 two consumers, public MCP wire`
on the cycle branch.

## C1.7.6 — C1-05 time-semantics UAT (commit 6)

New sandbox test file
`chronos-sandbox/tests/time_semantics_uncorrelated.rs`:

- Build a fixture of records with deliberately uncorrelated
  `seq`, `event_id`, `timestamp_ns`:
  ```text
  seq         0, 1, 2, …
  event_id    40, 90, 130, …
  timestamp   10_000_500, 25_320_700, 25_999_001, …
  ```
- Append via the canonical producer. Read via `events_read`,
  `execution_query`, `trace_slice`.
- Assert:
  - `record.seq != record.event_id` for every record.
  - `record.timestamp_ns` is not a Unix-epoch wall-clock value.
  - No encoder/projection substitutes one for another.

This is also covered by a unit test in
`chronos-services::projection::tests::time_semantics_round_trip`.

Commit: `test(rec-c1.7): UAT-REC-C1-05 time semantics uncorrelated`
on the cycle branch.

## C1.7.7 — TRUTH-001 ratchet (commit 7)

- `reconstruction-contracts.toml`:
  - `TRUTH-001.status = "verified"`.
  - `TRUTH-001.evidence = ["crates/chronos-services/src/projection.rs::build_engine", "crates/chronos-mcp/src/server.rs::build_and_store_engine (rewritten)"]`.
  - `TRUTH-001.notes` updated to reflect C1.7 (projection
    canonical; legacy paths remain, owned by REC-C2).
  - `TRUTH-001.verify = ["cargo test -p chronos-services --lib projection", "cargo test -p chronos-sandbox --test projection_restart_equivalence"]`.

Commit: `chore(rec-c1.7): TRUTH-001 partial → verified (projection authority)`
on the cycle branch.

## Closure

C1..C6 same as C1.6: T0 fmt+clippy, T3 workspace tests,
T4-smoke subset (projection_restart_equivalence,
dual_truth_characterization, time_semantics_uncorrelated,
e2e_connectivity), then merge + tag + apply-checkpoint finalize
+ vault drift CC sweep, matching the rec-c1 stream convention
(`archive_status = "ready"`, no
`.sddk-knowledge/changes/archive/rec-c1-7-*/` directory).
