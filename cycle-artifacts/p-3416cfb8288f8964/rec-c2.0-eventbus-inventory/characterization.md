# REC-C2.0 — EventBus legacy inventory + characterizations

**Cycle**: `rec-c2.0-eventbus-inventory`
**Route**: A-min (inventory + measurement; no behavior change)
**Base**: `main@063cbdf8` (M1 gap-segment repair CLOSED)
**Opens**: REC-C2

## Deliverables

1. A machine-readable inventory of every production EventBus / legacy-queue
   coupling: `legacy-evb-inventory.json` (37 entries).
2. A ratchet that enforces it: `scripts/check_legacy_evb.py`, wired into
   `scripts/check_architecture_contracts.py`.
3. Six characterizations that measure today's reality.

## The inventory

`legacy-evb-inventory.json` records one entry per production occurrence of a
protected token. The scan is deliberately narrow so the count is *productive*
coupling, not noise: only `crates/*/src/**/*.rs`, comments / string literals /
`#[cfg(test)]` regions / `#[test]` bodies / `mod tests` (even ungated) are
excluded, and `crates/*/tests/**` is never scanned.

Schema per entry:

```json
{
  "key": "<path>::<symbol>::<token>#<n>",
  "path": "...", "symbol": "...", "token": "event_bus_type",
  "kind": "destructive | queue | read | seam | type",
  "class": "CANONICAL | COMPATIBILITY | INTERNAL_LIVE",
  "owner": "REC-C2.1 | REC-C2.2 | REC-C2.3",
  "replacement": "...", "removal_gate": "REC-C2.x"
}
```

Current state (`baseline_total = 37`):

| token | count | kind |
|---|---|---|
| `event_bus_type` | 21 | type (bus coupling) |
| `fired_buffer` | 4 | queue |
| `drain_raw_events` | 4 | destructive |
| `dual_push` | 3 | seam |
| `read_since` | 2 | read |
| `snapshot` | 1 | destructive |
| `snapshot_raw` | 1 | destructive |
| `drain_fired` | 1 | destructive |

By class: **26 COMPATIBILITY, 11 CANONICAL**. The 11 CANONICAL entries are
exactly the destructive reads that must not survive REC-C2 close.

### Classes

- **CANONICAL** — an authoritative consumer still depends on it; must
  disappear before REC-C2 advances.
- **COMPATIBILITY** — survives temporarily with a waiver (e.g. the bus as a
  live mirror), removed by its `removal_gate`.
- **INTERNAL_LIVE** — live transport only, never authoritative evidence.

### The ratchet (four rules)

`scripts/check_legacy_evb.py` fails when:

1. a protected occurrence is not in the inventory (**new use**);
2. an inventory entry no longer exists in the code (**stale waiver**);
3. the total grows beyond `baseline_total` (**count may only shrink**);
4. under `--strict`, any `CANONICAL` entry with a `destructive` token
   remains (**REC-C2 close mode**).

Regenerate with `--write` (curated `class`/`owner`/`replacement` are preserved
for keys that are unchanged).

`python3 scripts/check_architecture_contracts.py --strict-legacy` currently
fails with 7 findings — the 7 CANONICAL destructive reads below. That is the
gate REC-C2 must clear, not a defect of this cycle.

## Characterizations (measured, not assumed)

All six are GREEN: they assert today's behavior, not the target behavior.

| # | Claim | Where | Measured |
|---|---|---|---|
| CHAR-C2-01 | log append failure is invisible once the bus published | `chronos-native/src/probe_backend.rs::char_c2_01_*` | sealed log ⇒ append fails, `bus.snapshot_raw()` still holds the event |
| CHAR-C2-02 | `observe(list)` consumes fired evidence globally | `chronos-services/src/observe.rs::char_c2_02_*` | 1st list `fired_count=1`, 2nd `=0` |
| CHAR-C2-03 | `retained_until_session_end` loses the evidence | `chronos-services/src/observe.rs::char_c2_03_*` | retained read returns 0 **and** the next drained read also returns 0 |
| CHAR-C2-04 | `fire_count` never reflects executions | `chronos-domain/src/tripwire.rs::char_c2_04_*` | 3 firings, `list()[0].fire_count == 0` |
| CHAR-C2-05 | `probe_drain`'s tripwire side-effect is not consumer-scoped | `chronos-domain/src/tripwire.rs::char_c2_05_*` | two evaluations of one event ⇒ two buffer entries; one drain empties for all |
| CHAR-C2-06 | `probe_stop` / `session_snapshot` read the bus destructively | `chronos-native/src/probe_backend.rs::char_c2_06_*` | 1st `drain_raw_events` = 2 events, 2nd = 0 |

Supporting reads (code, not tests):

- `dual_push` (`probe_backend.rs:288`) publishes first, appends best-effort
  (`debug!` on failure) — CHAR-C2-01's mechanism.
- `probe.rs:452` (`stop`) and `probe.rs:608` (`session_snapshot`) call
  `drain_raw_events()`; `browser_probe.rs:212` likewise.
- `probe.rs:500` (`drain`) uses `read_since` (non-destructive) but then calls
  `evaluate_semantic` per event (`probe.rs:526-535`), writing to the shared
  `fired_buffer` — CHAR-C2-05's cross-consumer coupling.
- `tripwires.rs:58` (`TripwiresService::list`) calls `manager.drain_fired()`
  unconditionally; `observe.rs:279` calls it before applying retention —
  CHAR-C2-02/03.
- `observe.rs:302` returns `[]` for `retained_until_session_end` after the
  drain has already happened.

## Two real defects found (not mere structural legacy)

1. **`retained_until_session_end` is a lie** (CHAR-C2-03): it does not retain;
   it destroys. C2.1 must either make it truly retained (consumer cursor) or
   reject it as `Unsupported`.
2. **`fire_count` is a parallel counter that never moves** (CHAR-C2-04): it
   reads 0 forever. C2.1 must derive the count from persisted evidence.

## Target for REC-C2.1 (recorded, not implemented here)

> **Persist first, derive second, fan-out last.** ExecutionLog owns
> occurrence; live channels only transport observations.

- Nothing is observed as having happened before its source event is accepted
  by ExecutionLog (`dual_push` inverted; EventBus may survive as a
  compatibility mirror).
- `TripwireFired` gets an explicit `ExecutionKind::TripwireFired` variant and
  a typed payload carrying `source_seq` (the firing must name the source
  evidence; `event_id` is not the authoritative identity and can be 0).
- The evaluator runs only on source event kinds — never on `TripwireFired`
  (no recursion); enforced by type/test, not by a payload-tag string.
- `observe list/query` reads firing evidence from the log/projection; a
  `retention=drained` view is emulated with a consumer cursor (evidence stays
  immutable, as proven in C1).

## Non-goals

- No removal of EventBus / dual_push / drain paths (that is C2.2/C2.3).
- No change to tripwire semantics, observe wire shape, or probe lifecycle.
- CC#18 / CC#39 remain in their `gov-*` slot.
