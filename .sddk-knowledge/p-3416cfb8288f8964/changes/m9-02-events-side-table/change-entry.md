# Change: m9-02 events side table

## Ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-02-events-side-table` |
| Path | `A-lite` |
| Status | CLOSED |
| Base SHA | `48a9cff54ec165987063705d5d4d3af453fcb6b3` |
| Head SHA | `1a8d104ba2da883b40bd424cdc079b344f7ed63c` |
| Tag | `v0.7.0` (annotated, peel matches published SHA) |

## Commits

| SHA | Subject |
|---|---|
| `a765d56` | docs(m9-02): scoping — bundle events side table |
| `b2a2455` | docs(m9-02): spec — bundle events side table |
| `3d758f3` | docs(m9-02): tasks — bundle events side table |
| `126d4e5` | docs(m9-02): design — bundle events side table |
| `9fc2211` | feat(chronos-store): schema_version=2 + BUNDLE_EVENTS_CHUNK_SIZE + events_count |
| `624c8f2` | feat(chronos-store): counterexample_bundle_events side table + save/load/count |
| `19bbf73` | feat(chronos-services,chronos-cli): events_count + bundle_events_or_legacy |
| `e410f44` | test(chronos-store,chronos-services,chronos-cli): 14 m9-02 tests |
| `a62c3b1` | fix(m9-02): correct events_count wire after side-table migration |
| `1a8d104` | chore(m9-02): rustfmt collapses use block |

## Scope

Moved `Vec<TraceEvent>` out of the `CounterexampleBundleRecord` bincode blob into a
chunked redb side table (`counterexample_bundle_events`), keyed by `(bundle_id,
chunk_index)` with chunk size 256. The canonical save wraps both tables in one redb
transaction. `bundle_events_or_legacy` is the single chokepoint (D5) that handles
legacy (blob-embedded) vs. post-m9-02 (side-table) events. `events_count` on the
summary enables O(1) count reads. Schema bumped to 2.

### Changed paths

- `crates/chronos-store/src/counterexample_storage.rs`
- `crates/chronos-services/src/counterexample.rs`
- `crates/chronos-cli/src/replay.rs`

## Disclosure R4 — CLOSED by m9-02

**R4 — bundle-as-blob → side table: events still ride inside bundle blob**
(m8-04 scoping §3 B-decisions, m8-04 scoping §5 R4)

> "bundle-as-blob storage — deferred to m9+. Acceptable: real events are small in
> the smoke fixtures; the inefficiency only matters for 10k+ events."

**Closure:** m9-02 created the `counterexample_bundle_events` side table and routed all
new saves through it. `Vec<TraceEvent>` no longer rides inside the bundle blob for
post-m9-02 bundles. Pre-m9-02 bundles continue to load via the legacy blob branch
in `bundle_events_or_legacy` (D5). The m9-02 scoping doc §9 declares R4 closed.

**Evidence:** `counterexample_storage.rs` (`COUNTEREXAMPLE_BUNDLE_EVENTS` table,
`save_bundle_record_and_events`, `bundle_events_or_legacy`).
**Closed by:** `m9-02-events-side-table` · SHA `1a8d104...` · tag `v0.7.0`

## Other disclosures (m9-02 R1–R8)

| ID | Título | Estado | Destino |
|---|---|---|---|
| m9-02 R1 | `schema_version` bumped to 2; future versions hard-rejected | disclosed | m9+ |
| m9-02 R2 | Pre-m9-02 bundles get `events_count: 0` from serde; fallback to blob | disclosed | m9+ |
| m9-02 R3 | Public `save_counterexample_bundle_events` not atomic w.r.t. record | disclosed | m9+ |
| m9-02 R4 | `counterexample_bundle_events` MCP tool is m9+ scope | disclosed | m9+ |
| m9-02 R5 | No chunk compression (lz4/zstd) | disclosed | m9+ |
| m9-02 R6 | `list_counterexample_bundles` still deserializes full blob per row | disclosed | m9+ |
| m9-02 R7 | Re-save overwrites all prior chunks (no append) | disclosed | m9+ |
| m9-02 R8 | Per-chunk load not publicly exposed | disclosed | m9+ |

## Deuda técnica

| ID | Cluster | Severity | Priority | Título |
|---|---|---|---|---|
| FIND-M9-02-DV-API-01 | api | LOW | P3 | `save_counterexample_bundle_events` dead API: no caller wires it standalone |
| FIND-M9-02-DV-DOC-01 | doc | LOW | P3 | `events_count` doc drift: fallback branch not documented at call site |
| FIND-M9-02-DV-OE-01 | overeng | MEDIUM | P2 | Side-table chunk-iteration skeleton duplicated in 3 places |
| FIND-M9-02-DV-COUP-01 | coupling | MEDIUM | P2 | Fallback outside D5 chokepoint: wrong module boundary |
| FIND-M9-02-DV-PERF-01 | perf | MEDIUM | P2 | Side-table key layout forces full-scan for single-bundle reads |

Verdict: `PASS_WITH_WARNINGS` (7 backlog findings) · All findings → m9+ backlog

## Artefactos

| Kind | Path |
|---|---|
| Apply checkpoint | `apply-checkpoint.json` |
| Scoping doc | `docs/milestones/m9-02-events-side-table-scoping.md` |
| Design doc | `docs/milestones/m9-02-events-side-table-design.md` |
| Merge receipt | `cycle-artifacts/m9-02-events-side-table/receipts/merge-receipt.md` |
| Release receipt | `cycle-artifacts/m9-02-events-side-table/receipts/release-receipt.md` |
| Archive manifest | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-02-events-side-table/archive-manifest.md` |
| Archive report | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-02-events-side-table/archive-report.md` |
