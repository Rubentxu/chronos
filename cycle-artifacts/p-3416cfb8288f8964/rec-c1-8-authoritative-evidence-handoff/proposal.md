# REC-C1.8 — Authoritative Evidence Handoff (proposal)

**Cycle**: `p-3416cfb8288f8964/rec-c1-8-authoritative-evidence-handoff`
**Branch**: `feat/rec-c1.8-authoritative-evidence-handoff`
**Path**: A-lite
**Base**: `main` at `20b222df` (REC-C1.7 CLOSED on tag `rec-c1-7-projection-authority-acceptance`; tag object `5f603bff` peels to merge `6190390d`)
**WIP**: 1 — research slot empty

## Why this cycle exists

REC-C1.7 was tagged CLOSED but three acceptance discrepancies remain on `main`
that prevent a clean REC-C1 handoff. The user surfaced them after verifying
remote state and the canonical contracts:

### Discrepancy 1 — documental truth has drifted

- `reconstruction-contracts.toml` line 3 still says
  `active_gate = "REC-C1.7"` after C1.7 is closed.
- `ROADMAP_CONTROL_PLANE.md` line 30 says the C1.7 tag peels to commit
  `9ee74f09`. The actual annotated tag `rec-c1-7-projection-authority-acceptance`
  peels to tag object `5f603bff` whose `objecttype=tag` points at merge commit
  `6190390d`; `9ee74f09` is the pre-merge chore commit. The control plane is
  documenting the wrong SHA.

### Discrepancy 2 — `dual_truth_characterization.rs` is still RED on `main`

The file's doc-comment (lines 9–11) literally states:

> "After C1.7.2 lands … these tests flip GREEN."

They did not. Three `#[tokio::test]`s still fail under
`cargo test -p chronos-services --lib`, masked by a file-level
`#![allow(unused_imports, dead_code)]` plus a hand-waved clippy exception.
"348 PASS / 3 RED by design" is not equivalent to a GREEN gate. The
rec-c1-7-projection-authority-acceptance tag message ("Tests proving the closure
(all GREEN)" followed by "348 passed / 3 RED") is a direct contradiction and
the **No Silent Lies** principle requires it to be closed before the handoff.

### Discrepancy 3 — UAT-REC-C1-01, C1-03 and C1-05 are not literally satisfied

`docs/.../MILESTONE_ACCEPTANCE.md` specifies:

| UAT | Literal spec | What C1.7 actually did |
|---|---|---|
| C1-01 | "Create 10,000 ordered records. A reads 100; B reads independently; producers advance; A resumes." | Used `test_busyloop` (~64 records), asserted `>= 10` floor |
| C1-03 | "Force bounded retention loss. The read crossing the lost interval returns an explicit gap/incomplete state and MUST NOT report `complete`." | Third test asserts the *negative*: clean session returns `Complete` (opposite of what C1-03 requires) |
| C1-05 | "Wall-clock/Unix timestamps are independently validated." | Proves 3 session-relative dimensions are independent. `ExecutionRecord` has no wall-clock field at all |

UAT-C1-02 and C1-04 were never exercised against the wire either. C1.7
shipped the projection builder; the literal acceptance text was not enforced.

## What this cycle is

A **handoff-and-close** cycle. No new architecture, no behavioral rework, no
legacy deletion. The architectural decisions of REC-C1 are already on `main`:
`chronos_services::projection::build_engine` is canonical; the
MCP-wrapper gate refuses `Truncated`/`Empty` projections; `events_read` is the
canonical read path. C1.8 enforces what was promised.

### Sub-deliverables

#### C1.8.0 — planning commit (no production code)

Single reconciling commit on the new branch:

- `reconstruction-contracts.toml`: `active_gate = "REC-C1.8"`; document C1.7
  closure with its real tag (object `5f603bff`, peel `6190390d`).
- `ROADMAP_CONTROL_PLANE.md`: bump `ACTIVE PRODUCT GATE` to REC-C1.8; correct
  the C1.7 peel reference from `9ee74f09` to `6190390d` with explanatory note
  about the tag/merge convention.
- `.sddk-knowledge/.../cycles/index.md`: append the C1.8 row (`ACTIVE`); bump
  `Total cycles` from 101 to 102. CC#39 grows from 2 to 3 — still pre-existing,
  still NOT masked.
- `cycle-artifacts/p-3416cfb8288f8964/rec-c1-8-authoritative-evidence-handoff/`:
  this `proposal.md`, `tasks.md`, `apply-checkpoint.json`.

#### C1.8.1 — Doc reconciliation (extends C1.8.0 if split)

Independent commit if C1.8.0 grew too large: same files, separate from the
planning commit, same content. The point is "doc matches reality".

#### C1.8.2 — Flip `dual_truth_characterization.rs` RED → GREEN

**Decision required (C1.8.2.a):** where does "rebuild engine from log" live?

The C1.7 design put it in the MCP wrapper. As a consequence, the services
**in isolation** do not read from the durable log; the three RED tests, which
construct a service `Context` directly, cannot flip GREEN without either
re-introducing the divergence the gate closes or moving the responsibility
into the service.

Two resolutions are available; the cycle's direction depends on user choice:

- **C1.8.2.W (wrapper, recommended):** keep the rebuild in the wrapper.
  Rewrite the three tests as **black-box integration tests** that drive the
  MCP wire (`chronos-sandbox/tests/rec_c1_8_dual_truth_closeout.rs`), seeded
  with a `SessionExecutionLog` and a real MCP spawn. Each test becomes a
  regression proving the *current* production behavior:
  1. log holds N records, engine map empty → `execution_query{kind=ExecutionSummary}`
     over the wire returns `summary.total_events == N` (not `SessionNotFound`).
  2. engine map built from a 3-record snapshot, then 3 records appended → next
     `execution_query` returns 6, not 3.
  3. log holds records, `state_query` over the wire returns a session-known
     payload (not `SessionNotFound`).
  File-level `#![allow(unused_imports, dead_code)]` removed. The
  `dual_truth_characterization.rs` file is deleted; its tests live as
  black-box regressions in the sandbox. Doc header rewritten to say:
  *"These integration tests prove the divergence is closed by the
  wrapper-side gate."*

- **C1.8.2.S (service, NOT recommended):** move the rebuild responsibility
  into `chronos-services` so the service can answer from the log without
  the wrapper. This is **architectural change** — out of scope for C1.8
  A-lite. Would require re-classifying the cycle as A-lite-with-fork or
  A-full.

**C1.8.2 defaults to the W resolution.** If the user selects S, the cycle
pauses and is re-scoped.

#### C1.8.3 — UAT-REC-C1-01 exact: 10,000 durable records on the real wire

New sandbox test file `chronos-sandbox/tests/rec_c1_8_uat_c1_01_exact.rs`:

- Seed a `SessionExecutionLog` with **10,000** deterministically generated
  records (`seq 0..10_000`, payload encoded `TraceEvent`, `event_id` and
  `monotonic_ns` deliberately uncorrelated).
- Spawn the MCP server over that durable root (no live probe needed — the
  acceptance is about reads, not producer behaviour).
- Consumer A calls `events_read{limit=100}`, captures cursor `c_a`.
- Consumer B calls `events_read{limit=100}`, captures cursor `c_b`.
- Assert: `c_a != c_b`, no record identity appears in both responses.
- Append 1,000 more records to the log (simulating producer advance).
- A resumes with `c_a`; B resumes with `c_b`. Both reads return records
  disjoint from each other and disjoint from their own prior reads.
- Assert: every consumer's view respects cursor non-destructivity
  (re-reading with the same cursor returns the same records).

The current C1.7 test (`rec_c1_7_uat_c1_01_two_consumers.rs`) asserts the
*floor* case. The new test asserts the literal spec. Both can coexist;
the C1.7 file's doc-comment is updated to note it is a fast smoke, the
C1.8 file is the literal acceptance.

#### C1.8.4 — UAT-REC-C1-03 exact: forced gap on the real wire

New sandbox test file `chronos-sandbox/tests/rec_c1_8_uat_c1_03_forced_gap.rs`:

- Seed a `SessionExecutionLog` with records at `seq 0..99`, then a gap
  (no records) at `seq 100..199`, then records at `seq 200..299`.
- Spawn MCP, call `events_read` with a range that spans the gap.
- Assert: response carries `completeness.status == "gap_detected"` and an
  exact gap range (`gap_summary.range == [100, 200)` or whatever the wire
  shape is — verified against the existing `CompletenessReport`).
- Assert: `completeness.status` is **never** `"complete"` for any read
  that crosses the gap.

The C1.7 third test asserts the opposite (clean session → `Complete`).
The new test asserts the literal spec; the C1.7 test is annotated as
*"clean-session negative of C1.03"* and retained for documentation.

#### C1.8.5 — UAT-REC-C1-05 exact: wall-clock dimension

**Decision required (C1.8.5 option):** A or B.

- **Option A (recommended):** add `captured_at_unix_ns: Option<u64>` to
  `ExecutionRecord` (`crates/chronos-log/src/record.rs`), with
  `#[serde(default, skip_serializing_if = "Option::is_none")]`. Existing v1
  records without the field deserialize unchanged (matches the pattern of
  `invocation_id`/`parent_invocation_id`/`symbol_id`). No producer is
  required to populate it. The UAT becomes a 4-dimension independence test:
  `seq`, `event_id`, `monotonic_ns`, `captured_at_unix_ns`. The new
  unit test asserts pairwise non-correlation on uncorrelated fixtures.
  `CONN-001` (owner_gate REC-C4) advances from `partial` toward
  `verified`-of-the-dimension; remaining CONN work stays in REC-C4.

- **Option B (NOT recommended):** amend
  `docs/.../MILESTONE_ACCEPTANCE.md` UAT-REC-C1-05 wording from
  *"Wall-clock/Unix timestamps are independently validated"* to
  *"Session-relative monotonic time is provably independent of `EventSeq`
  and `event_id`"*. Move "wall-clock validated" to a future
  UAT-REC-C4-NN. Add an ADR (`docs/.../reconstruction/adr/0006-c1-05-time-semantics-amendment.md`)
  recording the deferral. No code change to `ExecutionRecord`. `CONN-001`
  unchanged at `partial`.

**C1.8.5 defaults to Option A.** If the user selects B, the cycle includes
the doc amend + ADR instead of the `ExecutionRecord` change.

#### C1.8.6 — Mechanical handoff

- `reconstruction-contracts.toml`: TRUTH-001/002/003 and LOG-001/002
  `verify` commands re-run; all must be GREEN. Evidence arrays updated to
  cite the C1.8 wire tests (C1-01 exact, C1-03 forced gap, C1-05 four-dim).
- LEGACY-001 / LEGACY-002 (`status = "gap"`, `owner_gate = "REC-C2"`)
  explicitly re-classified as the canonical handoff envelope: the C1.7
  cycle left them as `gap`; C1.8 ratifies that they remain C2-owned and
  that no part of C1's canonical surface depends on them being filled
  before the handoff.
- `ROADMAP_CONTROL_PLANE.md`:
  - `ACTIVE PRODUCT GATE` becomes `REC-C2 (legacy deletion)` after C1.8
    closure.
  - `STATUS` declares `REC-C1 CLOSED on tag rec-c1-8-…`.
  - `REC-C1.8 handoff` moves from `BLOCKED / NEXT` to `DONE`.
  - `REC-C2` moves to `BLOCKED / NEXT` with sub-gates declared if known.

## What this cycle is NOT

- **Not REC-C2.** C1.8 does not delete `EventBus`, `fired_buffer`,
  `drain_raw_events`, `TripwireFired`, dual-write paths, or any shim. Those
  removals are REC-C2's job, gated by the C1.8 handoff. C1.8 only
  *classifies* the remaining legacy paths as LEGACY-001/002 owned by C2.
- **Not a hexagonal inversion.** We do not introduce an
  `ExecutionLogProvider` port or refactor `chronos-services` away from
  `SessionExecutionLog`. The C3 boundary closure owns that work; C1.8
  exercises the projection as it stands.
- **Not a wall-clock rewrite.** C1.8.5 Option A adds an *optional*
  `captured_at_unix_ns` field. No producer is required to fill it; no
  reader is required to interpret it as authoritative. Option B (if
  chosen) is a documentation amendment.
- **Not CC#39's fix.** The pre-existing drift between `cycles/index.md`
  `Total cycles` and CC#39's filesystem count grows by 1 with C1.8's own
  index entry. Still pre-existing, still NOT masked.
- **Not a wall-clock authority claim.** Even with Option A,
  `captured_at_unix_ns` is "captured at", not "occurred at". Producers
  that fill it from a wall clock are free to do so; producers that
  cannot (sandboxed, offline) leave it `None`. This is documented in the
  field's doc-comment and in the ADR for Option B.

## Position in the REC-C1 sequence

```text
C1.0..C1.4     restart/cursor/gap semantics, events_read canonical
C1.5           retention + restart UATs, bootstrap, durable delete/seal
C1.6           lifecycle-safe delete + retention/tail facts on the wire
C1.7           QueryEngine becomes a projection of ExecutionLog
               + UAT-REC-C1-01 / C1-05 (fast smoke variant)
               + TRUTH-001 partial → verified
C1.8 (this)    handoff + acceptance closure:
                 - flip dual_truth RED → GREEN
                 - UAT-C1-01 exact (10k records)
                 - UAT-C1-03 forced gap on the wire
                 - UAT-C1-05 four-dim (or wording amend + ADR)
                 - ratify LEGACY-001/002 as C2-owned
                 - REC-C1 CLOSED, REC-C2 ACTIVE
REC-C2         legacy deletion (after C1.8 handoff)
```

## Tier required

`T3 + T4-smoke`. We touch projection plumbing indirectly (C1.8.2 rewrites
the dual-truth tests at the wrapper surface), add new wire tests
(C1.8.3, C1.8.4, C1.8.5), and add an optional schema field (Option A).
Per AGENTS.md §2, T4-smoke is mandatory before merge even on A-min when
the cycle changes MCP plumbing — and C1.8.2/3/4/5 all touch the wire.

T4-smoke subset:

- `e2e_connectivity` (regression baseline)
- `rec_c1_8_dual_truth_closeout` (C1.8.2, replaces RED characterization)
- `rec_c1_8_uat_c1_01_exact` (C1.8.3)
- `rec_c1_8_uat_c1_03_forced_gap` (C1.8.4)
- `rec_c1_8_uat_c1_05_four_dim` or `rec_c1_8_uat_c1_05_unit` (C1.8.5)

## Closure criterion (mechanical)

- `cargo fmt --all -- --check` clean.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- T3: `cargo test --workspace --lib --tests --exclude chronos-sandbox --exclude chronos-e2e --no-fail-fast` + `cargo test -p chronos-native --lib -- --test-threads=1`. **Critical sub-criterion:** `cargo test -p chronos-services --lib` must report 0 failed. The 3 dual_truth RED tests are gone.
- T4-smoke: 5/5 above.
- TRUTH-001/002/003 and LOG-001/002 `verify` commands green.
- LEGACY-001/002 ratify as REC-C2-owned (no status change required; they
  were already `gap` with `owner_gate = "REC-C2"`).
- `ROADMAP_CONTROL_PLANE.md` reflects REC-C1 CLOSED + REC-C2 ACTIVE.
- `cycles/index.md` C1.8 row transitions ACTIVE → CLOSED.

## Risks / unknowns

- **C1.8.3 producer advance.** The literal spec says *"producers advance"*.
  The C1.7 cycle had no live probe in C1.8.3's path; we simulate producer
  advance by appending to the durable log via the registry while MCP is
  live. If MCP holds an open log handle that doesn't observe late appends,
  the test fails and we have to either restart the read after a tail poll
  or add a notification hook. Investigated during C1.8.3 implementation.
- **C1.8.4 wire shape.** `CompletenessReport` already exists per
  `reconstruction-contracts.toml` TRUTH-003 evidence. The exact JSON shape
  of `gap_summary` is read from `crates/chronos-services/src/events_log_read.rs`
  during C1.8.4 implementation. If the shape has changed since C1.7, the
  test adapts; no contract change.
- **C1.8.5 Option A schema migration.** Adding
  `captured_at_unix_ns: Option<u64>` is a v2 record variant. v1 records
  continue to deserialize (serde default). New writes that omit the field
  produce v1-shape records. Backward compatible. Forward writes that
  populate it require updating every `NewExecutionRecord` callsite to
  accept the field; C1.8.5 makes the field optional and lets each producer
  decide when to fill it.
- **CC#39 growth.** C1.8's index entry grows the drift from 2 to 3.
  Pre-existing, intentional, surfaced. No masking.
