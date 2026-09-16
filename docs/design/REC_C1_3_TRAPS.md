# REC-C1.3 — Traps to close before declaring the cutover done

Written down during C1.2a, before any C1.3 code exists, so the cutover cannot
quietly inherit them.

## 1. `read_log_with_stats()` is NOT the canonical cursor reader

`chronos_native::read_log_with_stats(log, since, limit)`:

- uses a fixed consumer id `"m1-03-query"`,
- always calls `read_after(&consumer, None)` — i.e. **fresh**,
- then implements `since` by filtering records manually.

That is fine for compatibility/decoding, but it does not implement the
`EventsCursorV1` contract. Reusing it as the canonical reader would silently
give `events_read` fresh-read semantics with a filtered tail.

## 2. Off-by-one at the cursor bridge

The backend log API distinguishes:

```text
cursor(last_seq = None)  -> fresh, INCLUDES seq#0
cursor(last_seq = N)     -> returns seq > N
```

Therefore:

```text
EventsCursorV1.next_seq = 0   MUST map to fresh/None
                          NOT to last_seq = 0   (that would lose seq#0)

EventsCursorV1.next_seq = N>0 MUST map to last_seq = N - 1
```

and after delivering through `seq = M`:

```text
EventsCursorV1.next_seq = M + 1
```

Required tests before C1.3 is complete: `seq#0` is delivered exactly once; page
boundary is exact (no duplicate, no skip across two reads); resume from a stored
cursor continues exactly at `M + 1`.

## 3. Cursor position ≠ number of filter matches

```text
cursor position = position in the ExecutionLog
NOT
cursor position = count of records matching the filters
```

If ten matching events require scanning up to `seq=147`, the next cursor points
at `148`, not at `10`. Filters never enter the cursor.

## 4. Three things C1.3 must remove

The current code still does all three:

1. `EventsReadContext` depends on `QueryEngine`.
2. `events_read` calls `DebugTraceService`.
3. The wire cursor is still `CursorDto { total_pushed, snapshot_len }`.

## 5. `completeness` must stop being hardcoded

The response currently hardcodes `completeness: "complete"`. Until C1.4 proves
gap/completeness detection, the conservative answer is **`Unknown`**, never
`Complete`. A `Complete` that no gap knowledge backs is exactly the Silent Lie
this reconstruction exists to remove.

## 6. Target shape

```text
session-owned ExecutionLog
        │
        ▼
   EventSeq scan            (fresh vs last_seq bridge above)
        │
        ▼
   opaque EventsCursorV1
```

No EventBus. No QueryEngine. No offsets.
