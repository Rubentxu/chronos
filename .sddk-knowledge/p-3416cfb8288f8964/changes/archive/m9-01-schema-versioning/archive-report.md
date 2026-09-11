# Archive Report — m9-01-schema-versioning

**Cycle:** `m9-01-schema-versioning` · **Path:** A-min · **Verdict:** success
**Published:** `2594814...` · **Tag:** `v0.6.0` · **Runtime:** CLOSED

---

## Cycle summary

Added `schema_version: u32` to `CounterexampleBundleRecord` and
`CounterexampleBundleSummary` in `chronos-store`. The store canonical writer
sets both fields atomically; `load()` rejects bundles with `schema_version >
CURRENT`. `list()` includes future-versioned rows (best-effort, bincode deserializes
fine).

## Disclosure closure

**m8-07 R2 — Unknown future wire fields are dropped**
Status: **CLOSED** ✔

The m8-07 scoping doc declared that unknown future `HypothesisInput` fields would be
silently dropped on persist, with a mitigation plan to add `schema_version` to
`CounterexampleBundleSummary` as m9+. m9-01 implemented that exact mitigation:
`schema_version` was added to both the record and the summary, with a loader guard
that rejects `schema_version > CURRENT` before attempting bincode deserialization.
Unknown wire fields are now protected by the bundle-level version guard.

**Evidence:** `counterexample_storage.rs:234-235` (canonical writer), `lines 286-299`
(loader guard).

## Disclosures left open (→ m9+)

| ID | Título | Rationale |
|---|---|---|
| m9-01 R1 | `schema_version` silently overwritten on save | Canonical writer overwrites both fields; intentionally not enforced on load path |
| m9-01 R2 | Future-versioned bundles in list (best-effort) | Deliberate product call; asymmetry documented, remediation deferred to v2 bump |
| m9-01 R3 | `schema_version` is internal-only | No MCP wire exposure; intentional boundary |
| m9-01 R4 | `KNOWN_BUNDLE_SCHEMA_VERSIONS` unused | Dead speculative code; suppressible at next wire-format break |

## Debt findings (→ m9+ backlog)

| ID | Cluster | Severity | Priority | Título |
|---|---|---|---|---|
| FIND-M9-01-DV-COUP-01 | coupling | MEDIUM | P2 | Duplicated version envelopes, no equality enforcement in loader |
| FIND-M9-01-DV-COUP-02 | coupling | LOW | P3 | List/load policy asymmetry |
| FIND-M9-01-DV-OE-01 | overeng | LOW | P3 | `KNOWN_BUNDLE_SCHEMA_VERSIONS` dead code |

No INC files created (cycle-7b: `PASS_WITH_WARNINGS` with zero pre-existing findings).

## Vault sync performed

- Created `.sddk-knowledge/p-3416cfb8288f8964/` vault structure
- Persisted archive manifest and report
- Created m9-01 change entry (`.sddk-knowledge/.../changes/m9-01-schema-versioning/`)
- Updated m8-07 change entry with R2 closure record
- Created terms index for m9+ follow-ups
- Updated cycles index

## Runtime transition

Ad-hoc cycle: no CLI storage. Orchestrator reconciled CLI ledger to `RELEASED/archive`.
`sddk cycle transition archive.complete` not executed (STORAGE_NOT_FOUND).
Recorded as CLOSED in this manifest.

## Artifacts

| Artifact | Path |
|---|---|
| Archive manifest | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-01-schema-versioning/archive-manifest.md` |
| Archive report | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-01-schema-versioning/archive-report.md` |
| Release receipt | `cycle-artifacts/m9-01-schema-versioning/receipts/release-receipt.json` |
| Merge receipt | `cycle-artifacts/m9-01-schema-versioning/receipts/merge-receipt.json` |
| Debt report | `cycle-artifacts/m9-01-schema-versioning/debt-verify/debt-report.json` |

---

**Closed:** 2026-09-11T17:43:00Z · **Cycle ID:** `m9-01-schema-versioning`
