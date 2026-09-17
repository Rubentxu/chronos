# REC-C1.8 — Authoritative Evidence Handoff (tasks)

**Cycle**: `p-3416cfb8288f8964/rec-c1-8-authoritative-evidence-handoff`
**Companion**: `proposal.md`

Tasks are ordered so each GREEN leaves the tree green. The C1.7 RED
characterization is the only test file on `main` in a non-green state at
the start of this cycle; it goes away in C1.8.2.

## BLOCKER before commit 1

The cycle is blocked on two user decisions, both captured in `proposal.md`:

- **C1.8.2.a — rebuild responsibility:** wrapper (W, default) vs service (S).
- **C1.8.5 option — wall-clock dimension:** A (add field, default) vs B
  (amend wording + ADR).

C1.8.0 (planning) and C1.8.1 (doc reconciliation) do not depend on either
decision and can land as soon as the user signals go.

## C1.8.0 — planning commit

Single commit. No production code.

1. Update `docs/chronos-agentic-reconstruction/docs/reconstruction/ROADMAP_CONTROL_PLANE.md`:
   - `STATUS`: append a clause declaring C1.7 closed on its real tag;
     note that the next gate is REC-C1.8 (active).
   - `ACTIVE PRODUCT GATE`: `REC-C1.8 (Authoritative Evidence Handoff)`.
   - DO NOT yet correct the `9ee74f09` line — that correction belongs to
     C1.8.1 (the doc reconciliation pass) so this commit stays free of
     content changes beyond declaring the cycle active.
2. Update `reconstruction-contracts.toml`:
   - `active_gate = "REC-C1.8"`.
   - `updated = "2026-09-17"`.
3. Update `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md`:
   - Append the C1.8 row (`status: ACTIVE` in the live row; flips to
     `CLOSED` on merge).
   - Bump `Total cycles` from 101 to 102.
   - CC#39 drift grows from 2 to 3. Do **not** touch CC#39's counting
     logic. Document the growth in the row's note.
4. Drop `apply-checkpoint.json` next to this `proposal.md` and
   `tasks.md`.

Commit: `chore(rec-c1.8): planning artifacts (C1.8.0)` on
`feat/rec-c1.8-authoritative-evidence-handoff`.

## C1.8.1 — Doc reconciliation

Single commit, doc-only.

1. `docs/.../ROADMAP_CONTROL_PLANE.md` line 30 (the C1.7 status block):
   - Replace `(9ee74f09)` with `(merge 6190390d; tag object 5f603bff peels
     to 6190390d)`.
   - Add a one-line note explaining the SDD convention: annotated tags
     peel to a tag object whose `objecttype=tag` points at the merge
     commit; cycle artifacts reference the merge commit, not the tag
     object SHA.
2. `reconstruction-contracts.toml`: add a `[[gate_history]]` entry (or
   whichever section captures closed gates; matches the existing schema)
   recording:
   ```
   REC-C1.7 = closed on tag rec-c1-7-projection-authority-acceptance
              (tag object 5f603bff, merge 6190390d)
   REC-C1.8 = active
   ```
   If no such section exists, do **not** invent one. Skip this step and
   record the decision in the apply-checkpoint `notes` instead.
3. No code changes.

Commit: `chore(rec-c1.8): reconcile C1.7 tag reference in control plane
(9ee74f09 → 6190390d)` on the cycle branch.

## C1.8.2 — flip dual_truth RED → GREEN

Depends on **C1.8.2.a decision** (default: W / wrapper).

### Resolution W (default)

1. Delete `crates/chronos-services/src/dual_truth_characterization.rs`.
2. Remove its `mod dual_truth_characterization;` declaration from
   `crates/chronos-services/src/lib.rs`.
3. New file `chronos-sandbox/tests/rec_c1_8_dual_truth_closeout.rs`:
   - Test 1: `wrapper_rebuilds_engine_from_log_when_map_is_empty` —
     seed a `SessionExecutionLog` with N=10 records (deterministic
     seq/event_id/timestamp), spawn MCP, call
     `execution_query{kind=ExecutionSummary}` over the wire, assert
     `summary.total_events == 10`. The wrapper must build the engine
     from the log on first query.
   - Test 2: `wrapper_rebuilds_engine_after_late_append` — same setup,
     call `execution_query` (engine built from first 3 records in the
     wrapper cache), append 3 more, call `execution_query` again, assert
     `summary.total_events == 6`. The wrapper must invalidate and rebuild
     the engine on the late append, OR re-project incrementally and
     preserve the projection-meta invariants.
   - Test 3: `wrapper_handles_state_query_against_log_only` — seed log
     with M=5 records, call `state_query` over the wire (no engine map
     build), assert response is session-known and not `SessionNotFound`.
4. Update the C1.7 `rec_c1_7_projection_restart_equivalence.rs`
   doc-comment to note these are C1.8 closeout regressions.
5. Confirm `cargo test -p chronos-services --lib` reports 0 failed.
6. Confirm `cargo clippy --workspace --all-targets -- -D warnings` clean.

Commit: `test(rec-c1.8): wrapper-side dual-truth closeout (3
black-box integration tests via MCP wire)` on the cycle branch.

### Resolution S (NOT default — only if user picks this)

Pause and re-scope. C1.8 becomes A-lite-with-fork or A-full. The cycle
does not start C1.8.2 until the user explicitly selects S and confirms
the architectural scope.

## C1.8.3 — UAT-REC-C1-01 exact: 10,000 durable records on the real wire

New file `chronos-sandbox/tests/rec_c1_8_uat_c1_01_exact.rs`:

1. Seed a `SessionExecutionLog` with **10,000** deterministic records
   (seq 0..10_000, payload encoded `TraceEvent`, `event_id` deliberately
   ≠ seq, `monotonic_ns` deliberately ≠ both).
2. Spawn MCP over that durable root.
3. Consumer A calls `events_read{limit=100}`, captures cursor `c_a`,
   asserts response contains 100 records.
4. Consumer B calls `events_read{limit=100}`, captures cursor `c_b`,
   asserts response contains 100 records.
5. Assert: `c_a != c_b`, no record identity appears in both responses.
6. Append 1,000 more records to the log via the registry (simulating
   producer advance while MCP is live).
7. A resumes with `c_a`; B resumes with `c_b`. Each gets 100 records
   disjoint from their own prior reads and disjoint from each other.
8. Re-read A's first 100 records with `c_a`; assert identical response
   (cursor non-destructive).

Commit: `test(rec-c1.8): UAT-REC-C1-01 exact — 10k records, two
consumers, producer advance, A resumes` on the cycle branch.

## C1.8.4 — UAT-REC-C1-03 exact: forced gap on the real wire

New file `chronos-sandbox/tests/rec_c1_8_uat_c1_03_forced_gap.rs`:

1. Seed a `SessionExecutionLog` with records at seq 0..99, no records at
   seq 100..199 (gap), records at seq 200..299.
2. Spawn MCP.
3. Call `events_read` with a range spanning the gap (e.g. from seq 50
   with limit 200).
4. Assert: `completeness.status == "gap_detected"` and `gap_summary`
   carries the exact gap range (whatever the wire shape is, captured
   from `events_log_read::completeness_for`).
5. Assert: `completeness.status` is **never** `"complete"` in this
   response, and is not `"unknown"` either (gap is provable from
   evidence per TRUTH-003).

Commit: `test(rec-c1.8): UAT-REC-C1-03 forced gap on the wire — never
Complete` on the cycle branch.

The C1.7 test that demonstrates the negative (clean session → `Complete`)
is annotated as *"clean-session negative of UAT-C1-03"* and retained
unchanged for documentation.

## C1.8.5 — UAT-REC-C1-05 exact

Depends on **C1.8.5 option** decision (default: A).

### Option A (default)

1. `crates/chronos-log/src/record.rs`: add to `ExecutionRecord`:
   ```rust
   /// Optional wall-clock capture timestamp in nanoseconds since the
   /// Unix epoch. Producers MAY fill this when they have access to a
   /// wall clock at capture time; they MAY leave it `None` when they
   /// do not (sandboxed, offline, or simply not yet wired).
   ///
   /// Distinct from `monotonic_ns` (which is session-relative). Do NOT
   /// use this field as a substitute for `monotonic_ns`; both must be
   /// readable independently.
   #[serde(default, skip_serializing_if = "Option::is_none")]
   pub captured_at_unix_ns: Option<u64>,
   ```
2. `crates/chronos-log/src/record.rs`: add the same `Option<u64>` field
   to `NewExecutionRecord` so producers can populate it.
3. Update every `NewExecutionRecord { ... }` literal across the
   workspace to include `captured_at_unix_ns: None` (preserves
   behavior). Or, if `NewExecutionRecord` uses `..Default::default()`,
   ensure the default is `None` and update the struct to `#[derive(Default)]`
   or a manual `impl Default`.
4. New file `crates/chronos-log/tests/execution_record_v2_capture.rs`
   or extension of an existing tests module:
   - Build a fixture of records with deliberately uncorrelated
     `(seq, event_id, monotonic_ns, captured_at_unix_ns)` values.
   - Round-trip via `serde_json` (v2 with field, v1 without).
   - Assert: all four dimensions pairwise independent.

5. Update `reconstruction-contracts.toml` TRUTH-001 evidence array to
   cite the new test file (it does not change the requirement status;
   TRUTH-001 stays `verified`).

Commit: `feat(rec-c1.8): ExecutionRecord.captured_at_unix_ns + 4-dim
independence UAT (UAT-REC-C1-05)` on the cycle branch.

### Option B (NOT default — only if user picks this)

1. New ADR
   `docs/.../reconstruction/adr/0006-c1-05-time-semantics-amendment.md`:
   - Status: accepted.
   - Context: UAT-REC-C1-05 currently requires wall-clock/Unix
     validation; `ExecutionRecord` does not carry a wall-clock field.
   - Decision: amend UAT-REC-C1-05 to *"Session-relative monotonic
     time is provably independent of `EventSeq` and `event_id`"*;
     move wall-clock validation to a future UAT-REC-C4-NN.
   - Consequences: CONN-001 stays `partial`, REC-C4 owns the
     wall-clock dimension.
2. `docs/.../roadmap/MILESTONE_ACCEPTANCE.md`: amend UAT-REC-C1-05
   wording per the ADR.
3. The C1.7 unit test
   `build_engine_preserves_seq_event_id_timestamp_ns_as_independent_dimensions`
   is renamed (no content change) to reflect the new wording.
4. No code change to `ExecutionRecord`.

Commit: `docs(rec-c1.8): amend UAT-REC-C1-05 wording + ADR 0006
(wall-clock deferred to REC-C4)` on the cycle branch.

## C1.8.6 — Mechanical handoff

1. Re-run every `verify = ...` command listed under TRUTH-001/002/003
   and LOG-001/002 in `reconstruction-contracts.toml`. All must be
   GREEN. Append any new test paths to the `evidence` arrays.
2. LEGACY-001 / LEGACY-002 (`status = "gap"`, `owner_gate = "REC-C2"`):
   ratify by adding a `notes` field on each (if not already there)
   stating they remain C2-owned and that C1.8 closes the handoff without
   requiring them to be filled.
3. `ROADMAP_CONTROL_PLANE.md`:
   - `ACTIVE PRODUCT GATE` becomes `REC-C2 (legacy deletion)`.
   - Add a `REC-C1` closure clause to `STATUS`.
   - Move `REC-C1.8 handoff` from `BLOCKED / NEXT` to `DONE`.
   - Add `REC-C2 legacy deletion` to `BLOCKED / NEXT` with its
     declared sub-gates (LEGACY-001, LEGACY-002).
4. `cycles/index.md`: the C1.8 row flips from ACTIVE → CLOSED on merge.
5. `apply-checkpoint.json`: final CLOSED state.

Commit: `chore(rec-c1.8): mechanical handoff — REC-C1 CLOSED, REC-C2
ACTIVE` on the cycle branch.

## Closure

Same as C1.7 closure convention:

- T0 fmt + clippy -D warnings.
- T3 split: workspace excluding sandbox/e2e + `chronos-native --lib
  --test-threads=1` per AGENTS.md §6.5.
- T4-smoke: 5/5 (e2e_connectivity + 4 new wire tests).
- Merge --no-ff, push annotated tag `rec-c1-8-authoritative-evidence-handoff`.
- `archive_status = "ready"` (rec-c1 stream convention; no
  `.sddk-knowledge/changes/archive/rec-c1-8-*/` directory).
- CC#39 grows by 1 (pre-existing, NOT masked).
