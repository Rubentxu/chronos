# Design: m9-02 — Bundle events side table

**Cycle:** `p-3416cfb8288f8964/m9-02-events-side-table`
**Path:** A-lite | **Base:** `48a9cff` (m9-01 close) | **Precedence:** `docs/milestones/m9-02-events-side-table-scoping.md` §3 D1–D8 + sibling `spec.md` + `tasks.md`.

## Technical Approach

Move `Vec<TraceEvent>` out of the bincode blob (`counterexample_bundles`) into a chunked redb side table (`counterexample_bundle_events`), keyed by `(bundle_id, chunk_index)`. Saves wrap both tables in one write transaction (D4). A free function `bundle_events_or_legacy(store, bundle)` is the single chokepoint consumers use (D5): pre-m9-02 bundles return their blob events; post-m9-02 bundles load chunks from the side table. The summary carries an `events_count: u64` (D2) so the wire tool reads the count without touching events.

The save path performs an in-place refactor — no public method signature change for `save_counterexample_bundle`. Consumers see no wire change; only `events_count`'s read site flips to the summary field (Phase 3.1).

## Architecture Decisions

### Decision D1 — Side table is chunked, not per-event

| Aspect | Choice |
|---|---|
| Shape | `bincode((String, u32))` key, `bincode(Vec<TraceEvent>)` value per chunk |
| Chunk size | `BUNDLE_EVENTS_CHUNK_SIZE: usize = 256` (≈ 25–125 KB bincode) |
| Rejected | per-event rows (1000 writes per save); single row per bundle (no lazy-load win) |

**Rationale:** 256 events keeps each row well under redb's default page size, gives 4 chunks for a typical 1000-event bundle, and bounds row count per bundle to ~10 for realistic sizes.

### Decision D2 — `events_count` lives on `CounterexampleBundleSummary`

| Aspect | Choice |
|---|---|
| Field | `pub events_count: u64` with `#[serde(default)] = 0` |
| Reader | `ChronosCounterexampleService::events_count` returns `summary.events_count` |
| Legacy | `summary.events_count == 0 && !record.events.is_empty()` → fall back to `record.events.len()` |

**Rationale:** O(1) wire path; survives legacy bundles (serde default 0). The fallback is the only code path that touches the blob for a count (R2).

### Decision D3 — `record.events: Vec<TraceEvent>` stays in the struct

The field carries `#[serde(default)]`. Pre-m9-02 blobs deserialize `events` populated; post-m9-02 saves write `events: vec![]`. The struct stays self-describing and the bincode on-disk format is forward-compatible.

### Decision D4 — Atomic save covers record + side table

`save_counterexample_bundle` calls a new private `save_bundle_record_and_events(record, events)` that opens ONE `begin_write()` and inserts both tables in one commit. Re-save deletes prior chunks for `bundle_id` first (R7). The public standalone `save_counterexample_bundle_events(bundle_id, events)` is NOT atomic w.r.t. the record (R3).

### Decision D5 — `bundle_events_or_legacy(store, bundle)` is the single chokepoint

```rust
pub fn bundle_events_or_legacy(
    store: &SessionStore,
    bundle: &CounterexampleBundleRecord,
) -> Result<Vec<TraceEvent>, StoreError> {
    if !bundle.events.is_empty() {
        return Ok(bundle.events.clone());    // legacy (pre-m9-02)
    }
    store.load_counterexample_bundle_events(&bundle.summary.bundle_id)
}
```

Lives in `chronos-store`. `chronos-cli::replay::run_replay` and any future consumer call this instead of `bundle.events.clone()`.

### Decision D6 / D7 / D8 (no new MCP tool / module-scoped constant / event_cas_hashes untouched)

D6: the `counterexample_bundle_events` MCP tool is m9+ (R4). D7: `BUNDLE_EVENTS_CHUNK_SIZE` lives next to `CURRENT_BUNDLE_SCHEMA_VERSION` at module scope (grep-able). D8: `event_cas_hashes` stays in the blob.

## Data Flow

    chronos-services::save()
        │
        ▼
    SessionStore::save_counterexample_bundle(record)
        │  ── mem::take(events)
        │  ── overwrite schema_version=2 on both record + summary
        │  ── set summary.events_count = events.len()
        ▼
    save_bundle_record_and_events(record, events)         (private; one write tx)
        │
        ├──► COUNTEREXAMPLE_BUNDLES        [bundle_id → bincode(record)]   (events: vec![])
        │
        └──► COUNTEREXAMPLE_BUNDLE_EVENTS  [(bundle_id, chunk_idx) → bincode(chunk)]
                delete prior chunks for bundle_id (R7)
                chunks(events, 256).enumerate().insert

    replay::run_replay(db, bundle_id)
        │
        ▼
    bundle_events_or_legacy(&store, &bundle)
        │
        ├── events.is_empty()? false  → bundle.events.clone()       (legacy v1)
        └── events.is_empty()? true   → load_counterexample_bundle_events(id)  (v2)

    services::events_count(bundle_id)
        │
        ▼
    load_counterexample_bundle(bundle_id)
        │
        ▼
    summary.events_count  (O(1); fallback to record.events.len() only when 0 + non-empty)

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `crates/chronos-store/src/counterexample_storage.rs` | Modify | Bump `CURRENT_BUNDLE_SCHEMA_VERSION`→2; `KNOWN_BUNDLE_SCHEMA_VERSIONS`→`[1,2]`; add `BUNDLE_EVENTS_CHUNK_SIZE` const; add `events_count` field on summary; add `COUNTEREXAMPLE_BUNDLE_EVENTS` table; add `save_bundle_record_and_events` (private); add `save_counterexample_bundle_events`, `load_counterexample_bundle_events`, `count_counterexample_bundle_events`, `bundle_events_or_legacy`. Refactor `save_counterexample_bundle` to delegate. ~120 LoC + tests. |
| `crates/chronos-services/src/counterexample.rs` | Modify | `events_count()` (~line 357) reads `record.summary.events_count` first, fallback to `record.events.len()` only when both conditions hold (D2/R2). Update `save()` (~line 524) — no logic change required; the store's atomic wrapper handles the split transparently (D4). ~5 LoC + 2 tests. |
| `crates/chronos-cli/src/replay.rs` | Modify | `run_replay` (line 76) replaces `QueryEngine::new(bundle.events.clone())` with `QueryEngine::new(bundle_events_or_legacy(&store, &bundle)?)`. Add `bundle_events_count: 0` to `synthetic_bundle` fixture. ~5 LoC + 2 tests. |
| (none) | — | No change in `crates/chronos-mcp` (D6) — wire tool just gets faster impl. |

## Interfaces / Contracts

```rust
// chronos-store/src/counterexample_storage.rs
pub const BUNDLE_EVENTS_CHUNK_SIZE: usize = 256;
pub const CURRENT_BUNDLE_SCHEMA_VERSION: u32 = 2;
const COUNTEREXAMPLE_BUNDLE_EVENTS: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("counterexample_bundle_events");

impl SessionStore {
    pub fn save_counterexample_bundle_events(&self, bundle_id: &str, events: Vec<TraceEvent>)
        -> Result<(), StoreError>;
    pub fn load_counterexample_bundle_events(&self, bundle_id: &str)
        -> Result<Vec<TraceEvent>, StoreError>;          // Ok(vec![]) when no chunks
    pub fn count_counterexample_bundle_events(&self, bundle_id: &str)
        -> Result<u64, StoreError>;                    // fallback-only path
}
pub fn bundle_events_or_legacy(
    store: &SessionStore, bundle: &CounterexampleBundleRecord,
) -> Result<Vec<TraceEvent>, StoreError>;

pub struct CounterexampleBundleSummary {
    // ... existing fields ...
    #[serde(default)] pub events_count: u64,            // m9-02
}
```

## Architecture Model

- **Impact:** local (one new redb table, schema bump, two consumer call-site edits). No new external boundary.
- **Observed baseline:** redb layout from m8-03 (`counterexample_bundles` blob carries events). Evidence: `crates/chronos-store/src/counterexample_storage.rs:42-43,182-207`.
- **Planned intent:** redb layout v2 — split events into `counterexample_bundle_events`, keep summary in `counterexample_bundles`. No new LikeC4 render required (impact is local).
- **Render:** not_applicable.

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit (`chronos-store`) | 10 new tests per scoping §5 (save/load/concat/count/idempotent/legacy/schema bump) | `cargo test -p chronos-store --lib` |
| Per-crate (`chronos-services`) | `events_count` reads from summary; `save` persists `events_count` | `cargo test -p chronos-services --tests` |
| Per-crate (`chronos-cli`) | Replay uses side-table events for v2 bundles; replay uses blob events for v1 bundles | `cargo test -p chronos-cli --tests` |
| Sandbox (T4-smoke) | `counterexample_tools` (ce1..ce12) — silent save→get→events_count→replay path through new layout; `e2e_connectivity` — server starts after D2/D4 changes | Pre-build `chronos-mcp`; export `CHRONOS_MCP_PATH`; `cargo test -p chronos-sandbox --test counterexample_tools --test e2e_connectivity` |

T0/T1/T2 are mandatory per AGENTS.md tier table. The 14 unit/integration tests pin every requirement in `spec.md`.

## Migration / Rollout

**No migration pass.** Pre-m9-02 bundles deserialize cleanly:
- `record.events` populated (from blob)
- `summary.events_count` defaults to 0 via `#[serde(default)]`
- `record.schema_version` defaults to 1
- `bundle_events_or_legacy` returns the blob events (legacy branch)

Post-m9-02 saves:
- `record.events` is `vec![]` (in blob)
- `summary.events_count` carries the actual count
- `record.schema_version` is 2
- `bundle_events_or_legacy` returns side-table events (chunks concated in `chunk_index` order)

The chokepoint is the ONLY place that knows about both shapes. Consumers are unchanged on the wire. A future m9+ cycle may add a `rewrite_legacy_bundles_to_v2` tool that moves blob events to the side table; out of scope here (R6).

Hard-reject policy on `record.schema_version > CURRENT` is preserved (m9-01 D3). Hand-injected v3 blobs → "newer than supported" error (test `m9_02_schema_version_bumped_to_2`).

## Open Questions

None — all 8 scoping decisions (D1–D8) are ratified by spec requirements.

## ADR Candidates

- **D4 (atomic save wraps both tables)** — hard to reverse (forces every future save to be atomic w.r.t. side tables), surprising (split API + standalone write), real trade-off (atomicity vs API ergonomics). → `docs/adr/0071-bundle-save-atomicity.md`

## Standard Envelope

```yaml
status: success
executive_summary: |
  m9-02 splits Vec<TraceEvent> out of the bundle blob into a chunked side table
  (counterexample_bundle_events, chunk size 256). A single chokepoint
  (bundle_events_or_legacy) handles legacy blob-embedded vs side-table cases;
  chronos-cli::replay and chronos-services::events_count read through it.
  Schema bumps to 2. No migration pass; pre-m9-02 bundles continue to load
  cleanly via serde defaults.
artifacts:
  - "sddk/p-3416cfb8288f8964/m9-02-events-side-table/design"
summary:
  approach: split events into chunked side table; chokepoint loader handles legacy/new branching
  key_decisions: 8 (D1-D8 from scoping)
  files_affected: 0 new, 3 modified, 0 deleted
  testing_strategy: unit (chronos-store) + per-crate (services + cli) + T4-smoke sandbox subset
  adr_candidates: 1 (D4 — atomic save wraps both tables)
architecture:
  impact: local
  manifest_ref: null
  semantic_status: not_applicable
  render_status: not_applicable
open_questions: []
next_recommended: sddk-tasks (already complete; proceed to sddk-apply)
risks:
  - "list path still deserializes full blob per row (R6) — bounded to summary + empty Vec post-m9-02; not a regression but listed as m9+ scope"
  - "chunk-boundary count formula in count_counterexample_bundle_events must use floor((n-1)) * CHUNK + last_len; covered by m9_02_count_events_handles_partial_last_chunk test"
```