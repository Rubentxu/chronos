# M9-02 scoping — Bundle `events` side table (lazy-loading + legacy-compat)

**Cycle:** `m9-02-events-side-table`
**Branch:** `feat/m9-02-events-side-table`
**Base:** `48a9cff` (main at m9-01 close)
**Status:** PROPOSED — 2026-09-11
**Precedence:** `docs/milestones/M8-CLOSE.md` §6 m8-04 R4 + §7 handoff;
`docs/milestones/m9-01-schema-versioning-scoping.md` (m9-01 lays the
`schema_version` plumbing that m9-02 reads).

## §1 Problem statement

m8-04 R4 (deferred from M8, surfaced again in `M8-CLOSE.md` §7 and in
`docs/milestones/m9-01-schema-versioning-scoping.md` §1) discloses that
the `CounterexampleBundleRecord` blob carries the full `events:
Vec<TraceEvent>` inside it:

> "events live inside the bundle blob in redb; only `events_count` +
> `minimised` are on the wire. `Vec<TraceEvent>` would bloat JSON and
> prevent pagination. A future `counterexample_bundle_events` tool can
> re-emit events on demand (m9+)."
> — `docs/milestones/M8-CLOSE.md` §4 m8-04 B3

The blob layout has four concrete failure modes:

1. **`list` deserializes every event of every bundle.** The current
   `list_counterexample_bundles` impl calls
   `bincode::deserialize::<CounterexampleBundleRecord>` per row just to
   read `record.summary`. A list of N bundles each carrying M events
   allocates N×M `TraceEvent`s, even though only the summary is
   returned to the caller. With `test_busyloop` fixtures that already
   capture thousands of events, even N=10 is a noticeable cost; N=100
   is unusable.

2. **`events_count` deserializes every event of one bundle.** The
   m8-05 B3 tool path is `record.events.len()` after a full
   `load_counterexample_bundle`. It exists precisely to avoid the JSON
   payload — but it still touches the blob. The whole point of the
   dedicated tool is defeated by the storage layout.

3. **GC + migration is impossible without rewriting the blob.** A
   future `chronos test bundle-gc` tool cannot drop individual events
   from a bundle without rewriting the whole bincode blob, locking
   out incremental GC. Migration to a future schema_version = 2 (when
   `MinimisedPayload` grows a new variant) similarly forces a full
   re-serialize of every bundle's events.

4. **No lazy / partial event access.** An LLM agent that wants "the
   last 50 events of bundle X" cannot get them without loading all
   events first. The `chronos test replay` path needs all events, but
   other consumers (the future `counterexample_bundle_events` tool
   referenced from `crates/chronos-services/src/output.rs:2585`) want
   ranges or windows. Both code paths share the same blob today; one
   pays for the other.

The m9-01 `schema_version` work laid the plumbing for evolution but
deliberately did not split the blob — events stayed inside the record
envelope because splitting is the larger, structural change. m9-02
makes that structural change.

### Goal in one sentence

Move `Vec<TraceEvent>` out of the bundle blob into a chunked side
table (`counterexample_bundle_events`) keyed by `(bundle_id,
chunk_index)` so that `list` and `events_count` skip the events
entirely, replay loads events from the side table (or from the blob
for pre-m9-02 bundles), and a future
`counterexample_bundle_events(bundle_id, offset, limit)` tool can
serve arbitrary event windows without paying for the full set.

## §2 Goals & non-goals

### Goals

1. Add a new redb table
   `counterexample_bundle_events` (constant
   `COUNTEREXAMPLE_BUNDLE_EVENTS`) in
   `crates/chronos-store/src/counterexample_storage.rs`. Key:
   `bincode((bundle_id: String, chunk_index: u32))` (8 + 4 = 12
   bytes, fixed-size). Value: `bincode(Vec<TraceEvent>)` for one
   chunk of up to `BUNDLE_EVENTS_CHUNK_SIZE = 256` events.

2. Add a public constant
   `pub const BUNDLE_EVENTS_CHUNK_SIZE: usize = 256;` so the chunk
   size is grep-able and the loader / saver agree on it. Document
   why 256 in the module-level doc comment (one TraceEvent is
   ~100–500 bytes bincode; 256 events ≈ 25–125 KB per chunk —
   comfortably below redb's default page size).

3. Add `events_count: u64` to
   `CounterexampleBundleSummary` with
   `#[serde(default)] defaulting to 0`. Pre-m9-02 bundles
   deserialize as `events_count: 0`; post-m9-02 saves always write
   the real count.

4. `save_counterexample_bundle` no longer writes events into the
   blob. The struct's `events` field stays with
   `#[serde(default)]` so pre-m9-02 blobs deserialize cleanly into
   `events: Vec<TraceEvent>`; post-m9-02 saves write
   `events: vec![]` to the blob and push the chunks to the side
   table in the same redb transaction.

5. Add three new `SessionStore` methods:
   - `save_counterexample_bundle_events(&self, bundle_id: &str,
     events: Vec<TraceEvent>) -> Result<(), StoreError>` —
     chunked write, all chunks in one redb transaction; deletes
     any prior chunks for the same `bundle_id` first (so re-save
     is idempotent).
   - `load_counterexample_bundle_events(&self, bundle_id: &str) ->
     Result<Vec<TraceEvent>, StoreError>` — concatenates all chunks
     in order; returns `vec![]` when no chunks exist (NOT an
     error — distinguishes "side table empty, try blob" from "row
     missing").
   - `count_counterexample_bundle_events(&self, bundle_id: &str)
     -> Result<u64, StoreError>` — counts chunks × chunk size for
     the last chunk's actual `len()`. Used only as a fallback
     path; the summary's `events_count` is the primary signal.

6. Add a `load_counterexample_bundle_with_events` helper (or
   equivalent method on `CounterexampleBundleRecord` itself) that
   takes the loaded record and returns the full event vector,
   applying the legacy / new branching:
   - If `record.events.is_empty()` (post-m9-02): load from side
     table via `load_counterexample_bundle_events`.
   - Else (pre-m9-02 legacy bundle): use `record.events.clone()`.
   This is the single chokepoint all consumers use, so the
   backward-compat policy lives in one place.

7. Update consumers in three crates:
   - **chronos-store**: `list_counterexample_bundles` still
     deserializes the full record per row (redb doesn't support
     field-level projection without a schema layout change), but
     post-m9-02 rows are `events: vec![]` so the deserialization
     is bounded to summary + minimised + target_hypothesis +
     empty `events` Vec + `event_cas_hashes`. **No code change
     in list path** — the empty `events` field is the
     optimization. Document this in the function doc comment.
   - **chronos-services**:
     `ChronosCounterexampleService::events_count` reads
     `record.summary.events_count` (O(1), no event deserialization).
     `ChronosCounterexampleService::save` continues to plumb
     `events: Vec<TraceEvent>` from `pull_engine_events` to
     `save_counterexample_bundle`, then calls
     `save_counterexample_bundle_events` in the same store call
     (the services-side wrapper).
     `ChronosCounterexampleService::get` continues to return just
     the summary (events are out-of-band, by design); unchanged
     from the wire's perspective.
   - **chronos-cli**: `replay::run_replay` calls the new
     helper to materialize events before rebuilding the
     `QueryEngine`. The pre-m9-02 `bundle.events.clone()` path
     is replaced with `bundle_events_or_legacy(&bundle)`.
   - **chronos-mcp**: no new tool in m9-02 (D6 below). The
     existing `counterexample_events_count` wrapper is unchanged
     on the wire — the savings come from reading
     `summary.events_count` instead of `record.events.len()`.

8. Schema versioning: bump `CURRENT_BUNDLE_SCHEMA_VERSION` from
   `1` to `2`. New envelopes (events moved out + `events_count`
   on summary) require a version bump per the m9-01 D3 policy.
   Loader still hard-rejects `> CURRENT`; pre-m9-02 bundles
   deserialize with `schema_version: 1` AND `events_count: 0`
   via the m9-01 `#[serde(default)]`. The version-bump tests
   in `m9-01` (D3) still apply.

9. Tests pin the contract (see §5).

### Non-goals (deferred to m9+)

- **`counterexample_bundle_events` MCP tool.** The output.rs
  comment at line 2585 references this tool as a future
  addition. m9-02 only adds the STORAGE primitive; the LLM
  wrapper that exposes `(bundle_id, offset, limit)` is a
  separate cycle (call it m9-03 or later) that consumes the
  m9-02 storage API. Documented in §6 R4.
- **GC of orphaned events.** `chronos test bundle-gc` /
  deleting individual events from a side-table bundle is a
  natural follow-up but requires its own scoping (chunk
  deletion policy, retention window). m9+ scope.
- **Migration pass over existing bundles.** Pre-m9-02 bundles
  keep `events` in the blob and `summary.events_count: 0`.
  No background re-write. The legacy loader branch handles
  them transparently. A future m9+ "rewrite legacy bundles to
  v2" tool could move them, but it's optional — the
  backward-compat path is correct and cheap.
- **`event_cas_hashes` migration.** m8-03 left this as a
  snapshot of CAS hashes backing the events; it's
  underused. m9-02 leaves it inside the blob (where it
  already lives) and does not move it to the side table.
- **Per-event partial-load protocol at the redb layer.**
  redb doesn't natively support sub-value reads. The chunk
  layout is the workaround; we don't try to make
  `load_counterexample_bundle_events_chunk` public (it's
  internal to `save_counterexample_bundle_events` /
  `load_counterexample_bundle_events`). The future MCP
  tool's `(offset, limit)` semantics ride on the in-memory
  concat, which is fine for typical bundle sizes (hundreds
  to low thousands of events).
- **Compression of side-table chunks.** `TraceEvent`s have
  variable-sized `String` payloads (function name, source
  file). Compression would help, but it's a non-trivial
  ergonomics hit (bincode + lz4 / zstd wrapper). Out of scope
  for m9-02; document as R5.
- **Changing the `chronos_bundles` key shape.** Bundles stay
  keyed by `bundle_id` (the existing primary key); the side
  table uses a composite key. No migration of the primary
  key.
- **Atomicity guarantees across tables for non-batch
  callers.** `save_counterexample_bundle` +
  `save_counterexample_bundle_events` are exposed as two
  separate methods so callers who don't have events (none
  today, but future m9+ bundles could be summary-only) can
  skip the events write. The chronos-services save path
  wraps both in a single redb write transaction via a new
  internal helper (see D4); outside of that wrapper, the
  pair is NOT atomic. Documented as R3.

## §3 Architectural decisions

### D1 — Side table is chunked, not per-event

A single-bundle side table could either:
- store one bincode row per bundle
  (`bincode(Vec<TraceEvent>)` — like the existing
  `session_events` precedent), or
- store N rows per bundle (chunked by event index).

Per-event storage (`bincode(TraceEvent)` per row) is too granular:
a 1000-event bundle would generate 1000 redb writes per save and
1000 redb reads per load. Single-bundle storage doesn't give any
lazy-loading benefit over the current blob layout — it just
shifts the bytes.

**Chunked** (`bincode(Vec<TraceEvent>)` per chunk of 256 events) is
the middle ground:
- Atomic save: a 1000-event bundle = 4 chunks = 4 redb writes
  (one transaction).
- Partial reads: a future `counterexample_bundle_events(offset,
  limit)` can load just the chunks covering the requested range.
- Bounded chunk size: 256 events ≈ 25–125 KB bincode — small
  enough to fit comfortably in a redb page, large enough to keep
  per-bundle row count under ~10 for typical bundles.

The constant `BUNDLE_EVENTS_CHUNK_SIZE: usize = 256` lives at
module scope (same precedent as
`CURRENT_BUNDLE_SCHEMA_VERSION` in m9-01). The chunked key is
`bincode((String, u32))`; u32 chunk_index supports up to ~1 trillion
chunks per bundle (effectively unbounded).

### D2 — `events_count` lives on the summary, not in a side-table count query

The `events_count` field on
`CounterexampleBundleSummary` makes the events_count tool O(1):
read summary.events_count, return it. No side-table scan, no
event deserialization.

Trade-off vs counting chunks: a chunked count
(`count_counterexample_bundle_events`) is O(chunks), not O(1),
and requires touching the side table. The summary field is
faster AND survives legacy bundles (which have empty side
tables but populated blob `events` Vecs).

**Legacy handling**: pre-m9-02 bundles have `events_count: 0`
(serde default). When the `events_count` tool reads such a bundle,
the loader must fall back to counting from the blob's
`record.events.len()`. This is the ONLY code path where the blob
is touched for a count; documented as a special case in
`ChronosCounterexampleService::events_count` to keep the
optimisation explicit.

### D3 — `events: Vec<TraceEvent>` stays in the struct for legacy reads

Rather than removing the field from
`CounterexampleBundleRecord`, we keep it and mark it
`#[serde(default)]`. Reasons:

- The struct stays self-describing — a reader doesn't need to
  call a second method to know what the legacy bundle held.
- `bincode::deserialize` is unchanged for legacy bundles; new
  bundles deserialize `events: vec![]` cleanly.
- Future code that wants to introspect a record in isolation
  (debugging, fixtures) sees the legacy events without needing
  a side-table read.

The save path always writes `events: vec![]` to the blob. The
side table is the source of truth post-m9-02. The field is
"load-only legacy" — there's no scenario where a post-m9-02
save populates it.

### D4 — Save wraps both tables in one redb transaction

`save_counterexample_bundle` and
`save_counterexample_bundle_events` are two separate public
methods, but the chronos-services save path (the only real
caller today) uses an internal `save_bundle_with_events`
helper that opens ONE write transaction and inserts into both
tables. This guarantees:

- A bundle record and its events are either both persisted or
  neither (no orphan records without events / events without
  records).
- A bundle read after a failed save returns
  `Ok(None)`, not a partial bundle.

The two public methods stay separate so that m9+ callers who
don't have events (e.g., summary-only bundles, future
counterexample metadata-only records) can skip the events
write. The atomic wrapper is the canonical save path.

### D5 — Loader policy: side table wins, blob is fallback

`bundle_events_or_legacy(bundle: &CounterexampleBundleRecord) ->
Vec<TraceEvent>` is the single chokepoint:

```rust
fn bundle_events_or_legacy(
    store: &SessionStore,
    bundle: &CounterexampleBundleRecord,
) -> Result<Vec<TraceEvent>, StoreError> {
    if !bundle.events.is_empty() {
        // Legacy (pre-m9-02) bundle: events were stored in the blob.
        return Ok(bundle.events.clone());
    }
    // Post-m9-02: load from side table.
    store.load_counterexample_bundle_events(&bundle.summary.bundle_id)
}
```

Pre-m9-02 bundles always have `events: Vec<TraceEvent>`
populated (the bincode blob stored them there), so the legacy
branch hits. Post-m9-02 bundles always have `events: vec![]`,
so the side-table branch hits. There is no "both" case —
saves are exclusively one or the other.

The function lives in `chronos-store` (next to the schema
types) so chronos-cli and chronos-services don't need to
re-implement the branching. It's a free function
(`pub fn bundle_events_or_legacy`) that takes the store
explicitly; we don't add a method on the record struct because
the record doesn't own the store.

### D6 — No new MCP tool in m9-02

The `counterexample_bundle_events` tool is referenced from
`crates/chronos-services/src/output.rs:2585` as a "future"
addition. m9-02 ships the storage primitive but does NOT ship
the wire DTO + wrapper tool. Reasons:

- Storage primitive + lazy loading are the
  foundation; the wire layer is straightforward (DTO +
  wrapper + handler) and deserves its own scoping.
- Avoids two simultaneous API additions in one cycle.
- The MCP wrapper would also need pagination semantics
  (offset/limit → which chunks to load), which is a
  separate design decision.

The m9-02 storage API is shaped so the future tool slots in
without further changes: `load_counterexample_bundle_events`
returns the full concat (so the tool can paginate in memory
after a single side-table read); a per-chunk load is not
exposed publicly because redb's transaction model makes it
cumbersome and the concat-then-paginate path is fast for any
realistic bundle size.

### D7 — `BUNDLE_EVENTS_CHUNK_SIZE` is module-scoped

Lives at the top of `counterexample_storage.rs`, same shape as
`CURRENT_BUNDLE_SCHEMA_VERSION` (m9-01 D2). One constant, one
file, easy to grep. If a future cycle wants to change it, the
only impact is chunk-boundary handling in
`save_counterexample_bundle_events` and the loader's concat
loop — both are written against the constant.

### D8 — `event_cas_hashes` is not migrated

Stays inside the blob. It's an underused m8-03 artifact (the
snapshot of CAS hashes) — moving it to a side table would be
premature. A future cycle that actually uses it can revisit.

## §4 Implementation sketch

### chronos-store (1 file, ~120 LoC delta)

`crates/chronos-store/src/counterexample_storage.rs`:

```rust
/// Chunk size for events stored in the side table. 256 events ≈
/// 25–125 KB bincode (TraceEvent has variable-sized String
/// payloads), well below redb's default page size. See D1.
pub const BUNDLE_EVENTS_CHUNK_SIZE: usize = 256;

/// m9-02: bumped from 1 to 2 to reflect events moved to side
/// table + events_count added to summary. Pre-m9-02 bundles
/// deserialize with schema_version=1 (m9-01 default); loader
/// still hard-rejects > CURRENT.
pub const CURRENT_BUNDLE_SCHEMA_VERSION: u32 = 2;

/// Side table for bundle events. Key: bincode((bundle_id,
/// chunk_index)). Value: bincode(Vec<TraceEvent>) for one
/// chunk of up to BUNDLE_EVENTS_CHUNK_SIZE events.
const COUNTEREXAMPLE_BUNDLE_EVENTS: TableDefinition<&[u8],
                                                    &[u8]> =
    TableDefinition::new("counterexample_bundle_events");

/// Bumped versions the loader accepts silently. Today 1 and 2;
/// future cycles add entries when they introduce a new
/// envelope shape. (R4 m9-02: now actually consulted.)
const KNOWN_BUNDLE_SCHEMA_VERSIONS: &[u32] = &[1, 2];

#[derive(...)]
pub struct CounterexampleBundleSummary {
    pub bundle_id: String,
    pub property_kind: String,
    pub workspace_id: String,
    pub created_at_ms: u64,
    pub rounds_used: u32,
    pub has_full_bundle: bool,
    /// m9-01: envelope version (default 1, bumped to 2 in
    /// m9-02).
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    /// m9-02: count of events in the bundle. Defaults to 0 for
    /// pre-m9-02 bundles (serde default; events live in the
    /// blob for those, so events_count() must fall back to
    /// record.events.len()).
    #[serde(default)]
    pub events_count: u64,
}

#[derive(...)]
pub struct CounterexampleBundleRecord {
    pub summary: CounterexampleBundleSummary,
    /// m9-02: kept for legacy reads (pre-m9-02 blobs stored
    /// events here). New saves always write events: vec![];
    /// events live in the counterexample_bundle_events side
    /// table instead.
    #[serde(default)]
    pub events: Vec<TraceEvent>,
    pub minimised: Option<MinimisedPayload>,
    #[serde(default)]
    pub event_cas_hashes: Vec<ContentHash>,
    #[serde(default)]
    pub target_hypothesis: Option<HypothesisInputWire>,
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
}

impl crate::storage::SessionStore {
    // Existing API: unchanged signature, internal split into
    // two redb writes via save_bundle_record_and_events below.
    pub fn save_counterexample_bundle(
        &self,
        record: CounterexampleBundleRecord,
    ) -> Result<String, StoreError> {
        // D5: enforce summary.schema_version == record.schema_version.
        let mut record = record;
        record.schema_version = CURRENT_BUNDLE_SCHEMA_VERSION;
        record.summary.schema_version = CURRENT_BUNDLE_SCHEMA_VERSION;
        record.summary.events_count = record.events.len() as u64;

        let events = std::mem::take(&mut record.events);
        let bundle_id = record.summary.bundle_id.clone();

        self.save_bundle_record_and_events(&record, events)?;
        Ok(bundle_id)
    }

    // m9-02: atomic save — bundle record + side-table events
    // in one redb write transaction.
    fn save_bundle_record_and_events(
        &self,
        record: &CounterexampleBundleRecord,
        events: Vec<TraceEvent>,
    ) -> Result<(), StoreError> {
        let tx = self.db().begin_write()?;
        {
            let bytes = bincode::serialize(record)?;
            let mut t = tx.open_table(COUNTEREXAMPLE_BUNDLES)?;
            t.insert(record.summary.bundle_id.as_bytes(),
                     bytes.as_slice())?;
        }
        {
            // Drop any pre-existing chunks for this bundle_id
            // (idempotent re-save).
            let mut t = tx.open_table(COUNTEREXAMPLE_BUNDLE_EVENTS)?;
            for entry in t.iter()? {
                let (k, _) = entry?;
                let key: (String, u32) = bincode::deserialize(k.value())?;
                if key.0 == record.summary.bundle_id {
                    t.remove(k.value())?;
                }
            }
            // Write chunks.
            for (chunk_idx, chunk) in events
                .chunks(BUNDLE_EVENTS_CHUNK_SIZE)
                .enumerate()
            {
                let key = (
                    record.summary.bundle_id.clone(),
                    chunk_idx as u32,
                );
                let key_bytes = bincode::serialize(&key)?;
                let val_bytes = bincode::serialize(chunk)?;
                t.insert(key_bytes.as_slice(), val_bytes.as_slice())?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    // m9-02: chunked events write (rare standalone use — most
    // callers go through save_counterexample_bundle).
    #[allow(clippy::result_large_err)]
    pub fn save_counterexample_bundle_events(
        &self,
        bundle_id: &str,
        events: Vec<TraceEvent>,
    ) -> Result<(), StoreError> {
        let tx = self.db().begin_write()?;
        {
            let mut t = tx.open_table(COUNTEREXAMPLE_BUNDLE_EVENTS)?;
            // Drop prior chunks for this bundle_id.
            for entry in t.iter()? {
                let (k, _) = entry?;
                let key: (String, u32) =
                    bincode::deserialize(k.value())?;
                if key.0 == bundle_id {
                    t.remove(k.value())?;
                }
            }
            for (chunk_idx, chunk) in events
                .chunks(BUNDLE_EVENTS_CHUNK_SIZE)
                .enumerate()
            {
                let key = (bundle_id.to_string(), chunk_idx as u32);
                let key_bytes = bincode::serialize(&key)?;
                let val_bytes = bincode::serialize(chunk)?;
                t.insert(key_bytes.as_slice(),
                         val_bytes.as_slice())?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    // m9-02: concat all chunks in order.
    #[allow(clippy::result_large_err)]
    pub fn load_counterexample_bundle_events(
        &self,
        bundle_id: &str,
    ) -> Result<Vec<TraceEvent>, StoreError> {
        let tx = self.db().begin_read()?;
        let t = match tx.open_table(COUNTEREXAMPLE_BUNDLE_EVENTS) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => {
                return Ok(Vec::new());
            }
            Err(e) => return Err(StoreError::Database(e.into())),
        };
        let mut chunks: Vec<(u32, Vec<TraceEvent>)> = Vec::new();
        for entry in t.iter()? {
            let (k, v) = entry?;
            let key: (String, u32) = bincode::deserialize(k.value())?;
            if key.0 != bundle_id {
                continue;
            }
            let events: Vec<TraceEvent> =
                bincode::deserialize(v.value())?;
            chunks.push((key.1, events));
        }
        chunks.sort_by_key(|(idx, _)| *idx);
        let mut out = Vec::new();
        for (_, c) in chunks {
            out.extend(c);
        }
        Ok(out)
    }

    // m9-02: count chunks × BUNDLE_EVENTS_CHUNK_SIZE; last
    // chunk contributes its actual len(). Used only as a
    // fallback when summary.events_count == 0.
    #[allow(clippy::result_large_err)]
    pub fn count_counterexample_bundle_events(
        &self,
        bundle_id: &str,
    ) -> Result<u64, StoreError> {
        let tx = self.db().begin_read()?;
        let t = match tx.open_table(COUNTEREXAMPLE_BUNDLE_EVENTS) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => {
                return Ok(0);
            }
            Err(e) => return Err(StoreError::Database(e.into())),
        };
        let mut total: u64 = 0;
        let mut last_chunk_size: u64 = 0;
        let mut any = false;
        for entry in t.iter()? {
            let (k, v) = entry?;
            let key: (String, u32) = bincode::deserialize(k.value())?;
            if key.0 != bundle_id {
                continue;
            }
            let events: Vec<TraceEvent> =
                bincode::deserialize(v.value())?;
            total += BUNDLE_EVENTS_CHUNK_SIZE as u64;
            last_chunk_size = events.len() as u64;
            any = true;
        }
        if !any {
            return Ok(0);
        }
        // Replace the chunk_size * N with actual size of all
        // chunks except the last + actual last chunk size.
        // (Simpler: total = (n_full_chunks × CHUNK) + last_size.)
        let n_chunks_minus_one = total / BUNDLE_EVENTS_CHUNK_SIZE as u64
            - 1;
        Ok(n_chunks_minus_one * BUNDLE_EVENTS_CHUNK_SIZE as u64
            + last_chunk_size)
    }
}

/// D5: single chokepoint for the legacy/new branching.
#[allow(clippy::result_large_err)]
pub fn bundle_events_or_legacy(
    store: &crate::storage::SessionStore,
    bundle: &CounterexampleBundleRecord,
) -> Result<Vec<TraceEvent>, StoreError> {
    if !bundle.events.is_empty() {
        return Ok(bundle.events.clone());
    }
    store.load_counterexample_bundle_events(&bundle.summary.bundle_id)
}
```

The `save_counterexample_bundle` overload now performs the
canonical save (with side-table writes); the public
`save_counterexample_bundle_events` exists for the rare caller
that wants to write events to an existing bundle record (e.g.,
a future m9+ "patch events" tool). The
`save_bundle_record_and_events` private helper is the atomic
chokepoint.

### chronos-services (3 file edits, ~15 LoC delta)

**`crates/chronos-services/src/counterexample.rs`**:

- `save()` (line 449): after
  `ctx.store.save_counterexample_bundle(record).map_err(...)?`,
  call
  `ctx.store.save_counterexample_bundle_events(&bundle_id, events)`?
  Wait — `save_counterexample_bundle` already writes events via
  the atomic helper. Re-reading D4: the chronos-services save
  path goes through the atomic wrapper; the public
  `save_counterexample_bundle_events` is for stand-alone use.
  Therefore: **no change to `save()`'s signature or control
  flow**, just the storage layer handles the split.

  Actually, looking again, the chronos-services save path does:

  ```rust
  let record = cs::CounterexampleBundleRecord {
      summary,
      events,             // <-- passed in here
      minimised: minimised_opt,
      ...
      schema_version: cs::CURRENT_BUNDLE_SCHEMA_VERSION,
  };
  let returned_id = ctx
      .store
      .save_counterexample_bundle(record)
      .map_err(...)?;
  ```

  The new `save_counterexample_bundle` internally does
  `std::mem::take(&mut record.events)` and routes the events to
  the side table. From chronos-services' perspective, this is
  **invisible**. No code change needed in `save()`. D4 holds.

- `events_count()` (line 357): change the count source from
  `record.events.len()` to `record.summary.events_count`. For
  pre-m9-02 bundles (events_count == 0 and
  record.events.is_empty() == false), fall back to
  `record.events.len()`. Pseudocode:

  ```rust
  let count = if record.summary.events_count > 0 {
      record.summary.events_count as usize
  } else {
      // Legacy bundle: events are in the blob.
      record.events.len()
  };
  ```

- `get()` (line 322): unchanged on the wire. Continue returning
  only the summary. Document in the function doc comment that
  events are NOT included in the Got output (already true
  pre-m9-02; the contract holds).

### chronos-cli (1 file, ~3 LoC delta)

**`crates/chronos-cli/src/replay.rs`**:

- `run_replay` (line 60): replace
  `let engine = QueryEngine::new(bundle.events.clone());` with
  the new helper:

  ```rust
  let events = chronos_store::counterexample_storage::bundle_events_or_legacy(&store, &bundle)
      .with_context(|| format!("loading events for bundle {bundle_id}"))?;
  let engine = QueryEngine::new(events);
  ```

- Test fixtures (`synthetic_bundle` etc.) update the record
  literal to include `events_count: 0` (matches pre-m9-02
  shape, so the helper's `!events.is_empty()` branch hits and
  reads from the in-memory Vec).

- New test: `replay_uses_side_table_for_post_m9_02_bundle` —
  insert a record via `save_counterexample_bundle` (which
  internally writes events to side table), call
  `bundle_events_or_legacy`, assert it returns the same events
  (not the empty blob Vec).

### chronos-mcp (no change)

The `counterexample_events_count` wrapper keeps its signature;
it just gets a faster implementation (the
chronos-services-side change above). No new DTOs, no new
tool, no new wrapper. The savings are silent — agents that
call the tool see no behavior change but get a faster
response.

### Sandbox (no new test in m9-02)

The T4-smoke subset re-runs the existing
`counterexample_tools` (ce1..ce12) +
`e2e_connectivity`. Every fixture in this file now exercises:

- save: events go to side table, blob has empty events
- get: events_count is read from summary
- replay (ce12): reads events via bundle_events_or_legacy

No new sandbox test needed because the existing suite
already covers the save → get → events_count → replay paths.
The chronos-store unit tests in §5 pin the storage
contract; the per-crate integration tests in §5 pin the
chronos-services and chronos-cli wrappers.

## §5 Tests

### Unit (chronos-store lib)

- `m9_02_save_writes_events_to_side_table_not_blob` — save a
  bundle with N events, load the record via
  `load_counterexample_bundle`, assert
  `record.events.is_empty()` (post-m9-02) AND the side table
  has the expected chunk count
  (`(N / 256) + (N % 256 != 0) as u32`).
- `m9_02_load_events_concatenates_chunks_in_order` — manually
  write 3 chunks (chunk_index 0, 1, 2) out of order, call
  `load_counterexample_bundle_events`, assert the events come
  back in chunk_index order.
- `m9_02_save_events_idempotent_overwrites_prior_chunks` —
  save N events, then save M > N events for the same
  bundle_id, assert only M events come back (old chunks
  dropped, not appended).
- `m9_02_count_events_handles_partial_last_chunk` — write 2
  chunks with 256 + 100 events, count returns 356 (not
  512).
- `m9_02_legacy_bundle_with_events_in_blob_loads_normally` —
  hand-craft a bincode blob with `schema_version: 1` and
  `events: vec![...]`, insert directly into the DB, load via
  `bundle_events_or_legacy`, assert legacy branch returns
  the blob events.
- `m9_02_post_m9_02_bundle_loads_events_from_side_table` —
  save a record via the API (events go to side table),
  `bundle_events_or_legacy` returns the side-table events
  (not the empty blob events).
- `m9_02_summary_events_count_default_zero_for_legacy` —
  construct a JSON summary without `events_count`, assert
  serde default is 0.
- `m9_02_load_unknown_bundle_returns_empty_events_vec` —
  call `load_counterexample_bundle_events("nonexistent")`,
  assert `Ok(vec![])` (not `Err`).
- `m9_02_schema_version_bumped_to_2` — save a bundle, load
  it, assert `record.schema_version == 2` AND
  `record.summary.schema_version == 2`.
- `m9_02_legacy_v1_bundle_still_loads_with_summary_events_count_0`
  — hand-craft a v1 blob, load it, assert
  `record.summary.events_count == 0` and the events come
  from the blob (D3 + D5).

### Per-crate integration

- `chronos_services::counterexample::tests::m9_02_save_persists_events_count_in_summary`
  — end-to-end: call `ChronosCounterexampleService::save`
  with N events, reload via
  `load_counterexample_bundle`, assert
  `summary.events_count == N`.
- `chronos_services::counterexample::tests::m9_02_events_count_reads_from_summary`
  — call `ChronosCounterexampleService::events_count`, assert
  it returns `N` WITHOUT touching the side table (covered by
  the unit test that asserts `summary.events_count > 0`
  branch is taken). This is the wire-side savings.
- `chronos_cli::replay::tests::m9_02_replay_uses_side_table_for_post_m9_02_bundle`
  — save a record via the API, call `run_replay`, assert
  the verdict is "violation" (the events actually flow
  through `QueryEngine::new`).
- `chronos_cli::replay::tests::m9_02_replay_legacy_bundle_uses_blob_events`
  — hand-craft a v1 bundle with events in blob, call
  `run_replay`, assert the events flow through (legacy path).

### Sandbox (T4-smoke subset re-runs)

- `counterexample_tools` — ce1..ce12 all green. The save →
  get → events_count → replay path is co-tested by these
  fixtures; every shrink now persists events to the side
  table, every events_count reads from summary, every replay
  reads via `bundle_events_or_legacy`.
- `e2e_connectivity` — confirms the MCP server starts and
  accepts the first tool call (no regression in the
  chronos-mcp wiring, which is unchanged on the wire).

## §6 Disclosures

### R1 (NEW) — `schema_version` is bumped to 2 in m9-02

The constant `CURRENT_BUNDLE_SCHEMA_VERSION` moves from 1
(m9-01) to 2 (m9-02). Bundles persisted by a future
chronos-store with `schema_version: 3` are hard-rejected by
the m9-01 D3 loader policy. Pre-m9-02 bundles continue to
deserialize as `schema_version: 1` via the m9-01
`#[serde(default)]` machinery and load through the legacy
branch (D5). No migration pass.

### R2 (NEW) — Pre-m9-02 bundles get `events_count: 0` from serde

`CounterexampleBundleSummary.events_count` defaults to 0 for
bundles persisted before m9-02. The
`ChronosCounterexampleService::events_count` impl detects
this (`record.summary.events_count == 0` AND
`record.events.is_empty() == false`) and falls back to
`record.events.len()`. This is the ONLY code path where the
blob is touched for a count; documented in the function to
keep the optimization explicit.

### R3 (NEW) — Public `save_counterexample_bundle_events` is not atomic with the record

The two public methods (`save_counterexample_bundle`,
`save_counterexample_bundle_events`) are separate. The
canonical save path goes through the internal
`save_bundle_record_and_events` wrapper (D4), which IS
atomic. A future caller that uses the public methods
sequentially does NOT get atomicity. Documented in the
public method's doc comment.

### R4 (NEW) — `counterexample_bundle_events` MCP tool is m9+ scope

The output.rs:2585 reference to a future
`counterexample_bundle_events` tool is m9+, not m9-02. m9-02
ships the storage primitive only. The wire layer + wrapper
+ handler is a separate cycle that consumes the m9-02
storage API. This split keeps the cycle scope tight.

### R5 (NEW) — No chunk compression

TraceEvent has variable-sized String payloads (function
names, source files). Chunk compression (lz4 / zstd) would
help, but adds ergonomics cost (a bincode wrapper that
handles compression transparently). Out of scope for m9-02.
The chunk size of 256 is small enough that the bincode
overhead is acceptable for typical bundles.

### R6 (NEW) — `list_counterexample_bundles` still deserializes the full blob

redb doesn't natively support sub-value reads. We could
split the bundle blob further (separate `bundle_summaries`
table, separate `bundle_minimised` table, etc.) for a fully
projection-aware list path, but that's a larger structural
change. m9-02's `events: vec![]` in the post-m9-02 blob IS
the optimization — the empty Vec deserializes cheaply. A
future m9+ cycle can split the blob further if profiling
demands it.

### R7 (NEW) — Re-save overwrites all prior chunks for the same bundle_id

`save_counterexample_bundle_events` (and the atomic wrapper)
delete all existing chunks for the bundle_id before writing
new ones. This is correct for the only current use
(re-saving on a new save call), but it means callers cannot
"append" events across saves. If m9+ ever wants append-only
events, a separate method would be needed.

### R8 (NEW) — Per-chunk load is not exposed publicly

`load_counterexample_bundle_events_chunk(bundle_id,
chunk_index)` is not part of m9-02's public API. The
concat-then-paginate path used by the future MCP tool is
fast enough for realistic bundles (hundreds to low thousands
of events). Per-chunk loading is an internal optimization
that future cycles can add if the concat becomes a
bottleneck.

## §7 Tier & gate plan

This is **A-lite** scope per the user's task contract:

* **T0**: fmt + clippy (`-D warnings`).
* **T1**: chronos-store lib (10 new unit tests pinning the
  side-table contract).
* **T2**: chronos-services + chronos-cli integration tests
  (4 new tests covering the wrapper updates).
* **T4-smoke**: re-run `counterexample_tools` (ce1..ce12) +
  `e2e_connectivity` (server-startup canary). No new sandbox
  test needed — the existing suite exercises every changed
  path.

No chronos-e2e (D bucket — out of scope, no ptrace changes).
No benches (E bucket — no perf changes that need
benchmarking).

## §8 Smoke subset chosen

Per AGENTS.md §2, the cycle touches:

- The bincode envelope of `counterexample_bundles` (every
  save / load — events field semantics change)
- A new redb table (`counterexample_bundle_events`)
- Three consumer crates (chronos-store,
  chronos-services, chronos-cli)
- One wire tool's count path (chronos-mcp
  counterexample_events_count, indirectly via chronos-services
  events_count)

The smoke subset is:

- `counterexample_tools` (full file, 12 tests) — every
  shrink fixture now saves events to the side table and
  every replay path goes through `bundle_events_or_legacy`.
- `e2e_connectivity` — confirms the MCP server starts and
  the first tool call succeeds (no regression in the
  chronos-mcp wiring).

This matches the m8-07 / m9-01 subset exactly. The chronos-store
lib unit tests in §5 run at T1 and pin the storage
contract; the chronos-services + chronos-cli integration
tests in §5 run at T2 and pin the consumer wrappers.

## §9 What's NOT closed by m9-02

- **m8-04 R4** (bundle-as-blob → side table) — closed by
  m9-02. The events live in `counterexample_bundle_events`,
  not in the bundle blob. Backward compatibility for
  pre-m9-02 bundles (events in blob) is preserved via
  `bundle_events_or_legacy`.
- **m8-06 R4** (cross-variant existence predicate
  shrinking) — still open, m9+ scope. m9-02 doesn't touch
  the shrinker.
- **m8-04 R-hypothesis-reconstruction-fidelity** — closed
  by m8-07. NOT a m9-02 deliverable.
- **Migration pass to rewrite legacy bundles** — m9-02
  does not touch pre-m9-02 rows on disk. The legacy
  loader branch handles them transparently. A future
  "rewrite to v2" tool is optional and out of scope.
- **`counterexample_bundle_events` MCP tool** — deferred
  to m9+. m9-02 ships the storage primitive (R4).
- **GC of orphaned events** — `chronos test bundle-gc` /
  chunk deletion policies are m9+ scope.
- **Compression of side-table chunks** — m9+ scope (R5).
- **Further blob splitting** (separate tables for summary,
  minimised, target_hypothesis) — m9+ scope if profiling
  demands it (R6).
