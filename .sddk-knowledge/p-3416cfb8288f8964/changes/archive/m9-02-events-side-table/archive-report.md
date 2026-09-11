# Archive Report — m9-02-events-side-table

**Cycle:** `m9-02-events-side-table` · **Path:** A-lite · **Verdict:** success
**Published:** `1a8d104...` · **Tag:** `v0.7.0` · **Runtime:** CLOSED

---

## Cycle summary

Moved `Vec<TraceEvent>` out of the `CounterexampleBundleRecord` bincode blob into a
chunked redb side table (`counterexample_bundle_events`), keyed by `(bundle_id,
chunk_index)`. Schema bumped to 2. `events_count: u64` added to summary for O(1)
count reads. The `bundle_events_or_legacy` chokepoint (D5) routes legacy
(pre-m9-02 blob-embedded) vs. post-m9-02 (side-table) events. Atomic save wraps
both tables in one redb transaction. No migration pass; pre-m9-02 bundles load via
serde defaults + legacy blob branch.

## Disclosure closure

**m8-04 R4 — bundle-as-blob → side table: events still ride inside bundle blob**
Status: **CLOSED** ✔

The m8-04 scoping declared bundle-as-blob a m9+ deferral, acceptable for M8's
small smoke fixtures. m9-02 implemented exactly this structural change: a new
`counterexample_bundle_events` side table, chunked at 256 events per row, with
`bundle_events_or_legacy` as the single routing chokepoint. Post-m9-02 saves write
`events: vec![]` to the blob; events live in the side table. Pre-m9-02 bundles
continue to load via the legacy blob branch transparently. The m9-02 scoping doc
§9 explicitly declares R4 closed.

**Evidence:** `counterexample_storage.rs`:
- `COUNTEREXAMPLE_BUNDLE_EVENTS` table (new)
- `save_bundle_record_and_events` — atomic record + chunks in one write tx
- `bundle_events_or_legacy` (D5) — chokepoint routing legacy vs. v2
- `load_counterexample_bundle_events` — concat chunks by `chunk_index`
- `save_counterexample_bundle` — sets `events: vec![]`, delegates to atomic helper

## Disclosures left open (→ m9+)

| ID | Título | Rationale |
|---|---|---|
| m9-02 R1 | `schema_version` bumped to 2; future versions hard-rejected | Loader policy unchanged; future wire breaks handled by m9+ |
| m9-02 R2 | Pre-m9-02 bundles get `events_count: 0`; fallback to blob | Deliberate: serde default; fallback is correct but could be optimised |
| m9-02 R3 | Public `save_counterexample_bundle_events` not atomic w.r.t. record | Only the internal wrapper is atomic; public method is standalone |
| m9-02 R4 | `counterexample_bundle_events` MCP tool is m9+ scope | Storage primitive shipped; wire layer deferred |
| m9-02 R5 | No chunk compression (lz4/zstd) | Chunk size 256 is acceptable; compression deferred |
| m9-02 R6 | `list_counterexample_bundles` still deserializes full blob per row | `events: vec![]` is the optimization; further split is m9+ |
| m9-02 R7 | Re-save overwrites all prior chunks (no append) | Correct for current use; append semantics deferred |
| m9-02 R8 | Per-chunk load not publicly exposed | Concat-then-paginate is fast enough; per-chunk API deferred |
| m8-06 R4 | Cross-variant existence predicate shrinking | Not addressed by m9-02; still open |

## Debt findings (→ m9+ backlog)

| ID | Cluster | Severity | Priority | Título |
|---|---|---|---|---|
| FIND-M9-02-DV-API-01 | api | LOW | P3 | `save_counterexample_bundle_events` dead API: no caller uses it standalone |
| FIND-M9-02-DV-DOC-01 | doc | LOW | P3 | `events_count` doc drift: fallback branch not documented at call site |
| FIND-M9-02-DV-OE-01 | overeng | MEDIUM | P2 | Side-table chunk-iteration skeleton duplicated in 3 places |
| FIND-M9-02-DV-COUP-01 | coupling | MEDIUM | P2 | Fallback outside D5 chokepoint: wrong module boundary |
| FIND-M9-02-DV-PERF-01 | perf | MEDIUM | P2 | Side-table key layout forces full-scan for single-bundle reads |

**Inherited (from m9-01):**

| ID | Cluster | Severity | Priority | Título |
|---|---|---|---|---|
| FIND-M9-01-DV-COUP-01 | coupling | MEDIUM | P2 | Duplicated version envelopes: `schema_version` on record + summary |
| FIND-M9-01-DV-COUP-02 | coupling | LOW | P3 | List/load policy asymmetry |
| FIND-M9-01-DV-OE-01 | overeng | LOW | P3 | `KNOWN_BUNDLE_SCHEMA_VERSIONS` dead code |

No INC files created (cycle-7b: `PASS_WITH_WARNINGS`; all findings target backlog).

## Artifact gaps noted

The verify-report.md and debt-report.json referenced in the release report are
absent from the cycle-artifacts directory. The release report cites paths with an
incorrect `p-3416cfb8288f8964/` prefix. These artifacts were not committed for
this cycle. Debt findings are recorded from the task contract and scoping
disclosures; no formal SHA/HMAC binding is available.

## Vault sync performed

- Created `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-02-events-side-table/change-entry.md`
- Created `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-02-events-side-table/archive-manifest.md`
- Created `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-02-events-side-table/archive-report.md`
- Updated `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` (closed m8-04 R4, added m9-02 R1–R8, added debt findings)
- Updated `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` (added m9-02 entry)

## Runtime transition

Ad-hoc cycle: no CLI storage. Orchestrator reconciled CLI ledger to `RELEASED/archive`.
`sddk cycle transition archive.complete` not executed (STORAGE_NOT_FOUND).
Recorded as CLOSED in this manifest.

---

**Closed:** 2026-09-11T22:38:00Z · **Cycle ID:** `m9-02-events-side-table`
