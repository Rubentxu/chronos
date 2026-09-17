# M1 — Gap segment accounting repair (proposal)

**Cycle**: `m1-gap-segment-accounting-repair`
**Route**: A-min (one root cause, `chronos-log` + one sandbox UAT)
**Base**: `main@4e0a1344` (REC-C1.8 CLOSED)
**Owner gate**: pre-REC-C2 confidence repair

## Why

REC-C1.8 surfaced **FIND-C1.8-01**: a durable segment that contains a
`Gap` cannot be reopened. The public UAT
`uat_rec_c1_03_forced_gap_reports_gap_detected` is `#[ignore]`-marked
because of it. REC-C2 is about to make `ExecutionLog` the *only*
authoritative evidence store (TripwireFired projected as log evidence,
canonical consumers off EventBus). Retiring EventBus while the durable
log can reject its own gap-bearing segments would be building C2 on a
foundation we already know is cracked.

This cycle is a **precondition of trust**, not a new evolution.

## Characterization (measured, not assumed)

The segment payload is a sequence of `SegmentEntry` items; each is either
an `ExecutionRecord` or a `Gap`. The 96-byte header carries a `u64` count
at byte offset 32. The two sides disagree on the unit:

| Side | Quantity | Source |
|---|---|---|
| writer | every persisted `SegmentEntry` (records **and** gaps) | `segmented.rs::flush_inner` → `buffer.len()` |
| replay | `ExecutionRecord` entries only | `replay.rs::build_replay_plan` → `records` |

Reproduced in `crates/chronos-log/tests/m1_gap_segment_accounting.rs`:

```
2 records, gap 2..=5, 1 record   →   4 entries
header declares 4                →   replay counts 3
reopen → ReplayIntegrity { RecordCountMismatch { declared: 4, actual: 3 } }
```

The writer has *always* emitted the entry count, so the on-disk bytes are
already correct under the entry interpretation. Only the validator (and
the field's name) are wrong. There is therefore **no data migration**:
fixing the validator makes existing gap-bearing segments valid again.

## Decision

One unit for all three: **the persisted `SegmentEntry` count**.

- Rename the header field `record_count` → `entry_count` (byte layout
  unchanged; offset 32 still holds a `u64`).
- Writer: unchanged quantity (`buffer.len()`), now named honestly.
- Replay: count **every** entry (records + gaps) and compare to
  `entry_count`.
- Format doc updated to state that the count is of `SegmentEntry` items,
  and that a `Gap` occupies exactly one entry while spanning a seq range.

Rationale for renaming rather than redefining: the field must not keep a
name (`record_count`) that means something else. After this cycle a
reader can trust the name.

## Deliverables

1. Characterization test (already written, RED): pins the arithmetic.
2. Fix: rename + replay counts entries + docs.
3. `GAP-PERSIST-1..6` unit tests (GREEN).
4. `uat_rec_c1_03_forced_gap_reports_gap_detected` loses `#[ignore]` and
   passes on the real MCP wire (the C1.8 acceptance sentence "the read
   crossing the lost interval returns an explicit gap/incomplete state
   and MUST NOT report 'complete'" becomes literally satisfied).

## Non-goals

- No change to gap semantics, seq allocation, retention, compaction, or
  cursor behaviour.
- No EventBus / REC-C2 work.
- CC#18 and CC#39 stay in their `gov-*` slot; not touched here.

## Acceptance

- `cargo test -p chronos-log --test m1_gap_segment_accounting` all GREEN.
- `cargo test -p chronos-sandbox --test rec_c1_8_uat_c1_03_forced_gap`
  (no ignored) GREEN with `CHRONOS_MCP_PATH` set.
- T0 fmt + clippy `-D warnings`; T3 split; T4-smoke.
