# Handoff: m9+ Backlog (Blocked on Human / m10+ Scope)

## Status

As of 2026-09-12T07:27:00Z, the auto-mode-safe portion of the m9+ backlog
is closed. The remaining items fall into three buckets:

1. **By-design disclosures** — accepted tradeoffs, documented in the cycle
   scoping doc; not bugs, not technical debt.
2. **Architectural redesign** — requires multi-crate planning + compat
   shims; exceeds B-direct scope.
3. **Pre-vault-reorg documentation drift** — closed findings whose
   source artifacts are not in this repo.

This document is the formal handoff. The next session that picks this up
should treat any of these items as **blocked** unless the user provides
a new scope (m10+) or an explicit decision.

## Bucket 1: By-design disclosures

These are explicitly documented as accepted tradeoffs in the cycle
scoping docs. Touching them would change the documented design intent.

| ID | Source | Title | Why by-design |
|---|---|---|---|
| m9-01-R1 | m9-01 scoping §R1 | `schema_version` silently overwritten on save (D5 canonical writer) | Saves always canonicalize to CURRENT_BUNDLE_SCHEMA_VERSION. Caller-set values are intentionally ignored — see m9-01 D5. |
| m9-01-R2 | m9-01 scoping §R2 | Future-versioned bundles appear in list (best-effort; load() rejects individually) | m9-02 R2 disclosure: best-effort list is the deliberate product call for forward-compat. m9-08 added the `SchemaTooNew` variant so callers can distinguish at load time. |
| m9-01-R3 | m9-01 scoping §R3 | `schema_version` is internal-only (not on MCP wire or services-side summary) | The version is a redb-internal concern. Surfacing it on the wire would commit to a public-API version contract that the project explicitly deferred. |
| m9-02-R1 | m9-02 scoping §R1 | `schema_version` bumped to 2; bundles with >2 hard-rejected by loader | CURRENT is now 3; this disclosure is historical. The loader continues to enforce the floor. |
| m9-02-R2 | m9-02 scoping §R2 | Pre-m9-02 bundles get `events_count: 0` via serde default | Backward-compat via #[serde(default)]. The m9-05 cleanup removed the `Option<u64>` wrapper around `events_count` so this is now a non-issue at the type level. |
| m9-02-R3 | m9-02 scoping §R3 | Public `save_counterexample_bundle_events` is not atomic with the record | Resolved by m9-03: the public standalone API was deleted; only the atomic internal wrapper `save_bundle_record_and_events` remains. |
| m9-02-R4 | m9-02 scoping §R4 | `counterexample_bundle_events` MCP tool deferred to m9+ | Feature scope, not a fix. New MCP tool requires DTO + dispatcher + tests + doc. See "Bucket 4: feature work" below. |
| m9-02-R5 | m9-02 scoping §R5 | No chunk compression (lz4/zstd) | Ergonomics cost vs payload size trade-off. Sized at 256 events per chunk; current bundles fit. |
| m9-02-R6 | m9-02 scoping §R6 | `list_counterexample_bundles` still deserializes full blob per row | redb doesn't support sub-value reads natively. The summary-only projection is the design. |
| m9-02-R7 | m9-02 scoping §R7 | Re-save overwrites all prior chunks (no append semantics) | Idempotent re-save is the design intent; chunk deletion is the cleanup mechanism. |
| m9-02-R8 | m9-02 scoping §R8 | Per-chunk load not publicly exposed | concat-then-paginate is fast enough at current scale. |
| m9-04-R1 | m9-04 scoping §R1 | Side-table key layout changed to v3 (blake3 prefix, 20-byte fixed) | Disclosed in m9-04 design; v2 → v3 migration is the actual change. |
| m9-04-R2 | m9-04 scoping §R2 | Hash-collision risk is bounded by 16-byte truncation | ~2^-128 birthday bound. Defense in depth: per-chunk bundle_id identity check. |
| m9-04-R3 | m9-04 scoping §R3 | Save does one defensive full-table scan | Only remaining full scan. Runs once per save, only reads keys. |
| m9-04-R4 | m9-04 scoping §R4 | V2 bundles never re-saved keep using the legacy path | Documented fallback policy. The full scan is bounded by total table size. |
| m9-04-R5 | m9-04 scoping §R5 | Bumped `CURRENT_BUNDLE_SCHEMA_VERSION` to 3 | Closed-loop with m9-06 (compile-time invariant) and m9-08 (SchemaTooNew variant). |
| m9-04-R6 | m9-04 scoping §R6 | `KNOWN_BUNDLE_SCHEMA_VERSIONS` grew to [1, 2, 3] | Closed-loop with m9-06 (the const-eval invariant references it). |
| m8-04-R-hypothesis-fallback | m8-04 | `property_target` lost in fallback reconstruction | Non-issue post m8-07; pre-m8-07 bundles retain synthetic defaults. |
| m8-06-R4 | m8-06 | Cross-variant existence predicate shrinking | Variant still fixed in ExistencePredicateShrinker (bounded). |

**Action:** None. These are explicit design disclosures. Do not "fix" them
without an explicit design change from the project owner.

## Bucket 2: Architectural redesign

These require planning + compat shims. They exceed B-direct scope.

| ID | Severity | Title | Scope |
|---|---|---|---|
| cc-001-god-module | MEDIUM | `counterexample_storage.rs` at ~2.6K LoC; 5 distinct concerns (key encoding, schema versioning, record DTOs, persistence, side-table decoding) | Split into `keys.rs`, `record.rs`, `storage.rs`, `schema_version.rs`, `v2_legacy.rs`. Requires compat shim for any internal callers (currently m9-05 chokepoints only). File grew +141 LoC net m9-04..m9-08 from 2,556 → 2,650; growth trend will continue without a split. |
| cc-004-implicit-io-toctou | LOW | `save_bundle_record_and_events` opens read-then-write (TOCTOU pattern) | Concurrency design required. The function currently reads the legacy key set, then opens a write transaction that may be invalidated by a concurrent save. Multiple save paths may need restructuring into a single atomic primitive. |

**Action:** Defer to m10+ unless the user explicitly requests the work.
Both items are tracked in `terms/index.md` Active section under
"Debt findings from m9-04".

## Bucket 3: Pre-vault-reorg documentation drift

These are closed findings whose apply-checkpoint.json never existed in
this repo (the vault reorg moved to the `p-3416cfb8288f8964/` path
mid-m9; cycles before the reorg were committed without the standard
artifact set).

| Terminated ID | Closed by | Why not rebuildable |
|---|---|---|
| m8-04-R4 | m9-02 | m8-04's merge-receipt / release-receipt / verify-report are not in this checkout. The Terminated row in terms/index.md is the only record of the closure. |
| m8-07-R2 | m9-01 | Same: m8-07 source artifacts pre-date this checkout. |

**Action:** Out of scope for auto-mode. The source artifacts would need
to be recovered from a pre-reorg snapshot (e.g. before the commit that
introduced the `p-3416cfb8288f8964/` path layout). If the user has
those artifacts, they can be committed as part of a dedicated m9-11
"vault-reorg-completion" cycle.

## Bucket 4: Feature work (out of scope for "exhausted" treatment)

| ID | Severity | Title | Scope |
|---|---|---|---|
| m9-02-R4 | — | `counterexample_bundle_events` MCP tool | Storage primitive exists (load_counterexample_bundle_events + count_counterexample_bundle_events); needs DTO, dispatcher wiring, integration tests. Larger than B-direct. |

**Action:** Treat as feature backlog, not debt. Requires product
acceptance + scope sizing.

## Cross-references

- The full active backlog is at `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md`
- The completed cycles index is at `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md`
- The standing maintenance procedure (vault-drift-sweep) is at
  `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`

## When to break the handoff

Break this handoff and start a new cycle if any of the following occurs:

1. The user explicitly requests work on cc-001 or cc-004 (these require
   design phase before apply).
2. A new debt report is generated (m9-X debt-verify produces findings).
3. A new failure mode is discovered through user-facing reports.
4. The user requests a feature from "Bucket 4" (e.g. the
   counterexample_bundle_events MCP tool).

Do **not** break the handoff to:

- Touch any "Bucket 1" item. They are by-design.
- Attempt to rebuild any "Bucket 3" item without the user providing the
  source artifacts.
