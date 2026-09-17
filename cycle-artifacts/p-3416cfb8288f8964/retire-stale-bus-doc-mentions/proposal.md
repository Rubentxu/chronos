# Retire stale EventBus doc-comment mentions

## Why now

REC-C2.3 already retired the EventBus as a runtime construct: the
`scripts/check_legacy_evb.py` ratchet is at `baseline_total=0` and `EventBus`
no longer compiles from `chronos-domain`. What survives the ratchet is
**doc-comment residue**: a small number of `//` and `///` lines that
describe behavior the production code no longer has. They live in three
flavors:

1. **Stale narration** — describes a removed behavior as if it still
   exists. Example: chronos-native `start_probe` doc says ptrace events
   "pushed to the EventBus in real-time".
2. **Stale public API wording** — describes a runtime model that no
   longer exists. Example: `chronos-mcp` `live_probes` field says events
   "stream to an EventBus ring buffer".
3. **Stale comparator / option list** — mentions `EventBus` in a
   comparison or option list where it no longer fits. Example:
   `chronos-log/memory.rs` benchmarks against "the existing EventBus";
   `chronos-native/capture_runner.rs` lists `EventBus` as one of several
   caller-pushable sinks.

None of these lines are wrong about the history; they are wrong about
the present. They mislead a reader who skims the docs to understand the
current seam.

## What is *not* touched

The cycle intentionally leaves in place:

- **All `// REC-C2.3:` inline markers** — these mark the seam where the
  bus used to be constructed. They are the archaeological trail of the
  removal and help future readers locate the change.
- **`probe_backend.rs::accept_and_publish` integration
  characterization comments** (lines 1158, 1210, 1303) — they document
  how property tests evolved across the boundary.
- **`mcp server.rs:5221` public tool description** — "instead of the
  legacy in-memory EventBus" is intentional UX copy that helps
  consumers migrate from history.
- **`output.rs:874`, `canonical_drain.rs:610`, `probe.rs:545-547`** —
  narrate the contract change (snapshot repeatability, drain
  non-destructiveness). Removing them would lose the rationale.
- **`events_cursor.rs:26`, `session_log.rs:38`** — scope notes for
  earlier cycles that briefly mention "no EventBus". Accurate history.
- **`canonical_drain.rs:16`** — "may transport the same view live, but
  it is not this reader's backing store". Accurate: with no bus
  existing, "may" is vacuously true and the negation is the load-bearing
  statement.

## Scope

Edit 5 doc-only sites across 4 production files. Each edit replaces a
description of *removed* behavior with a description of *current*
behavior, while preserving a short reference to REC-C2.3 so the
archaeological trail survives:

| Site | Change |
|---|---|
| `crates/chronos-mcp/src/server.rs:148-152` (`live_probes` field doc) | "stream to an `EventBus` ring buffer" → "stream into the session's durable `ExecutionLog` via the accepted-Raw seam (REC-C2.3 retired the parallel in-memory bus)" |
| `crates/chronos-native/src/probe_backend.rs:438-449` (`start_probe` doc) | "pushed to the `EventBus` in real-time" / "reach both `EventBus` and the attached `SegmentedExecutionLog` through the same `dual_push` seam" → record the accepted-Raw seam; drop the bus and dual_push nouns |
| `crates/chronos-native/src/probe_backend.rs:831-839` (function-capture branch inline comment) | "reach both EventBus and the attached SegmentedExecutionLog" → "reach the attached SegmentedExecutionLog (REC-C2.3 retired the EventBus half)" |
| `crates/chronos-native/src/capture_runner.rs:785-790` (`run_function_frame_capture_with_callback` doc) | "push to an `EventBus`, an `ExecutionLog`" → "push to an `ExecutionLog`" with note that the callback is generic and REC-C2.3 retired `EventBus` as a chronos-produced sink |
| `crates/chronos-log/src/memory.rs:1-13` (m1-01 module doc) | "benchmarked to scale to the same throughput as the existing `EventBus`" → drop the comparator with a marker note |

The cycle also removes two leftover `let backend_unused_eventbus_arg_removed = ();`
no-op statements in `chronos-sandbox/tests/m1_acceptance.rs` (lines 385
and 557) — these were residual stubs from when the test used to take a
bus argument and clippy now flags them as unused. Same flavor of cleanup
as the doc-comment edits.

## Out of scope

- The `REC-C2.3` cycle itself stays closed. This is a follow-on
  editorial pass, not new ratchet work.
- The `legacy-evb-inventory.json` ratchet is unchanged: the regex the
  script uses counts *production call-sites*, not doc comments.
- No Rust semantic change. `git diff --stat` should be < 30 lines across
  5 files plus 4 lines of test stub.

## Tier

B-direct (per AGENTS.md §2). Tier T0 + T1+T2 of touched crates. No
sandbox smoke (MCP wire surface unchanged).
