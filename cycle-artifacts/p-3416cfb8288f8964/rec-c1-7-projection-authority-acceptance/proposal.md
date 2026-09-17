# REC-C1.7 — Projection Authority + final REC-C1 acceptance (proposal)

**Cycle**: `p-3416cfb8288f8964/rec-c1-7-projection-authority-acceptance`
**Branch**: `feat/rec-c1.7-projection-authority-acceptance`
**Path**: A-lite
**Base**: `main` at `e4af63b6` (REC-C1.6 CLOSED on `83ee38e2`)
**WIP**: 1 — research slot empty

## Why this cycle exists

The REC-C1 ledger entry `TRUTH-001` is still `status = "partial"` with this
honest note:

> "chronos-log exists and native can dual-write, but EventBus-only and
> QueryEngine-authoritative paths remain."

The user prompt for this cycle put it plainly:

```
ExecutionLog
   │
   ├── events_read       ← verdad canónica ✅
   │
   └── restart/recovery  ← verdad canónica ✅

EventBus
   │
   └── stop/snapshot
          ↓
      QueryEngine map
          ↓
 execution_query
 state_query
 trace_slice
```

That second tree is exactly the partial claim. `ProbeService::stop()` still
drains from the backend (`crates/chronos-services/src/probe.rs:452,
crates/chronos-services/src/probe.rs:608`), `session_snapshot()` does the
same (`crates/chronos-services/src/probe.rs:608`), the MCP wrapper stores the
resulting `Vec<TraceEvent>` into a `QueryEngine` map
(`crates/chronos-mcp/src/server.rs:110` + `:2140`), and three operations
(`execution_query`, `state_query`, `trace_slice`) read straight from that map.

Until today those reads have no authoritative backing: the engine is whatever
the drain happened to return at stop time. Anything the probe captured *before*
the stop, anything that lived through a restart, anything the durable
ExecutionLog retained — none of it is consulted when `execution_query`
answers.

**C1.7 makes `QueryEngine` a projection of the ExecutionLog, not a second
authority.** After C1.7 the only thing that knows what really happened is the
log; the engine is recomputable from the log and consistent with `events_read`
by construction.

The other two C1 acceptance items the user named — **C1-01** (10k records,
two independent consumers) and **C1-05** (time semantics: `EventSeq ≠
timestamp_ns ≠ event_id`) — are part of this cycle's acceptance because they
were never exercised against the real MCP wire and C1.7 is the last cycle
that touches projection plumbing before the C1.8 handoff.

## What this cycle is NOT

- **Not REC-C2.** C1.7 does not delete `EventBus`, `fired_buffer`,
  `drain_raw_events`, `TripwireFired → EventBus`, dual-write paths, or any
  shim. Those moves are C2's job, gated by the C1.8 handoff. C1.7 may
  introduce the *projection builder* and the *single canonical write path*
  for the engine map; it must not delete the legacy tree.
- **Not a hexagonal inversion.** We do not introduce an
  `ExecutionLogProvider` port or refactor `chronos-services` away from
  `SessionExecutionLog`. The C3 boundary closure owns that work; C1.7
  builds the projection on top of `SessionExecutionLog` as it stands.
- **Not a time-semantics rewrite.** C1-05 only proves the model is
  honest; we do not change `TraceEvent::timestamp_ns`'s semantics in
  this cycle.
- **Not CC#39's fix.** The pre-existing 1-cycle drift between
  `cycles/index.md` Total and CC#39's filesystem count is a governance
  debt correctly classified as pre-existing and intentionally surfaced.
  A separate `gov-*` cycle owns it.

## Position in the REC-C1 sequence

```
C1.0..C1.4      restart/cursor/gap semantics, events_read canonical
C1.5            retention + restart UATs, bootstrap, durable delete/seal
C1.6            lifecycle-safe delete + retention/tail facts on the wire
C1.7 (this)     QueryEngine becomes a projection of ExecutionLog
                + public UATs for C1-01 and C1-05
                + TRUTH-001 partial → verified
C1.8            handoff only: freeze C1 receipts, classify remaining
                EventBus paths as LEGACY owned by C2, declare
                REC-C1 CLOSED + REC-C2 ACTIVE
```

## Sub-deliverables

### C1.7.0 — reconciliation (planning commit, no production code)

Single reconciling commit on the new branch:

- `docs/chronos-agentic-reconstruction/docs/reconstruction/ROADMAP_CONTROL_PLANE.md`:
  bump `STATUS` and `ACTIVE PRODUCT GATE` to REC-C1.7; remove the
  frozen-at-C1.5 framing.
- `reconstruction-contracts.toml`: `active_gate = "REC-C1.7"`,
  `updated = "2026-09-17"`.
- `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md`: append the C1.7
  row. This bumps `Total cycles` to 101, growing the pre-existing CC#39
  drift from 1 to 2 — it is still pre-existing, intentionally
  surfaced, NOT masked.
- `cycle-artifacts/p-3416cfb8288f8964/rec-c1-7-projection-authority-acceptance/`:
  proposal, design, tasks, apply-checkpoint.

This is the only commit that touches docs/ outside cycle-specific
artifacts. It is the first commit of the cycle.

### C1.7.1 — characterization of dual truth (RED first)

Before touching any projection code, write a test that demonstrates the
problem the user described. The test must be **non-implementing**: it must
fail on `main` because the dual path is still there.

Two complementary characterizations, both RED on `main`:

- **CHAR-DUAL-1 (services-level).** In a single session:
  1. Append records {A, B, C} to the `SessionExecutionLog`.
  2. Call `ProbeService::stop`-equivalent path that drains from the
     backend and builds a `QueryEngine`.
  3. Append record {D} to the ExecutionLog *after* the engine was built.
  4. Query the engine via `execution_query` / `state_query` / `trace_slice`.
  5. Assert: the engine returns {A, B, C} only, missing D, even though
     the ExecutionLog holds A..D.

  This is the divergence: the engine and the durable log disagree.

- **CHAR-DUAL-2 (MCP-real).** Same divergence, but driven through the
  actual MCP wire (server.rs handlers). The assertion is on the
  `state_query` JSON response, not on internal types.

Both tests get skipped only when C1.7.2 lands (GREEN). Until then they
prove the problem exists and that fixing it is non-cosmetic.

### C1.7.2 — Projection Builder (GREEN for C1.7.1)

A single canonical builder, `chronos_services::projection::build_engine`,
with this contract:

```rust
pub fn build_engine(
    log: &SessionExecutionLog,
) -> Result<ProjectionResult, ServiceError>;
```

Internally it:

1. Reads records from the log via `SessionExecutionLog::handle()` in
   seq order (the same `read_page_with` plumbing used by `events_read`,
   minus filters/limit — we want every record that exists today).
2. Decodes each `ExecutionRecord` to `TraceEvent` via the existing
   `decode()` helper in `events_log_read.rs` (already
   JSON-`TraceEvent`-aware; this is the single shared decoder).
3. Applies the same noisy-event filter `build_and_store_engine`
   currently applies (`!Custom/Registers`, `!Unknown`).
4. Pushes through the existing `IndexBuilder` + `QueryEngine::with_indices`.
5. Returns a `ProjectionResult { engine, projection_meta }`.

The MCP `build_and_store_engine` call sites stay — they just route
through this builder. Two of the three current callers (`probe.stop` and
`session_snapshot`) hand the builder a `SessionExecutionLog` directly
instead of `Vec<TraceEvent>` drained from the backend. The third caller
(after a freshly-appended event) re-projects incrementally — bounded
growth of the engine without breaking completeness (rebuild-from-log
remains the source of truth).

### C1.7.3 — projection position / provenance / retention honesty

The projection must know four facts about itself:

```rust
pub struct ProjectionMeta {
    pub session_id: SessionId,
    pub projected_from: EventSeq,   // first seq examined
    pub projected_through: EventSeq, // last seq examined
    pub source: ProjectionSource,    // always ExecutionLog
    pub completeness: ProjectionCompleteness,
}

pub enum ProjectionCompleteness {
    /// log is fully readable from `retained_from` to `tail_seq`;
    /// engine matches the log verbatim.
    Full,
    /// `retained_from > 0`: history was retired before the log we
    /// could read. The engine is *only* the surviving tail — querying
    /// for "all events" would silently answer from a partial history.
    Truncated { retained_from: EventSeq },
    /// No records present (empty session).
    Empty,
}
```

The rule, matching what the user proposed:

```text
query requires full history
retained_from > 0
→ ServiceError::EvidenceUnavailableDueToRetention { retained_from }
```

(variant already exists in `chronos_services::error`). This is the
*only* way `execution_query` / `state_query` / `trace_slice` learn about
truncation. We do not invent a partial-result-without-envelope mode —
absence of envelope would be a lie.

The three operations gain `ProjectionCompleteness` on their responses so
the MCP wire can show the agent the truth, not a synthetic "everything
is fine". This is the explicit version of the "no fake engine" rule.

### C1.7.4 — restart equivalence UAT

End-to-end acceptance:

1. Create a session, append 1,000 records via the canonical producer
   (the same one already used by `session_start` events).
2. Run `execution_query`, `state_query`, `trace_slice`; capture JSON
   responses.
3. Stop the probe cleanly.
4. **Restart Chronos from scratch** (close the MCP server, drop the
   in-memory `engines` map, reopen `bootstrap` for the session).
5. Re-run the same three queries.
6. Assert: response bodies are semantically equal — same record
   identities, same `event_id`s, same `timestamp_ns`s, same gap
   information, same completeness.

The invariant this proves is the user's strongest one:

> Delete the in-memory `QueryEngine` projection
> → Nothing authoritative is lost
> → Rebuild it from ExecutionLog
> → Canonical queries return the same answers

If after restart `events_read` works but `execution_query` returns
`SessionNotFound`, REC-C1 is not actually closed. C1.7.4 is the test
that proves it.

### C1.7.5 — C1-01 public, on the real wire

10,000 records, two independent consumers via `events_read` (real MCP,
not unit tests), pause/resume, producer advance. The forced-gap fixture
from C1.4 is re-used here:

```text
A cursor != B cursor
no read steals data
gap traverses range → GapDetected
Complete never appears over the gap
```

This is the UAT-MILESTONE_ACCEPTANCE.md `UAT-REC-C1-01` spec,
exercised against the production wire.

### C1.7.6 — C1-05 time semantics

The contract being proved:

```text
EventSeq != timestamp_ns
event_id != timestamp_ns
monotonic/session-relative != wall-clock Unix
```

A fixture uses deliberately uncorrelated values:

```text
seq         0, 1, 2, …
event_id    40, 90, 130, …
timestamp   10_000_500, 25_320_700, 25_999_001, …
```

Encoded into the ExecutionLog, then read via `events_read`, the
`execution_query` projection, and `trace_slice`. Assertions:

- `record.seq != record.event_id` for every record (the model
  intentionally distinguishes them).
- `record.timestamp_ns` is *not* a Unix-epoch wall-clock value (the
  fixture uses values that would never appear from a real Unix
  clock, e.g. session-relative ns).
- No encoder/projection substitutes one for another; if any
  intermediate value collapsed them, the round-trip would surface
  the substitution.

### C1.7.7 — TRUTH-001 ratchet

Once the three canonical operations derive from ExecutionLog via
`build_engine`, the ledger entry moves:

```text
TRUTH-001
ExecutionLog is authoritative for every agentic session
status = partial → verified
```

with UAT and command verifiable. The note in
`reconstruction-contracts.toml` becomes:

> "C1.7: QueryEngine is a projection of SessionExecutionLog.
>  `chronos_services::projection::build_engine` is the single canonical
>  builder; CHAR-DUAL-1 and CHAR-DUAL-2 (now GREEN) demonstrate the
>  removed divergence; restart equivalence UAT (C1.7.4) proves
>  canonical queries survive an in-memory wipe. EventBus / fired_buffer
>  / drain_raw_events / TripwireFired remain as legacy paths owned by
>  REC-C2; their deletion is out of scope here."

Verify command (added under `verify = [...]`):

```bash
cargo test -p chronos-services --lib projection
cargo test -p chronos-sandbox --test projection_restart_equivalence
```

## Tier required

`T3 + T4-smoke`. We touch projection plumbing, three canonical
operations, and the MCP wire for `execution_query` /
`state_query` / `trace_slice`. Per AGENTS.md §2, T4-smoke is
mandatory before merge even on A-min when the cycle changes MCP
plumbing — and this cycle does, in three operations.

T4-smoke subset:

- `projection_restart_equivalence` (C1.7.4) — new suite, this cycle's
  strongest acceptance.
- `dual_truth_characterization` (C1.7.1) — flips RED → GREEN.
- `time_semantics_uncorrelated` (C1.7.6) — new.
- `e2e_connectivity` — regression baseline.

## Closure criterion (mechanical)

- `cargo fmt --all -- --check` clean.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- T0+T3 zero new failures.
- T4-smoke 4/4 (the four suites above).
- TRUTH-001 transitions `partial → verified` with a verify command
  that runs green.
- Restart-equivalence UAT proves canonical queries survive an
  in-memory wipe.
- `cycles/index.md` reflects C1.7 closure.

## Risks / unknowns

- The `QueryEngine` API (`with_indices`, `with_causality`,
  `with_performance`, `merge`) is large. C1.7.2 may discover it has
  invariants not expressible from `Vec<TraceEvent>` alone (e.g.
  incremental refresh correctness). If so, the builder may need a
  `rebuild` mode distinct from `merge`. Either way, the canonical
  invariant is `log-rebuild = engine-after-rebuild`.
- Some existing `state_query` operations touch `DebugTraceService`
  (memory, registers) which are address-driven. These do not depend
  on event identity and are unaffected by C1.7's projection change
  — the engine map is still required for them, just built from the
  log. C1.7 must not regress them.
- The `trace_slice` test corpus may depend on internal event ids
  produced by `drain_raw_events`; the projection must preserve
  `event_id` exactly. Covered by the round-trip assertion in C1.7.4.
