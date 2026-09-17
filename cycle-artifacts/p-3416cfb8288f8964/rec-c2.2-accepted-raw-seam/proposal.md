# REC-C2.2 — accepted-Raw seam (proposal)

**Cycle**: `rec-c2.2-accepted-raw-seam`
**Route**: A-lite
**Base**: `main@e4fd938c` (REC-C2.1 CLOSED)
**Closes**: FIND-C2.1-03

## The seam

```text
TraceEvent
   -> accept_raw()                      (persist, return the authoritative seq)
   -> AcceptedRaw { source_seq, event }
   -> AcceptanceSeam.observer           (application policy: services)
   -> live fan-out
```

`chronos-native` captures and persists; `services` decides what a firing
means. The dependency direction stays `services -> native`, never the reverse.

`AcceptanceSeam { log, observer }` is threaded once into both probe loops, so
spawn and attach cannot drift into two producers with different rules.

## Already done in this step (C2.2.0)

- `accept_raw()` split out of the old `dual_push()`; `dual_push` renamed to
  `accept_and_publish`, because the old name described the dual-write shape
  REC-C2 is retiring (ratchet: `dual_push` 3 -> 0).
- `AcceptedRawObserver` type + `with_accepted_raw_observer` builder.
- `services::ProbeService::accepted_raw_observer` builds the derivation hook.
- Spawn AND attach both go through the seam (C2.2.1 pulled forward: attach
  previously published Raw with no ExecutionLog at all).
- Productive UAT: real probe -> real captured events -> production derivation
  -> durable firing -> restart -> same `firing_seq` / `source_seq`.

## Remaining in REC-C2.2

| Step | Work |
|---|---|
| C2.2.2 | `probe_drain` reads canonical evidence, is not a destructive authority |
| C2.2.3 | `probe_stop` / `session_snapshot` consumers off EventBus truth |
| C2.2.4 | remaining CANONICAL destructive reads -> 0 (currently 6) |
| C2.2.5 | UAT-C2-01/02/03 pre-close characterization |

Rule for C2.2.2: `probe_drain` may keep serving a live view and the
`tripwires_fired` live statistic, but it must not be the mechanism that
creates durable firing evidence. That is decided at the accepted-Raw seam.
