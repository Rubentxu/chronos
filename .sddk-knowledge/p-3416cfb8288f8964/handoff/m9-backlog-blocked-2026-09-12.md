# Handoff: m9+ Backlog (Updated 2026-09-12T14:38Z)

## Status

As of 2026-09-12T14:38:00Z, the m9 vault is canonical-schema-clean across
all 51 cycles in CA p-3416cfb8288f8964 (plus 2 legacy). 45 cross-checks
all pass.

## Bucket 1: By-design disclosures (unchanged)

These remain by-design and are **not** "fixable" without an explicit
design change. See
`.sddk-knowledge/p-3416cfb8288f8964/handoff/m9-backlog-blocked-2026-09-12.md`
for the full list.

| ID | Source | Title |
|---|---|---|
| m9-01-R1..R3 | m9-01 scoping | schema_version disclosures |
| m9-02-R1..R8 | m9-02 scoping | Bundle storage disclosures |
| m9-04-R1..R6 | m9-04 scoping | Side-table key layout disclosures |
| m8-04-R-hypothesis-fallback | m8-04 | property_target fallback |
| m8-06-R4 | m8-06 | Cross-variant existence predicate shrinking |

**Action:** None.

## Bucket 2: Architectural redesign (deferred to m10+)

| ID | Title | Status |
|---|---|---|
| cc-001-god-module | counterexample_storage.rs at 2,823 LoC | DEFERRED |
| cc-004-implicit-io-toctou | save_bundle_record_and_events TOCTOU pattern | DEFERRED |

**Action:** Defer to m10+ unless explicitly requested by user.

## Bucket 3: Pre-vault-reorg documentation drift

| Terminated ID | Closed by | Why not rebuildable |
|---|---|---|
| m8-04-R4 | m9-02 | m8-04 source artifacts not in checkout |
| m8-07-R2 | m9-01 | m8-07 source artifacts not in checkout |

**Action:** Out of scope for auto-mode. Recoverable from pre-reorg
snapshot only.

## Bucket 4: Feature work (not debt)

| ID | Title | Status |
|---|---|---|
| m9-02-R4 | counterexample_bundle_events MCP tool | BACKLOG |

**Action:** Treat as feature backlog.

## Cross-checks summary (as of 2026-09-12T14:38Z)

All 45 cross-checks pass. Breakdown:

- **C1-C25 (legacy schema normalization)**: PASS
- **C26-C35 (canonical schema enforcement)**: PASS
- **C36-C41 (comprehensive metadata schema)**: PASS
- **C42 (peel accuracy + tag existence)**: PASS
- **C43 (head_sha consistency)**: PASS
- **C44 (verify-report Summary)**: PASS
- **C45 (cycles-index SHA 40-char + tag match)**: PASS

## Sessions

| Session | Cycles | Drift classes |
|---|---|---|
| m9-34..m9-43 | 10 | 10 |
| m9-44..m9-49 | 6 | 6 |
| m9-50..m9-53 | 4 | 4 |

## When to break the handoff

Same as before. Bucket 1 unchanged, Bucket 3 requires source artifacts,
Bucket 4 requires product scope.
