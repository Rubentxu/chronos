# M9-04 scoping — Side-table key layout for `counterexample_bundle_events` (range-scan friendly)

**Cycle:** `m9-04-side-table-key-layout`
**Branch:** `feat/m9-04-side-table-key-layout`
**Base:** `eb2cccd` (main at m9-03 close)
**Path:** A-min (per cycle status; one bounded store-layer refactor + compat layer)
**Status:** PROPOSED — 2026-09-11
**Precedence:** m9-02 R1–R8 (side table contract),
`docs/milestones/m9-03-side-table-debt-cleanup` (helper extraction),
`FIND-M9-02-DV-PERF-01` (debt ledger).

## §1 Problem statement

The `counterexample_bundle_events` side table (m9-02) persists one
row per chunk of `Vec<TraceEvent>` and is queried three ways:

1. **Save** — delete prior chunks for `bundle_id`, write new chunks.
2. **Load** — return all chunks for `bundle_id` concatenated in order.
3. **Count** — return the total event count for `bundle_id`.

Today, every one of those three operations does a **full table scan**
of `counterexample_bundle_events`. The reason is the current key
layout (lines 60–91 of `crates/chronos-store/src/counterexample_storage.rs`):

```
[len(bundle_id) as u32 BE] [bundle_id bytes...] [chunk_index as u32 BE]
```

`len(bundle_id)` is a 4-byte big-endian length prefix followed by a
**variable-length** UTF-8 id (typically 36 bytes for a uuid::v7). The
chunk_index is appended at the tail. The redb B-tree iterates keys in
lexicographic order (`impl Key for &[u8]` in
`redb-2.6.3/src/types.rs:317` does `data1.cmp(data2)`), so adjacent
chunks of one bundle WOULD land next to each other IF the prefix were
fixed-width. As it stands, the prefix length is bundle-id-dependent
and redb cannot bound a range scan to one bundle's chunks — there is
no "all keys starting with `len=36 uuid-v7-bytes`" trick when `len`
varies.

Three concrete consequences (m9-02 debt-report `FIND-M9-02-DV-PERF-01`,
mediuM/P2):

1. **Cost grows with total stored counterexamples, not with the
   target bundle.** A `count_counterexample_bundle_events` call on
   bundle X visits every chunk of every bundle in the table. With
   100 bundles × 4 chunks each = 400 row reads to compute X's count
   (3 useless deserializations per useless chunk).
2. **Save-path prior-chunk cleanup also full-scans.** Today the save
   helper opens a read transaction, calls
   `collect_bundle_chunks(&read_tx, &bundle_id)`, which iterates the
   whole table and decodes every key. On a 100-bundle store this is
   ~99 wasted key decodes per save. Cheap individually; the
   asymptotic regression matters once a long-running chronos
   accumulates hundreds of bundles.
3. **The 14 m9-02 tests pin the chunk-scan skeleton with three
   near-identical iteration loops** (one per path) because the
   layout forces the same `table.iter() → decode_chunk_key → filter`
   pattern in save-prior-cleanup, load, and count. The m9-03 cycle
   factored this into `collect_bundle_chunks`; m9-04 changes the
   helper so it no longer needs to scan, only the prior-cleanup
   variant keeps a fallback for legacy v=2 rows.

The m9-02 layout was chosen because (a) it round-trips
`(bundle_id, chunk_index)` directly through `bincode` without a
hash, and (b) `decode_chunk_key` is a one-step lookup that does not
require a secondary index. Both are real wins for code simplicity;
m9-04 trades them for O(chunks) read cost via a **fixed-width prefix
that allows bounded range scans**.

### Goal in one sentence

Switch the `counterexample_bundle_events` key from
`[len(bundle_id)][bundle_id][chunk_index]` (variable-width prefix)
to `[blake3(bundle_id) (16 bytes)][chunk_index (4 bytes BE)]`
(fixed-width 20-byte prefix) so reads for one bundle can use
redb's `range(prefix..prefix+chunk_index_max)` instead of a full
table scan, while keeping full backward compatibility for bundles
written by m9-02 / m9-03 (key layout v2 on disk).

## §2 Goals & non-goals

### Goals

1. Replace `encode_chunk_key` / `decode_chunk_key` with a
   fixed-width layout:
   **key = `[blake3(bundle_id)[..16] || chunk_index.to_be_bytes()]`**
   (20 bytes total, no length prefix). `blake3` is already a
   chronos-store dependency (`crates/chronos-store/Cargo.toml:9`,
   used elsewhere — see `crate::cas::ContentHash`); truncating to 16
   bytes gives a 2^128-prefix key space, which is ample to keep
   collisions vanishingly rare.

2. Replace `collect_bundle_chunks` with
   **`collect_bundle_chunks_range`** that uses redb's
   `range(prefix..prefix || 0xFF 0xFF 0xFF 0xFF)` (the half-open
   upper bound covers chunk_index ∈ [0, u32::MAX]). Iteration is
   bounded to one bundle's chunks. The function decodes
   `(bundle_id, chunk_index)` from the **value** (which carries
   `bundle_id` alongside `Vec<TraceEvent>` — see D3) instead of the
   key, since the key only carries the hash.

3. Keep a **legacy reader** for bundles that were persisted under
   the m9-02 / m9-03 layout (v2 chunks already on disk). The legacy
   reader scans the whole table and matches via
   `decode_chunk_key_legacy` on the variable-width v2 layout. It
   is invoked only when the range scan returns 0 entries for a
   bundle_id whose `summary.events_count > 0` — i.e., the bundle
   has chunks under v2 layout but none yet under v3 layout. This
   is the **same fallback pattern** m9-02 D5 used for the
   blob-vs-side-table branching, applied one level down (key layout
   instead of payload location).

4. **Bump `CURRENT_BUNDLE_SCHEMA_VERSION` to 3.** The constant
   moves from 2 (m9-02) to 3 in m9-04 because the side-table value
   shape changes (D3: `bundle_id` moves from key to value). Pre-
   m9-04 bundles deserialize with `schema_version == 2` via
   `#[serde(default)]` and continue to load through the legacy
   reader path; the version field is the discriminator between
   "new layout (v3+)" and "legacy layout (v2-)". `KNOWN_BUNDLE_SCHEMA_VERSIONS`
   grows from `[1, 2]` to `[1, 2, 3]`.

5. **Save path always writes the v3 layout.** It computes
   `prefix = blake3(bundle_id)[..16]`, opens a write transaction,
   deletes prior chunks for the bundle via the v3 range scan
   (zero-or-many rows), AND deletes any legacy v2 chunks via a
   defensive full-table scan that filters by `decode_chunk_key_legacy`
   matching `bundle_id` (this is the one remaining full-table scan
   in the codebase; it runs only on save and only reads — see R3).
   Then writes new chunks under v3 keys. The atomicity contract is
   preserved (single write transaction, single `commit()`).

6. **Save triggers lazy migration.** When a v2 bundle is re-saved
   (re-shrink, replay-re-save, future tools), the prior chunks are
   in v2 layout. The save deletes them (defensive scan, R3) and
   writes new chunks in v3 layout. The bundle's `schema_version`
   is updated to 3. Subsequent reads hit the v3 range-scan path and
   never touch the v2 reader. No background migration pass is
   required.

7. **Tests pin the v3 contract** (§5):
   range scan returns the same chunks as the legacy scan for a
   v2 bundle, save + reload survives a round-trip, lazy migration
   on re-save removes v2 chunks, hash-prefix collisions are tested
   by construction (R2), and the v2 reader is gated to never be
   invoked when the v3 range scan returns >= 1 row (perf cliff
   defense).

8. **Public API is unchanged.** `save_counterexample_bundle`,
   `load_counterexample_bundle_events`,
   `count_counterexample_bundle_events`, `bundle_events_or_legacy`,
   `bundle_events_count_or_legacy` keep their signatures. Callers
   in chronos-services, chronos-cli, chronos-mcp are unchanged.
   The single chokepoint
   (`bundle_events_or_legacy` and the underlying
   `load_counterexample_bundle_events`) absorbs the v2 / v3
   branching internally; this matches the m9-02 D5 precedent.

9. **Disclosures R1–R4** enumerate the on-disk compat contract
    for future readers (§6).

### Non-goals (deferred to m9+)

- **GC of orphaned v2 chunks.** A v2 bundle re-saved in v3 removes
  its old chunks (D6 + R3). But bundles that were never re-saved
  still carry v2 chunks on disk. They keep loading via the legacy
  reader. A future "v2 → v3 rewrite tool" or
  `chronos test bundle-gc` migration is m9+ scope. Documented as
  R4.
- **Hash-truncation collision avoidance via 32-byte prefix.** The
  16-byte truncation is enough for our scale (see R2). Bumping to
  a full 32-byte prefix would add 16 bytes per key and is
  unnecessary.
- **Bloom filter or secondary index.** m9-04 makes the side-table
  primary key range-scan-friendly; that is sufficient for all
  current read paths. A future cycle that adds
  "list bundles whose events contain property X" semantics would
  need an index; out of scope.
- **Compression of side-table chunks** (m9-02 R5) — m9+ scope.
- **Wider schema-version migration tooling** — the m9-01 D3
  hard-reject policy remains; m9-04 does not relax it.
- **Per-bundle v2/v3 layout marker on disk.** The discriminator is
  the result of the v3 range scan (empty → try v2). No new column
  on the bundle record; the only "schema_version = 3" signal is
  the bumped constant.
- **Reworking `BUNDLE_EVENTS_CHUNK_SIZE`.** Still 256 events per
  chunk. The m9-02 R5 / R7 trade-offs are unchanged.
- **Storing the bundle_id twice on disk.** The v3 layout stores
  bundle_id once in the side-table value (alongside
  `Vec<TraceEvent>`); the legacy v2 layout stored it once in the
  key. The on-disk byte count per chunk goes from
  `4 + N_id + 4 + payload` to `16 + 4 + 4 + N_id + payload`
  (16-byte hash replaces 4-byte length prefix; bundle_id moves
  from key to value). Net delta: +20 bytes per chunk for a
  36-byte uuid::v7 (16 hash bytes − 4 length bytes + 4 id-be-write
  cost ≈ +16 bytes per row). Acceptable: at 4 chunks per typical
  bundle, ~64 bytes per bundle.

## §3 Architectural decisions

### D1 — 16-byte blake3 prefix (not 32, not SHA-256)

The prefix is `blake3(bundle_id)[..16]`. Reasoning:

- blake3 is already a chronos-store dependency
  (`crates/chronos-store/Cargo.toml:9`); no new crate, no
  audit, no Cargo.lock churn.
- 16 bytes is enough. Collision probability for 10^9 distinct
  bundle_ids is ~10^-21 (birthday bound at
  `sqrt(2^128) ≈ 1.8 × 10^19`). Even at 10^12 bundles
  (unrealistic for a local on-disk store) the per-save collision
  probability is ~10^-15. Documented as R2.
- Storing 32 bytes instead would give 16 extra bytes per key and
  no measurable safety improvement at our scale.
- blake3 over a 36-byte uuid::v7 string is ~50 ns per hash —
  negligible compared to the bincode::serialize of 256 events
  (~25 µs at typical event sizes).
- The hash is stored in the key; bundle_id is stored in the
  value. The reader verifies the value's bundle_id matches the
  requested bundle_id (defense against a hypothetical
  truncation-collision; ~free).

### D2 — Why we keep the legacy reader (vs only v3)

m9-04 must not invalidate bundles written by m9-02 / m9-03. Those
bundles already exist on disk with v2 keys (and may exist in CI
caches, in operator backups, in deployed chronos instances). The
two paths to handle them:

- **(a) Reject with `StoreError::Serialization`.** Hard-reject
  pre-m9-04 bundles the way the m9-01 D3 policy rejects future-
  versioned bundles. Incompatible with R1 (compatibility with
  v2 rows is the whole point of this cycle).
- **(b) Fall back to a v2 reader.** Open the side table, scan
  all rows, decode via `decode_chunk_key_legacy`, filter by
  bundle_id. Cost is identical to the current
  `collect_bundle_chunks`. Use only when the v3 range scan
  returns 0 entries for a bundle whose `summary.events_count > 0`.

**Choice: (b).** The fallback is invisible to callers, runs only
on the read path for v2 bundles, and disappears naturally as soon
as the bundle is re-saved (lazy migration). It costs us one
`if v3_range_empty && summary.events_count > 0 { try_legacy() }`
branch in the read helper, which is a 5-line conditional.

### D3 — Value carries `bundle_id` (vs decoding from key)

In v2 the key was self-describing: `[len][id][chunk_idx]` decoded
straight back to `(bundle_id, chunk_index)`. In v3 the key is
`[hash][chunk_idx]`, which decodes to `(hash_prefix,
chunk_index)` only — the bundle_id bytes are not in the key.

Two options:

- **(a) Value carries bundle_id:**
  `bincode((String, Vec<TraceEvent>))`. The reader extracts
  bundle_id from each value to verify it matches the requested
  bundle_id (R2: defense against hypothetical hash collisions).
  Adds ~36 bytes per row for uuid::v7 ids, but enables the
  identity verification.
- **(b) Value carries only `Vec<TraceEvent>`:**
  `bincode(Vec<TraceEvent>)`. The reader trusts the hash prefix
  to disambiguate. Saves 36 bytes per row but loses the
  identity-verification defense.

**Choice: (a).** The 36 bytes per row cost is negligible (4 chunks
per bundle × 36 bytes = ~144 bytes per bundle) and the
identity-check makes the fallback / hash-collision reasoning
clean. This is also future-proof: a "list bundles whose events
match predicate X" tool can use the value's bundle_id without a
separate lookup.

### D4 — Range-scan upper bound trick

redb's `range(impl RangeBounds<KR>)` with `&[u8]` keys uses
lexicographic comparison. We want "all keys starting with
`prefix || anything`" = `[prefix || 0x00..0x00,
prefix || 0xFF..0xFF]`. Since we want a half-open `[start, end)`
and the next bundle's prefix is different, we use
`prefix || 0xFF 0xFF 0xFF 0xFF` as the exclusive upper bound —
this is the maximum possible `[prefix || chunk_index]` value
(chunk_index ∈ [0, u32::MAX]). Concretely:

```rust
let mut upper = Vec::with_capacity(20);
upper.extend_from_slice(prefix);          // 16 bytes
upper.extend_from_slice(&[0xFF; 4]);      // chunk_index upper bound

table.range(prefix.as_slice()..upper.as_slice())
```

Iteration yields keys whose first 16 bytes equal `prefix` (i.e.,
all chunks of the bundle). The upper bound excludes the next
bundle's first key (whose first 16 bytes differ). Verified by
hand with redb's `impl Key for &[u8]` (`redb-2.6.3/src/types.rs:317`).

### D5 — `CURRENT_BUNDLE_SCHEMA_VERSION = 3`

The version bump is mechanical and follows the m9-02 R1 policy:

- m9-01: bumped from "absent" to 1 (added the field).
- m9-02: bumped from 1 to 2 (events moved to side table +
  `events_count` added).
- m9-04: bumped from 2 to 3 (side-table value carries
  `bundle_id`).

The `KNOWN_BUNDLE_SCHEMA_VERSIONS` array grows from `[1, 2]` to
`[1, 2, 3]`. The m9-01 D3 hard-reject policy
(`record.schema_version > CURRENT` → `Err(Serialization)`) is
unchanged. Bundles written by future cycles (v4+) are still
rejected until a future cycle bumps the constant again.

The version bump does NOT appear in the side-table key or value
schema; it's a record-level field on `CounterexampleBundleRecord`
(m9-01 D1) and on `CounterexampleBundleSummary` (m9-01 D1). The
side-table itself stores values for both v=2 (legacy layout) and
v=3 (new layout) bundles without a per-row version stamp; the
distinction is the key bytes themselves.

### D6 — Save deletes both v3 and v2 chunks

`save_bundle_record_and_events` (the m9-02 D4 atomic wrapper)
now does, in this order inside one write transaction:

1. Compute `prefix = blake3(bundle_id)[..16]`.
2. Open the v3 read range `[prefix..prefix || 0xFFFFFFFF]`,
   collect existing v3 keys for this bundle, remove them.
3. **Defensive legacy cleanup**: open a read transaction (R3
   explains the WHY of this read), scan the whole table, decode
   each key via `decode_chunk_key_legacy`, filter by
   `bundle_id`, collect those keys. This is the ONE remaining
   full-table scan in the codebase; it runs only on save, only
   for the bundle being saved, and only over the side table
   (which is bounded by `BUNDLE_EVENTS_CHUNK_SIZE × total
   bundles` rows).
4. Open the events table inside the write transaction, remove
   both v3 keys (from step 2) and v2 keys (from step 3).
5. Write new chunks under v3 keys.
6. `commit()`.

After step 6 the side table contains ONLY v3 chunks for this
bundle. Any v2 chunks for the same bundle are gone. Future
reads hit the v3 range scan and never touch the legacy reader.
The migration window for this bundle is closed.

### D7 — Read path: v3 range-scan first, v2 full-scan fallback

`load_counterexample_bundle_events(bundle_id)`:

```rust
let prefix = blake3(bundle_id)[..16].to_vec();
let mut upper = Vec::with_capacity(20);
upper.extend_from_slice(&prefix);
upper.extend_from_slice(&[0xFF; 4]);

// 1) v3 range scan
let v3_chunks: Vec<(u32, Vec<u8>)> = collect_v3_range(&tx, &prefix, &upper)?;
// 2) verify: each value's bundle_id must match (R2 defense)
let v3_decoded: Vec<(u32, Vec<u8>)> = v3_chunks
    .into_iter()
    .filter(|(_, bytes)| {
        // decode (bundle_id, Vec<TraceEvent>) from value; keep if bundle_id matches
        decode_value_bundle_id(bytes)
            .map(|id| id == bundle_id)
            .unwrap_or(false)
    })
    .collect();
if !v3_decoded.is_empty() {
    return Ok(deserialize_and_concat(v3_decoded));
}
// 3) v2 fallback (only if v3 was empty AND summary.events_count > 0)
if bundle_has_v2_chunks {
    return Ok(legacy_full_scan_and_filter(&tx, bundle_id));
}
Ok(Vec::new())
```

The `bundle_has_v2_chunks` signal in step 3 is the
`summary.events_count > 0` check on the already-loaded
`CounterexampleBundleRecord` (the caller has it from
`load_counterexample_bundle`). If the record's events_count is
0, the bundle has no side-table chunks at all — v2 OR v3 — so
the legacy scan would only return empty rows. Skipping it keeps
a "fresh, never-shrunk bundle" cheap.

The caller is `bundle_events_or_legacy(bundle)` (the m9-02 D5
chokepoint), which already loads the record before calling
`load_counterexample_bundle_events`, so the `events_count`
signal is in scope.

### D8 — Hash-collision defense in depth (R2)

Two defense layers in case the truncated-blake3 prefix collides:

1. **Per-chunk identity check.** Every v3 chunk carries
   `bundle_id` in its value (D3). The reader verifies
   `decoded_bundle_id == requested_bundle_id` per chunk. A
   collision would manifest as a chunk with the right prefix but
   the wrong bundle_id; the filter in D7 step 2 drops it. The
   cost is one bincode deserialize per chunk of the value's
   `(String, Vec<TraceEvent>)` header — we already pay this to
   extract the events, so the identity check is amortized into
   the existing deserialize.

2. **Cross-bundle sanity check (R2 test).** A test inserts two
   synthetic bundles with bundle_ids chosen to have the same
   blake3[..16] prefix (forced via a helper that bypasses
   `blake3::hash`), saves them, loads each, and asserts that
   each load returns ONLY its own chunks. This pins the
   "prefix collision is contained" invariant.

Both layers are documented in §5 tests.

## §4 Implementation sketch

### chronos-store (1 file, ~80 LoC delta)

`crates/chronos-store/src/counterexample_storage.rs`:

```rust
// Existing constants / structs unchanged above this line.

/// Side table for counterexample bundle events.
///
/// Key layout (m9-04):
///   [blake3(bundle_id)[..16] (16 bytes)][chunk_index (4 bytes BE)]
///   = 20 bytes total, fixed width.
///
/// Value layout (m9-04):
///   bincode((bundle_id: String, Vec<TraceEvent>))
///   — bundle_id is the identity verification (D3 + R2).
///
/// Legacy v2 layout (pre-m9-04) is still readable via
/// `decode_chunk_key_legacy` + a defensive full-table scan; see
/// `load_counterexample_bundle_events` (D7).
const COUNTEREXAMPLE_BUNDLE_EVENTS: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("counterexample_bundle_events");

/// m9-04: bumped from 2 (m9-02) to 3 (side-table value carries
/// bundle_id; key layout moves to fixed-width hash prefix).
/// Pre-m9-04 bundles keep schema_version 2 (or 1) via
/// `#[serde(default)]`; the loader reads v2 chunks via the
/// legacy fallback when v3 range scan is empty.
pub const CURRENT_BUNDLE_SCHEMA_VERSION: u32 = 3;

const KNOWN_BUNDLE_SCHEMA_VERSIONS: &[u32] = &[1, 2, 3];

/// Encode `(bundle_id, chunk_index)` into a v3 byte key.
///
/// Layout: [blake3(bundle_id)[..16]][chunk_index as u32 BE]
///        = 20 bytes total.
fn encode_chunk_key(bundle_id: &str, chunk_index: u32) -> Vec<u8> {
    let hash = blake3::hash(bundle_id.as_bytes());
    let mut key = Vec::with_capacity(20);
    key.extend_from_slice(&hash.as_bytes()[..16]);
    key.extend_from_slice(&chunk_index.to_be_bytes());
    key
}

/// Decode a v3 byte key back to (hash_prefix, chunk_index).
/// The bundle_id bytes are not in the key; see D3 (the value
/// carries them).
fn decode_chunk_key(key: &[u8]) -> Option<([u8; 16], u32)> {
    if key.len() != 20 { return None; }
    let mut prefix = [0u8; 16];
    prefix.copy_from_slice(&key[..16]);
    let chunk_bytes: [u8; 4] = key[16..20].try_into().ok()?;
    Some((prefix, u32::from_be_bytes(chunk_bytes)))
}

/// Decode a v2 (pre-m9-04) byte key back to (bundle_id,
/// chunk_index). Variable-width; used only by the legacy
/// fallback (D7 step 3) and by the save-time defensive cleanup
/// (D6 step 3).
fn decode_chunk_key_legacy(key: &[u8]) -> Option<(String, u32)> {
    // existing m9-02 / m9-03 implementation, unchanged.
}

/// Encode `(bundle_id, chunk_index, chunk)` into a v3 value.
///   bincode((String, Vec<TraceEvent>))
fn encode_chunk_value(bundle_id: &str, chunk: &[TraceEvent]) -> Vec<u8> {
    bincode::serialize(&(bundle_id.to_string(), chunk.to_vec()))
        .expect("Vec<TraceEvent> + String bincode is infallible")
}

/// Decode a v3 byte value back to (bundle_id, Vec<TraceEvent>).
/// Used for identity verification (D8) and chunk extraction.
fn decode_chunk_value(bytes: &[u8]) -> Option<(String, Vec<TraceEvent>)> {
    bincode::deserialize(bytes).ok()
}

/// Compute the 16-byte range-scan prefix for `bundle_id`.
fn bundle_prefix(bundle_id: &str) -> [u8; 16] {
    let hash = blake3::hash(bundle_id.as_bytes());
    let mut prefix = [0u8; 16];
    prefix.copy_from_slice(&hash.as_bytes()[..16]);
    prefix
}

/// Read chunks for `bundle_id` via the v3 range scan (D7 step 1).
/// Returns `Ok(vec![])` when the table does not exist or no
/// chunks match.
fn collect_bundle_chunks_range(
    tx: &redb::ReadTransaction,
    bundle_id: &str,
) -> Result<Vec<(u32, Vec<u8>)>, StoreError> {
    let table = match tx.open_table(COUNTEREXAMPLE_BUNDLE_EVENTS) {
        Ok(t) => t,
        Err(redb::TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
        Err(e) => return Err(StoreError::Database(e.into())),
    };
    let prefix = bundle_prefix(bundle_id);
    let mut upper = Vec::with_capacity(20);
    upper.extend_from_slice(&prefix);
    upper.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);

    let mut chunks = Vec::new();
    for entry in table.range(prefix.as_slice()..upper.as_slice())
        .map_err(|e| StoreError::Database(e.into()))?
    {
        let (k, v) = entry.map_err(|e| StoreError::Database(e.into()))?;
        let key_bytes: &[u8] = k.value();
        let (_, chunk_index) = match decode_chunk_key(key_bytes) {
            Some(decoded) => decoded,
            None => continue,  // skip malformed key (defensive)
        };
        // D8 step 1: identity check via the value's bundle_id.
        if let Some((ref id, _)) = decode_chunk_value(v.value()) {
            if id != bundle_id {
                continue;  // R2 defense: collision skip
            }
        } else {
            continue;  // malformed value skip
        }
        chunks.push((chunk_index, v.value().to_vec()));
    }
    Ok(chunks)
}

/// Legacy v2 full-table scan for `bundle_id`. Used only when
/// the v3 range scan is empty AND the bundle's
/// `summary.events_count > 0` (D7 step 3). Cost: O(table size).
fn collect_bundle_chunks_legacy(
    tx: &redb::ReadTransaction,
    bundle_id: &str,
) -> Result<Vec<(u32, Vec<u8>)>, StoreError> {
    let table = match tx.open_table(COUNTEREXAMPLE_BUNDLE_EVENTS) {
        Ok(t) => t,
        Err(redb::TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
        Err(e) => return Err(StoreError::Database(e.into())),
    };
    let mut chunks = Vec::new();
    for entry in table.iter().map_err(|e| StoreError::Database(e.into()))? {
        let (k, v) = entry.map_err(|e| StoreError::Database(e.into()))?;
        if let Some((ref id, idx)) = decode_chunk_key_legacy(k.value()) {
            if id == bundle_id {
                chunks.push((idx, v.value().to_vec()));
            }
        }
    }
    Ok(chunks)
}

/// Collect all chunk rows for `bundle_id`, trying v3 range
/// scan first then v2 legacy fallback.
///
/// This replaces the m9-02 / m9-03 `collect_bundle_chunks`.
/// Cost: O(chunks of bundle) on the v3 path (typical);
/// O(table size) on the v2 path (only during the migration
/// window for never-re-saved v2 bundles).
#[allow(clippy::result_large_err)]
fn collect_bundle_chunks(
    tx: &redb::ReadTransaction,
    bundle_id: &str,
) -> Result<Vec<(u32, Vec<u8>)>, StoreError> {
    let v3 = collect_bundle_chunks_range(tx, bundle_id)?;
    if !v3.is_empty() {
        return Ok(v3);
    }
    collect_bundle_chunks_legacy(tx, bundle_id)
}
```

### Save path: `save_bundle_record_and_events` changes

```rust
fn save_bundle_record_and_events(
    &self,
    record: CounterexampleBundleRecord,
    events: Vec<TraceEvent>,
) -> Result<String, StoreError> {
    let bundle_id = record.summary.bundle_id.clone();
    let bytes = bincode::serialize(&record)
        .map_err(|e| StoreError::Serialization(e.to_string()))?;

    let tx = self.db().begin_write()
        .map_err(|e| StoreError::Database(e.into()))?;

    // 1. Write the record (events field is empty at this point).
    {
        let mut table = tx.open_table(COUNTEREXAMPLE_BUNDLES)
            .map_err(|e| StoreError::Database(e.into()))?;
        table.insert(bundle_id.as_bytes(), bytes.as_slice())
            .map_err(|e| StoreError::Database(e.into()))?;
    }

    // 2. Collect prior v3 keys for this bundle (D6 step 2).
    let prefix = bundle_prefix(&bundle_id);
    let mut upper = Vec::with_capacity(20);
    upper.extend_from_slice(&prefix);
    upper.extend_from_slice(&[0xFF; 4]);
    let prior_v3_keys: Vec<Vec<u8>> = {
        let read_tx = self.db().begin_read()
            .map_err(|e| StoreError::Database(e.into()))?;
        let table = match read_tx.open_table(COUNTEREXAMPLE_BUNDLE_EVENTS) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => {
                // skip — no events table at all yet.
            }
            Err(e) => return Err(StoreError::Database(e.into())),
        };
        let mut keys = Vec::new();
        for entry in table.range(prefix.as_slice()..upper.as_slice())
            .map_err(|e| StoreError::Database(e.into()))?
        {
            let (k, _v) = entry.map_err(|e| StoreError::Database(e.into()))?;
            let key_bytes: &[u8] = k.value();
            if decode_chunk_key(key_bytes).is_some() {
                keys.push(key_bytes.to_vec());
            }
        }
        keys
    };

    // 3. Defensive v2 cleanup (D6 step 3, R3).
    let prior_v2_keys: Vec<Vec<u8>> = {
        let read_tx = self.db().begin_read()
            .map_err(|e| StoreError::Database(e.into()))?;
        collect_bundle_chunks_legacy(&read_tx, &bundle_id)?
            .into_iter()
            .map(|(idx, _)| encode_chunk_key_legacy(&bundle_id, idx))
            .collect()
    };

    // 4. Remove prior chunks (v3 + v2) and write new v3 chunks.
    {
        let mut events_table = tx.open_table(COUNTEREXAMPLE_BUNDLE_EVENTS)
            .map_err(|e| StoreError::Database(e.into()))?;
        for key in prior_v3_keys.iter().chain(prior_v2_keys.iter()) {
            events_table.remove(key.as_slice())
                .map_err(|e| StoreError::Database(e.into()))?;
        }
        for (chunk_idx, chunk) in events.chunks(BUNDLE_EVENTS_CHUNK_SIZE).enumerate() {
            let key = encode_chunk_key(&bundle_id, chunk_idx as u32);
            let value = encode_chunk_value(&bundle_id, chunk);
            events_table.insert(key.as_slice(), value.as_slice())
                .map_err(|e| StoreError::Database(e.into()))?;
        }
    }

    tx.commit().map_err(|e| StoreError::Database(e.into()))?;
    Ok(bundle_id)
}
```

### Save public entry: bump schema_version (m9-01 D5 precedent)

```rust
pub fn save_counterexample_bundle(
    &self,
    record: CounterexampleBundleRecord,
) -> Result<String, StoreError> {
    // ... existing validation ...
    let mut record = record;
    record.schema_version = CURRENT_BUNDLE_SCHEMA_VERSION;     // 3
    record.summary.schema_version = CURRENT_BUNDLE_SCHEMA_VERSION;
    let events = mem::take(&mut record.events);
    record.summary.events_count = events.len() as u64;
    self.save_bundle_record_and_events(record, events)
}
```

### Load path: chokepoint gains v3 / v2 branching

The m9-02 `load_counterexample_bundle_events` keeps its signature
and delegates to `collect_bundle_chunks` (which now does v3
range + v2 legacy fallback — D7). No caller changes.

### chronos-services / chronos-cli / chronos-mcp (no change)

`bundle_events_or_legacy`, `bundle_events_count_or_legacy`,
`save_counterexample_bundle` (services-side), and
`run_replay` are unchanged. The chokepoint hides the
v3/v2 branching. The wire (`counterexample_events_count`,
`counterexample_get`, future `counterexample_bundle_events`)
is unchanged.

## §5 Tests

### Unit (chronos-store lib)

- `m9_04_save_then_load_roundtrip_under_v3_layout` — save a
  bundle with N events, load via
  `load_counterexample_bundle_events`, assert the loaded
  events match (positions 0..N). Asserts the v3 encode/decode
  pair is correct.
- `m9_04_save_uses_fixed_width_v3_keys` — save 3 bundles (b1,
  b2, b3), assert each bundle's chunks have 20-byte keys starting
  with the same 16-byte prefix within a bundle and a different
  prefix across bundles (sanity check on `encode_chunk_key`).
- `m9_04_legacy_v2_bundle_loads_via_fallback` — write v2 keys
  (variable-width `[len][id][chunk_idx]`) directly into the
  events table using `encode_chunk_key_legacy`, then call
  `load_counterexample_bundle_events(bundle_id)`. Assert
  events are returned correctly. This is the migration-window
  test.
- `m9_04_resave_migrates_v2_to_v3` — write v2 chunks for a
  bundle, call `save_counterexample_bundle(record)` for the
  same bundle, assert the side table now contains only v3
  keys (no v2 keys remain for this bundle_id). The legacy
  reader would return empty if called after the resave.
- `m9_04_range_scan_is_bounded_to_one_bundle` — save chunks
  for bundles A and B, call
  `collect_bundle_chunks_range(tx, "A")` directly, assert the
  returned chunks have key prefix == `blake3("A")[..16]` and
  the iteration count equals A's chunk count (not A + B's).
- `m9_04_collision_containment` — synthesize two bundle_ids
  `b_real` and `b_collision` whose `blake3[..16]` prefixes
  collide (forced via a test-only helper that bypasses
  `blake3::hash` and writes a chosen prefix into the key
  directly), save chunks for both, load each via
  `load_counterexample_bundle_events`, assert each load
  returns ONLY its own chunks (D8 step 1 identity check).
- `m9_04_unknown_bundle_returns_empty_without_legacy_scan` —
  call `load_counterexample_bundle_events("ghost")` on a
  fresh store, assert `Ok(vec![])` AND assert no full-table
  scan was needed (D7 step 3 guard: events_count == 0).
- `m9_04_save_writes_value_carrying_bundle_id` — save a bundle,
  inspect a side-table value directly, assert
  `bincode::decode::<(String, Vec<TraceEvent>)>` succeeds and
  the String equals `bundle_id`.
- `m9_04_loader_rejects_v4_bundle` — hand-craft a bundle with
  `schema_version = 4` (future), assert
  `load_counterexample_bundle` returns
  `Err(StoreError::Serialization(_))` (m9-01 D3 unchanged).
- `m9_04_summary_carries_schema_version_3_after_save` — save
  a bundle, load it, assert
  `record.summary.schema_version == 3` AND
  `record.schema_version == 3` (D5 + m9-01 D1).
- `m9_04_v2_bundle_with_events_count_zero_loads_normally` —
  insert a bundle with `schema_version = 2` and `events_count =
  0` directly into the DB, load via
  `bundle_events_or_legacy`, assert legacy branch returns
  `vec![]` (events live in blob OR in v2 side table; both
  empty here).

### Per-crate integration (chronos-services, chronos-cli)

- `chronos_services::counterexample::tests::m9_04_save_load_roundtrip_through_services`
  — call `ChronosCounterexampleService::save` with N events,
  then `events_count` (O(1) via summary), then
  `bundle_events_or_legacy` and assert the events match the
  input. This pins the chokepoint under the v3 layout end-to-
  end.
- `chronos_cli::replay::tests::m9_04_replay_uses_v3_layout`
  — save a bundle via the API, call `run_replay`, assert the
  verdict is "violation" (events flow through
  `QueryEngine::new`).
- `chronos_cli::replay::tests::m9_04_replay_v2_bundle_uses_legacy_path`
  — hand-craft a v2 bundle with chunks in the legacy key
  layout, call `run_replay`, assert the events flow through
  (legacy branch is exercised).

### Sandbox (T4-smoke subset re-runs)

- `counterexample_tools` (ce1..ce12) — every shrink now
  persists events under v3 keys; every events_count + replay
  reads via the v3 range scan (or the v2 fallback for any
  fixture that constructs a v2 bundle directly).
- `e2e_connectivity` — confirms the MCP server starts and
  the first tool call succeeds (no regression in the
  chronos-mcp wiring).

No new sandbox tests required; the existing fixtures cover
save → get → events_count → replay exhaustively. The
chronos-store lib unit tests in this section pin the storage
contract; the chronos-services + chronos-cli integration
tests pin the consumer wrappers.

## §6 Disclosures

### R1 (NEW) — Side-table key layout changed (m9-04)

The `counterexample_bundle_events` key changed from
`[len(bundle_id)][bundle_id bytes][chunk_index]` (variable-
width, m9-02 / m9-03 layout, "v2 layout" on disk) to
`[blake3(bundle_id)[..16]][chunk_index]` (fixed-width 20 bytes,
m9-04 layout, "v3 layout" on disk). The value layout changed
from `bincode(Vec<TraceEvent>)` to
`bincode((bundle_id, Vec<TraceEvent>))`. Both changes are
invisible to callers (the `load_counterexample_bundle_events`
chokepoint absorbs the branching) but matter for any future
reader that introspects the side table directly. Documented at
the module-level doc comment (see R7).

### R2 (NEW) — Hash-collision risk is bounded by 16-byte truncation

`blake3(bundle_id)[..16]` is a 128-bit prefix. The birthday-
collision probability per pair of distinct bundle_ids is
~2^-128; per-save collision probability for a store with N
distinct bundles is `~N^2 / 2^129` (e.g., 10^6 bundles →
~10^-19). At our realistic scale (thousands of bundles over
the lifetime of an instance) the expected number of
collisions is effectively zero. Defense in depth: the value
carries the bundle_id (D3), and the reader verifies it
matches (D8 step 1). A test (`m9_04_collision_containment`)
forces a collision via a test-only helper and asserts the
defense holds. If the truncation ever proves insufficient,
bumping to 32 bytes (or to a full SHA-256 prefix) is a single-
constant change (`bundle_prefix` returns the first N bytes
of `blake3::hash`).

### R3 (NEW) — Save does one defensive full-table scan

`save_bundle_record_and_events` opens a read transaction,
calls `collect_bundle_chunks_legacy(tx, bundle_id)`, and uses
the returned keys to remove any v2 chunks for the bundle
being saved. This is the **only** remaining full-table scan
in the codebase. It runs once per save, only reads (no
deserialization of event payloads — only key decoding), and
disappears once a bundle is re-saved (D6: the v2 keys are
deleted, so subsequent saves find no v2 keys to remove).
Long-term (after every existing v2 bundle has been re-saved
at least once) this scan returns an empty set and its cost
collapses to a single iteration over zero rows. Until then,
it is the migration cost.

### R4 (NEW) — V2 bundles never re-saved keep using the legacy path

A v2 bundle that is never re-saved (no subsequent shrink,
no replay-re-save, no future `chronos test bundle-migrate`
tool call) will keep its chunks in v2 layout forever. Every
read for that bundle triggers the v2 fallback scan (D7 step
3). The cost is bounded by the table size and grows linearly
with the number of never-re-saved v2 bundles. For an operator
with hundreds of v2 bundles, this is the expected overhead
until they re-shrink each one. A future "rewrite v2 → v3"
migration tool is m9+ scope. Until then, the legacy fallback
is the compatibility layer.

### R5 (NEW) — Bumped `CURRENT_BUNDLE_SCHEMA_VERSION` to 3 (m9-04)

The constant moves from 2 (m9-02) to 3 (m9-04). Bundles
persisted by future chronos-store builds with `schema_version
= 4` are hard-rejected by the m9-01 D3 loader policy.
Pre-m9-04 bundles continue to deserialize as `schema_version
= 2` via `#[serde(default)]` and load through the legacy
fallback when present. No migration pass.

### R6 (NEW) — `KNOWN_BUNDLE_SCHEMA_VERSIONS` grew to `[1, 2, 3]`

The array that the loader consults (m9-01 D2) grows from
`[1, 2]` to `[1, 2, 3]`. The array is currently unused
(`#[allow(dead_code)]` on the m9-01 constant); m9-04 leaves
the allow in place and adds 3 to the array for symmetry.
A future cycle that uses the array for telemetry
("how many bundles of each known version exist?") will
consult this list.

### R7 (NEW) — Module-level doc comment gains a "Key layout (m9-04)"
section

`crates/chronos-store/src/counterexample_storage.rs` has a
module-level doc comment that documents the side table's
chunk size and the legacy-vs-new branching (m9-02). m9-04
extends it with a "Key layout (m9-04)" section describing
the v3 key format (`[blake3(bundle_id)[..16]][chunk_index]`,
20 bytes fixed width), the value format
(`bincode((bundle_id, Vec<TraceEvent>))`), and a one-paragraph
explanation of the v2-compat fallback. R3 of m9-01 set the
precedent for module-level doc comments as the place where
schema-level decisions live.

## §7 Tier & gate plan

This is **A-min** scope per the cycle status (one bounded
store-layer refactor + 11 unit tests + 3 integration tests,
no architectural fork, no MCP surface change):

* **T0**: fmt + clippy (`-D warnings`).
* **T1**: chronos-store lib (11 new unit tests pinning the
  v3 layout + v2 fallback + collision defense).
* **T2**: chronos-services + chronos-cli integration tests
  (3 new tests covering the chokepoint under v3 + v2).
* **T4-smoke**: re-run `counterexample_tools` (ce1..ce12) +
  `e2e_connectivity` (server-startup canary). The existing
  suite already exercises every changed path under v3 layout
  (saves write v3 keys; loads hit the v3 range scan).

No chronos-e2e (D bucket — out of scope, no ptrace changes).
No benches (E bucket — out of scope; the perf improvement
shows up in the per-save timing of m9-02 fixtures but the
bench suite today doesn't measure it, and the cycle is
"close the debt finding" not "publish a benchmark").

## §8 Smoke subset chosen

Per AGENTS.md §2, the cycle touches:

- The `counterexample_bundle_events` side table key + value
  encoding (every save, load, count).
- The `save_bundle_record_and_events` atomic wrapper
  (adds v2 cleanup branch).
- The `collect_bundle_chunks` helper (now a v3-first +
  v2-fallback wrapper).
- The `CURRENT_BUNDLE_SCHEMA_VERSION` constant (bumped 2 → 3).

No consumer crate in chronos-services, chronos-cli, or
chronos-mcp needs a code change; the chokepoint hides the
v3/v2 branching. The smoke subset is therefore:

- `counterexample_tools` (full file, 12 tests) — every shrink
  fixture now persists events under v3 keys; every
  events_count + replay path exercises the chokepoint.
- `e2e_connectivity` — confirms the MCP server starts and
  the first tool call succeeds (no regression in the
  chronos-mcp wiring, which is unchanged on the wire).

This matches the m9-02 / m9-03 subset exactly. The chronos-
store lib unit tests in §5 run at T1 and pin the storage
contract; the chronos-services + chronos-cli integration
tests in §5 run at T2 and pin the consumer wrappers.

## §9 What's NOT closed by m9-04

- **FIND-M9-02-DV-PERF-01** — closed by m9-04. The side-table
  read for one bundle is now O(chunks-of-bundle), not
  O(table-size).
- **FIND-M9-02-DV-API-01, FIND-M9-02-DV-DOC-01,
  FIND-M9-02-DV-OE-01, FIND-M9-02-DV-COUP-01** — closed by
  m9-03. m9-04 does not touch them.
- **m9-02 R1 (schema_version bump)** — closed by m9-04 (R5).
- **m9-02 R5 (chunk compression)** — still open, m9+ scope.
  m9-04 doesn't touch compression; the chunk size of 256 is
  unchanged.
- **m9-02 R6 (list path deserializes full blob per row)** —
  still open, m9+ scope. m9-04 doesn't touch the
  `counterexample_bundles` table; the list path is unchanged.
- **m9-02 R7 (re-save overwrites all prior chunks)** — closed
  by m9-02 D4. m9-04 inherits the contract: re-save deletes
  prior chunks (now both v3 and v2) before writing new ones.
- **R4 (v2 bundles never re-saved keep using the legacy
  path)** — open as m9-04 R4; future cycles can ship a
  `chronos test bundle-migrate-v2-to-v3` tool to eliminate
  the fallback path.
- **GC of orphaned v2 chunks** (m9-02 non-goal) — m9+ scope.
  m9-04's D6 step 3 (defensive cleanup on save) is
  per-bundle and per-save; it does not garbage-collect
  bundles that are deleted without a re-save (e.g., future
  `chronos test bundle-gc` would still need its own
  cross-bundle scan).
- **`counterexample_bundle_events` MCP tool** — m9+ scope.
  The m9-04 v3 layout is range-scan-friendly and makes the
  future `(offset, limit)` semantics efficient (load just
  the chunks covering the requested range), but the wire
  layer is still a separate cycle.

---

**Cross-references**:
- Precedence: `docs/milestones/m9-02-events-side-table-scoping.md`
  §3 D1–D8 (side table contract), `docs/milestones/m9-01-schema-versioning-scoping.md`
  §3 D1–D6 (schema version contract).
- Debt finding: `FIND-M9-02-DV-PERF-01` (m9-02 debt-report.md §"Warning
  Findings" #6 / `terms/index.md`).
- Implementation: `crates/chronos-store/src/counterexample_storage.rs`
  (lines 60–91 for current v2 encode/decode; lines 107–127 for
  `collect_bundle_chunks`; lines 350–414 for `save_bundle_record_and_events`;
  lines 421–454 for `load_counterexample_bundle_events` and
  `count_counterexample_bundle_events`).