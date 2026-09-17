# REC-C2.1 — tasks

**Cycle**: `rec-c2.1-tripwire-evidence`
**Companion**: `proposal.md`

| Step | State | Notes |
|---|---|---|
| C2.1.0 planning | DONE | proposal.md + this file |
| C2.1.1 `ExecutionKind::TripwireFired` + `TripwireFiredEvidence` | DONE | appended last (Raw=0/GapMarker=1 pinned by test); payload codec; `NewExecutionRecord.kind` plumbed (backends previously hardcoded `Raw`) |
| C2.1.2 persist-first source acceptance | DONE | `dual_push` appends first, publishes only on success; no-log path stays COMPATIBILITY; attach-loop path untouched (COMPATIBILITY, C2.2) |
| C2.1.3 derive from accepted Raw + recursion barrier | DONE | `tripwire_evidence::{derive_firings_from_record, derive_firings_from_event, read_firings}`; barrier dispatches on `ExecutionKind`; failures reported, never a `Gap` |
| C2.1.4a async boundary + `resolve_session` | **DONE** | `observe` is async; explicit scope wins; `active_session` fallback; guard dropped before any I/O; 4 direct tests incl. lock-release; 25 call sites / 20 tests migrated |
| C2.1.4b log-backed firing reads + `EventsCursorV1` | **PENDING** | groundwork (`read_firings_page`, `firing_counts`) already landed |
| C2.1.5 `fire_count` derived; `retained_until_session_end` honest | **PENDING** | depends on C2.1.4 |
| C2.1.6 remove `fired_buffer` | **PENDING** | depends on C2.1.5 |
| C2.1.7 real-process restart UAT + ratchet update | **PENDING** | depends on C2.1.4-6 |

## C2.1.4a note

`observe` is async now; `create`/`delete`/`query` stay synchronous. The test
harness dropped `with_ctx` for `async fn observe(&self, input)` because an
`FnOnce -> R` closure cannot contain `.await`.

## Superseded: why C2.1.4 was originally blocked

`observe` is a **synchronous** service function:

```rust
pub fn observe(ctx: &ObserveContext<'_>, input: ObserveInput) -> Result<ObserveOutput, ServiceError>
```

To read firings from the session's ExecutionLog it needs the active session id,
which lives behind a `tokio::sync::Mutex` (`ProbeContext::active_session`). A
sync function cannot lock it. Two honest options, both larger than a
one-commit change:

1. make `observe` async (ripples through the MCP handler and ~40 tests), or
2. add a synchronous handle to the active session's log to the context
   (a second source of the active session id, which is exactly the kind of
   duplicated identity REC-C1.2a spent effort removing).

Option 1 is preferred; it deserves its own commit and a full test pass.

## What is already true after C2.1.0-3

- firings have a typed durable representation with `source_seq`;
- a firing can only derive from an `ExecutionLog`-accepted `Raw` record;
- the recursion barrier is by `ExecutionKind`, not by payload tag;
- the producer no longer observes (bus or semantic fan-out) anything the log
  refused;
- a failed derived append is surfaced, not hidden, and never becomes a `Gap`.

## Still missing from the DoD

- `observe list` still drains the legacy `fired_buffer` (FIND-C2.0-01),
  `fire_count` is still the stale mutable counter (FIND-C2.0-02);
- two consumers cannot yet read firings independently over the wire;
- the restart UAT asserts nothing yet for firings.

## Ratchet

`fired_buffer` 4 -> 4 (removal is C2.1.6). CANONICAL destructive reads 11 -> 11
(`drain_fired` is the C2.1.4/6 target). The inventory dropped 37 -> 36 because a
string-literal false positive was corrected — the ratchet demanded the shrink.
