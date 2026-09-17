# REC-C1.6 — lifecycle-safe delete + retention/tail facts on wire (proposal)

**Cycle**: `p-3416cfb8288f8964/rec-c1-6-lifecycle-retention-wire`
**Branch**: `feat/rec-c1.6-lifecycle-retention-wire`
**Path**: A-lite
**Base**: `main` at `5bbf7748` (REC-C1.5 CLOSED)
**WIP**: 1 — research slot empty

## Why this cycle exists

REC-C1.5 left two gaps that the user-approved plan explicitly kept
out of scope:

1. **Lifecycle-safe delete.** A destructive operation (`delete_session`)
   must not silently reach into a live probe's writer and tear down
   state. C1.5 proved deletion works against a cleanly stopped session;
   it did not assert anything about a still-live one.
2. **Retention and tail facts on the wire.** The internal log already
   carries `retained_from`, `tail_state`, and the read path distinguishes
   "no event", "retired", "lost", and "tail uncertain". The wire does
   not yet expose these in a way the agent can read directly — today
   the agent has to infer the state from errors.

C1.6 closes both. After it, REC-C1's remaining acceptance gates (C1-01,
C1-05) can be exercised end-to-end against the production wire.

## What this cycle is NOT

- It is **not** the start of REC-C2. The control plane still blocks
  EventBus / fired_buffer / TripwireFired / dual-writes removal behind
  the REC-C1.8 handoff. This cycle does not touch any of those.
- It is **not** a "retention policy API". If no authoritative policy
  source exists in the code, we do not invent the name. Only the facts
  that already exist (`retained_from`, `tail.state`, `tail.tail_seq`,
  `history_truncated`) become visible.

## WIP / drift fix (first commit, C1.6.0)

Single reconciling commit:

- `docs/ROADMAP.md`: REC-C0 → CLOSED (it was closed at REC-C1.5), REC-C1.5 → CLOSED, REC-C1.6 → ACTIVE, REC-C2 → BLOCKED until REC-C1.8 handoff, research slot → EMPTY.
- `reconstruction-contracts.toml`: `active_gate = "REC-C1.6"`, `updated = "2026-09-17"`.
- `cycle-artifacts/p-3416cfb8288f8964/cycles/index.md` (if it indexes these): recompute rows.

This is the only commit that touches `docs/` outside of cycle-specific
documents. It is not split into a separate cycle; the first commit of
C1.6 carries it.

## First sub-deliverable: lifecycle-safe `delete_session`

### Behavior

```text
delete_session(target)
├─ target ∈ live_probes (probe writer is attached)
│  → Err(SessionStillActive { session_id, action: "stop the probe first" })
│  → NO store delete, NO registry remove, NO directory removal
│  → probe still alive, evidence still readable, dir untouched
└─ target ∉ live_probes (or recovered unclean without a live writer)
   → existing durable-delete path runs
   → Deletion succeeds OR is refused for a different reason
     (not-found, manifest read failure, etc.) — but never
     because the probe is live
```

### Where the liveness check lives

- **Source of truth:** `ChronosServer::live_probes: HashMap<session_id, LiveProbeSession>`.
- The check goes into `SessionsService::delete_session` only as a
  **precondition passed by the caller** in `SessionsContext`. The
  service never auto-stops a probe (that would be a silent
  lifecycle transition — exactly what the user wants to forbid).

### Typed error

```rust
#[error("session '{session_id}' is still active ({hint}); stop the probe before deletion")]
SessionStillActive { session_id: String, hint: &'static str },
```

The wire envelope mirrors the existing pattern (`isError: true`,
`content[0].text = "session '…' is still active (…); stop the probe before deletion"`).
No new error category — same surface as `SessionNotFound`, `DeleteFailed`,
etc.

### UATs

| ID | Scenario | Pass criterion |
|----|----------|----------------|
| DEL-LIVE-1 | `probe_start → delete_session(live)` → typed refusal; evidence still readable, dir intact, probe still alive | typed refusal text contains session id and the action hint; `events_read` returns the same first page that was readable before; `<root>/<session_id>/execution-log.manifest.json` still exists with `tail_state != Sealed`; `live_probes` still contains the session |
| DEL-LIVE-2 | `probe_start → delete_session` (refused) → `probe_stop` (seal) → `delete_session` → restart → session absent | store has no row; durable directory removed; bootstrap rediscovery does not resurrect |
| DEL-LIVE-3 | unclean-recovered session, no live writer (simulate by killing the MCP process mid-run, restarting cleanly, NOT auto-resuming the probe) → `delete_session` allowed | succeeds; durable dir removed; restart sees nothing |
| DEL-LIVE-4 | while session A's probe is live, `delete_session(B)` (B is a separate, cleanly stopped session) | A's manifest, tail, evidence, registry entry all byte-identical before and after (no side effect on a sibling) |

The first two run as a `restart_uat` extension (`tests/lifecycle_delete.rs`);
the last two are unit-level (services + a small MCP integration).

### Out of scope for C1.6 (explicit)

- Auto-stop on delete. The refusal is permanent until the user (or another tool) calls `session_stop`.
- Background sessions (`background_sessions` map). If the user runs `delete_session` while the session is also in `background_sessions` but NOT in `live_probes`, current behavior stands. We may add `background_sessions` to the liveness check in a future cycle if real UATs demand it; for C1.6, we only refuse on live-probe writers.
- `drop_session`. That is the in-memory-only path and already returns idempotently. We do not change it.

## Second sub-deliverable: retention/tail facts on the wire

### Today (event_log_read → events_read surface)

| Surface | Field today |
|---------|-------------|
| `LogReadPage` | `records`, `next`, `position_after`, `gaps`, `completeness` |
| Wire envelope (e.g. `events_read` Query success) | the above five fields, plus session_id, returned_count, total_matching |
| `CompletenessReport` | `status`, `scope="examined_range"`, `from_seq`, `to_seq_exclusive` |
| `ServiceError::CursorStale` | `requested_next_seq`, `retained_from_seq` |

Not on the wire today: `retained_from_seq`, `history_truncated`,
`tail.state`, `tail.tail_seq`, `policy`.

### What C1.6 adds

```jsonc
{
  "events": [ ... ],
  "returned_count": 17,
  "total_matching": 17,
  "next_cursor": "...",
  "position_after": 17,
  "gaps": [],
  "completeness": { ... },
  "retention": {
    "retained_from_seq": 500,
    "history_truncated": true
  },
  "tail": {
    "state": "sealed",          // "open" | "sealed" | "unclean" | "unknown"
    "tail_seq": 9999            // null when state == "unknown" and we genuinely don't know
  }
  // "policy" is intentionally absent. There is no authoritative policy
  // source yet. We will NOT invent one.
}
```

Notes on the design choices:

- `tail.state` mirrors the four internal lifecycle states; the wire
  vocabulary matches the manifest JSON. A future C1.8 may add
  `policy` once a real policy source exists.
- `tail.tail_seq` is `null` when state is `unknown` AND we cannot
  infer it. Today the path always has a `tail_seq` once a log is
  registered (the segmented log carries it), so null is rare. We still
  allow null because the contract is "facts, not inferred values".
- `retention.history_truncated = (retained_from_seq > 0)`. This is the
  "events were retired" signal. The agent can still read `retained_from_seq`
  directly — the boolean is a convenience for tools that only want to
  answer "could the absence be retention, vs never-observed?".

### CursorStale on the wire

Today the wire serializes `CursorStale` as a flat string. C1.6 makes
it a structured response:

```jsonc
{
  "error": "cursor_stale",
  "requested_next_seq": 0,
  "retained_from_seq": 5,
  "message": "cursor at seq 0 is before the retention boundary 5"
}
```

The existing `isError: true, content[0].text = "…"` envelope can stay
(the `message` field is the same text), but the wire tools that need
to consume the numbers can now read them without a regex on the error
text. The MCP tool handler parses the `ServiceError::CursorStale`
payload and exposes its two fields as a sibling JSON object inside
the error envelope.

### UATs

| ID | Scenario | Pass criterion |
|----|----------|----------------|
| WIRE-RET-1 | `events_read` success path returns `retention.retained_from_seq` and `retention.history_truncated` matching the manifest | both present; numbers match the manifest on disk |
| WIRE-RET-2 | `events_read` on a session whose `retained_from == 0` returns `history_truncated: false` | boolean correct |
| WIRE-TAIL-1 | `events_read` returns `tail.state == "sealed"` and `tail.tail_seq == N` for a cleanly stopped session | both present and correct |
| WIRE-TAIL-2 | `events_read` returns `tail.state == "open"` while the probe is alive | state correct, tail_seq advances as more events are written |
| WIRE-TAIL-3 | `events_read` on an `unclean` recovered session returns `tail.state == "unclean"` | state correct, tail_seq equals the recovered tail position |
| WIRE-TAIL-4 | `events_read` returns `tail.state == "unknown"` only when the registry returned `Unavailable` and the manifest cannot be parsed | (covered by the existing `ExecutionLogUnavailable` mapping) |
| WIRE-CURSOR-1 | `CursorStale` envelope contains both numbers | `requested_next_seq` and `retained_from_seq` present in the parsed JSON |

### Out of scope

- "Policy" name. Not invented.
- `session_list` extensions. C1.6 does not add per-session retention to the list envelope; the per-`events_read` fact is enough for the agent's reasoning.
- `CompletenessReport::scope` widening. Today it is `"examined_range"`; that is the only authoritative scope and we keep it that way.

## Steps (proposed commit decomposition)

1. **C1.6.0 reconciliation (single commit):** ROADMAP + reconstruction-contracts.toml + cycles/index.md.
2. `feat(rec-c1.6): refuse delete_session on live probe with typed SessionStillActive`. Sources: `SessionsService::delete_session`, `ServiceError::SessionStillActive`, MCP tool handler.
3. `test(rec-c1.6): DEL-LIVE-1/2/4 in tests/lifecycle_delete.rs`, plus a unit test for DEL-LIVE-3 (unclean-recovered deletable).
4. `feat(rec-c1.6): expose retention.retained_from_seq + history_truncated in events_read success envelope`. `LogReadPage` gains a `retention_facts: RetentionFacts` field; the MCP envelope flattens it.
5. `feat(rec-c1.6): expose tail.state + tail.tail_seq in events_read success envelope`. New `TailFacts` field on `LogReadPage`; flat on the wire.
6. `test(rec-c1.6): WIRE-RET-1/2 + WIRE-TAIL-1/2/3 in tests/wire_retention_facts.rs`.
7. `feat(rec-c1.6): CursorStale envelope carries structured requested_next_seq + retained_from_seq`.
8. `test(rec-c1.6): WIRE-CURSOR-1`.
9. `chore(rec-c1.6): full regression — T0 fmt+clippy, T1 lib, T2/T3 per-crate, T4-smoke (lifecycle_delete + wire_retention_facts + restart_uat + e2e_connectivity)`.
10. `chore(rec-c1.6): ledger closure (apply-checkpoint + verify-findings + implementation-receipt + merge --no-ff into main + tag + push)`.

Steps 2..3 close first sub-deliverable; 4..8 close second. Step 9 is
their shared regression gate. Step 10 ships.

## Acceptance for "ready to close"

- All DEL-LIVE-* UATs pass.
- All WIRE-* UATs pass.
- Existing `restart_uat` R1..R4 still pass — lifecycle_delete should
  not perturb them.
- T0/T1/T3/T4-smoke all green.
- Vault drift sweep PASS (CC#4 cascade regenerated; CC#56 still clean).
- `sddk cycle verify-references --cycle rec-c1-6-lifecycle-retention-wire` PASSED.
