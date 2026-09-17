# REC-C2.2 — tasks

**Companion**: `proposal.md`

| Step | State |
|---|---|
| C2.2.0 accepted-Raw seam + FIND-C2.1-03 | DONE |
| C2.2.1 spawn/attach producer convergence | DONE (attach now uses the same seam) |
| C2.2.2 `probe_drain` not a destructive authority | DONE | canonical reader + DRAIN-1..7 + explicit ResolveContext + wire switchover on `ecv1`; completeness converged on the C1 model; decode is fail-closed |
| C2.2.3 `probe_stop`/`session_snapshot` consumers off EventBus | DONE | both read `canonical_drain::read_all_raw_events`; STOP-1..4 + `rec_c2_2_canonical_consumers.rs` (tiny ring, repeatable snapshot) |
| C2.2.4 remaining CANONICAL destructive reads -> 0 | DONE | `ProbeBackend::drain_events`/`drain_raw_events` REMOVED; ratchet CANONICAL=0 and `--strict` (REC-C2 close mode) PASSES |
| C2.2.5 UAT-C2-01/02/03 characterization | DONE | identifiers were never defined anywhere; defined here from the cycle's laws and executed end-to-end against the real MCP server |

## Evidence

- `accept_and_publish` ordering test (`probe_backend::tests::c2_2_*`): the
  observer runs AFTER the record is durable, BEFORE the event is fanned out,
  and never runs when the append is refused.
- `chronos-sandbox/tests/rec_c2_2_producer_derivation.rs`: the productive UAT.
  NEGATIVE assertion included: if the observer is unwired, it fails loudly
  instead of passing on hand-written fixtures.
- Ratchet: 31 -> 28 (`dual_push` 3 -> 0). CANONICAL destructive reads
  unchanged at 6; those are C2.2.2-4.

## C2.2.2 in progress

Landed: `NativeProbeBackend::project_semantic(&TraceEvent) -> SemanticEvent`, a
pure projection that never touches the EventBus (test:
`c2_2_projecting_semantics_reads_no_bus`). It is the piece `probe_drain` needs
to build its wire events from durable `Raw` records.

Remaining (measured blast radius, not yet changed):

- `ProbeService::drain` reads `EventBus::read_since` and recomputes
  `tripwires_fired` with `matching_semantic`. Target: one scan of the
  session's `ExecutionLog`, `Raw -> project_semantic`, `TripwireFired ->
  tripwires_fired += 1`, `GapMarker -> explicit completeness`, and
  `TripwireManager` dropped from the drain path entirely.
- Cursor: `EventsCursorV1` directly (same laws as events_read/observe:
  foreign malformed/stale are typed errors, never a silent re-anchor). The
  legacy ring cursor (`total_pushed`/`snapshot_len`) is NOT translated; if
  the old wire shape is kept it stays a separate COMPATIBILITY field.
- Page boundary: the cursor advances **after the last ExecutionRecord
  examined**, and a `Raw` plus its immediately following `TripwireFired`
  records are one logical read unit, so a page can never end with
  `tripwires_fired = 2` and zero source events.
- Wire change: `ProbeDrainInput.cursor` / `ProbeDrainResult.new_cursor`
  move from the ring cursor to the opaque `ecv1` token; the MCP handler and
  every sandbox test that touches `probe_drain` move with them (this is the
  blast radius that made it a separate commit).

Mandatory tests for the step:
- UAT-REC-C2-01 on this surface: drain the EventBus (or perturb it) with the
  log intact -> same events, same `tripwires_fired`, same cursor.
- Double-evaluation killer: log holds `Raw S` + `TripwireFired F(source=S)`
  and the `TripwireManager` is EMPTY -> the Raw event is visible and
  `tripwires_fired == 1`.

## C2.2.2 second increment

Landed:
- `project_semantic(event, ctx: &ResolveContext)` — the caller supplies the
  session's resolution context, so replaying the same durable `Raw` produces
  the same projection the producer produced. `resolve_context(binary_path)`
  exposes the backend's own context. (Rebuilding the context from ambient
  state was a real divergence risk: live capture uses
  `binary_path: Some(program_path)`, the old projector used `None`.)
- `crates/chronos-services/src/canonical_drain.rs` — the canonical reader:
  one scan over the `ExecutionLog`, `Raw -> project_semantic`,
  `TripwireFired -> tripwires_fired += 1`, gap -> explicit completeness.
  **No `TripwireManager` parameter and no EventBus.**
- Six DRAIN properties + the falsification, all green:
  DRAIN-2 evidence-not-configuration, DRAIN-3 Raw+derived as one logical unit
  (`max_raw=1` over `Raw0 F1 F2 Raw3` -> Raw + 2 firings + cursor at 3),
  DRAIN-4 a cursor inside a derived cluster is a typed `InvalidInput` (never a
  re-anchor), DRAIN-5 early stop still progresses, DRAIN-6 a traversed gap
  contaminates the page, and the falsification that bus perturbation cannot
  change the output.
- Budgets: `max_raw_events` + `max_examined_records` + `max_derived_per_source`
  (a pathological cluster fails explicitly instead of making the unit
  unbounded).

Remaining (the wire switchover, deliberately its own commit):
- `ProbeService::drain` calls the reader and drops `matching_semantic`;
  `TripwireManager` leaves the signature entirely.
- `ProbeDrainInput`/`ProbeDrainResult` stop using `EventCursor`/`CursorDto`
  for the canonical route: `evidence_cursor: "ecv1:..."` is CANONICAL, and any
  surviving ring cursor is a separate, explicitly-named COMPATIBILITY field.
  The ring cursor is NEVER converted into an EventSeq.
- `cursor_stale: bool` is replaced (on the canonical route) by the
  completeness vocabulary: Complete / GapDetected / Partial.
- The MCP handler and every sandbox test touching `probe_drain` move with it.


## C2.2.4 — the destructive read API is gone

`ProbeBackend` no longer declares `drain_events` or `drain_raw_events`. Both
were destructive reads of an adapter-owned buffer, so neither could ever be the
authoritative source, and a shared backend handed one consumer the power to
empty the buffer another consumer still needed.

Removed from the trait and from `NativeProbeBackend`, `EbpfAdapter`,
`MockEbpfAdapter` and `BrowserAdapter`. Where a browser-local consuming read is
still genuinely wanted (`browser_probe_drain` is documented as consuming), it
survives as an inherent, browser-local `take_semantic_events`, not as a method
on the shared trait. `BrowserAdapter::raw_events()` is the non-destructive read
the stop path now uses.

Side effect worth recording: the ebpf mock's semantic read used to hardcode
`source_event_id: 0`. Ids are now assigned over the whole snapshot, so a paged
read keeps the same identity for the same event.

### Ratchet

CANONICAL 6 -> 0, tracked total 28 -> 21. Every step was a real disappearance:
the ratchet reported each entry as a stale waiver before it was removed, and
`--strict` (the REC-C2 close gate) now passes.

### Characterization retired

`CHAR-C2-06` asserted that `drain_raw_events()` consumed the bus across
consumers. The behaviour it characterized no longer exists, so the test was
rewritten to assert the invariant that outlives it: publishing twice must leave
both observations readable, because a read path must not empty a shared
transport. Recorded here rather than deleted silently.

### FIND-C2.2-04 (new, no action in this cycle)

Browser sessions have NO `ExecutionLog`. `chronos-browser` does not depend on
`chronos-log`, and `BrowserProbeContext` carries no log registry, so browser
evidence has no durable home: the adapter buffer is bounded and drops the oldest
events at capacity. C2.2.4 removed the destructive read and stopped the stop
path from destroying evidence, but "ExecutionLog owns occurrence" is still not
true for browser captures. That needs the browser capture loop to append through
an accepted-Raw seam, which is its own cycle.


## C2.2.5 — the pre-close characterization

`proposal.md` and `tasks.md` both name UAT-C2-01/02/03 as this cycle's final
step, and nothing in the repository ever defined them. They are therefore
DEFINED here, once, and then executed (`rec_c2_2_uat_c2.rs`, real MCP server):

```text
UAT-C2-01  probe_drain is not an authority and does not create evidence
UAT-C2-02  probe_stop / session_snapshot report the log and its completeness
UAT-C2-03  durable evidence exceeds the ring, so the ring is not the source
```

Each is falsifiable against the pre-cycle architecture. UAT-C2-01 in particular
adds a SECOND matching subscription after the first read and bounds the growth
of the firing count by the growth of the examined range; a live recomputation
over the returned range would inflate the historical prefix and break the
bound.

### FIND-C2.2-05 — a false green this step caught

`m0_04` had been passing *vacuously* since REC-C2.2.2. Two compounding mistakes,
both mine:

1. `tripwire_create` rejects `"SyscallEnter"`; the accepted spelling is
   `"syscall_enter"`. So the condition C2.2.2 introduced never created a
   subscription.
2. `m0_04`'s error branch for `tripwire_create` printed a diagnostic and
   `return`ed. With the subscription failing, the test exited before reaching
   its assertion and reported PASS.

The `.ok()` calls that swallowed the same error were present in
`probe_drain_canonical.rs` too. All of them now `expect`/`assert`, so a missing
subscription is a failure rather than a skip. This is worth naming: a green test
that cannot fail is worse than a red one.

Separately, `m0_04`'s post-hoc-subscription leg asked for EXACT firing-count
equality across two live reads of "the same" range. That is not a property of
the system: a live probe keeps appending, and the page's last `Raw` can gain its
trailing derived firings between two reads (measured 98 -> 100 with no
subscription-shaped cause). The property is real but needs a race-free
formulation, so it now lives in UAT-C2-01 (bounded growth) and
`probe_drain_canonical` (no inflation of a persisted range), and `m0_04` keeps
its own subject: a subscription that predates the capture fires on the canonical
flow and is visible through `probe_drain`.


## Verify gate result (sddk-verify, PASS)

Verdict **PASS**. Subject HEAD `585dfc45`, base `e4fd938c`, clean tree. Nine
deterministic commands ran on the verified HEAD, no caching and no flakes:

```text
cargo build --bin chronos-mcp                     exit 0
cargo fmt --all -- --check                        exit 0
cargo clippy --workspace --all-targets -D warnings exit 0
cargo test --workspace (excl sandbox/e2e/native)   exit 0
cargo test -p chronos-native --lib --test-threads=1 107 passed / 0 failed
cargo test -p chronos-sandbox (3 suites, serial)  9/9 passed, no server timeouts
check_legacy_evb.py                               PASS, 21 tracked, CANONICAL=0
check_legacy_evb.py --strict                      PASS
```

Vacuity hunt: no vacuous patterns in the four touched sandbox suites; the
`m0_04` false green is confirmed closed. `m0_acceptance.rs:318`'s `.ok()` is
anti-vacuous (the surrounding assertion REQUIRES the call to fail).

Explicit answers to the two questions put to it:

- **Does any production path still obtain evidence from the EventBus
  destructively? NO.** Zero production call sites for
  `read_since|snapshot*|drain_*`. Trait impls still exist but the canonical path
  does not invoke them.
- **Is "the destructive read is gone in the browser path" true or a rename?
  RECLASSIFICATION WITH MITIGATION.** `ProbeBackend::{drain_events,
  drain_raw_events}` are genuinely gone from the trait, and
  `BrowserProbeService::stop` now uses the non-destructive
  `BrowserAdapter::raw_events`. But `BrowserAdapter::take_semantic_events` still
  evicts `event_buffer`; it is an inherent browser-local method, documented as
  destructive, used only by `browser_probe_drain`. Honest because the browser
  still has no `ExecutionLog` (FIND-C2.2-04, named and deferred). The ratchet's
  CANONICAL=0 is not achieved by that reclassification: the tracked tokens
  (`drain_raw_events`, `snapshot_raw`, `snapshot`) genuinely disappeared.

### FIND-C2.2-06 — a destructive read inside a read path (fixed)

Raised by verify. `EbpfAdapter::read_since` fell back to
`inner.drain_events()` when no cursor was supplied, i.e. it EVICTED the BPF
ring from inside a method the trait documents as "Non-destructive". Same class
as everything this cycle removes, and it was latent only because the `ebpf`
feature is off in the default build (verified: the feature does build here).

Fixed: it now returns a typed `CursorStale` refusal instead of consuming.
Nothing is lost, because `read_since` has no production caller left after
C2.2.2. Honest evidence level: the property is enforced by construction and
both feature configurations build and pass (default 30/30, `--features ebpf`
34/34), but a runtime assertion for it is impossible on this host — a real
`EbpfAdapter` needs BPF privileges, and a test that bails out when it cannot
construct one is exactly the vacuous pattern this cycle removed. Recorded as
runtime-unverified rather than dressed up.
