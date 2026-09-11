# Design: m9-04 — Side-table key layout v3 (range-scan friendly)

**Cycle:** `p-3416cfb8288f8964/m9-04-side-table-key-layout`
**Path:** A-min | **Base:** `eb2cccd` | **Precedence:** scoping §3 D1–D8 + sibling `spec.md`.

## Technical Approach

Switch `counterexample_bundle_events` key from variable-width `[len][bundle_id][chunk_index]` to fixed-width 20-byte `[blake3(bundle_id)[..16] || chunk_index.to_be_bytes()]`. Bounded redb range scan per bundle replaces full scans. Values gain `bundle_id` for per-chunk identity defense. A v2 fallback (`collect_bundle_chunks_legacy`) decodes legacy rows on miss; wrapper `collect_bundle_chunks` is v3-first / v2-fallback. Save writes v3 and removes both v3 and v2 prior chunks atomically (D6 lazy migration). Schema bumps 2 → 3. Public APIs unchanged; chokepoint `bundle_events_or_legacy` absorbs the branching.

## Architecture Decisions

### D1 — 16-byte blake3 prefix (not 32)

| Aspect | Choice |
|---|---|
| Hash | `blake3(bundle_id).as_bytes()[..16]` |
| Width | 20 B = 16 B prefix + 4 B `u32` BE chunk_index |
| Rationale | blake3 already in `chronos-store/Cargo.toml:9`; 16 B ≈ 2¹²⁸ space; birthday at 10⁹ ids ≈ 10⁻²¹; net +16 B/chunk for a 36 B uuid::v7; bump to 32 is one constant. |

### D2 — Keep legacy reader as fallback

Pre-m9-04 bundles carry v2 keys on disk. Two options: (a) hard-reject (m9-01 D3 precedent); (b) read via a v2 fallback that fires only when the v3 range scan returns 0 rows. **Choice (b)** — invisible to callers, disappears after a re-save (D6).

### D3 — Value carries `bundle_id` (D8 defense)

```rust
fn encode_chunk_value(id: &str, chunk: &[TraceEvent]) -> Vec<u8> {
    bincode::serialize(&(id.to_string(), chunk.to_vec())).unwrap()
}
```

Reader decodes `(String, Vec<TraceEvent>)` and drops any chunk whose bundle_id differs. Identity check is amortized into the existing decode. Trade-off: ~36 B/chunk extra vs trusting the hash alone.

### D4 — Range-scan upper bound

Half-open `[start, end)` range; upper = `prefix || 0xFFFF_FFFF`. Covers every chunk_index `[0, u32::MAX]` while excluding the next bundle (whose first 16 B differ).

### D5 — Schema bump to 3

`CURRENT_BUNDLE_SCHEMA_VERSION = 3`; `KNOWN_BUNDLE_SCHEMA_VERSIONS = [1, 2, 3]`. Save sets both `record.schema_version` and `summary.schema_version`. m9-01 D3 hard-reject on `> CURRENT` is unchanged.

### D6 — Atomic save: remove v3 + v2 prior, write v3 new

`save_bundle_record_and_events` (single `begin_write`): read range → v3 keys; read full → v2 keys; write tx removes both sets and inserts new v3 chunks (D4 + D3 encoding); `commit()`. Re-saving a v2 bundle leaves only v3 chunks (D6 lazy migration).

### D7 — Read: v3 first, v2 second

Wrapper `collect_bundle_chunks` tries `collect_bundle_chunks_range` (D4 + identity filter); if empty, falls back to `collect_bundle_chunks_legacy` (full scan + `decode_chunk_key_legacy`).

### D8 — Test-only helper forces a collision

`__force_collision_prefix(b1, b2)` writes two bundles whose `blake3[..16]` matches by patching the key directly. Used by `m9_04_collision_containment` to verify the identity check survives a forced collision.

### D9 — Naming: keep `encode_chunk_key` for v3

| New | Body | Caller |
|---|---|---|
| `bundle_prefix(bundle_id)` | `blake3[..16]` | save, read |
| `encode_chunk_key` | v3 (blake3, 20 B) | save |
| `decode_chunk_key` | v3 → `(prefix, chunk_index)` | range + identity filter |
| `encode_chunk_key_legacy` | v2 (moved from old `encode_chunk_key`) | save cleanup |
| `decode_chunk_key_legacy` | v2 (moved) | `collect_bundle_chunks_legacy` |
| `encode_chunk_value`, `decode_chunk_value` | `(String, Vec<TraceEvent>)` | save, range scan |
| `collect_bundle_chunks_range` | v3 only | wrapper + save |
| `collect_bundle_chunks_legacy` | v2 only | wrapper + save cleanup |
| `collect_bundle_chunks` | v3 → v2 wrapper | load, count, save cleanup |

The 14 m9-02 unit tests keep passing: `encode_chunk_key` now writes v3, and `bincode::serialize(&chunk)` inside `save_bundle_record_and_events` becomes `encode_chunk_value(&bundle_id, chunk)` — chunk-storage contract is unaffected.

## Data Flow

```
save → one write tx:
  drop v3 keys (range scan) + v2 keys (full scan)
  insert v3 chunks via encode_chunk_key/encode_chunk_value
  commit

read → collect_bundle_chunks:
  v3 range scan + identity filter   (hot path)
  └→ v2 full scan via decode_chunk_key_legacy  (migration window)
```

## File Changes

| File | Action | Description |
|---|---|---|
| `crates/chronos-store/src/counterexample_storage.rs` | Modify | Bump schema constants. Rename v2 encode/decode → `_legacy`. Add v3 `encode_chunk_key`, `decode_chunk_key`, `bundle_prefix`, `encode_chunk_value`, `decode_chunk_value`, `collect_bundle_chunks_range`, `collect_bundle_chunks_legacy`. `collect_bundle_chunks` becomes a v3→v2 wrapper. `save_bundle_record_and_events` gains a v2 cleanup pass (D6). Module doc gains a "Key layout (m9-04)" section (R7). ~80 LoC delta + 11 unit tests. |
| (none in src/) | — | `chronos-services`, `chronos-cli`, `chronos-mcp` keep `bundle_events_or_legacy` chokepoint callers unchanged. 3 integration tests added in `services/tests/` and `cli/tests/` only. |

## Interfaces / Contracts

```rust
// chronos-store/src/counterexample_storage.rs
pub const CURRENT_BUNDLE_SCHEMA_VERSION: u32 = 3;
const KNOWN_BUNDLE_SCHEMA_VERSIONS: &[u32] = &[1, 2, 3];

fn bundle_prefix(bundle_id: &str) -> [u8; 16];
fn encode_chunk_key(id: &str, idx: u32) -> Vec<u8>;          // v3: 20 B
fn decode_chunk_key(k: &[u8]) -> Option<([u8; 16], u32)>;   // v3
fn encode_chunk_key_legacy(id: &str, idx: u32) -> Vec<u8>;  // v2: variable
fn decode_chunk_key_legacy(k: &[u8]) -> Option<(String, u32)>;
fn encode_chunk_value(id: &str, c: &[TraceEvent]) -> Vec<u8>;
fn decode_chunk_value(b: &[u8]) -> Option<(String, Vec<TraceEvent>)>;

fn collect_bundle_chunks_range(tx, id) -> Result<Vec<(u32, Vec<u8>)>, StoreError>;
fn collect_bundle_chunks_legacy(tx, id) -> Result<Vec<(u32, Vec<u8>)>, StoreError>;
fn collect_bundle_chunks(tx, id)        -> Result<Vec<(u32, Vec<u8>)>, StoreError>; // v3 → v2

// Public API: signatures unchanged.
impl SessionStore { /* save/load/count/bundle_events_or_legacy */ }
pub fn bundle_events_or_legacy(...) -> Result<Vec<TraceEvent>, StoreError>;
pub fn bundle_events_count_or_legacy(bundle: &CounterexampleBundleRecord) -> u64;
```

## Architecture Model

- **Impact:** local (one redb table key/value shape evolves; one schema constant bumped). No new external boundary.
- **Observed baseline:** m9-02 side table with variable-width key (lines 60–91 of `counterexample_storage.rs`).
- **Planned intent:** same table, fixed-width 20-byte blake3-keyed rows with `(bundle_id, Vec<TraceEvent>)` values; legacy rows still readable via fallback.
- **Render:** not_applicable (local impact).

## Testing Strategy

| Layer | What to Test | Approach |
|---|---|---|
| Unit (`chronos-store`) | 11 new tests per scoping §5 | `cargo test -p chronos-store --lib` |
| Per-crate (`chronos-services`) | save + events_count + bundle_events_or_legacy round-trip | `cargo test -p chronos-services --tests` |
| Per-crate (`chronos-cli`) | replay uses v3 path; replay uses v2 fallback for legacy bundle | `cargo test -p chronos-cli --tests` |
| Sandbox (T4-smoke) | `counterexample_tools` (ce1..ce12); `e2e_connectivity` | Pre-build `chronos-mcp`; export `CHRONOS_MCP_PATH` |

14 m9-02 unit tests keep passing (chunk-storage contract, not key encoding); 11 m9-04 tests pin the v3 contract (round-trip, key shape, v2 fallback, resave migrates v2→v3, range bound, collision containment, fresh-store ghost, value embeds bundle_id, v4 rejected, summary `schema_version == 3`, v2 bundle with `events_count == 0` loads normally).

## Migration / Rollout

**No migration pass.** Pre-m9-04 bundles deserialize via `#[serde(default)]` (m9-01) → `schema_version == 2`. v2 chunks stay on disk; the loader uses them through `collect_bundle_chunks_legacy` whenever the v3 range scan returns 0 rows. Re-saving any v2 bundle deletes its v2 chunks in the same transaction. Long-term, legacy scan returns empty and the cost collapses. Bundles never re-saved keep using the legacy path (R4); future migration tool is m9+.

## Open Questions

None. All scoping D1–D9 are ratified by spec requirements.

## ADR Candidates

- **D3 (value carries `bundle_id`)** — hard to reverse, surprising (splits "key has identity" vs "value has identity"), trade-off (+36 B/chunk). → `docs/adr/0072-side-table-value-carries-bundle-id.md`
- **D6 + D7 (dual cleanup + on-read fallback)** — hard to reverse, surprising, trade-off (lazy migration cost vs hard migration tool). → `docs/adr/0073-side-table-v2-fallback-lazy-migration.md`

## Standard Envelope

```yaml
status: success
executive_summary: |
  m9-04 fixes the side-table key layout to 20-byte fixed-width
  [blake3(bundle_id)[..16] || chunk_index] for bounded per-bundle
  range reads. Values gain bundle_id for identity defense. v2 rows
  stay readable via a fallback full scan gated to "v3 empty". Save
  atomically removes v3 and v2 prior chunks before writing v3
  (lazy migration). Schema bumps 2 → 3. Public APIs unchanged.
artifacts:
  - "sddk/p-3416cfb8288f8964/m9-04-side-table-key-layout/design"
summary:
  approach: fix-width blake3 prefix + v2 fallback reader + atomic dual cleanup
  key_decisions: 9 (D1-D9)
  files_affected: 0 new, 1 modified, 0 deleted
  testing_strategy: unit (chronos-store) + per-crate (services + cli) + T4-smoke subset
  adr_candidates: 2 (D3; D6+D7)
architecture:
  impact: local
  manifest_ref: null
  semantic_status: not_applicable
  render_status: not_applicable
open_questions: []
next_recommended: sddk-tasks
risks:
  - "save-time defensive v2 scan is the only remaining full-table scan (R3); documented & gated"
  - "v2 bundles never re-saved keep the legacy path on read (R4); bounded by table size"
```
