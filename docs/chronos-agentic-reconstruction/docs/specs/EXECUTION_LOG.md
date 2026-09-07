# Specification: ExecutionLog, persistence and projections

## Purpose

Replace shared destructive buffers with one authoritative execution history that can feed multiple agents, indexes, UI clients and persistence independently.

## Record

Illustrative shape:

```rust
struct ExecutionRecord {
    session_id: SessionId,
    seq: EventSeq,
    monotonic_ns: u64,
    wall_clock: Option<SystemTime>,
    process_id: ProcessId,
    thread_id: Option<ThreadId>,
    task_id: Option<TaskId>,
    invocation_id: Option<InvocationId>,
    parent_invocation_id: Option<InvocationId>,
    links: Vec<CausalLink>,
    symbol_id: Option<SymbolId>,
    source_location: Option<SourceLocation>,
    external_context: Option<ExternalContext>,
    kind: ExecutionKind,
    payload: ExecutionPayload,
    provenance: EvidenceProvenance,
    completeness: EvidenceCompleteness,
}
```

`monotonic_ns` means nanoseconds from the session monotonic epoch. Wall clock is separate.

## Sequence and cursor semantics

- `EventSeq` is strictly monotonic within one session.
- Upstream event IDs may be preserved as metadata but never replace `EventSeq`.
- Consumer cursor = last sequence successfully processed.
- `read(after=72550, limit=200)` returns the next records and `next_cursor`.

## Independent consumers

Agent A, agent B, projection worker, persistence worker, GUI and OTLP exporter can consume the same records without interference. No consumer API drains the global log.

## Gaps

If a producer cannot retain/deliver records, write an explicit gap descriptor when known:

```text
Gap { first_missing, last_missing, reason, source }
```

Reasons include `KernelRingOverflow`, `AdapterBufferOverflow`, `ProcessDetached`, `TransportFailure`, `CorruptSegment` and `UnsupportedEvidence`.

A missing event and a negative fact are not equivalent.

## Append path

```text
probe/runtime source
 -> normalize minimal envelope
 -> ExecutionLog.append
 -> deterministic enrichment/projection
```

Raw evidence is not discarded because semantic resolution fails.

## Persistence

Persist compressed immutable event segments rather than content-addressing every event.

Segment metadata:

```text
session_id
start_seq
end_seq
record_count
schema_version
compression
checksum
created_at
```

Benchmark an initial segment range around 1–16 MiB; do not hard-code before measurement.

BLAKE3/CAS remains useful for binaries, debug symbols, source snapshots, large values, repeated artifacts and projection checkpoints.

## Checkpoints

Large sessions load:

```text
latest compatible projection checkpoint + replay delta
```

A checkpoint records projection kind/version and `at_seq`.

## Replay

Replay supports rebuilding projections, trying new resolvers, evaluating properties against old sessions when required evidence exists, and re-running comparison algorithms.

Replay is **event replay**, not deterministic CPU instruction replay.

## Session lifecycle

Recommended states:

```text
Starting Running Stopping Stopped Incomplete Corrupt
```

`stop` completes only when producers acknowledge termination/detach or the session is explicitly `Incomplete` with missing tail documented.

## Backpressure

At every bounded boundary define capacity, high-water metric, overflow policy and loss reporting. Never `drop oldest` silently.

## Required tests

1. Two consumers independently read the same 1000 events.
2. Reading limit 100 does not discard 101..1000.
3. Cursor resume returns exactly the next event.
4. Concurrent append preserves `EventSeq`.
5. Simulated overflow produces `Gap`.
6. Crash during segment write leaves prior segments readable.
7. Checkpoint + delta equals full replay.
8. Same log + same projection version replays deterministically.
